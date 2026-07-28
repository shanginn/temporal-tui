use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use base64::prelude::*;
use chrono::{DateTime, Utc};
use futures_util::{StreamExt, future::BoxFuture};
use reqwest::{
    header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use temporalio_client::{
    Client, ClientOptions, ClientTlsOptions, Connection, ConnectionOptions, TlsOptions,
    WorkflowCancelOptions, WorkflowDescribeOptions, WorkflowExecutionInfo, WorkflowHandle,
    WorkflowTerminateOptions,
    tonic::{IntoRequest, Request, Status},
};
use temporalio_common::{
    UntypedWorkflow,
    data_converters::{PayloadConverter, RawValue},
    payload_visitor::{AsyncPayloadVisitor, PayloadField, PayloadFieldData, PayloadVisitable},
    protos::{
        proto_ts_to_system_time,
        temporal::api::{
            common::v1::{Payload, Payloads, WorkflowExecution as ProtoWorkflowExecution},
            deployment::v1::{
                WorkerDeploymentInfo as ProtoWorkerDeploymentInfo,
                WorkerDeploymentVersion as ProtoDeploymentVersion,
            },
            enums::v1::{
                EventType, PendingActivityState, RoutingConfigUpdateState,
                TaskQueueType as ProtoTaskQueueType, VersionDrainageStatus,
                WorkerDeploymentVersionStatus, WorkerStatus, WorkflowExecutionStatus,
            },
            failure::v1::{Failure, failure::FailureInfo},
            history::v1::{HistoryEvent, history_event::Attributes},
            taskqueue::v1::{TaskQueue as ProtoTaskQueue, TaskQueueStats as ProtoTaskQueueStats},
            worker::v1::{WorkerHeartbeat, WorkerListInfo, WorkerSlotsInfo},
            workflow::v1::PendingActivityInfo,
            workflow::v1::WorkflowExecutionInfo as ProtoWorkflowExecutionInfo,
            workflowservice::v1::{
                CountWorkflowExecutionsRequest, DescribeNamespaceResponse,
                DescribeTaskQueueRequest, DescribeTaskQueueResponse,
                DescribeWorkerDeploymentRequest, DescribeWorkerRequest, GetClusterInfoRequest,
                GetWorkflowExecutionHistoryReverseRequest, ListNamespacesRequest,
                ListWorkerDeploymentsRequest, ListWorkersRequest, ListWorkflowExecutionsRequest,
                SignalWorkflowExecutionRequest, list_worker_deployments_response,
            },
        },
    },
};
use thiserror::Error;
use url::Url;

use crate::model::{
    ClusterInfo, DeploymentVersion, DeploymentVersionSummary, FailureSummary, HistoryEventSummary,
    HistoryPage, NamespaceSummary, PendingActivitySummary, PollerSummary, StructuredField,
    TaskQueueStats, TaskQueueSummary, TaskQueueType, WorkerDeploymentDetails, WorkerDeploymentPage,
    WorkerDeploymentSummary, WorkerDetails, WorkerPage, WorkerSlots, WorkerSummary, WorkflowCount,
    WorkflowCountGroup, WorkflowDetails, WorkflowKey, WorkflowPage, WorkflowStatus,
    WorkflowSummary,
};

/// TLS settings loaded by the client at startup.
#[derive(Debug, Clone)]
pub struct ClientTlsConfig {
    pub server_ca: Option<PathBuf>,
    pub client_certificate: Option<PathBuf>,
    pub client_private_key: Option<PathBuf>,
    pub server_name: Option<String>,
}

/// Connection settings independent of the UI.
#[derive(Debug, Clone)]
pub struct TemporalConnectionConfig {
    pub address: String,
    pub api_key: Option<String>,
    pub headers: HashMap<String, String>,
    pub tls: Option<ClientTlsConfig>,
    pub payload_codec: Option<PayloadCodecConfig>,
}

/// Remote Temporal Codec Server settings.
#[derive(Debug, Clone)]
pub struct PayloadCodecConfig {
    pub endpoint: String,
    pub headers: HashMap<String, String>,
}

/// Errors returned by the Temporal service adapter.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("invalid Temporal address `{address}`: {source}")]
    InvalidAddress {
        address: String,
        source: url::ParseError,
    },

    #[error("could not read {kind} `{path}`: {source}")]
    CredentialFile {
        kind: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not connect to Temporal: {0}")]
    Connect(String),

    #[error("Temporal {operation} RPC failed: {source}")]
    Rpc {
        operation: &'static str,
        source: Status,
    },

    #[error("Temporal {operation} failed: {message}")]
    Client {
        operation: &'static str,
        message: String,
    },

    #[error("invalid Payload Codec configuration: {0}")]
    CodecConfig(String),

    #[error("Payload Codec {operation} failed: {message}")]
    Codec {
        operation: &'static str,
        message: String,
    },
}

const MAX_CODEC_CHUNK_PAYLOADS: usize = 128;
const MAX_CODEC_CHUNK_BYTES: usize = 4 * 1024 * 1024;
const MAX_CODEC_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
enum CodecOperation {
    Encode,
    Decode,
}

impl CodecOperation {
    const fn path(self) -> &'static str {
        match self {
            Self::Encode => "encode",
            Self::Decode => "decode",
        }
    }
}

#[derive(Clone)]
struct HttpPayloadCodec {
    client: reqwest::Client,
    endpoint_template: String,
    headers: HeaderMap,
}

