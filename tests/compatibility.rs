//! Read-only compatibility contract against an isolated Temporal dev server.
//!
//! `scripts/compatibility.sh` runs this test against every supported server
//! line using checksum-verified, project-local Temporal CLI releases.

use std::{
    collections::HashMap,
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};

use temporal_tui::{
    model::{Capability, CapabilityAvailability},
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
#[ignore = "requires a checksum-verified project-local Temporal CLI"]
async fn negotiated_read_only_contract() {
    let temporal_cli = temporal_cli();
    assert!(
        temporal_cli.is_file(),
        "Temporal CLI does not exist: {}",
        temporal_cli.display()
    );
    let port = free_port();
    let address = format!("127.0.0.1:{port}");
    let _server = start_dev_server(&temporal_cli, port);
    let service = connect_with_retry(&address).await;

    let cluster = service.cluster_info().await.expect("cluster info");
    let namespaces = service.list_namespaces().await.expect("list namespaces");
    let workflows = service
        .list_workflows("default", "", 1, Vec::new())
        .await
        .expect("list workflows");
    let count = service
        .count_workflows("default", "")
        .await
        .expect("count workflows");
    let capabilities = service
        .server_capabilities("default")
        .await
        .expect("negotiate capabilities");

    assert!(!cluster.cluster_name.is_empty());
    assert!(!cluster.cluster_id.is_empty());
    assert_eq!(cluster.server_version, capabilities.server_version);
    assert!(
        namespaces
            .iter()
            .any(|namespace| namespace.name == "default")
    );
    assert!(workflows.workflows.is_empty());
    assert_eq!(count.total, 0);
    assert_eq!(capabilities.namespace, "default");
    assert_eq!(capabilities.features.len(), 9);
    for feature in &capabilities.features {
        let expected = expected_capability(&cluster.server_version, feature.capability);
        assert_eq!(
            feature.availability,
            expected,
            "unexpected {} capability on Temporal Server {}: {}",
            feature.capability.label(),
            cluster.server_version,
            feature.detail
        );
    }

    let matrix = capabilities
        .features
        .iter()
        .map(|feature| {
            format!(
                "{}={}",
                feature.capability.label(),
                feature.availability.label()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!("Temporal Server {}: {matrix}", cluster.server_version);
}

fn expected_capability(server_version: &str, capability: Capability) -> CapabilityAvailability {
    match (server_version, capability) {
        ("1.29.1", Capability::WorkflowPause | Capability::WorkerHeartbeats)
        | ("1.30.2" | "1.31.2", Capability::WorkflowPause) => CapabilityAvailability::Unavailable,
        ("1.29.1" | "1.30.2" | "1.31.2", _) => CapabilityAvailability::Available,
        _ => panic!("compatibility contract has no expectation for Server {server_version}"),
    }
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
    for _ in 0..120 {
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
        last_error.expect("at least one connection attempt")
    );
}
