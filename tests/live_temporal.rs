//! Live control-plane contract test against an ephemeral Temporal dev server.
//!
//! Install the pinned CLI with `scripts/install-temporal-cli.sh`, then run:
//! `cargo test --test live_temporal -- --ignored --nocapture`.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::prelude::*;
use temporal_tui::{
    model::WorkflowStatus,
    service::{GrpcTemporalService, PayloadCodecConfig, TemporalConnectionConfig, TemporalService},
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

struct CodecServer {
    address: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl CodecServer {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind Codec Server");
        let address = listener.local_addr().expect("Codec Server address");
        listener
            .set_nonblocking(true)
            .expect("non-blocking Codec Server");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = read_http_request(&mut stream)
                            .and_then(|request| codec_response(&request))
                            .unwrap_or_else(|error| {
                                (
                                    "400 Bad Request",
                                    serde_json::json!({"error": error}).to_string(),
                                )
                            });
                        write_http_response(&mut stream, response.0, &response.1);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("Codec Server accept failed: {error}"),
                }
            }
        });
        Self {
            address: format!("http://{address}"),
            stop,
            thread: Some(thread),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/{{namespace}}", self.address)
    }
}

impl Drop for CodecServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("join Codec Server");
        }
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

    let task_queues = eventually(Duration::from_secs(10), || async {
        service
            .list_task_queues("default", vec!["temporal-tui-smoke".to_string()])
            .await
            .ok()
            .filter(|queues| {
                queues.len() == 2
                    && queues.iter().any(|queue| {
                        queue.queue_type.label() == "WORKFLOW"
                            && queue.stats.approximate_backlog_count >= 2
                    })
            })
    })
    .await
    .expect("Task Queue diagnostics should expose the queued Workflow Tasks");
    assert!(task_queues.iter().any(|queue| {
        queue.queue_type.label() == "WORKFLOW"
            && queue.pollers.is_empty()
            && queue.stats.approximate_backlog_count >= 2
    }));
    assert!(
        task_queues
            .iter()
            .any(|queue| queue.queue_type.label() == "ACTIVITY")
    );

    let workers = service
        .list_workers("default", "", 10, Vec::new())
        .await
        .expect("experimental Worker observability endpoint");
    assert!(workers.workers.is_empty());
    assert!(workers.next_page_token.is_empty());

    let deployments = service
        .list_worker_deployments("default", 10, Vec::new())
        .await
        .expect("GA Worker Deployment endpoint");
    assert!(deployments.deployments.is_empty());
    assert!(deployments.next_page_token.is_empty());

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

    let codec_server = CodecServer::start();
    let codec_service = GrpcTemporalService::connect(TemporalConnectionConfig {
        address: address.clone(),
        api_key: None,
        headers: HashMap::new(),
        tls: None,
        payload_codec: Some(PayloadCodecConfig {
            endpoint: codec_server.endpoint(),
            headers: HashMap::new(),
        }),
    })
    .await
    .expect("connect client with Codec Server");
    codec_service
        .signal_workflow(
            "default",
            &workflow.key,
            "codec-signal",
            serde_json::json!({"codec_value":"roundtrip"}),
        )
        .await
        .expect("send encoded signal");
    let encoded = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .and_then(|details| {
                details
                    .events
                    .into_iter()
                    .find(|event| event.detail == "codec-signal")
            })
    })
    .await
    .expect("encoded signal event");
    assert!(encoded.fields.iter().any(|field| {
        field.encoding == "binary/encrypted" && !field.value.contains("roundtrip")
    }));
    let decoded = codec_service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("decode Workflow history through Codec Server");
    let decoded = decoded
        .events
        .iter()
        .find(|event| event.detail == "codec-signal")
        .expect("decoded signal event");
    assert!(
        decoded
            .fields
            .iter()
            .any(|field| { field.encoding == "json/plain" && field.value.contains("roundtrip") })
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
            payload_codec: None,
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

fn read_http_request(stream: &mut std::net::TcpStream) -> Result<String, String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut expected_length = None;
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if expected_length.is_none()
            && let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
        {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            });
            expected_length = content_length.map(|length| header_end.saturating_add(4 + length));
        }
        if expected_length.is_some_and(|length| request.len() >= length) {
            break;
        }
    }
    String::from_utf8(request).map_err(|error| error.to_string())
}

fn codec_response(request: &str) -> Result<(&'static str, String), String> {
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .ok_or_else(|| "missing HTTP body".to_string())?;
    let operation = if headers.starts_with("POST ") && headers.contains("/encode HTTP/") {
        "encode"
    } else if headers.starts_with("POST ") && headers.contains("/decode HTTP/") {
        "decode"
    } else {
        return Err("unsupported Codec Server path".to_string());
    };
    let mut payloads: serde_json::Value =
        serde_json::from_str(body).map_err(|error| error.to_string())?;
    let payloads = payloads
        .get_mut("payloads")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| "missing payloads array".to_string())?;
    for payload in &mut *payloads {
        if operation == "encode" {
            let raw = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
            *payload = serde_json::json!({
                "metadata": {
                    "encoding": BASE64_STANDARD.encode("binary/encrypted")
                },
                "data": BASE64_STANDARD.encode(raw)
            });
            continue;
        }
        let encoding = payload
            .get("metadata")
            .and_then(|metadata| metadata.get("encoding"))
            .and_then(serde_json::Value::as_str)
            .and_then(|value| BASE64_STANDARD.decode(value).ok());
        if encoding.as_deref() == Some(b"binary/encrypted") {
            let data = payload
                .get("data")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "encrypted payload has no data".to_string())?;
            let data = BASE64_STANDARD
                .decode(data)
                .map_err(|error| error.to_string())?;
            *payload = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
        }
    }
    Ok(("200 OK", payloads_to_body(payloads)))
}

fn payloads_to_body(payloads: &[serde_json::Value]) -> String {
    serde_json::json!({ "payloads": payloads }).to_string()
}

fn write_http_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write Codec Server response");
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