impl HttpPayloadCodec {
    fn new(config: PayloadCodecConfig) -> Result<Self, ServiceError> {
        let mut headers = HeaderMap::new();
        for (name, value) in config.headers {
            if matches!(
                name.as_str(),
                "content-type" | "content-length" | "host" | "x-namespace"
            ) {
                return Err(ServiceError::CodecConfig(format!(
                    "header `{name}` is managed by temporal-tui"
                )));
            }
            let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
                ServiceError::CodecConfig("a Codec Server header name is invalid".to_string())
            })?;
            let value = HeaderValue::from_str(&value).map_err(|_| {
                ServiceError::CodecConfig(format!(
                    "Codec Server header `{name}` contains invalid bytes"
                ))
            })?;
            headers.insert(name, value);
        }

        let codec = Self {
            client: reqwest::Client::builder()
                .redirect(Policy::none())
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("temporal-tui/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|error| ServiceError::CodecConfig(error.to_string()))?,
            endpoint_template: config.endpoint,
            headers,
        };
        codec.url("validation", CodecOperation::Decode)?;
        Ok(codec)
    }

    async fn transform(
        &self,
        namespace: &str,
        operation: CodecOperation,
        payloads: Vec<Payload>,
    ) -> Result<Vec<Payload>, ServiceError> {
        if payloads.is_empty() {
            return Ok(Vec::new());
        }

        let expected = payloads.len();
        let mut transformed = Vec::with_capacity(expected);
        let mut chunk = Vec::new();
        let mut chunk_bytes = 0_usize;
        for payload in payloads {
            let payload_bytes = payload_wire_size(&payload);
            if payload_bytes > MAX_CODEC_CHUNK_BYTES {
                return Err(codec_error(
                    operation,
                    "one payload exceeds the 4 MiB Codec Server safety limit",
                ));
            }
            if !chunk.is_empty()
                && (chunk.len() >= MAX_CODEC_CHUNK_PAYLOADS
                    || chunk_bytes.saturating_add(payload_bytes) > MAX_CODEC_CHUNK_BYTES)
            {
                transformed.extend(
                    self.transform_chunk(namespace, operation, std::mem::take(&mut chunk))
                        .await?,
                );
                chunk_bytes = 0;
            }
            chunk_bytes = chunk_bytes.saturating_add(payload_bytes);
            chunk.push(payload);
        }
        if !chunk.is_empty() {
            transformed.extend(self.transform_chunk(namespace, operation, chunk).await?);
        }
        if transformed.len() != expected {
            return Err(codec_error(
                operation,
                format!(
                    "response contained {} payloads; expected {expected}",
                    transformed.len()
                ),
            ));
        }
        Ok(transformed)
    }

    async fn transform_chunk(
        &self,
        namespace: &str,
        operation: CodecOperation,
        payloads: Vec<Payload>,
    ) -> Result<Vec<Payload>, ServiceError> {
        let expected = payloads.len();
        let body =
            serde_json::to_vec(&CodecWirePayloads::from_payloads(payloads)).map_err(|error| {
                codec_error(operation, format!("could not encode request: {error}"))
            })?;
        if body.len() > MAX_CODEC_RESPONSE_BYTES {
            return Err(codec_error(
                operation,
                "encoded request exceeds the 8 MiB Codec Server safety limit",
            ));
        }
        let namespace_header = HeaderValue::from_str(namespace)
            .map_err(|_| codec_error(operation, "namespace contains invalid header bytes"))?;
        let response = self
            .client
            .post(self.url(namespace, operation)?)
            .headers(self.headers.clone())
            .header("x-namespace", namespace_header)
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| codec_error(operation, format!("request failed: {error}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(codec_error(
                operation,
                format!("server returned HTTP {status}"),
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_CODEC_RESPONSE_BYTES as u64)
        {
            return Err(codec_error(
                operation,
                "response exceeds the 8 MiB Codec Server safety limit",
            ));
        }
        let mut response_body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|error| codec_error(operation, format!("response failed: {error}")))?;
            if response_body.len().saturating_add(chunk.len()) > MAX_CODEC_RESPONSE_BYTES {
                return Err(codec_error(
                    operation,
                    "response exceeds the 8 MiB Codec Server safety limit",
                ));
            }
            response_body.extend_from_slice(&chunk);
        }
        let wire: CodecWirePayloads = serde_json::from_slice(&response_body).map_err(|error| {
            codec_error(
                operation,
                format!("response is not Payloads protobuf JSON: {error}"),
            )
        })?;
        let transformed = wire.into_payloads(operation)?;
        if transformed.len() != expected {
            return Err(codec_error(
                operation,
                format!(
                    "response contained {} payloads; expected {expected}",
                    transformed.len()
                ),
            ));
        }
        Ok(transformed)
    }

    fn url(&self, namespace: &str, operation: CodecOperation) -> Result<Url, ServiceError> {
        let encoded_namespace = encode_path_segment(namespace);
        let rendered = self
            .endpoint_template
            .replace("{namespace}", &encoded_namespace);
        let mut url = Url::parse(&rendered)
            .map_err(|error| ServiceError::CodecConfig(format!("invalid endpoint URL: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(ServiceError::CodecConfig(
                "endpoint must use http or https".to_string(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ServiceError::CodecConfig(
                "endpoint must not contain credentials; use secret headers".to_string(),
            ));
        }
        if url.fragment().is_some() {
            return Err(ServiceError::CodecConfig(
                "endpoint must not contain a fragment".to_string(),
            ));
        }
        let replace_operation = url
            .path_segments()
            .and_then(Iterator::last)
            .is_some_and(|segment| matches!(segment, "encode" | "decode"));
        {
            let mut segments = url.path_segments_mut().map_err(|()| {
                ServiceError::CodecConfig("endpoint cannot be used as a base URL".to_string())
            })?;
            segments.pop_if_empty();
            if replace_operation {
                segments.pop();
            }
            segments.push(operation.path());
        }
        Ok(url)
    }
}

fn codec_error(operation: CodecOperation, message: impl Into<String>) -> ServiceError {
    ServiceError::Codec {
        operation: operation.path(),
        message: message.into(),
    }
}

fn encode_path_segment(value: &str) -> String {
    let mut url = Url::parse("http://codec.invalid/").expect("static URL is valid");
    url.path_segments_mut()
        .expect("HTTP URL accepts path segments")
        .push(value);
    url.path().trim_start_matches('/').to_string()
}

fn payload_wire_size(payload: &Payload) -> usize {
    payload.data.len()
        + payload
            .metadata
            .iter()
            .map(|(key, value)| key.len().saturating_add(value.len()))
            .sum::<usize>()
        + payload.external_payloads.len().saturating_mul(16)
}

#[derive(Serialize, Deserialize)]
struct CodecWirePayloads {
    #[serde(default)]
    payloads: Vec<CodecWirePayload>,
}

impl CodecWirePayloads {
    fn from_payloads(payloads: Vec<Payload>) -> Self {
        Self {
            payloads: payloads.into_iter().map(CodecWirePayload::from).collect(),
        }
    }

    fn into_payloads(self, operation: CodecOperation) -> Result<Vec<Payload>, ServiceError> {
        self.payloads
            .into_iter()
            .map(|payload| payload.into_payload(operation))
            .collect()
    }
}

#[derive(Serialize, Deserialize)]
struct CodecWirePayload {
    #[serde(default)]
    metadata: HashMap<String, String>,
    #[serde(default)]
    data: String,
    #[serde(
        rename = "externalPayloads",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    external_payloads: Vec<CodecWireExternalPayload>,
}

impl From<Payload> for CodecWirePayload {
    fn from(payload: Payload) -> Self {
        Self {
            metadata: payload
                .metadata
                .into_iter()
                .map(|(key, value)| (key, BASE64_STANDARD.encode(value)))
                .collect(),
            data: BASE64_STANDARD.encode(payload.data),
            external_payloads: payload
                .external_payloads
                .into_iter()
                .map(|details| CodecWireExternalPayload {
                    size_bytes: details.size_bytes.to_string(),
                })
                .collect(),
        }
    }
}

impl CodecWirePayload {
    fn into_payload(self, operation: CodecOperation) -> Result<Payload, ServiceError> {
        let metadata = self
            .metadata
            .into_iter()
            .map(|(key, value)| BASE64_STANDARD.decode(value).map(|value| (key, value)))
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|_| codec_error(operation, "response metadata contains invalid base64"))?;
        let data = BASE64_STANDARD
            .decode(self.data)
            .map_err(|_| codec_error(operation, "response data contains invalid base64"))?;
        let external_payloads = self
            .external_payloads
            .into_iter()
            .map(|details| {
                details
                    .size_bytes
                    .parse::<i64>()
                    .map(|size_bytes| {
                        temporalio_common::protos::temporal::api::common::v1::payload::ExternalPayloadDetails {
                            size_bytes,
                        }
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                codec_error(
                    operation,
                    "response external payload size is not a protobuf int64 string",
                )
            })?;
        Ok(Payload {
            metadata,
            data,
            external_payloads,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct CodecWireExternalPayload {
    #[serde(rename = "sizeBytes")]
    size_bytes: String,
}

#[derive(Default)]
struct CollectPayloads {
    payloads: Vec<Payload>,
}

impl AsyncPayloadVisitor for CollectPayloads {
    fn visit<'a>(&'a mut self, field: PayloadField<'a>) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            match field.data {
                PayloadFieldData::Single(payload) => self.payloads.push(payload.clone()),
                PayloadFieldData::Repeated(payloads) => {
                    self.payloads.extend(payloads.iter().cloned());
                }
                PayloadFieldData::Payloads(payloads) => {
                    self.payloads.extend(payloads.payloads.iter().cloned());
                }
            }
        })
    }
}

struct ReplacePayloads {
    payloads: VecDeque<Payload>,
    missing: bool,
}

impl ReplacePayloads {
    fn replace(&mut self, target: &mut Payload) {
        if let Some(payload) = self.payloads.pop_front() {
            *target = payload;
        } else {
            self.missing = true;
        }
    }
}

impl AsyncPayloadVisitor for ReplacePayloads {
    fn visit<'a>(&'a mut self, field: PayloadField<'a>) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            match field.data {
                PayloadFieldData::Single(payload) => self.replace(payload),
                PayloadFieldData::Repeated(payloads) => {
                    for payload in payloads {
                        self.replace(payload);
                    }
                }
                PayloadFieldData::Payloads(payloads) => {
                    for payload in &mut payloads.payloads {
                        self.replace(payload);
                    }
                }
            }
        })
    }
}

/// Operations consumed by the dashboard.
#[async_trait]
pub trait TemporalService: Send + Sync {
    async fn cluster_info(&self) -> Result<ClusterInfo, ServiceError>;

    async fn list_namespaces(&self) -> Result<Vec<NamespaceSummary>, ServiceError>;

    async fn list_workflows(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkflowPage, ServiceError>;

    async fn count_workflows(
        &self,
        namespace: &str,
        query: &str,
    ) -> Result<WorkflowCount, ServiceError>;

    async fn describe_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
    ) -> Result<WorkflowDetails, ServiceError>;

    async fn load_history_page(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        next_page_token: Vec<u8>,
    ) -> Result<HistoryPage, ServiceError>;

