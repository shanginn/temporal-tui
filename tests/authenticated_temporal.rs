use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use bytes::{BufMut, Bytes, BytesMut};
use chrono::Utc;
use futures_util::future::BoxFuture;
use http_body_util::{BodyExt, Full};
use prost::Message;
use serde_json::{Value, json};
use temporal_tui::{
    auth::{
        AuthSession, CredentialStore, MemoryCredentialStore, TemporalAuthProfile,
        credential_binding_for_profile, credential_item_for_profile,
    },
    service::{GrpcTemporalService, ServiceError, TemporalConnectionConfig, TemporalService},
};
use temporalio_client::tonic::{
    body::Body,
    codegen::{Service, http},
    transport::Server,
};
use temporalio_common::protos::temporal::api::workflowservice::v1::{
    GetClusterInfoResponse, GetSystemInfoResponse,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinHandle,
    time::{Instant, sleep, timeout},
};
use tokio_stream::wrappers::TcpListenerStream;

const KEYRING_SERVICE: &str = "io.temporal.temporal-tui";
const GET_SYSTEM_INFO: &str = "/temporal.api.workflowservice.v1.WorkflowService/GetSystemInfo";
const GET_CLUSTER_INFO: &str = "/temporal.api.workflowservice.v1.WorkflowService/GetClusterInfo";
const TEST_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, PartialEq, Eq)]
struct GrpcObservation {
    path: String,
    authorization: Option<String>,
    rotated_refresh_was_persisted: bool,
}

#[derive(Clone)]
struct WorkflowWireService {
    observations: Arc<Mutex<Vec<GrpcObservation>>>,
    store: Arc<MemoryCredentialStore>,
    keyring_item: String,
}

impl Service<http::Request<Body>> for WorkflowWireService {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let observations = Arc::clone(&self.observations);
        let store = Arc::clone(&self.store);
        let keyring_item = self.keyring_item.clone();
        Box::pin(async move {
            let path = request.uri().path().to_owned();
            let authorization = request
                .headers()
                .get(http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned);
            let rotated_refresh_was_persisted =
                persisted_refresh(&store, &keyring_item).as_deref() == Some("refresh-3");
            observations
                .lock()
                .expect("gRPC observation lock")
                .push(GrpcObservation {
                    path: path.clone(),
                    authorization: authorization.clone(),
                    rotated_refresh_was_persisted,
                });

            // Consume the request frame so the HTTP/2 stream closes cleanly.
            let _body = request.into_body().collect().await;
            let (expected_authorization, payload) = match path.as_str() {
                GET_SYSTEM_INFO => (
                    "Bearer access-1",
                    encode_message(&GetSystemInfoResponse {
                        server_version: "wire-system-1".to_owned(),
                        capabilities: None,
                    }),
                ),
                GET_CLUSTER_INFO => (
                    "Bearer access-2",
                    encode_message(&GetClusterInfoResponse {
                        server_version: "wire-cluster-2".to_owned(),
                        cluster_id: "wire-cluster-id".to_owned(),
                        cluster_name: "wire-cluster".to_owned(),
                        history_shard_count: 4,
                        persistence_store: "wire-persistence".to_owned(),
                        visibility_store: "wire-visibility".to_owned(),
                        ..GetClusterInfoResponse::default()
                    }),
                ),
                _ => return Ok(grpc_status_response(12)),
            };
            if authorization.as_deref() != Some(expected_authorization) {
                return Ok(grpc_status_response(16));
            }
            Ok(grpc_message_response(&payload))
        })
    }
}

