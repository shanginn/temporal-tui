//! Live control-plane contract test against an ephemeral Temporal dev server.
//!
//! Install the pinned CLI with `scripts/install-temporal-cli.sh`, then run:
//! `cargo test --test live_temporal -- --ignored --nocapture`.

use std::{
    collections::HashMap,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use temporal_tui::{
    model::WorkflowStatus,
    service::{GrpcTemporalService, TemporalConnectionConfig, TemporalService},
};
use tokio::time::sleep;

struct DevServer {
    child: Child,
}

impl Drop for DevServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the project-local Temporal CLI"]
async fn live_dashboard_and_control_operations() {
    let temporal_cli = temporal_cli();
    assert!(
        temporal_cli.is_file(),
        "install Temporal CLI first: scripts/install-temporal-cli.sh"
    );
    let port = free_port();
    let address = format!("127.0.0.1:{port}");
    let _server = start_dev_server(&temporal_cli, port);
    let service = connect_with_retry(&address).await;

    let cluster = service.cluster_info().await.expect("cluster info");
    assert!(!cluster.server_version.is_empty());
    assert!(
        service
            .list_namespaces()
            .await
            .expect("namespaces")
            .iter()
            .any(|namespace| namespace.name == "default")
    );

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let workflow_id = format!("temporal-tui-smoke-{suffix}");
    let second_workflow_id = format!("temporal-tui-smoke-{suffix}-second");
    start_smoke_workflow(&temporal_cli, &address, &workflow_id);
    start_smoke_workflow(&temporal_cli, &address, &second_workflow_id);

    let type_query = "WorkflowType = 'TemporalTuiSmokeWorkflow'";
    let count = eventually(Duration::from_secs(10), || async {
        service
            .count_workflows("default", type_query)
            .await
            .ok()
            .filter(|count| count.total >= 2)
    })
    .await
    .expect("visibility count should include both smoke workflows");
    assert!(count.total >= 2);
    let first_page = service
        .list_workflows("default", type_query, 1, Vec::new())
        .await
        .expect("first cursor page");
    assert_eq!(first_page.workflows.len(), 1);
    assert!(!first_page.next_page_token.is_empty());
    let first_key = first_page.workflows[0].key.clone();
    let second_page = service
        .list_workflows("default", type_query, 1, first_page.next_page_token)
        .await
        .expect("second cursor page");
    assert_eq!(second_page.workflows.len(), 1);
    assert_ne!(first_key, second_page.workflows[0].key);

    let query = format!("WorkflowId = '{workflow_id}'");
    let workflow = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
    })
    .await
    .expect("started workflow should appear in visibility");
    assert_eq!(workflow.status, WorkflowStatus::Running);
    let chain = service
        .list_workflow_chain("default", &workflow_id)
        .await
        .expect("workflow chain");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].key.workflow_id, workflow_id);

    let details = service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("workflow details");
    assert_eq!(details.summary.key.workflow_id, workflow_id);
    assert!(
        details
            .events
            .iter()
            .any(|event| event.event_type == "WORKFLOW EXECUTION STARTED")
    );

    service
        .signal_workflow(
            "default",
            &workflow.key,
            "smoke-signal",
            serde_json::json!({"approved": true}),
        )
        .await
        .expect("signal workflow");
    let signaled = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .filter(|details| {
                details.events.iter().any(|event| {
                    event.event_type == "WORKFLOW EXECUTION SIGNALED"
                        && event.detail == "smoke-signal"
                })
            })
    })
    .await;
    assert!(
        signaled.is_some(),
        "signal event should be visible in history"
    );
    service
        .signal_workflow(
            "default",
            &workflow.key,
            "redaction-signal",
            serde_json::json!({"customer":"Ada","credentials":{"api_key":"never-export"}}),
        )
        .await
        .expect("signal sensitive payload");
    for index in 0..205 {
        service
            .signal_workflow(
                "default",
                &workflow.key,
                "pagination-signal",
                serde_json::json!({"index": index}),
            )
            .await
            .expect("signal for history pagination");
    }

    let first_history_page = service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("paginated workflow details");
    assert!(
        !first_history_page.history_next_page_token.is_empty(),
        "history over 200 events should expose another cursor"
    );
    let mut all_events = first_history_page.events;
    let mut history_token = first_history_page.history_next_page_token;
    while !history_token.is_empty() {
        let page = service
            .load_history_page("default", &workflow.key, history_token)
            .await
            .expect("older history page");
        all_events.extend(page.events);
        history_token = page.next_page_token;
    }
    all_events.sort_by_key(|event| event.event_id);
    all_events.dedup_by_key(|event| event.event_id);
    assert!(
        all_events
            .iter()
            .any(|event| event.event_type == "WORKFLOW EXECUTION STARTED")
    );
    let redacted_event = all_events
        .iter()
        .find(|event| event.detail == "redaction-signal")
        .expect("redaction signal event");
    assert!(redacted_event.fields.iter().any(|field| {
        field.redacted
            && field.value.contains("<redacted>")
            && !field.value.contains("never-export")
    }));

    service
        .cancel_workflow("default", &workflow.key, "live contract test")
        .await
        .expect("request cancellation");
    let cancel_requested = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .filter(|details| {
                details
                    .events
                    .iter()
                    .any(|event| event.event_type == "WORKFLOW EXECUTION CANCEL REQUESTED")
            })
    })
    .await;
    assert!(
        cancel_requested.is_some(),
        "cancellation request should be visible in history"
    );

    service
        .terminate_workflow("default", &workflow.key, "live contract test cleanup")
        .await
        .expect("terminate workflow");
    let terminated = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
            .filter(|workflow| workflow.status == WorkflowStatus::Terminated)
    })
    .await;
    assert!(
        terminated.is_some(),
        "terminated workflow should reach terminal visibility state"
    );

    let second = service
        .list_workflows(
            "default",
            &format!("WorkflowId = '{second_workflow_id}'"),
            10,
            Vec::new(),
        )
        .await
        .expect("second workflow visibility")
        .workflows
        .into_iter()
        .next()
        .expect("second workflow");
    service
        .terminate_workflow("default", &second.key, "live contract test cleanup")
        .await
        .expect("terminate second workflow");
}