    async fn list_workflow_chain(
        &self,
        namespace: &str,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowSummary>, ServiceError>;

    async fn list_task_queues(
        &self,
        namespace: &str,
        names: Vec<String>,
    ) -> Result<Vec<TaskQueueSummary>, ServiceError>;

    async fn list_workers(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkerPage, ServiceError>;

    async fn describe_worker(
        &self,
        namespace: &str,
        instance_key: &str,
    ) -> Result<WorkerDetails, ServiceError>;

    async fn list_worker_deployments(
        &self,
        namespace: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkerDeploymentPage, ServiceError>;

    async fn describe_worker_deployment(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<WorkerDeploymentDetails, ServiceError>;

    async fn cancel_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError>;

    async fn terminate_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError>;

    async fn signal_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        signal_name: &str,
        input: Value,
    ) -> Result<(), ServiceError>;
}

/// Temporal's official Rust client adapted to the dashboard boundary.
#[derive(Clone)]
pub struct GrpcTemporalService {
    connection: Connection,
    payload_codec: Option<HttpPayloadCodec>,
}

impl GrpcTemporalService {
    /// Connect and verify the Temporal frontend with `GetSystemInfo`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid address, unreadable TLS material, invalid
    /// client configuration, or an unreachable Temporal frontend.
    pub async fn connect(config: TemporalConnectionConfig) -> Result<Self, ServiceError> {
        let payload_codec = config
            .payload_codec
            .map(HttpPayloadCodec::new)
            .transpose()?;
        let address = normalize_address(&config.address, config.tls.is_some());
        let target = Url::parse(&address).map_err(|source| ServiceError::InvalidAddress {
            address: address.clone(),
            source,
        })?;
        let mut options = ConnectionOptions::new(target)
            .identity(client_identity())
            .build();
        options.api_key = config.api_key;
        options.headers = (!config.headers.is_empty()).then_some(config.headers);
        options.tls_options = match config.tls {
            Some(tls) => Some(load_tls_options(tls).await?),
            None => None,
        };

        let connection = Connection::connect(options)
            .await
            .map_err(|error| ServiceError::Connect(error.to_string()))?;
        Ok(Self {
            connection,
            payload_codec,
        })
    }

    fn client(&self, namespace: &str) -> Result<Client, ServiceError> {
        Client::new(
            self.connection.clone(),
            ClientOptions::new(namespace.to_string()).build(),
        )
        .map_err(|error| ServiceError::Client {
            operation: "create client",
            message: error.to_string(),
        })
    }

    fn workflow_handle(
        &self,
        namespace: &str,
        key: &WorkflowKey,
    ) -> Result<WorkflowHandle<Client, UntypedWorkflow>, ServiceError> {
        let client = self.client(namespace)?;
        Ok(WorkflowHandle::new(
            client,
            WorkflowExecutionInfo {
                namespace: namespace.to_string(),
                workflow_id: key.workflow_id.clone(),
                run_id: Some(key.run_id.clone()),
                first_execution_run_id: None,
            },
        ))
    }

    async fn decode_message<Message>(
        &self,
        namespace: &str,
        message: &mut Message,
    ) -> Result<(), ServiceError>
    where
        Message: PayloadVisitable + Send,
    {
        let Some(codec) = &self.payload_codec else {
            return Ok(());
        };
        let mut collector = CollectPayloads::default();
        message.visit_payloads_mut(&mut collector).await;
        if collector.payloads.is_empty() {
            return Ok(());
        }
        let expected = collector.payloads.len();
        let payloads = codec
            .transform(namespace, CodecOperation::Decode, collector.payloads)
            .await?;
        let mut replacer = ReplacePayloads {
            payloads: payloads.into(),
            missing: false,
        };
        message.visit_payloads_mut(&mut replacer).await;
        if replacer.missing || !replacer.payloads.is_empty() {
            return Err(codec_error(
                CodecOperation::Decode,
                format!("payload traversal changed while replacing {expected} values"),
            ));
        }
        Ok(())
    }

    async fn encode_payloads(
        &self,
        namespace: &str,
        payloads: Vec<Payload>,
    ) -> Result<Vec<Payload>, ServiceError> {
        match &self.payload_codec {
            Some(codec) => {
                codec
                    .transform(namespace, CodecOperation::Encode, payloads)
                    .await
            }
            None => Ok(payloads),
        }
    }

    async fn recent_history(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        next_page_token: Vec<u8>,
    ) -> Result<HistoryPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let mut response = service
            .get_workflow_execution_history_reverse(
                GetWorkflowExecutionHistoryReverseRequest {
                    namespace: namespace.to_string(),
                    execution: Some(ProtoWorkflowExecution {
                        workflow_id: key.workflow_id.clone(),
                        run_id: key.run_id.clone(),
                    }),
                    maximum_page_size: 200,
                    next_page_token,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "get recent workflow history",
                source,
            })?
            .into_inner();
        self.decode_message(namespace, &mut response).await?;
        let mut events = response
            .history
            .map(|history| history.events)
            .unwrap_or_default();
        events.reverse();
        Ok(HistoryPage {
            events: events.iter().map(history_event_summary).collect(),
            next_page_token: response.next_page_token,
            archived: false,
        })
    }
}

#[async_trait]
impl TemporalService for GrpcTemporalService {
    async fn cluster_info(&self) -> Result<ClusterInfo, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .get_cluster_info(Request::new(GetClusterInfoRequest::default()))
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "get cluster info",
                source,
            })?
            .into_inner();