struct RefreshWireServer {
    address: std::net::SocketAddr,
    requests: Arc<Mutex<Vec<String>>>,
    errors: Arc<Mutex<Vec<String>>>,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl RefreshWireServer {
    async fn start(refresh_expires_at: i64) -> Self {
        let listener = random_loopback_listener().await;
        let address = listener.local_addr().expect("refresh listener address");
        let requests = Arc::new(Mutex::new(Vec::new()));
        let errors = Arc::new(Mutex::new(Vec::new()));
        let (shutdown, mut shutdown_receiver) = oneshot::channel();
        let task_requests = Arc::clone(&requests);
        let task_errors = Arc::clone(&errors);
        let task = tokio::spawn(async move {
            let mut rotation = 0_usize;
            loop {
                tokio::select! {
                    _ = &mut shutdown_receiver => break,
                    accepted = listener.accept() => {
                        let Ok((mut stream, _peer)) = accepted else {
                            task_errors.lock().expect("refresh error lock")
                                .push("could not accept refresh connection".to_owned());
                            break;
                        };
                        let request = match timeout(
                            Duration::from_secs(2),
                            read_http_request(&mut stream),
                        ).await {
                            Ok(Ok(request)) => request,
                            Ok(Err(error)) => {
                                task_errors.lock().expect("refresh error lock").push(error);
                                continue;
                            }
                            Err(_) => {
                                task_errors.lock().expect("refresh error lock")
                                    .push("timed out reading refresh request".to_owned());
                                continue;
                            }
                        };
                        let body = request_body(&request).unwrap_or_default().to_owned();
                        task_requests.lock().expect("refresh request lock").push(body.clone());
                        let expected_refresh = match rotation {
                            0 => "refresh-1",
                            1 => "refresh-2",
                            _ => {
                                task_errors.lock().expect("refresh error lock")
                                    .push("received an unexpected third refresh".to_owned());
                                write_http_response(&mut stream, 400, "{}").await;
                                continue;
                            }
                        };
                        let expected_body = format!(
                            "grant_type=refresh_token&client_id=temporal-cli&refresh_token={expected_refresh}"
                        );
                        let request_text = String::from_utf8_lossy(&request);
                        let valid = request_text.starts_with("POST /oauth/token HTTP/1.1\r\n")
                            && request_text
                                .to_ascii_lowercase()
                                .contains("content-type: application/x-www-form-urlencoded\r\n")
                            && body == expected_body;
                        if !valid {
                            task_errors.lock().expect("refresh error lock").push(
                                format!("invalid refresh request at rotation {rotation}"),
                            );
                            write_http_response(&mut stream, 400, "{}").await;
                            continue;
                        }

                        rotation += 1;
                        let expires_in = if rotation == 1 { 61 } else { 900 };
                        let response = json!({
                            "access_token": format!("access-{rotation}"),
                            "refresh_token": format!("refresh-{}", rotation + 1),
                            "token_type": "Bearer",
                            "expires_in": expires_in,
                            "refresh_expires_at": refresh_expires_at,
                        })
                        .to_string();
                        write_http_response(&mut stream, 200, &response).await;
                    }
                }
            }
        });
        Self {
            address,
            requests,
            errors,
            shutdown,
            task,
        }
    }

    async fn stop(self) {
        let _ = self.shutdown.send(());
        timeout(Duration::from_secs(2), self.task)
            .await
            .expect("refresh server shutdown timed out")
            .expect("refresh server task panicked");
        assert_eq!(
            *self.errors.lock().expect("refresh error lock"),
            Vec::<String>::new()
        );
        assert_eq!(
            *self.requests.lock().expect("refresh request lock"),
            [
                "grant_type=refresh_token&client_id=temporal-cli&refresh_token=refresh-1",
                "grant_type=refresh_token&client_id=temporal-cli&refresh_token=refresh-2",
            ]
        );
    }
}

#[tokio::test]
async fn authenticated_connection_rotates_refresh_and_updates_grpc_bearer() {
    let refresh_expires_at = Utc::now().timestamp() + 3_600;
    let refresh_server = RefreshWireServer::start(refresh_expires_at).await;
    let profile = TemporalAuthProfile {
        url: format!("http://{}/", refresh_server.address),
        username: "wire-user".to_owned(),
        token_endpoint: format!("http://{}/oauth/token", refresh_server.address),
        allow_insecure: true,
    };
    let keyring_item = credential_item_for_profile("wire", &profile).expect("wire credential item");
    let binding =
        credential_binding_for_profile("wire", &profile).expect("wire credential binding");
    let store = Arc::new(MemoryCredentialStore::default());
    store
        .set(
            KEYRING_SERVICE,
            &keyring_item,
            &json!({
                "binding_sha256": binding,
                "refresh_token": "refresh-1",
                "refresh_expires_at": refresh_expires_at,
            })
            .to_string(),
        )
        .expect("seed refresh credential");

    let grpc_listener = random_loopback_listener().await;
    let grpc_address = grpc_listener.local_addr().expect("gRPC listener address");
    let observations = Arc::new(Mutex::new(Vec::new()));
    let grpc_service = WorkflowWireService {
        observations: Arc::clone(&observations),
        store: Arc::clone(&store),
        keyring_item: keyring_item.clone(),
    };
    let (grpc_shutdown, grpc_shutdown_receiver) = oneshot::channel();
    let grpc_task = tokio::spawn(async move {
        Server::builder()
            .serve_with_incoming_shutdown(
                grpc_service,
                TcpListenerStream::new(grpc_listener),
                async {
                    let _ = grpc_shutdown_receiver.await;
                },
            )
            .await
    });

    let session = AuthSession::load_with_store("wire", profile, store.clone())
        .expect("load seeded auth session");
    let service = timeout(
        TEST_TIMEOUT,
        GrpcTemporalService::connect_with_auth(
            TemporalConnectionConfig {
                address: grpc_address.to_string(),
                api_key: None,
                headers: HashMap::default(),
                tls: None,
                payload_codec: None,
            },
            Some(session),
        ),
    )
    .await
    .expect("authenticated Temporal connection timed out")
    .expect("authenticated Temporal connection");

    tokio::time::pause();
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(50)).await;
    wait_for_persisted_refresh(&store, &keyring_item, "refresh-3").await;
    tokio::task::yield_now().await;
    tokio::time::resume();
    let cluster = timeout(TEST_TIMEOUT, service.cluster_info())
        .await
        .expect("GetClusterInfo timed out")
        .expect("GetClusterInfo");
    assert_eq!(cluster.cluster_name, "wire-cluster");
    assert_eq!(cluster.cluster_id, "wire-cluster-id");
    assert_eq!(cluster.server_version, "wire-cluster-2");