fn temporal_cli() -> PathBuf {
    std::env::var_os("TEMPORAL_CLI").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(".tools")
                .join("bin")
                .join("temporal")
        },
        PathBuf::from,
    )
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral local address")
        .port()
}

fn start_dev_server(temporal_cli: &PathBuf, port: u16) -> DevServer {
    let port = port.to_string();
    let child = Command::new(temporal_cli)
        .args([
            "server",
            "start-dev",
            "--headless",
            "--ip",
            "127.0.0.1",
            "--port",
            &port,
            "--log-level",
            "error",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start Temporal dev server");
    DevServer { child }
}

async fn connect_with_retry(address: &str) -> GrpcTemporalService {
    let mut last_error = None;
    for _ in 0..80 {
        match GrpcTemporalService::connect(TemporalConnectionConfig {
            address: address.to_string(),
            api_key: None,
            headers: HashMap::new(),
            tls: None,
        })
        .await
        {
            Ok(service) => return service,
            Err(error) => last_error = Some(error),
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!(
        "Temporal dev server did not become ready: {}",
        last_error.unwrap()
    );
}

fn run_cli(temporal_cli: &PathBuf, arguments: &[&str]) {
    let output = Command::new(temporal_cli)
        .args(arguments)
        .output()
        .expect("run Temporal CLI");
    assert!(
        output.status.success(),
        "Temporal CLI failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start_smoke_workflow(temporal_cli: &PathBuf, address: &str, workflow_id: &str) {
    run_cli(
        temporal_cli,
        &[
            "workflow",
            "start",
            "--address",
            address,
            "--namespace",
            "default",
            "--workflow-id",
            workflow_id,
            "--type",
            "TemporalTuiSmokeWorkflow",
            "--task-queue",
            "temporal-tui-smoke",
            "--static-summary",
            "Temporal TUI live smoke test",
            "--input",
            r#"{"source":"temporal-tui"}"#,
            "--output",
            "none",
        ],
    );
}

async fn eventually<T, FutureType, Factory>(timeout: Duration, mut factory: Factory) -> Option<T>
where
    FutureType: std::future::Future<Output = Option<T>>,
    Factory: FnMut() -> FutureType,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(value) = factory().await {
            return Some(value);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(100)).await;
    }
}