        Ok(ClusterInfo {
            cluster_name: response.cluster_name,
            cluster_id: response.cluster_id,
            server_version: response.server_version,
            persistence_store: response.persistence_store,
            visibility_store: response.visibility_store,
            history_shard_count: response.history_shard_count,
        })
    }

    async fn list_namespaces(&self) -> Result<Vec<NamespaceSummary>, ServiceError> {
        let mut service = self.connection.workflow_service();
        let mut namespaces = Vec::new();
        let mut next_page_token = Vec::new();

        loop {
            let response = service
                .list_namespaces(
                    ListNamespacesRequest {
                        page_size: 200,
                        next_page_token: std::mem::take(&mut next_page_token),
                        ..Default::default()
                    }
                    .into_request(),
                )
                .await
                .map_err(|source| ServiceError::Rpc {
                    operation: "list namespaces",
                    source,
                })?
                .into_inner();

            namespaces.extend(response.namespaces.into_iter().map(namespace_summary));
            next_page_token = response.next_page_token;
            if next_page_token.is_empty() {
                break;
            }
        }

        namespaces.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(namespaces)
    }

    async fn list_workflows(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkflowPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .list_workflow_executions(
                ListWorkflowExecutionsRequest {
                    namespace: namespace.to_string(),
                    page_size: i32::try_from(page_size).unwrap_or(i32::MAX),
                    next_page_token,
                    query: query.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list workflows",
                source,
            })?
            .into_inner();

        Ok(WorkflowPage {
            workflows: response
                .executions
                .iter()
                .map(workflow_summary_from_proto)
                .collect(),
            next_page_token: response.next_page_token,
        })
    }

    async fn count_workflows(
        &self,
        namespace: &str,
        query: &str,
    ) -> Result<WorkflowCount, ServiceError> {
        let mut service = self.connection.workflow_service();
        let mut response = service
            .count_workflow_executions(
                CountWorkflowExecutionsRequest {
                    namespace: namespace.to_string(),
                    query: query.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "count workflows",
                source,
            })?
            .into_inner();
        self.decode_message(namespace, &mut response).await?;

        Ok(WorkflowCount {
            total: response.count,
            groups: response
                .groups
                .into_iter()
                .map(|group| WorkflowCountGroup {
                    values: group
                        .group_values
                        .iter()
                        .enumerate()
                        .map(|(index, payload)| {
                            payload_field(format!("group {}", index + 1), payload).value
                        })
                        .collect(),
                    count: group.count,
                })
                .collect(),
        })
    }

    async fn describe_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
    ) -> Result<WorkflowDetails, ServiceError> {
        let handle = self.workflow_handle(namespace, key)?;
        let describe = async {
            handle
                .describe(WorkflowDescribeOptions::default())
                .await
                .map_err(|error| ServiceError::Client {
                    operation: "describe workflow",
                    message: error.to_string(),
                })
        };
        let recent_history = self.recent_history(namespace, key, Vec::new());
        let (description, history) = tokio::try_join!(describe, recent_history)?;
        let mut raw = description.into_raw();
        self.decode_message(namespace, &mut raw).await?;
        let user_metadata = raw
            .execution_config
            .as_ref()
            .and_then(|config| config.user_metadata.as_ref());
        let static_summary = user_metadata
            .and_then(|metadata| metadata.summary.as_ref())
            .and_then(payload_display_text);
        let static_details = user_metadata
            .and_then(|metadata| metadata.details.as_ref())
            .and_then(payload_display_text);
        let raw_info =
            raw.workflow_execution_info
                .as_ref()
                .ok_or_else(|| ServiceError::Client {
                    operation: "describe workflow",
                    message: "response did not include workflow execution info".to_string(),
                })?;
        let summary = workflow_summary_from_proto(raw_info);

        let extended = raw.workflow_extended_info.as_ref();
        let parent = raw_info.parent_execution.as_ref();
        let root = raw_info.root_execution.as_ref();
        Ok(WorkflowDetails {
            summary,
            first_run_id: raw_info.first_run_id.clone(),
            parent_workflow_id: parent
                .map(|execution| execution.workflow_id.clone())
                .filter(|id| !id.is_empty()),
            parent_run_id: parent
                .map(|execution| execution.run_id.clone())
                .filter(|id| !id.is_empty()),
            root_workflow_id: root
                .map(|execution| execution.workflow_id.clone())
                .filter(|id| !id.is_empty()),
            root_run_id: root
                .map(|execution| execution.run_id.clone())
                .filter(|id| !id.is_empty()),
            reset_run_id: extended
                .map(|value| value.reset_run_id.clone())
                .filter(|id| !id.is_empty()),
            cancel_requested: extended.is_some_and(|value| value.cancel_requested),
            pending_activities: raw.pending_activities.len(),
            pending_activity_details: raw
                .pending_activities
                .iter()
                .map(pending_activity_summary)
                .collect(),
            pending_children: raw.pending_children.len(),
            pending_nexus_operations: raw.pending_nexus_operations.len(),
            state_transition_count: raw_info.state_transition_count,
            static_summary,
            static_details,
            memo: raw_info
                .memo
                .as_ref()
                .map_or_else(Vec::new, |memo| payload_map(&memo.fields)),
            search_attributes: raw_info
                .search_attributes
                .as_ref()
                .map_or_else(Vec::new, |attributes| {
                    payload_map(&attributes.indexed_fields)
                }),
            events: history.events,
            history_next_page_token: history.next_page_token,
            history_archived: history.archived,
        })
    }

    async fn load_history_page(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        next_page_token: Vec<u8>,
    ) -> Result<HistoryPage, ServiceError> {
        self.recent_history(namespace, key, next_page_token).await
    }

    async fn list_workflow_chain(
        &self,
        namespace: &str,
        workflow_id: &str,
    ) -> Result<Vec<WorkflowSummary>, ServiceError> {
        let escaped = workflow_id.replace('\'', "''");
        let query = format!("WorkflowId = '{escaped}'");
        let mut token = Vec::new();
        let mut workflows = Vec::new();
        loop {
            let page = self
                .list_workflows(namespace, &query, 200, std::mem::take(&mut token))
                .await?;
            workflows.extend(page.workflows);
            token = page.next_page_token;
            if token.is_empty() {
                break;
            }
        }
        workflows.sort_by_key(|workflow| std::cmp::Reverse(workflow.start_time));
        Ok(workflows)
    }

    async fn list_task_queues(
        &self,
        namespace: &str,
        mut names: Vec<String>,
    ) -> Result<Vec<TaskQueueSummary>, ServiceError> {
        names.retain(|name| !name.trim().is_empty());
        names.sort();
        names.dedup();
        let mut summaries = Vec::with_capacity(names.len().saturating_mul(2));
        let mut service = self.connection.workflow_service();
        for name in names {
            for (queue_type, proto_type) in [
                (TaskQueueType::Workflow, ProtoTaskQueueType::Workflow),
                (TaskQueueType::Activity, ProtoTaskQueueType::Activity),
            ] {
                let response = service
                    .describe_task_queue(
                        DescribeTaskQueueRequest {
                            namespace: namespace.to_string(),
                            task_queue: Some(ProtoTaskQueue {
                                name: name.clone(),
                                ..Default::default()
                            }),
                            task_queue_type: proto_type as i32,
                            report_stats: true,
                            report_config: true,
                            ..Default::default()
                        }
                        .into_request(),
                    )
                    .await
                    .map_err(|source| ServiceError::Rpc {
                        operation: "describe task queue",
                        source,
                    })?
                    .into_inner();
                summaries.push(task_queue_summary(name.clone(), queue_type, &response));
            }
        }
        Ok(summaries)
    }

    async fn list_workers(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkerPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .list_workers(
                ListWorkersRequest {
                    namespace: namespace.to_string(),
                    page_size: i32::try_from(page_size).unwrap_or(i32::MAX),
                    next_page_token,
                    query: query.to_string(),
                    include_system_workers: false,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list workers",
                source,
            })?
            .into_inner();
        Ok(WorkerPage {
            workers: response.workers.iter().map(worker_summary).collect(),
            next_page_token: response.next_page_token,
        })
    }

    async fn describe_worker(
        &self,
        namespace: &str,
        instance_key: &str,
    ) -> Result<WorkerDetails, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .describe_worker(
                DescribeWorkerRequest {
                    namespace: namespace.to_string(),
                    worker_instance_key: instance_key.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "describe worker",
                source,
            })?
            .into_inner();
        let heartbeat = response
            .worker_info
            .and_then(|info| info.worker_heartbeat)
            .ok_or_else(|| ServiceError::Client {
                operation: "describe worker",
                message: "response did not include a worker heartbeat".to_string(),
            })?;
        Ok(worker_details(&heartbeat))
    }

    async fn list_worker_deployments(
        &self,
        namespace: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<WorkerDeploymentPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .list_worker_deployments(
                ListWorkerDeploymentsRequest {
                    namespace: namespace.to_string(),
                    page_size: i32::try_from(page_size).unwrap_or(i32::MAX),
                    next_page_token,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list worker deployments",
                source,
            })?
            .into_inner();
        Ok(WorkerDeploymentPage {
            deployments: response
                .worker_deployments
                .iter()
                .map(worker_deployment_summary)
                .collect(),
            next_page_token: response.next_page_token,
        })
    }

    async fn describe_worker_deployment(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<WorkerDeploymentDetails, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .describe_worker_deployment(
                DescribeWorkerDeploymentRequest {
                    namespace: namespace.to_string(),
                    deployment_name: name.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "describe worker deployment",
                source,
            })?
            .into_inner();
        let info = response
            .worker_deployment_info
            .ok_or_else(|| ServiceError::Client {
                operation: "describe worker deployment",
                message: "response did not include deployment information".to_string(),
            })?;
        Ok(worker_deployment_details(&info))
    }

    async fn cancel_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError> {
        self.workflow_handle(namespace, key)?
            .cancel(
                WorkflowCancelOptions::builder()
                    .reason(reason.to_string())
                    .build(),
            )
            .await
            .map_err(|error| ServiceError::Client {
                operation: "request workflow cancellation",
                message: error.to_string(),
            })
    }

    async fn terminate_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError> {
        self.workflow_handle(namespace, key)?
            .terminate(
                WorkflowTerminateOptions::builder()
                    .reason(reason.to_string())
                    .build(),
            )
            .await
            .map_err(|error| ServiceError::Client {
                operation: "terminate workflow",
                message: error.to_string(),
            })
    }

    async fn signal_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        signal_name: &str,
        input: Value,
    ) -> Result<(), ServiceError> {
        let converter = PayloadConverter::serde_json();
        let input = RawValue::from_value(&input, &converter);
        let payloads = self.encode_payloads(namespace, input.payloads).await?;
        let mut service = self.connection.workflow_service();
        service
            .signal_workflow_execution(
                SignalWorkflowExecutionRequest {
                    namespace: namespace.to_string(),
                    workflow_execution: Some(ProtoWorkflowExecution {
                        workflow_id: key.workflow_id.clone(),
                        run_id: key.run_id.clone(),
                    }),
                    signal_name: signal_name.to_string(),
                    input: Some(Payloads { payloads }),
                    identity: client_identity(),
                    request_id: operation_request_id(),
                    ..Default::default()
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "signal workflow execution",
                source,
            })
            .map(|_| ())
    }
}

fn workflow_summary_from_proto(info: &ProtoWorkflowExecutionInfo) -> WorkflowSummary {
    let execution = info.execution.as_ref();
    WorkflowSummary {
        key: WorkflowKey {
            workflow_id: execution
                .map(|value| value.workflow_id.clone())
                .unwrap_or_default(),
            run_id: execution
                .map(|value| value.run_id.clone())
                .unwrap_or_default(),
        },
        workflow_type: info
            .r#type
            .as_ref()
            .map(|value| value.name.clone())
            .unwrap_or_default(),
        task_queue: info.task_queue.clone(),
        status: workflow_status(info.status),
        start_time: info.start_time.as_ref().and_then(proto_datetime),
        close_time: info.close_time.as_ref().and_then(proto_datetime),
        history_length: info.history_length,
        history_size_bytes: info.history_size_bytes,
    }
}

fn workflow_status(raw: i32) -> WorkflowStatus {
    match WorkflowExecutionStatus::try_from(raw) {
        Ok(WorkflowExecutionStatus::Running) => WorkflowStatus::Running,
        Ok(WorkflowExecutionStatus::Completed) => WorkflowStatus::Completed,
        Ok(WorkflowExecutionStatus::Failed) => WorkflowStatus::Failed,
        Ok(WorkflowExecutionStatus::Canceled) => WorkflowStatus::Canceled,
        Ok(WorkflowExecutionStatus::Terminated) => WorkflowStatus::Terminated,
        Ok(WorkflowExecutionStatus::ContinuedAsNew) => WorkflowStatus::ContinuedAsNew,
        Ok(WorkflowExecutionStatus::TimedOut) => WorkflowStatus::TimedOut,
        Ok(WorkflowExecutionStatus::Paused) => WorkflowStatus::Paused,
        Ok(WorkflowExecutionStatus::Unspecified) => WorkflowStatus::Unspecified,
        Err(_) => WorkflowStatus::Unknown(raw),
    }
}

fn namespace_summary(namespace: DescribeNamespaceResponse) -> NamespaceSummary {
    let info = namespace.namespace_info.unwrap_or_default();
    let config = namespace.config.unwrap_or_default();
    let replication = namespace.replication_config.unwrap_or_default();
    let state = format!("{:?}", info.state()).to_ascii_uppercase();
    NamespaceSummary {
        name: info.name,
        id: info.id,
        description: info.description,
        state,
        retention: config
            .workflow_execution_retention_ttl
            .as_ref()
            .map_or_else(|| "server default".to_string(), format_duration),
        active_cluster: replication.active_cluster_name,
        is_global: namespace.is_global_namespace,
    }
}