    let observed = observations.lock().expect("gRPC observation lock").clone();
    assert_eq!(
        observed,
        [
            GrpcObservation {
                path: GET_SYSTEM_INFO.to_owned(),
                authorization: Some("Bearer access-1".to_owned()),
                rotated_refresh_was_persisted: false,
            },
            GrpcObservation {
                path: GET_CLUSTER_INFO.to_owned(),
                authorization: Some("Bearer access-2".to_owned()),
                rotated_refresh_was_persisted: true,
            },
        ]
    );

    drop(service);
    let _ = grpc_shutdown.send(());
    timeout(Duration::from_secs(2), grpc_task)
        .await
        .expect("gRPC server shutdown timed out")
        .expect("gRPC server task panicked")
        .expect("gRPC server failed");
    refresh_server.stop().await;
}

#[tokio::test]
async fn authenticated_service_rejects_remote_plaintext_before_refresh() {
    let profile = TemporalAuthProfile {
        url: "http://127.0.0.1:9".to_owned(),
        username: "wire-user".to_owned(),
        token_endpoint: "http://127.0.0.1:9/oauth/token".to_owned(),
        allow_insecure: true,
    };
    let keyring_item =
        credential_item_for_profile("boundary", &profile).expect("boundary credential item");
    let binding = credential_binding_for_profile("boundary", &profile).expect("boundary binding");
    let store = Arc::new(MemoryCredentialStore::default());
    store
        .set(
            KEYRING_SERVICE,
            &keyring_item,
            &json!({
                "binding_sha256": binding,
                "refresh_token": "must-not-be-sent",
                "refresh_expires_at": Utc::now().timestamp() + 3_600,
            })
            .to_string(),
        )
        .expect("seed boundary credential");
    let session =
        AuthSession::load_with_store("boundary", profile, store).expect("load boundary session");

    let result = GrpcTemporalService::connect_with_auth(
        TemporalConnectionConfig {
            address: "temporal.example.test:7443".to_owned(),
            api_key: None,
            headers: HashMap::default(),
            tls: None,
            payload_codec: None,
        },
        Some(session),
    )
    .await;
    assert!(matches!(result, Err(ServiceError::ConnectionConfig(_))));
}

