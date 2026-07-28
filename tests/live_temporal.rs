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
    run_cli(
        &temporal_cli,
        &[
            "workflow",
            "start",
            "--address",
            &address,
            "--namespace",
            "default",
            "--workflow-id",
            &workflow_id,
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

    let query = format!("WorkflowId = '{workflow_id}'");
    let workflow = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &query, 10)
            .await
            .ok()
            .and_then(|workflows| workflows.into_iter().next())
    })
    .await
    .expect("started workflow should appear in visibility");
    assert_eq!(workflow.status, WorkflowStatus::Running);

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
            .list_workflows("default", &query, 10)
            .await
            .ok()
            .and_then(|workflows| workflows.into_iter().next())
            .filter(|workflow| workflow.status == WorkflowStatus::Terminated)
    })
    .await;
    assert!(
        terminated.is_some(),
        "terminated workflow should reach terminal visibility state"
    );
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