fn task_queue_summary(
    name: String,
    queue_type: TaskQueueType,
    response: &DescribeTaskQueueResponse,
) -> TaskQueueSummary {
    let versioning = response.versioning_info.as_ref();
    TaskQueueSummary {
        name,
        queue_type,
        pollers: response
            .pollers
            .iter()
            .map(|poller| {
                let deployment = poller.deployment_options.as_ref();
                PollerSummary {
                    identity: poller.identity.clone(),
                    last_access_time: poller.last_access_time.as_ref().and_then(proto_datetime),
                    rate_per_second: poller.rate_per_second,
                    deployment_name: deployment
                        .map(|options| options.deployment_name.clone())
                        .unwrap_or_default(),
                    build_id: deployment
                        .map(|options| options.build_id.clone())
                        .unwrap_or_default(),
                }
            })
            .collect(),
        stats: response
            .stats
            .as_ref()
            .map_or_else(TaskQueueStats::default, task_queue_stats),
        current_deployment: versioning
            .and_then(|info| info.current_deployment_version.as_ref())
            .map(deployment_version),
        ramping_deployment: versioning
            .and_then(|info| info.ramping_deployment_version.as_ref())
            .map(deployment_version),
        ramping_percentage: versioning.map_or(0.0, |info| info.ramping_version_percentage),
        effective_rate_limit: response
            .effective_rate_limit
            .as_ref()
            .map(|limit| limit.requests_per_second),
    }
}

fn task_queue_stats(stats: &ProtoTaskQueueStats) -> TaskQueueStats {
    TaskQueueStats {
        approximate_backlog_count: stats.approximate_backlog_count,
        approximate_backlog_age_seconds: stats
            .approximate_backlog_age
            .as_ref()
            .map_or(0.0, duration_seconds),
        tasks_add_rate: stats.tasks_add_rate,
        tasks_dispatch_rate: stats.tasks_dispatch_rate,
    }
}

fn worker_summary(worker: &WorkerListInfo) -> WorkerSummary {
    WorkerSummary {
        instance_key: worker.worker_instance_key.clone(),
        identity: worker.worker_identity.clone(),
        task_queue: worker.task_queue.clone(),
        deployment: worker.deployment_version.as_ref().map(deployment_version),
        sdk_name: worker.sdk_name.clone(),
        sdk_version: worker.sdk_version.clone(),
        status: enum_label::<WorkerStatus>(worker.status),
        start_time: worker.start_time.as_ref().and_then(proto_datetime),
        host_name: worker.host_name.clone(),
        process_id: worker.process_id.clone(),
        plugins: worker
            .plugins
            .iter()
            .map(|plugin| {
                if plugin.version.is_empty() {
                    plugin.name.clone()
                } else {
                    format!("{}@{}", plugin.name, plugin.version)
                }
            })
            .collect(),
    }
}

fn worker_summary_from_heartbeat(worker: &WorkerHeartbeat) -> WorkerSummary {
    let host = worker.host_info.as_ref();
    WorkerSummary {
        instance_key: worker.worker_instance_key.clone(),
        identity: worker.worker_identity.clone(),
        task_queue: worker.task_queue.clone(),
        deployment: worker.deployment_version.as_ref().map(deployment_version),
        sdk_name: worker.sdk_name.clone(),
        sdk_version: worker.sdk_version.clone(),
        status: enum_label::<WorkerStatus>(worker.status),
        start_time: worker.start_time.as_ref().and_then(proto_datetime),
        host_name: host.map(|host| host.host_name.clone()).unwrap_or_default(),
        process_id: host.map(|host| host.process_id.clone()).unwrap_or_default(),
        plugins: worker
            .plugins
            .iter()
            .map(|plugin| {
                if plugin.version.is_empty() {
                    plugin.name.clone()
                } else {
                    format!("{}@{}", plugin.name, plugin.version)
                }
            })
            .collect(),
    }
}

fn worker_details(worker: &WorkerHeartbeat) -> WorkerDetails {
    let host = worker.host_info.as_ref();
    WorkerDetails {
        summary: worker_summary_from_heartbeat(worker),
        heartbeat_time: worker.heartbeat_time.as_ref().and_then(proto_datetime),
        elapsed_since_heartbeat_seconds: worker
            .elapsed_since_last_heartbeat
            .as_ref()
            .map_or(0.0, duration_seconds),
        host_cpu_usage: host.map_or(0.0, |host| host.current_host_cpu_usage),
        host_memory_usage: host.map_or(0.0, |host| host.current_host_mem_usage),
        workflow_slots: worker
            .workflow_task_slots_info
            .as_ref()
            .map_or_else(WorkerSlots::default, worker_slots),
        activity_slots: worker
            .activity_task_slots_info
            .as_ref()
            .map_or_else(WorkerSlots::default, worker_slots),
        local_activity_slots: worker
            .local_activity_slots_info
            .as_ref()
            .map_or_else(WorkerSlots::default, worker_slots),
        nexus_slots: worker
            .nexus_task_slots_info
            .as_ref()
            .map_or_else(WorkerSlots::default, worker_slots),
        workflow_pollers: worker
            .workflow_poller_info
            .as_ref()
            .map_or(0, |poller| poller.current_pollers),
        activity_pollers: worker
            .activity_poller_info
            .as_ref()
            .map_or(0, |poller| poller.current_pollers),
        nexus_pollers: worker
            .nexus_poller_info
            .as_ref()
            .map_or(0, |poller| poller.current_pollers),
        sticky_cache_hits: worker.total_sticky_cache_hit,
        sticky_cache_misses: worker.total_sticky_cache_miss,
        sticky_cache_size: worker.current_sticky_cache_size,
    }
}

fn worker_slots(slots: &WorkerSlotsInfo) -> WorkerSlots {
    WorkerSlots {
        available: slots.current_available_slots,
        used: slots.current_used_slots,
        supplier: slots.slot_supplier_kind.clone(),
        processed: slots.total_processed_tasks,
        failed: slots.total_failed_tasks,
    }
}

fn worker_deployment_summary(
    deployment: &list_worker_deployments_response::WorkerDeploymentSummary,
) -> WorkerDeploymentSummary {
    let routing = deployment.routing_config.as_ref();
    WorkerDeploymentSummary {
        name: deployment.name.clone(),
        create_time: deployment.create_time.as_ref().and_then(proto_datetime),
        current_version: routing
            .and_then(|value| value.current_deployment_version.as_ref())
            .map(deployment_version),
        ramping_version: routing
            .and_then(|value| value.ramping_deployment_version.as_ref())
            .map(deployment_version),
        ramping_percentage: routing.map_or(0.0, |value| value.ramping_version_percentage),
        latest_version: deployment
            .latest_version_summary
            .as_ref()
            .and_then(|version| version.deployment_version.as_ref())
            .map(deployment_version),
    }
}

fn worker_deployment_details(info: &ProtoWorkerDeploymentInfo) -> WorkerDeploymentDetails {
    let routing = info.routing_config.as_ref();
    let current_version = routing
        .and_then(|value| value.current_deployment_version.as_ref())
        .map(deployment_version);
    let ramping_version = routing
        .and_then(|value| value.ramping_deployment_version.as_ref())
        .map(deployment_version);
    let latest_version = info
        .version_summaries
        .iter()
        .max_by_key(|version| version.create_time.as_ref().map(timestamp_key))
        .and_then(|version| version.deployment_version.as_ref())
        .map(deployment_version);
    let summary = WorkerDeploymentSummary {
        name: info.name.clone(),
        create_time: info.create_time.as_ref().and_then(proto_datetime),
        current_version: current_version.clone(),
        ramping_version: ramping_version.clone(),
        ramping_percentage: routing.map_or(0.0, |value| value.ramping_version_percentage),
        latest_version,
    };
    let versions = info
        .version_summaries
        .iter()
        .filter_map(|version| {
            let deployment = version
                .deployment_version
                .as_ref()
                .map(deployment_version)?;
            let is_current = current_version.as_ref() == Some(&deployment);
            let is_ramping = ramping_version.as_ref() == Some(&deployment);
            Some(DeploymentVersionSummary {
                version: deployment,
                status: enum_label::<WorkerDeploymentVersionStatus>(version.status),
                create_time: version.create_time.as_ref().and_then(proto_datetime),
                is_current,
                is_ramping,
                ramp_percentage: if is_ramping {
                    summary.ramping_percentage
                } else {
                    0.0
                },
                drainage_status: version.drainage_info.as_ref().map_or_else(
                    || "UNSPECIFIED".to_string(),
                    |drainage| enum_label::<VersionDrainageStatus>(drainage.status),
                ),
                drainage_last_checked: version
                    .drainage_info
                    .as_ref()
                    .and_then(|drainage| drainage.last_checked_time.as_ref())
                    .and_then(proto_datetime),
            })
        })
        .collect();
    WorkerDeploymentDetails {
        summary,
        versions,
        manager_identity: info.manager_identity.clone(),
        last_modifier_identity: info.last_modifier_identity.clone(),
        routing_update_state: enum_label::<RoutingConfigUpdateState>(
            info.routing_config_update_state,
        ),
    }
}

fn deployment_version(version: &ProtoDeploymentVersion) -> DeploymentVersion {
    DeploymentVersion {
        deployment_name: version.deployment_name.clone(),
        build_id: version.build_id.clone(),
    }
}

fn enum_label<Enum>(raw: i32) -> String
where
    Enum: TryFrom<i32> + std::fmt::Debug,
{
    Enum::try_from(raw).map_or_else(
        |_| format!("UNKNOWN ({raw})"),
        |value| prettify_debug_name(&format!("{value:?}")),
    )
}

fn timestamp_key(timestamp: &prost_wkt_types::Timestamp) -> (i64, i32) {
    (timestamp.seconds, timestamp.nanos)
}