async fn random_loopback_listener() -> TcpListener {
    loop {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind random loopback port");
        if listener.local_addr().expect("loopback address").port() != 7_233 {
            return listener;
        }
    }
}

async fn wait_for_persisted_refresh(
    store: &MemoryCredentialStore,
    keyring_item: &str,
    expected: &str,
) {
    let deadline = Instant::now() + TEST_TIMEOUT;
    loop {
        if persisted_refresh(store, keyring_item).as_deref() == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "rotated refresh credential was not persisted"
        );
        sleep(Duration::from_millis(10)).await;
    }
}

fn persisted_refresh(store: &MemoryCredentialStore, keyring_item: &str) -> Option<String> {
    store
        .get(KEYRING_SERVICE, keyring_item)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|value| value["refresh_token"].as_str().map(str::to_owned))
}

async fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut request = Vec::with_capacity(1_024);
    let mut content_length = None;
    loop {
        let mut chunk = [0_u8; 1_024];
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|error| format!("could not read refresh request: {error}"))?;
        if read == 0 {
            return Err("refresh connection closed before request completed".to_owned());
        }
        request.extend_from_slice(&chunk[..read]);
        if request.len() > 16 * 1_024 {
            return Err("refresh request exceeded 16 KiB".to_owned());
        }
        if content_length.is_none()
            && let Some(header_end) = find_header_end(&request)
        {
            let headers = std::str::from_utf8(&request[..header_end])
                .map_err(|_| "refresh request headers were not UTF-8".to_owned())?;
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .ok_or_else(|| "refresh request omitted content-length".to_owned())?;
            content_length = Some((header_end, length));
        }
        if let Some((header_end, length)) = content_length
            && request.len() >= header_end + 4 + length
        {
            request.truncate(header_end + 4 + length);
            return Ok(request);
        }
    }
}

fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}

fn request_body(request: &[u8]) -> Option<&str> {
    let body_start = find_header_end(request)? + 4;
    std::str::from_utf8(&request[body_start..]).ok()
}

async fn write_http_response(stream: &mut TcpStream, status: u16, body: &str) {
    let reason = if status == 200 { "OK" } else { "Bad Request" };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(response.as_bytes())
        .await
        .expect("write refresh response");
    stream.shutdown().await.expect("close refresh response");
}

fn encode_message(message: &impl Message) -> Bytes {
    let mut payload = BytesMut::with_capacity(message.encoded_len());
    message
        .encode(&mut payload)
        .expect("encode protobuf message");
    payload.freeze()
}

fn grpc_message_response(payload: &Bytes) -> http::Response<Body> {
    let length = u32::try_from(payload.len()).expect("test protobuf fits a gRPC frame");
    let mut frame = BytesMut::with_capacity(payload.len() + 5);
    frame.put_u8(0);
    frame.put_u32(length);
    frame.extend_from_slice(payload);
    http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/grpc")
        .header("grpc-status", "0")
        .body(Body::new(Full::new(frame.freeze())))
        .expect("build gRPC response")
}

fn grpc_status_response(status: u16) -> http::Response<Body> {
    http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/grpc")
        .header("grpc-status", status.to_string())
        .body(Body::empty())
        .expect("build gRPC status response")
}