fn duration_seconds(duration: &prost_wkt_types::Duration) -> f64 {
    std::time::Duration::try_from(*duration).map_or(0.0, |value| value.as_secs_f64())
}

fn history_event_summary(event: &HistoryEvent) -> HistoryEventSummary {
    let event_type = EventType::try_from(event.event_type).map_or_else(
        |_| format!("EVENT_{}", event.event_type),
        |kind| prettify_debug_name(&format!("{kind:?}")),
    );
    let (fields, failure) = history_event_data(event);
    HistoryEventSummary {
        event_id: event.event_id,
        event_type,
        event_time: event.event_time.as_ref().and_then(proto_datetime),
        detail: history_event_detail(event),
        fields,
        failure,
    }
}

fn history_event_data(event: &HistoryEvent) -> (Vec<StructuredField>, Option<FailureSummary>) {
    match event.attributes.as_ref() {
        Some(Attributes::WorkflowExecutionStartedEventAttributes(attributes)) => (
            payload_fields("input", attributes.input.as_ref()),
            attributes.continued_failure.as_ref().map(failure_summary),
        ),
        Some(Attributes::WorkflowExecutionCompletedEventAttributes(attributes)) => {
            (payload_fields("result", attributes.result.as_ref()), None)
        }
        Some(Attributes::WorkflowExecutionFailedEventAttributes(attributes)) => {
            (Vec::new(), attributes.failure.as_ref().map(failure_summary))
        }
        Some(Attributes::WorkflowExecutionCanceledEventAttributes(attributes)) => {
            (payload_fields("details", attributes.details.as_ref()), None)
        }
        Some(Attributes::WorkflowExecutionTerminatedEventAttributes(attributes)) => {
            (payload_fields("details", attributes.details.as_ref()), None)
        }
        Some(Attributes::WorkflowExecutionSignaledEventAttributes(attributes)) => {
            (payload_fields("input", attributes.input.as_ref()), None)
        }
        Some(Attributes::ActivityTaskScheduledEventAttributes(attributes)) => {
            (payload_fields("input", attributes.input.as_ref()), None)
        }
        Some(Attributes::ActivityTaskCompletedEventAttributes(attributes)) => {
            (payload_fields("result", attributes.result.as_ref()), None)
        }
        Some(Attributes::ActivityTaskFailedEventAttributes(attributes)) => {
            (Vec::new(), attributes.failure.as_ref().map(failure_summary))
        }
        Some(Attributes::ActivityTaskCanceledEventAttributes(attributes)) => {
            (payload_fields("details", attributes.details.as_ref()), None)
        }
        Some(Attributes::MarkerRecordedEventAttributes(attributes)) => {
            let mut fields = attributes
                .details
                .iter()
                .flat_map(|(name, payloads)| payload_fields(name, Some(payloads)))
                .collect::<Vec<_>>();
            fields.sort_by(|left, right| left.name.cmp(&right.name));
            (fields, attributes.failure.as_ref().map(failure_summary))
        }
        _ => (Vec::new(), None),
    }
}

fn history_event_detail(event: &HistoryEvent) -> String {
    let detail = match event.attributes.as_ref() {
        Some(Attributes::WorkflowExecutionStartedEventAttributes(attributes)) => {
            let workflow_type = attributes
                .workflow_type
                .as_ref()
                .map_or("", |value| value.name.as_str());
            let task_queue = attributes
                .task_queue
                .as_ref()
                .map_or("", |value| value.name.as_str());
            join_detail([workflow_type, task_queue])
        }
        Some(Attributes::ActivityTaskScheduledEventAttributes(attributes)) => {
            let activity_type = attributes
                .activity_type
                .as_ref()
                .map_or("", |value| value.name.as_str());
            join_detail([attributes.activity_id.as_str(), activity_type])
        }
        Some(Attributes::WorkflowExecutionSignaledEventAttributes(attributes)) => {
            attributes.signal_name.clone()
        }
        Some(Attributes::WorkflowExecutionTerminatedEventAttributes(attributes)) => {
            attributes.reason.clone()
        }
        Some(Attributes::WorkflowExecutionCancelRequestedEventAttributes(attributes)) => {
            attributes.cause.clone()
        }
        Some(Attributes::MarkerRecordedEventAttributes(attributes)) => {
            attributes.marker_name.clone()
        }
        Some(Attributes::TimerStartedEventAttributes(attributes)) => attributes.timer_id.clone(),
        Some(Attributes::WorkflowExecutionFailedEventAttributes(attributes)) => attributes
            .failure
            .as_ref()
            .map(|failure| failure.message.clone())
            .unwrap_or_default(),
        Some(Attributes::ActivityTaskFailedEventAttributes(attributes)) => attributes
            .failure
            .as_ref()
            .map(|failure| failure.message.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };

    if detail.is_empty() {
        event
            .principal
            .as_ref()
            .map(|principal| join_detail([principal.r#type.as_str(), principal.name.as_str()]))
            .unwrap_or_default()
    } else {
        detail
    }
}

fn pending_activity_summary(activity: &PendingActivityInfo) -> PendingActivitySummary {
    let state = PendingActivityState::try_from(activity.state).map_or_else(
        |_| format!("STATE_{}", activity.state),
        |state| prettify_debug_name(&format!("{state:?}")),
    );
    PendingActivitySummary {
        activity_id: activity.activity_id.clone(),
        activity_type: activity
            .activity_type
            .as_ref()
            .map(|value| value.name.clone())
            .unwrap_or_default(),
        state,
        attempt: activity.attempt,
        maximum_attempts: activity.maximum_attempts,
        last_worker_identity: activity.last_worker_identity.clone(),
        paused: activity.paused,
        last_failure: activity.last_failure.as_ref().map(failure_summary),
    }
}

fn failure_summary(failure: &Failure) -> FailureSummary {
    let kind = match failure.failure_info.as_ref() {
        Some(FailureInfo::ApplicationFailureInfo(info)) if !info.r#type.is_empty() => {
            format!("APPLICATION ({})", info.r#type)
        }
        Some(FailureInfo::ApplicationFailureInfo(_)) => "APPLICATION".to_string(),
        Some(FailureInfo::TimeoutFailureInfo(_)) => "TIMEOUT".to_string(),
        Some(FailureInfo::CanceledFailureInfo(_)) => "CANCELED".to_string(),
        Some(FailureInfo::TerminatedFailureInfo(_)) => "TERMINATED".to_string(),
        Some(FailureInfo::ServerFailureInfo(_)) => "SERVER".to_string(),
        Some(FailureInfo::ResetWorkflowFailureInfo(_)) => "RESET".to_string(),
        Some(FailureInfo::ActivityFailureInfo(_)) => "ACTIVITY".to_string(),
        Some(FailureInfo::ChildWorkflowExecutionFailureInfo(_)) => "CHILD WORKFLOW".to_string(),
        Some(FailureInfo::NexusOperationExecutionFailureInfo(_)) => "NEXUS OPERATION".to_string(),
        Some(FailureInfo::NexusHandlerFailureInfo(_)) => "NEXUS HANDLER".to_string(),
        None => "FAILURE".to_string(),
    };
    FailureSummary {
        message: truncate_text(&failure.message, 4_096),
        source: truncate_text(&failure.source, 256),
        kind,
        stack_trace: truncate_text(&failure.stack_trace, 16_384),
        encoded_attributes: failure
            .encoded_attributes
            .as_ref()
            .map(|payload| payload_field("encoded attributes", payload)),
        cause: failure.cause.as_deref().map(failure_summary).map(Box::new),
    }
}

fn payload_map(values: &HashMap<String, Payload>) -> Vec<StructuredField> {
    let mut fields = values
        .iter()
        .map(|(name, payload)| payload_field(name.clone(), payload))
        .collect::<Vec<_>>();
    fields.sort_by(|left, right| left.name.cmp(&right.name));
    fields
}

fn payload_fields(prefix: &str, payloads: Option<&Payloads>) -> Vec<StructuredField> {
    payloads.map_or_else(Vec::new, |payloads| {
        payloads
            .payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| {
                let name = if payloads.payloads.len() == 1 {
                    prefix.to_string()
                } else {
                    format!("{prefix}[{}]", index + 1)
                };
                payload_field(name, payload)
            })
            .collect()
    })
}

fn payload_field(name: impl Into<String>, payload: &Payload) -> StructuredField {
    const MAX_PREVIEW_BYTES: usize = 4_096;

    let name = name.into();
    let encoding = payload
        .metadata
        .get("encoding")
        .and_then(|value| std::str::from_utf8(value).ok())
        .unwrap_or("binary/unknown")
        .to_string();
    let size_bytes = payload.data.len()
        + payload
            .external_payloads
            .iter()
            .filter_map(|value| usize::try_from(value.size_bytes).ok())
            .sum::<usize>();
    let mut redacted = looks_sensitive_field_name(&name);
    let value = if redacted {
        "<redacted>".to_string()
    } else if !payload.external_payloads.is_empty() && payload.data.is_empty() {
        format!("<{size_bytes} externally stored bytes>")
    } else if matches!(encoding.as_str(), "json/plain" | "json/protobuf") {
        serde_json::from_slice::<Value>(&payload.data).map_or_else(
            |_| truncate_bytes_lossy(&payload.data, MAX_PREVIEW_BYTES),
            |mut value| {
                redacted |= redact_json_value(&mut value);
                let rendered =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                truncate_text(&rendered, MAX_PREVIEW_BYTES)
            },
        )
    } else if matches!(encoding.as_str(), "binary/plain" | "binary/null") {
        truncate_bytes_lossy(&payload.data, MAX_PREVIEW_BYTES)
    } else {
        let preview = &payload.data[..payload.data.len().min(MAX_PREVIEW_BYTES)];
        let mut rendered = BASE64_STANDARD.encode(preview);
        if preview.len() < payload.data.len() {
            rendered.push('…');
        }
        rendered
    };
    StructuredField {
        name,
        encoding,
        value,
        size_bytes,
        redacted,
    }
}

fn payload_display_text(payload: &Payload) -> Option<String> {
    if payload.data.is_empty() {
        return None;
    }
    let encoding = payload
        .metadata
        .get("encoding")
        .and_then(|value| std::str::from_utf8(value).ok());
    if matches!(encoding, Some("json/plain" | "json/protobuf"))
        && let Ok(Value::String(value)) = serde_json::from_slice::<Value>(&payload.data)
    {
        return Some(truncate_text(&value, 20_000));
    }
    Some(payload_field("user metadata", payload).value)
}

fn redact_json_value(value: &mut Value) -> bool {
    match value {
        Value::Object(object) => {
            let mut redacted = false;
            for (key, value) in object {
                if looks_sensitive_field_name(key) {
                    *value = Value::String("<redacted>".to_string());
                    redacted = true;
                } else {
                    redacted |= redact_json_value(value);
                }
            }
            redacted
        }
        Value::Array(values) => {
            let mut redacted = false;
            for value in values {
                redacted |= redact_json_value(value);
            }
            redacted
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => false,
    }
}

fn looks_sensitive_field_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace(['-', '.'], "_");
    [
        "password",
        "passwd",
        "secret",
        "token",
        "api_key",
        "authorization",
        "credential",
        "cookie",
    ]
    .iter()
    .any(|needle| normalized.contains(needle))
}

fn truncate_bytes_lossy(value: &[u8], limit: usize) -> String {
    let preview = &value[..value.len().min(limit)];
    let mut rendered = String::from_utf8_lossy(preview).into_owned();
    if preview.len() < value.len() {
        rendered.push('…');
    }
    rendered
}

fn truncate_text(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut boundary = limit;
    while !value.is_char_boundary(boundary) {
        boundary = boundary.saturating_sub(1);
    }
    format!("{}…", &value[..boundary])
}

fn join_detail<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn prettify_debug_name(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 8);
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            result.push(' ');
        }
        result.push(character.to_ascii_uppercase());
    }
    result
}

fn proto_datetime(timestamp: &prost_wkt_types::Timestamp) -> Option<DateTime<Utc>> {
    proto_ts_to_system_time(timestamp).map(DateTime::<Utc>::from)
}

fn format_duration(duration: &prost_wkt_types::Duration) -> String {
    let seconds = duration.seconds.max(0).cast_unsigned();
    if seconds == 0 {
        "0s".to_string()
    } else if seconds.is_multiple_of(86_400) {
        format!("{}d", seconds / 86_400)
    } else if seconds.is_multiple_of(3_600) {
        format!("{}h", seconds / 3_600)
    } else if seconds.is_multiple_of(60) {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

fn normalize_address(address: &str, tls: bool) -> String {
    let address = address.trim();
    if tls && let Some(rest) = address.strip_prefix("http://") {
        return format!("https://{rest}");
    }
    if address.contains("://") {
        address.to_string()
    } else if tls {
        format!("https://{address}")
    } else {
        format!("http://{address}")
    }
}

fn client_identity() -> String {
    let host = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    format!("temporal-tui@{host}-{}", std::process::id())
}

fn operation_request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("temporal-tui-{}-{nanos}", std::process::id())
}

async fn load_tls_options(config: ClientTlsConfig) -> Result<TlsOptions, ServiceError> {
    let server_root_ca_cert = read_optional(config.server_ca, "server CA certificate").await?;
    let client_cert = read_optional(config.client_certificate, "client certificate").await?;
    let client_private_key = read_optional(config.client_private_key, "client private key").await?;
    let client_tls_options =
        client_cert
            .zip(client_private_key)
            .map(|(client_cert, client_private_key)| ClientTlsOptions {
                client_cert,
                client_private_key,
            });

    Ok(TlsOptions {
        server_root_ca_cert,
        domain: config.server_name,
        client_tls_options,
        server_cert_verifier: None,
    })
}

async fn read_optional(
    path: Option<PathBuf>,
    kind: &'static str,
) -> Result<Option<Vec<u8>>, ServiceError> {
    let Some(path) = path else {
        return Ok(None);
    };
    tokio::fs::read(&path)
        .await
        .map(Some)
        .map_err(|source| ServiceError::CredentialFile { kind, path, source })
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc,
        thread,
    };

    use super::*;
    use prost_wkt_types;
    use temporalio_common::protos::temporal::api::{
        common::v1::{ActivityType, WorkflowExecution, WorkflowType},
        deployment::v1::{
            RoutingConfig, VersionDrainageInfo, WorkerDeploymentOptions, WorkerDeploymentVersion,
            worker_deployment_info::WorkerDeploymentVersionSummary,
        },
        enums::v1::{
            RoutingConfigUpdateState, VersionDrainageStatus, WorkerDeploymentVersionStatus,
            WorkerStatus,
        },
        history::v1::ActivityTaskScheduledEventAttributes,
        taskqueue::v1::{PollerInfo, TaskQueueVersioningInfo},
        worker::v1::{WorkerHostInfo, WorkerPollerInfo},
    };

    #[test]
    fn normalizes_plain_and_tls_addresses() {
        assert_eq!(
            normalize_address("localhost:7233", false),
            "http://localhost:7233"
        );
        assert_eq!(
            normalize_address("localhost:7233", true),
            "https://localhost:7233"
        );
        assert_eq!(
            normalize_address("http://cloud.example:7233", true),
            "https://cloud.example:7233"
        );
    }

    #[test]
    fn maps_forward_compatible_workflow_status() {
        assert_eq!(workflow_status(1), WorkflowStatus::Running);
        assert_eq!(workflow_status(8), WorkflowStatus::Paused);
        assert_eq!(workflow_status(9_999), WorkflowStatus::Unknown(9_999));
    }

    #[test]
    fn maps_visibility_workflow_without_panicking_on_missing_fields() {
        let info = ProtoWorkflowExecutionInfo {
            execution: Some(WorkflowExecution {
                workflow_id: "order-42".to_string(),
                run_id: "run-1".to_string(),
            }),
            r#type: Some(WorkflowType {
                name: "OrderWorkflow".to_string(),
            }),
            status: WorkflowExecutionStatus::Running as i32,
            history_length: 17,
            task_queue: "orders".to_string(),
            ..Default::default()
        };
        let summary = workflow_summary_from_proto(&info);
        assert_eq!(summary.key.workflow_id, "order-42");
        assert_eq!(summary.workflow_type, "OrderWorkflow");
        assert_eq!(summary.status, WorkflowStatus::Running);
    }

    #[test]
    fn extracts_safe_history_event_detail() {
        let event = HistoryEvent {
            event_id: 8,
            event_type: EventType::ActivityTaskScheduled as i32,
            attributes: Some(Attributes::ActivityTaskScheduledEventAttributes(
                ActivityTaskScheduledEventAttributes {
                    activity_id: "charge-card".to_string(),
                    activity_type: Some(ActivityType {
                        name: "ChargeCard".to_string(),
                    }),
                    ..Default::default()
                },
            )),
            ..Default::default()
        };
        let summary = history_event_summary(&event);
        assert_eq!(summary.event_type, "ACTIVITY TASK SCHEDULED");
        assert_eq!(summary.detail, "charge-card · ChargeCard");
    }

    #[test]
    fn formats_retention_durations() {
        assert_eq!(
            format_duration(&prost_wkt_types::Duration {
                seconds: 0,
                nanos: 0,
            }),
            "0s"
        );
        assert_eq!(
            format_duration(&prost_wkt_types::Duration {
                seconds: 259_200,
                nanos: 0,
            }),
            "3d"
        );
    }

    #[test]
    fn redacts_sensitive_keys_inside_json_payloads() {
        let payload = Payload {
            metadata: HashMap::from([("encoding".to_string(), b"json/plain".to_vec())]),
            data: br#"{"customer":"Ada","credentials":{"api_key":"do-not-export"}}"#.to_vec(),
            ..Default::default()
        };
        let field = payload_field("input", &payload);
        assert!(field.redacted);
        assert!(field.value.contains("Ada"));
        assert!(field.value.contains("<redacted>"));
        assert!(!field.value.contains("do-not-export"));
    }

    #[test]
    fn maps_task_queue_backlog_pollers_and_routing() {
        let response = DescribeTaskQueueResponse {
            pollers: vec![PollerInfo {
                identity: "worker-a".to_string(),
                rate_per_second: 12.5,
                deployment_options: Some(WorkerDeploymentOptions {
                    deployment_name: "payments".to_string(),
                    build_id: "2026.07.28".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            stats: Some(ProtoTaskQueueStats {
                approximate_backlog_count: 7,
                approximate_backlog_age: Some(prost_wkt_types::Duration {
                    seconds: 12,
                    nanos: 500_000_000,
                }),
                tasks_add_rate: 4.5,
                tasks_dispatch_rate: 3.0,
            }),
            versioning_info: Some(TaskQueueVersioningInfo {
                current_deployment_version: Some(WorkerDeploymentVersion {
                    deployment_name: "payments".to_string(),
                    build_id: "v1".to_string(),
                }),
                ramping_deployment_version: Some(WorkerDeploymentVersion {
                    deployment_name: "payments".to_string(),
                    build_id: "v2".to_string(),
                }),
                ramping_version_percentage: 15.0,
                ..Default::default()
            }),
            ..Default::default()
        };

        let summary = task_queue_summary(
            "payments-tasks".to_string(),
            TaskQueueType::Activity,
            &response,
        );
        assert_eq!(summary.name, "payments-tasks");
        assert_eq!(summary.queue_type, TaskQueueType::Activity);
        assert_eq!(summary.pollers[0].identity, "worker-a");
        assert_eq!(summary.pollers[0].deployment_name, "payments");
        assert_eq!(summary.stats.approximate_backlog_count, 7);
        assert!((summary.stats.approximate_backlog_age_seconds - 12.5).abs() < f64::EPSILON);
        assert_eq!(summary.current_deployment.unwrap().build_id, "v1");
        assert_eq!(summary.ramping_deployment.unwrap().build_id, "v2");
        assert!((summary.ramping_percentage - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn maps_worker_heartbeat_resource_and_slot_diagnostics() {
        let heartbeat = WorkerHeartbeat {
            worker_instance_key: "instance-a".to_string(),
            worker_identity: "worker-a".to_string(),
            task_queue: "payments".to_string(),
            sdk_name: "temporal-sdk-rust".to_string(),
            sdk_version: "0.2.0".to_string(),
            status: WorkerStatus::Running as i32,
            host_info: Some(WorkerHostInfo {
                host_name: "worker-host".to_string(),
                process_id: "4242".to_string(),
                current_host_cpu_usage: 0.25,
                current_host_mem_usage: 0.5,
                ..Default::default()
            }),
            elapsed_since_last_heartbeat: Some(prost_wkt_types::Duration {
                seconds: 3,
                nanos: 250_000_000,
            }),
            workflow_task_slots_info: Some(WorkerSlotsInfo {
                current_available_slots: 8,
                current_used_slots: 2,
                slot_supplier_kind: "Fixed".to_string(),
                total_processed_tasks: 100,
                total_failed_tasks: 1,
                ..Default::default()
            }),
            workflow_poller_info: Some(WorkerPollerInfo {
                current_pollers: 4,
                ..Default::default()
            }),
            total_sticky_cache_hit: 30,
            total_sticky_cache_miss: 2,
            current_sticky_cache_size: 12,
            ..Default::default()
        };

        let details = worker_details(&heartbeat);
        assert_eq!(details.summary.instance_key, "instance-a");
        assert_eq!(details.summary.status, "RUNNING");
        assert_eq!(details.summary.host_name, "worker-host");
        assert!((details.elapsed_since_heartbeat_seconds - 3.25).abs() < f64::EPSILON);
        assert!((details.host_cpu_usage - 0.25).abs() < f32::EPSILON);
        assert!((details.host_memory_usage - 0.5).abs() < f32::EPSILON);
        assert_eq!(details.workflow_slots.available, 8);
        assert_eq!(details.workflow_slots.processed, 100);
        assert_eq!(details.workflow_pollers, 4);
        assert_eq!(details.sticky_cache_hits, 30);
    }

    #[test]
    fn maps_worker_deployment_routing_and_drainage() {
        let current = WorkerDeploymentVersion {
            deployment_name: "payments".to_string(),
            build_id: "v1".to_string(),
        };
        let inactive = WorkerDeploymentVersion {
            deployment_name: "payments".to_string(),
            build_id: "v2".to_string(),
        };
        let info = ProtoWorkerDeploymentInfo {
            name: "payments".to_string(),
            version_summaries: vec![
                WorkerDeploymentVersionSummary {
                    status: WorkerDeploymentVersionStatus::Current as i32,
                    deployment_version: Some(current.clone()),
                    create_time: Some(prost_wkt_types::Timestamp {
                        seconds: 10,
                        nanos: 0,
                    }),
                    ..Default::default()
                },
                WorkerDeploymentVersionSummary {
                    status: WorkerDeploymentVersionStatus::Drained as i32,
                    deployment_version: Some(inactive),
                    create_time: Some(prost_wkt_types::Timestamp {
                        seconds: 20,
                        nanos: 0,
                    }),
                    drainage_info: Some(VersionDrainageInfo {
                        status: VersionDrainageStatus::Drained as i32,
                        last_checked_time: Some(prost_wkt_types::Timestamp {
                            seconds: 30,
                            nanos: 0,
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            routing_config: Some(RoutingConfig {
                current_deployment_version: Some(current),
                ..Default::default()
            }),
            manager_identity: "release-controller".to_string(),
            last_modifier_identity: "operator-a".to_string(),
            routing_config_update_state: RoutingConfigUpdateState::Completed as i32,
            ..Default::default()
        };

        let details = worker_deployment_details(&info);
        assert_eq!(details.summary.name, "payments");
        assert_eq!(
            details.summary.current_version.as_ref().unwrap().build_id,
            "v1"
        );
        assert_eq!(
            details.summary.latest_version.as_ref().unwrap().build_id,
            "v2"
        );
        assert!(details.versions[0].is_current);
        assert_eq!(details.versions[1].drainage_status, "DRAINED");
        assert_eq!(details.manager_identity, "release-controller");
        assert_eq!(details.routing_update_state, "COMPLETED");
    }

    #[tokio::test]
    async fn codec_server_uses_temporal_proto_json_and_namespace_routing() {
        let decoded_data = br#"{"customer":"Ada"}"#;
        let response = serde_json::json!({
            "payloads": [{
                "metadata": {
                    "encoding": BASE64_STANDARD.encode("json/plain")
                },
                "data": BASE64_STANDARD.encode(decoded_data)
            }]
        })
        .to_string();
        let (endpoint, request, server) = one_shot_http_server(response);
        let codec = HttpPayloadCodec::new(PayloadCodecConfig {
            endpoint: format!("{endpoint}/codec/{{namespace}}"),
            headers: HashMap::from([(
                "authorization".to_string(),
                "Bearer test-token".to_string(),
            )]),
        })
        .unwrap();
        let encrypted = Payload {
            metadata: HashMap::from([("encoding".to_string(), b"binary/encrypted".to_vec())]),
            data: b"ciphertext".to_vec(),
            ..Default::default()
        };

        let decoded = codec
            .transform("team one", CodecOperation::Decode, vec![encrypted])
            .await
            .unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].metadata["encoding"], b"json/plain");
        assert_eq!(decoded[0].data, decoded_data);

        let request = request.recv_timeout(Duration::from_secs(2)).unwrap();
        server.join().unwrap();
        assert!(request.starts_with("POST /codec/team%20one/decode HTTP/1.1\r\n"));
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("\r\nx-namespace: team one\r\n"));
        assert!(lower.contains("\r\nauthorization: bearer test-token\r\n"));
        let body = request.split_once("\r\n\r\n").unwrap().1;
        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(
            body["payloads"][0]["metadata"]["encoding"],
            BASE64_STANDARD.encode("binary/encrypted")
        );
        assert_eq!(
            body["payloads"][0]["data"],
            BASE64_STANDARD.encode("ciphertext")
        );
    }

    #[test]
    fn codec_url_replaces_an_existing_operation_and_rejects_credentials() {
        let codec = HttpPayloadCodec::new(PayloadCodecConfig {
            endpoint: "https://codec.example/namespaces/{namespace}/decode".to_string(),
            headers: HashMap::new(),
        })
        .unwrap();
        assert_eq!(
            codec
                .url("payments/prod", CodecOperation::Encode)
                .unwrap()
                .as_str(),
            "https://codec.example/namespaces/payments%2Fprod/encode"
        );
        assert!(
            HttpPayloadCodec::new(PayloadCodecConfig {
                endpoint: "https://user:secret@codec.example".to_string(),
                headers: HashMap::new(),
            })
            .is_err()
        );
    }

    fn one_shot_http_server(
        response_body: String,
    ) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            let mut expected_length = None;
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..count]);
                if expected_length.is_none()
                    && let Some(header_end) =
                        request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers.lines().find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    });
                    expected_length =
                        content_length.map(|length| header_end.saturating_add(4 + length));
                }
                if expected_length.is_some_and(|length| request.len() >= length) {
                    break;
                }
            }
            sender.send(String::from_utf8(request).unwrap()).unwrap();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .unwrap();
        });
        (format!("http://{address}"), receiver, server)
    }
}
