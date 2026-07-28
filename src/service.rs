use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::IpAddr,
    path::PathBuf,
    sync::Arc,
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
    tonic::{Code, IntoRequest, Request, Status},
};
use temporalio_common::{
    UntypedWorkflow,
    data_converters::{PayloadConverter, RawValue},
    payload_visitor::{AsyncPayloadVisitor, PayloadField, PayloadFieldData, PayloadVisitable},
    protos::{
        proto_ts_to_system_time,
        temporal::api::{
            batch::v1::{
                BatchOperationCancellation, BatchOperationDeletion, BatchOperationInfo,
                BatchOperationSignal, BatchOperationTermination,
            },
            common::v1::{
                Payload, Payloads, WorkflowExecution as ProtoWorkflowExecution, WorkflowType,
            },
            deployment::v1::{
                WorkerDeploymentInfo as ProtoWorkerDeploymentInfo,
                WorkerDeploymentVersion as ProtoDeploymentVersion,
            },
            enums::v1::{
                BatchOperationState, BatchOperationType, EventType, IndexedValueType,
                PendingActivityState, RoutingConfigUpdateState, ScheduleOverlapPolicy,
                TaskQueueType as ProtoTaskQueueType, UpdateWorkflowExecutionLifecycleStage,
                VersionDrainageStatus, WorkerDeploymentVersionStatus, WorkerStatus,
                WorkflowExecutionStatus,
            },
            failure::v1::{Failure, failure::FailureInfo},
            history::v1::{HistoryEvent, history_event::Attributes},
            namespace::v1::namespace_info::Capabilities as NamespaceCapabilities,
            operatorservice::v1::{
                AddSearchAttributesRequest, ListSearchAttributesRequest,
                RemoveSearchAttributesRequest,
            },
            query::v1::WorkflowQuery,
            schedule::v1::{
                BackfillRequest as ProtoScheduleBackfillRequest, CalendarSpec, IntervalSpec, Range,
                Schedule, ScheduleAction, ScheduleActionResult as ProtoScheduleActionResult,
                ScheduleListEntry, SchedulePatch, ScheduleSpec, ScheduleState,
                StructuredCalendarSpec, TriggerImmediatelyRequest, schedule_action,
            },
            taskqueue::v1::{TaskQueue as ProtoTaskQueue, TaskQueueStats as ProtoTaskQueueStats},
            update::v1::{
                Input as UpdateInput, Meta as UpdateMeta, Outcome as UpdateOutcome,
                Request as UpdateRequest, WaitPolicy, outcome,
            },
            worker::v1::{WorkerHeartbeat, WorkerListInfo, WorkerSlotsInfo},
            workflow::v1::WorkflowExecutionInfo as ProtoWorkflowExecutionInfo,
            workflow::v1::{NewWorkflowExecutionInfo, PendingActivityInfo},
            workflowservice::v1::{
                CountWorkflowExecutionsRequest, CreateScheduleRequest, DeleteScheduleRequest,
                DescribeBatchOperationRequest, DescribeBatchOperationResponse,
                DescribeNamespaceRequest, DescribeNamespaceResponse, DescribeScheduleRequest,
                DescribeScheduleResponse, DescribeTaskQueueRequest, DescribeTaskQueueResponse,
                DescribeWorkerDeploymentRequest, DescribeWorkerDeploymentResponse,
                DescribeWorkerRequest, GetClusterInfoRequest, GetSystemInfoRequest,
                GetWorkflowExecutionHistoryReverseRequest, ListBatchOperationsRequest,
                ListNamespacesRequest, ListSchedulesRequest, ListWorkerDeploymentsRequest,
                ListWorkersRequest, ListWorkflowExecutionsRequest, PatchScheduleRequest,
                PauseWorkflowExecutionRequest, PollWorkflowExecutionUpdateRequest,
                QueryWorkflowRequest, ResetWorkflowExecutionRequest,
                SetWorkerDeploymentCurrentVersionRequest, SetWorkerDeploymentRampingVersionRequest,
                SignalWorkflowExecutionRequest, StartBatchOperationRequest,
                StopBatchOperationRequest, UnpauseWorkflowExecutionRequest,
                UpdateScheduleRequest as ProtoUpdateScheduleRequest,
                UpdateWorkflowExecutionRequest, list_worker_deployments_response,
                start_batch_operation_request,
            },
        },
    },
};
use thiserror::Error;
use url::{Host, Url};

use crate::{
    auth::AuthSession,
    model::{
        BatchOperationDetails, BatchOperationKind, BatchOperationPage, BatchOperationRequest,
        BatchOperationSummary, Capability, CapabilityAvailability, CapabilitySummary, ClusterInfo,
        DeploymentVersion, DeploymentVersionSummary, FailureSummary, HistoryEventSummary,
        HistoryPage, NamespaceSummary, PendingActivitySummary, PollerSummary, ScheduleActionResult,
        ScheduleBackfillRequest, ScheduleCreateRequest, ScheduleDetails, SchedulePage,
        ScheduleSummary, ScheduleUpdateRequest, SearchAttributeSummary, ServerCapabilities,
        StructuredField, TaskQueueStats, TaskQueueSummary, TaskQueueType, WorkerDeploymentDetails,
        WorkerDeploymentPage, WorkerDeploymentSummary, WorkerDetails, WorkerPage, WorkerSlots,
        WorkerSummary, WorkflowCallResult, WorkflowCount, WorkflowCountGroup, WorkflowDetails,
        WorkflowKey, WorkflowPage, WorkflowStatus, WorkflowSummary,
    },
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
#[derive(Clone)]
pub struct TemporalConnectionConfig {
    pub address: String,
    pub api_key: Option<String>,
    pub headers: HashMap<String, String>,
    pub tls: Option<ClientTlsConfig>,
    pub payload_codec: Option<PayloadCodecConfig>,
}

impl fmt::Debug for TemporalConnectionConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut header_names = self.headers.keys().collect::<Vec<_>>();
        header_names.sort_unstable();
        formatter
            .debug_struct("TemporalConnectionConfig")
            .field("address", &self.address)
            .field("api_key", &self.api_key.as_ref().map(|_| "[redacted]"))
            .field("header_names", &header_names)
            .field("tls", &self.tls)
            .field("payload_codec", &self.payload_codec)
            .finish()
    }
}

/// Remote Temporal Codec Server settings.
#[derive(Clone)]
pub struct PayloadCodecConfig {
    pub endpoint: String,
    pub headers: HashMap<String, String>,
}

impl fmt::Debug for PayloadCodecConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut header_names = self.headers.keys().collect::<Vec<_>>();
        header_names.sort_unstable();
        formatter
            .debug_struct("PayloadCodecConfig")
            .field("endpoint", &self.endpoint)
            .field("header_names", &header_names)
            .finish()
    }
}

/// Errors returned by the Temporal service adapter.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("invalid Temporal address `{address}`: {source}")]
    InvalidAddress {
        address: String,
        source: url::ParseError,
    },

    #[error("invalid Temporal connection configuration: {0}")]
    ConnectionConfig(String),

    #[error("could not read {kind} `{path}`: {source}")]
    CredentialFile {
        kind: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("could not connect to Temporal: {0}")]
    Connect(String),

    #[error("Temporal login failed: {0}")]
    Authentication(String),

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
        let url = self.url(namespace, operation)?;
        let mut attempt = 0_u8;
        let response = loop {
            let result = self
                .client
                .post(url.clone())
                .headers(self.headers.clone())
                .header("x-namespace", namespace_header.clone())
                .header(CONTENT_TYPE, "application/json")
                .body(body.clone())
                .send()
                .await;
            match result {
                Ok(response) => break response,
                Err(_) if attempt == 0 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
                Err(error) => {
                    return Err(codec_error(
                        operation,
                        format!("request failed after retry: {error}"),
                    ));
                }
            }
        };
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

    async fn server_capabilities(
        &self,
        namespace: &str,
    ) -> Result<ServerCapabilities, ServiceError>;

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

    async fn list_search_attributes(
        &self,
        namespace: &str,
    ) -> Result<Vec<SearchAttributeSummary>, ServiceError>;

    async fn add_search_attribute(
        &self,
        namespace: &str,
        name: &str,
        value_type: &str,
    ) -> Result<(), ServiceError>;

    async fn remove_search_attribute(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<(), ServiceError>;

    async fn set_worker_deployment_current_version(
        &self,
        namespace: &str,
        deployment_name: &str,
        build_id: &str,
    ) -> Result<(), ServiceError>;

    async fn set_worker_deployment_ramping_version(
        &self,
        namespace: &str,
        deployment_name: &str,
        build_id: &str,
        percentage: f32,
    ) -> Result<(), ServiceError>;

    async fn list_batch_operations(
        &self,
        namespace: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<BatchOperationPage, ServiceError>;

    async fn describe_batch_operation(
        &self,
        namespace: &str,
        job_id: &str,
    ) -> Result<BatchOperationDetails, ServiceError>;

    async fn start_batch_operation(
        &self,
        namespace: &str,
        request: BatchOperationRequest,
    ) -> Result<(), ServiceError>;

    async fn stop_batch_operation(
        &self,
        namespace: &str,
        job_id: &str,
        reason: &str,
    ) -> Result<(), ServiceError>;

    async fn query_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        query_name: &str,
        arguments: Vec<Value>,
    ) -> Result<WorkflowCallResult, ServiceError>;

    async fn update_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        update_name: &str,
        arguments: Vec<Value>,
    ) -> Result<WorkflowCallResult, ServiceError>;

    async fn pause_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError>;

    async fn unpause_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError>;

    async fn reset_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        event_id: i64,
        reason: &str,
    ) -> Result<String, ServiceError>;

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

    async fn list_schedules(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<SchedulePage, ServiceError>;

    async fn describe_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<ScheduleDetails, ServiceError>;

    async fn create_schedule(
        &self,
        namespace: &str,
        request: ScheduleCreateRequest,
    ) -> Result<(), ServiceError>;

    async fn update_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        request: ScheduleUpdateRequest,
    ) -> Result<(), ServiceError>;

    async fn pause_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        note: &str,
    ) -> Result<(), ServiceError>;

    async fn unpause_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        note: &str,
    ) -> Result<(), ServiceError>;

    async fn trigger_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<(), ServiceError>;

    async fn backfill_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        request: ScheduleBackfillRequest,
    ) -> Result<(), ServiceError>;

    async fn delete_schedule(&self, namespace: &str, schedule_id: &str)
    -> Result<(), ServiceError>;
}

/// Temporal's official Rust client adapted to the dashboard boundary.
#[derive(Clone)]
pub struct GrpcTemporalService {
    connection: Connection,
    payload_codec: Option<HttpPayloadCodec>,
    _auth_refresh: Option<Arc<AuthRefreshTask>>,
}

struct AuthRefreshTask {
    abort: tokio::task::AbortHandle,
}

impl Drop for AuthRefreshTask {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

impl GrpcTemporalService {
    /// Connect and verify the Temporal frontend with `GetSystemInfo`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid address, unreadable TLS material, invalid
    /// client configuration, or an unreachable Temporal frontend.
    pub async fn connect(config: TemporalConnectionConfig) -> Result<Self, ServiceError> {
        Self::connect_with_auth(config, None).await
    }

    /// Connect with a refreshable local-auth session.
    ///
    /// The access token is acquired before the SDK's initial `GetSystemInfo`
    /// call. A background task rotates it early and updates the shared
    /// connection in place; dropping the last service clone cancels that task.
    ///
    /// # Errors
    ///
    /// Returns an error when the session cannot produce an initial access token
    /// or when the underlying Temporal connection fails.
    pub async fn connect_with_auth(
        mut config: TemporalConnectionConfig,
        auth: Option<AuthSession>,
    ) -> Result<Self, ServiceError> {
        let address = normalize_address(&config.address, config.tls.is_some());
        let target = Url::parse(&address).map_err(|source| ServiceError::InvalidAddress {
            address: address.clone(),
            source,
        })?;
        validate_connection_target(&target, &config)?;

        if let Some(session) = &auth {
            if config.api_key.is_some()
                || config
                    .headers
                    .keys()
                    .any(|name| name.eq_ignore_ascii_case("authorization"))
            {
                return Err(ServiceError::ConnectionConfig(
                    "local login cannot be combined with an API key or authorization header"
                        .to_string(),
                ));
            }
            validate_authenticated_target(&target, session.profile().allow_insecure)?;
            config.api_key = Some(
                session
                    .access_token()
                    .await
                    .map_err(|error| ServiceError::Authentication(error.to_string()))?,
            );
        }

        let payload_codec = config
            .payload_codec
            .map(HttpPayloadCodec::new)
            .transpose()?;
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
        let auth_refresh = auth.map(|session| {
            let refresh_connection = connection.clone();
            let task = tokio::spawn(async move {
                let mut retry_delay = Duration::from_secs(5);
                loop {
                    tokio::time::sleep(
                        session
                            .next_refresh_delay()
                            .await
                            .max(Duration::from_secs(1)),
                    )
                    .await;
                    match session.force_refresh().await {
                        Ok(token) => {
                            refresh_connection.set_api_key(Some(token));
                            retry_delay = Duration::from_secs(5);
                        }
                        Err(error) => {
                            if !error.is_retryable() {
                                tracing::warn!(
                                    %error,
                                    "Temporal login session requires a new sign-in; stopping automatic refresh"
                                );
                                break;
                            }
                            tracing::warn!(%error, "could not refresh Temporal login session");
                            tokio::time::sleep(retry_delay).await;
                            retry_delay = (retry_delay * 2).min(Duration::from_mins(1));
                        }
                    }
                }
            });
            Arc::new(AuthRefreshTask {
                abort: task.abort_handle(),
            })
        });
        Ok(Self {
            connection,
            payload_codec,
            _auth_refresh: auth_refresh,
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

    async fn encode_json_arguments(
        &self,
        namespace: &str,
        arguments: &[Value],
    ) -> Result<Payloads, ServiceError> {
        let converter = PayloadConverter::serde_json();
        let payloads = arguments
            .iter()
            .flat_map(|argument| RawValue::from_value(argument, &converter).payloads)
            .collect();
        Ok(Payloads {
            payloads: self.encode_payloads(namespace, payloads).await?,
        })
    }

    async fn describe_schedule_raw(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<DescribeScheduleResponse, ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .describe_schedule(
                DescribeScheduleRequest {
                    namespace: namespace.to_string(),
                    schedule_id: schedule_id.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "describe schedule",
                source,
            })
            .map(temporalio_client::tonic::Response::into_inner)
    }

    async fn describe_worker_deployment_raw(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<DescribeWorkerDeploymentResponse, ServiceError> {
        let mut service = self.connection.workflow_service();
        service
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
            })
            .map(temporalio_client::tonic::Response::into_inner)
    }

    async fn patch_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        patch: SchedulePatch,
        operation: &'static str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .patch_schedule(
                PatchScheduleRequest {
                    namespace: namespace.to_string(),
                    schedule_id: schedule_id.to_string(),
                    patch: Some(patch),
                    identity: client_identity(),
                    request_id: operation_request_id(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc { operation, source })
            .map(|_| ())
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

    async fn server_capabilities(
        &self,
        namespace: &str,
    ) -> Result<ServerCapabilities, ServiceError> {
        let mut service = self.connection.workflow_service();
        let system = service
            .get_system_info(Request::new(GetSystemInfoRequest::default()))
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "get system capabilities",
                source,
            })?
            .into_inner();
        let system_capabilities = system.capabilities;
        let namespace_capabilities = service
            .describe_namespace(
                DescribeNamespaceRequest {
                    namespace: namespace.to_string(),
                    id: String::new(),
                    weak_consistency: true,
                }
                .into_request(),
            )
            .await
            .map(|response| {
                response
                    .into_inner()
                    .namespace_info
                    .and_then(|info| info.capabilities)
            });
        drop(service);

        let (
            schedules_probe,
            workers_probe,
            deployments_probe,
            batches_probe,
            search_attributes_probe,
        ) = tokio::join!(
            self.list_schedules(namespace, "", 1, Vec::new()),
            self.list_workers(namespace, "", 1, Vec::new()),
            self.list_worker_deployments(namespace, 1, Vec::new()),
            self.list_batch_operations(namespace, 1, Vec::new()),
            self.list_search_attributes(namespace),
        );

        let features = vec![
            reported_capability(
                Capability::VisibilityAggregations,
                system_capabilities
                    .as_ref()
                    .map(|capabilities| capabilities.count_group_by_execution_status),
                "GetSystemInfo",
            ),
            reported_capability(
                Capability::EncodedFailureAttributes,
                system_capabilities
                    .as_ref()
                    .map(|capabilities| capabilities.encoded_failure_attributes),
                "GetSystemInfo",
            ),
            namespace_capability(
                Capability::WorkflowUpdate,
                &namespace_capabilities,
                |capabilities| capabilities.sync_update || capabilities.async_update,
            ),
            namespace_capability(
                Capability::WorkflowPause,
                &namespace_capabilities,
                |capabilities| capabilities.workflow_pause,
            ),
            reported_and_probed_capability(
                Capability::Schedules,
                system_capabilities
                    .as_ref()
                    .map(|capabilities| capabilities.supports_schedules),
                &schedules_probe,
                "GetSystemInfo",
            ),
            namespace_and_probed_capability(
                Capability::WorkerHeartbeats,
                &namespace_capabilities,
                |capabilities| capabilities.worker_heartbeats,
                &workers_probe,
            ),
            probed_capability(Capability::WorkerDeployments, &deployments_probe),
            probed_capability(Capability::BatchOperations, &batches_probe),
            probed_capability(Capability::SearchAttributes, &search_attributes_probe),
        ];

        Ok(ServerCapabilities {
            server_version: system.server_version,
            namespace: namespace.to_string(),
            features,
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
        let response = self.describe_worker_deployment_raw(namespace, name).await?;
        let info = response
            .worker_deployment_info
            .ok_or_else(|| ServiceError::Client {
                operation: "describe worker deployment",
                message: "response did not include deployment information".to_string(),
            })?;
        Ok(worker_deployment_details(&info))
    }

    async fn list_search_attributes(
        &self,
        namespace: &str,
    ) -> Result<Vec<SearchAttributeSummary>, ServiceError> {
        let mut service = self.connection.operator_service();
        let response = service
            .list_search_attributes(
                ListSearchAttributesRequest {
                    namespace: namespace.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list search attributes",
                source,
            })?
            .into_inner();
        let mut attributes = response
            .system_attributes
            .into_iter()
            .map(|(name, value_type)| SearchAttributeSummary {
                storage_type: response
                    .storage_schema
                    .get(&name)
                    .cloned()
                    .unwrap_or_default(),
                name,
                value_type: enum_label::<IndexedValueType>(value_type),
                custom: false,
            })
            .chain(
                response
                    .custom_attributes
                    .into_iter()
                    .map(|(name, value_type)| SearchAttributeSummary {
                        storage_type: response
                            .storage_schema
                            .get(&name)
                            .cloned()
                            .unwrap_or_default(),
                        name,
                        value_type: enum_label::<IndexedValueType>(value_type),
                        custom: true,
                    }),
            )
            .collect::<Vec<_>>();
        attributes.sort_by(|left, right| {
            right
                .custom
                .cmp(&left.custom)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(attributes)
    }

    async fn add_search_attribute(
        &self,
        namespace: &str,
        name: &str,
        value_type: &str,
    ) -> Result<(), ServiceError> {
        let value_type = parse_search_attribute_type(value_type)?;
        let mut service = self.connection.operator_service();
        service
            .add_search_attributes(
                AddSearchAttributesRequest {
                    search_attributes: HashMap::from([(name.to_string(), value_type as i32)]),
                    namespace: namespace.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "add search attribute",
                source,
            })
            .map(|_| ())
    }

    async fn remove_search_attribute(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.operator_service();
        service
            .remove_search_attributes(
                RemoveSearchAttributesRequest {
                    search_attributes: vec![name.to_string()],
                    namespace: namespace.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "remove search attribute",
                source,
            })
            .map(|_| ())
    }

    async fn set_worker_deployment_current_version(
        &self,
        namespace: &str,
        deployment_name: &str,
        build_id: &str,
    ) -> Result<(), ServiceError> {
        let described = self
            .describe_worker_deployment_raw(namespace, deployment_name)
            .await?;
        validate_deployment_build_id(
            described.worker_deployment_info.as_ref(),
            build_id,
            "set current Worker Deployment version",
        )?;
        let mut service = self.connection.workflow_service();
        service
            .set_worker_deployment_current_version(
                SetWorkerDeploymentCurrentVersionRequest {
                    namespace: namespace.to_string(),
                    deployment_name: deployment_name.to_string(),
                    build_id: build_id.to_string(),
                    conflict_token: described.conflict_token,
                    identity: client_identity(),
                    ignore_missing_task_queues: false,
                    allow_no_pollers: false,
                    ..Default::default()
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "set current Worker Deployment version",
                source,
            })
            .map(|_| ())
    }

    async fn set_worker_deployment_ramping_version(
        &self,
        namespace: &str,
        deployment_name: &str,
        build_id: &str,
        percentage: f32,
    ) -> Result<(), ServiceError> {
        if !percentage.is_finite() || !(0.0..=100.0).contains(&percentage) {
            return Err(ServiceError::Client {
                operation: "set ramping Worker Deployment version",
                message: "ramp percentage must be between 0 and 100".to_string(),
            });
        }
        if build_id.is_empty() && percentage != 0.0 {
            return Err(ServiceError::Client {
                operation: "set ramping Worker Deployment version",
                message: "clearing the ramping version requires a 0% ramp".to_string(),
            });
        }
        let described = self
            .describe_worker_deployment_raw(namespace, deployment_name)
            .await?;
        validate_deployment_build_id(
            described.worker_deployment_info.as_ref(),
            build_id,
            "set ramping Worker Deployment version",
        )?;
        let mut service = self.connection.workflow_service();
        service
            .set_worker_deployment_ramping_version(
                SetWorkerDeploymentRampingVersionRequest {
                    namespace: namespace.to_string(),
                    deployment_name: deployment_name.to_string(),
                    build_id: build_id.to_string(),
                    percentage,
                    conflict_token: described.conflict_token,
                    identity: client_identity(),
                    ignore_missing_task_queues: false,
                    allow_no_pollers: false,
                    ..Default::default()
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "set ramping Worker Deployment version",
                source,
            })
            .map(|_| ())
    }

    async fn list_batch_operations(
        &self,
        namespace: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<BatchOperationPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .list_batch_operations(
                ListBatchOperationsRequest {
                    namespace: namespace.to_string(),
                    page_size: i32::try_from(page_size).unwrap_or(i32::MAX),
                    next_page_token,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list batch operations",
                source,
            })?
            .into_inner();
        Ok(BatchOperationPage {
            operations: response
                .operation_info
                .iter()
                .map(batch_operation_summary)
                .collect(),
            next_page_token: response.next_page_token,
        })
    }

    async fn describe_batch_operation(
        &self,
        namespace: &str,
        job_id: &str,
    ) -> Result<BatchOperationDetails, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .describe_batch_operation(
                DescribeBatchOperationRequest {
                    namespace: namespace.to_string(),
                    job_id: job_id.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "describe batch operation",
                source,
            })?
            .into_inner();
        Ok(batch_operation_details(&response))
    }

    async fn start_batch_operation(
        &self,
        namespace: &str,
        request: BatchOperationRequest,
    ) -> Result<(), ServiceError> {
        validate_batch_operation_request(&request)?;
        let operation = match request.kind {
            BatchOperationKind::Cancel => {
                start_batch_operation_request::Operation::CancellationOperation(
                    BatchOperationCancellation {
                        identity: client_identity(),
                    },
                )
            }
            BatchOperationKind::Terminate => {
                let details = self
                    .encode_json_arguments(namespace, &[Value::String(request.reason.clone())])
                    .await?;
                start_batch_operation_request::Operation::TerminationOperation(
                    BatchOperationTermination {
                        details: Some(details),
                        identity: client_identity(),
                    },
                )
            }
            BatchOperationKind::Signal => {
                let input = self
                    .encode_json_arguments(namespace, std::slice::from_ref(&request.signal_input))
                    .await?;
                start_batch_operation_request::Operation::SignalOperation(BatchOperationSignal {
                    signal: request.signal_name.clone(),
                    input: Some(input),
                    header: None,
                    identity: client_identity(),
                })
            }
            BatchOperationKind::Delete => {
                start_batch_operation_request::Operation::DeletionOperation(
                    BatchOperationDeletion {
                        identity: client_identity(),
                    },
                )
            }
        };
        let mut service = self.connection.workflow_service();
        service
            .start_batch_operation(
                StartBatchOperationRequest {
                    namespace: namespace.to_string(),
                    visibility_query: request.visibility_query,
                    job_id: request.job_id,
                    reason: request.reason,
                    executions: Vec::new(),
                    max_operations_per_second: request.max_operations_per_second,
                    operation: Some(operation),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "start batch operation",
                source,
            })
            .map(|_| ())
    }

    async fn stop_batch_operation(
        &self,
        namespace: &str,
        job_id: &str,
        reason: &str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .stop_batch_operation(
                StopBatchOperationRequest {
                    namespace: namespace.to_string(),
                    job_id: job_id.to_string(),
                    reason: reason.to_string(),
                    identity: client_identity(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "stop batch operation",
                source,
            })
            .map(|_| ())
    }

    async fn query_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        query_name: &str,
        arguments: Vec<Value>,
    ) -> Result<WorkflowCallResult, ServiceError> {
        let query_args = self.encode_json_arguments(namespace, &arguments).await?;
        let mut service = self.connection.workflow_service();
        let mut response = service
            .query_workflow(
                QueryWorkflowRequest {
                    namespace: namespace.to_string(),
                    execution: Some(ProtoWorkflowExecution {
                        workflow_id: key.workflow_id.clone(),
                        run_id: key.run_id.clone(),
                    }),
                    query: Some(WorkflowQuery {
                        query_type: query_name.to_string(),
                        query_args: Some(query_args),
                        header: None,
                    }),
                    query_reject_condition: 0,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "query workflow",
                source,
            })?
            .into_inner();
        self.decode_message(namespace, &mut response).await?;
        if let Some(rejected) = response.query_rejected {
            return Err(ServiceError::Client {
                operation: "query workflow",
                message: format!(
                    "query was rejected for workflow status {}",
                    enum_label::<WorkflowExecutionStatus>(rejected.status)
                ),
            });
        }
        Ok(WorkflowCallResult {
            handler: query_name.to_string(),
            update_id: None,
            fields: payload_fields("result", response.query_result.as_ref()),
            failure: None,
        })
    }

    async fn update_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        update_name: &str,
        arguments: Vec<Value>,
    ) -> Result<WorkflowCallResult, ServiceError> {
        let args = self.encode_json_arguments(namespace, &arguments).await?;
        let update_id = operation_request_id();
        let mut service = self.connection.workflow_service();
        let mut response = service
            .update_workflow_execution(
                UpdateWorkflowExecutionRequest {
                    namespace: namespace.to_string(),
                    workflow_execution: Some(ProtoWorkflowExecution {
                        workflow_id: key.workflow_id.clone(),
                        run_id: key.run_id.clone(),
                    }),
                    first_execution_run_id: String::new(),
                    wait_policy: Some(WaitPolicy {
                        lifecycle_stage: UpdateWorkflowExecutionLifecycleStage::Completed as i32,
                    }),
                    request: Some(UpdateRequest {
                        meta: Some(UpdateMeta {
                            update_id: update_id.clone(),
                            identity: client_identity(),
                        }),
                        input: Some(UpdateInput {
                            header: None,
                            name: update_name.to_string(),
                            args: Some(args),
                        }),
                        request_id: operation_request_id(),
                        completion_callbacks: Vec::new(),
                        links: Vec::new(),
                    }),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "update workflow",
                source,
            })?
            .into_inner();
        self.decode_message(namespace, &mut response).await?;

        let mut outcome = response.outcome;
        let mut update_ref = response.update_ref;
        while outcome.is_none() {
            let reference = update_ref.clone().ok_or_else(|| ServiceError::Client {
                operation: "update workflow",
                message: "server returned neither an outcome nor an update reference".to_string(),
            })?;
            let mut poll = service
                .poll_workflow_execution_update(
                    PollWorkflowExecutionUpdateRequest {
                        namespace: namespace.to_string(),
                        update_ref: Some(reference),
                        identity: client_identity(),
                        wait_policy: Some(WaitPolicy {
                            lifecycle_stage: UpdateWorkflowExecutionLifecycleStage::Completed
                                as i32,
                        }),
                    }
                    .into_request(),
                )
                .await
                .map_err(|source| ServiceError::Rpc {
                    operation: "poll workflow update",
                    source,
                })?
                .into_inner();
            self.decode_message(namespace, &mut poll).await?;
            outcome = poll.outcome;
            if poll.update_ref.is_some() {
                update_ref = poll.update_ref;
            }
            if outcome.is_none()
                && poll.stage == UpdateWorkflowExecutionLifecycleStage::Unspecified as i32
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        workflow_update_result(
            update_name,
            update_id,
            outcome.expect("outcome was checked above"),
        )
    }

    async fn pause_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .pause_workflow_execution(
                PauseWorkflowExecutionRequest {
                    namespace: namespace.to_string(),
                    workflow_id: key.workflow_id.clone(),
                    run_id: key.run_id.clone(),
                    identity: client_identity(),
                    reason: reason.to_string(),
                    request_id: operation_request_id(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "pause workflow execution",
                source,
            })
            .map(|_| ())
    }

    async fn unpause_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        reason: &str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .unpause_workflow_execution(
                UnpauseWorkflowExecutionRequest {
                    namespace: namespace.to_string(),
                    workflow_id: key.workflow_id.clone(),
                    run_id: key.run_id.clone(),
                    identity: client_identity(),
                    reason: reason.to_string(),
                    request_id: operation_request_id(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "unpause workflow execution",
                source,
            })
            .map(|_| ())
    }

    async fn reset_workflow(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        event_id: i64,
        reason: &str,
    ) -> Result<String, ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .reset_workflow_execution(
                ResetWorkflowExecutionRequest {
                    namespace: namespace.to_string(),
                    workflow_execution: Some(ProtoWorkflowExecution {
                        workflow_id: key.workflow_id.clone(),
                        run_id: key.run_id.clone(),
                    }),
                    reason: reason.to_string(),
                    workflow_task_finish_event_id: event_id,
                    request_id: operation_request_id(),
                    identity: client_identity(),
                    ..Default::default()
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "reset workflow execution",
                source,
            })
            .map(|response| response.into_inner().run_id)
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

    async fn list_schedules(
        &self,
        namespace: &str,
        query: &str,
        page_size: usize,
        next_page_token: Vec<u8>,
    ) -> Result<SchedulePage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
            .list_schedules(
                ListSchedulesRequest {
                    namespace: namespace.to_string(),
                    maximum_page_size: i32::try_from(page_size).unwrap_or(i32::MAX),
                    next_page_token,
                    query: query.to_string(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "list schedules",
                source,
            })?
            .into_inner();
        Ok(SchedulePage {
            schedules: response.schedules.iter().map(schedule_summary).collect(),
            next_page_token: response.next_page_token,
        })
    }

    async fn describe_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<ScheduleDetails, ServiceError> {
        let mut response = self.describe_schedule_raw(namespace, schedule_id).await?;
        self.decode_message(namespace, &mut response).await?;
        schedule_details(schedule_id, &response)
    }

    async fn create_schedule(
        &self,
        namespace: &str,
        request: ScheduleCreateRequest,
    ) -> Result<(), ServiceError> {
        let input = self
            .encode_json_arguments(namespace, &request.arguments)
            .await?;
        let schedule = Schedule {
            spec: Some(ScheduleSpec {
                cron_string: vec![request.schedule_expression],
                timezone_name: request.timezone,
                ..Default::default()
            }),
            action: Some(ScheduleAction {
                action: Some(schedule_action::Action::StartWorkflow(
                    NewWorkflowExecutionInfo {
                        workflow_id: request.workflow_id,
                        workflow_type: Some(WorkflowType {
                            name: request.workflow_type,
                        }),
                        task_queue: Some(ProtoTaskQueue {
                            name: request.task_queue,
                            ..Default::default()
                        }),
                        input: Some(input),
                        ..Default::default()
                    },
                )),
            }),
            policies: None,
            state: Some(ScheduleState {
                notes: request.notes,
                paused: request.paused,
                ..Default::default()
            }),
        };
        let mut service = self.connection.workflow_service();
        service
            .create_schedule(
                CreateScheduleRequest {
                    namespace: namespace.to_string(),
                    schedule_id: request.schedule_id,
                    schedule: Some(schedule),
                    initial_patch: None,
                    identity: client_identity(),
                    request_id: operation_request_id(),
                    memo: None,
                    search_attributes: None,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "create schedule",
                source,
            })
            .map(|_| ())
    }

    async fn update_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        request: ScheduleUpdateRequest,
    ) -> Result<(), ServiceError> {
        let response = self.describe_schedule_raw(namespace, schedule_id).await?;
        let mut schedule = response.schedule.ok_or_else(|| ServiceError::Client {
            operation: "update schedule",
            message: "describe response did not include a schedule definition".to_string(),
        })?;

        if request.schedule_expression.is_some() || request.timezone.is_some() {
            let current_timezone = schedule
                .spec
                .as_ref()
                .map(|spec| spec.timezone_name.clone())
                .unwrap_or_default();
            if let Some(expression) = request.schedule_expression {
                schedule.spec = Some(ScheduleSpec {
                    cron_string: vec![expression],
                    timezone_name: request.timezone.unwrap_or(current_timezone),
                    ..Default::default()
                });
            } else if let Some(timezone) = request.timezone {
                schedule
                    .spec
                    .get_or_insert_with(ScheduleSpec::default)
                    .timezone_name = timezone;
            }
        }
        let state = schedule.state.get_or_insert_with(ScheduleState::default);
        state.notes = request.notes;

        let mut service = self.connection.workflow_service();
        service
            .update_schedule(
                ProtoUpdateScheduleRequest {
                    namespace: namespace.to_string(),
                    schedule_id: schedule_id.to_string(),
                    schedule: Some(schedule),
                    conflict_token: response.conflict_token,
                    identity: client_identity(),
                    request_id: operation_request_id(),
                    search_attributes: None,
                    memo: None,
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "update schedule",
                source,
            })
            .map(|_| ())
    }

    async fn pause_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        note: &str,
    ) -> Result<(), ServiceError> {
        self.patch_schedule(
            namespace,
            schedule_id,
            SchedulePatch {
                pause: note.to_string(),
                ..Default::default()
            },
            "pause schedule",
        )
        .await
    }

    async fn unpause_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        note: &str,
    ) -> Result<(), ServiceError> {
        self.patch_schedule(
            namespace,
            schedule_id,
            SchedulePatch {
                unpause: note.to_string(),
                ..Default::default()
            },
            "unpause schedule",
        )
        .await
    }

    async fn trigger_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<(), ServiceError> {
        self.patch_schedule(
            namespace,
            schedule_id,
            SchedulePatch {
                trigger_immediately: Some(TriggerImmediatelyRequest {
                    overlap_policy: ScheduleOverlapPolicy::Unspecified as i32,
                    scheduled_time: None,
                }),
                ..Default::default()
            },
            "trigger schedule",
        )
        .await
    }

    async fn backfill_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
        request: ScheduleBackfillRequest,
    ) -> Result<(), ServiceError> {
        let overlap_policy = parse_schedule_overlap_policy(&request.overlap_policy)?;
        self.patch_schedule(
            namespace,
            schedule_id,
            SchedulePatch {
                backfill_request: vec![ProtoScheduleBackfillRequest {
                    start_time: Some(datetime_to_proto(request.start_time)),
                    end_time: Some(datetime_to_proto(request.end_time)),
                    overlap_policy: overlap_policy as i32,
                }],
                ..Default::default()
            },
            "backfill schedule",
        )
        .await
    }

    async fn delete_schedule(
        &self,
        namespace: &str,
        schedule_id: &str,
    ) -> Result<(), ServiceError> {
        let mut service = self.connection.workflow_service();
        service
            .delete_schedule(
                DeleteScheduleRequest {
                    namespace: namespace.to_string(),
                    schedule_id: schedule_id.to_string(),
                    identity: client_identity(),
                }
                .into_request(),
            )
            .await
            .map_err(|source| ServiceError::Rpc {
                operation: "delete schedule",
                source,
            })
            .map(|_| ())
    }
}

fn workflow_update_result(
    handler: &str,
    update_id: String,
    outcome: UpdateOutcome,
) -> Result<WorkflowCallResult, ServiceError> {
    match outcome.value {
        Some(outcome::Value::Success(payloads)) => Ok(WorkflowCallResult {
            handler: handler.to_string(),
            update_id: Some(update_id),
            fields: payload_fields("result", Some(&payloads)),
            failure: None,
        }),
        Some(outcome::Value::Failure(failure)) => Ok(WorkflowCallResult {
            handler: handler.to_string(),
            update_id: Some(update_id),
            fields: Vec::new(),
            failure: Some(failure_summary(&failure)),
        }),
        None => Err(ServiceError::Client {
            operation: "update workflow",
            message: "completed update did not contain an outcome value".to_string(),
        }),
    }
}

fn schedule_summary(entry: &ScheduleListEntry) -> ScheduleSummary {
    let info = entry.info.as_ref();
    ScheduleSummary {
        schedule_id: entry.schedule_id.clone(),
        paused: info.is_some_and(|value| value.paused),
        notes: info.map(|value| value.notes.clone()).unwrap_or_default(),
        workflow_type: info
            .and_then(|value| value.workflow_type.as_ref())
            .map(|value| value.name.clone())
            .unwrap_or_default(),
        next_action_time: info
            .and_then(|value| value.future_action_times.first())
            .and_then(proto_datetime),
        recent_action_time: info
            .and_then(|value| value.recent_actions.first())
            .and_then(schedule_action_time),
        state_size_bytes: info.map_or(0, |value| value.state_size_bytes),
    }
}

fn schedule_details(
    schedule_id: &str,
    response: &DescribeScheduleResponse,
) -> Result<ScheduleDetails, ServiceError> {
    let schedule = response
        .schedule
        .as_ref()
        .ok_or_else(|| ServiceError::Client {
            operation: "describe schedule",
            message: "response did not include a schedule definition".to_string(),
        })?;
    let state = schedule.state.as_ref();
    let info = response.info.as_ref();
    let action = schedule
        .action
        .as_ref()
        .and_then(|value| value.action.as_ref());
    let start_workflow = action.map(|schedule_action::Action::StartWorkflow(workflow)| workflow);
    let workflow_type = start_workflow
        .and_then(|workflow| workflow.workflow_type.as_ref())
        .map(|value| value.name.clone())
        .unwrap_or_default();
    let summary = ScheduleSummary {
        schedule_id: schedule_id.to_string(),
        paused: state.is_some_and(|value| value.paused),
        notes: state.map(|value| value.notes.clone()).unwrap_or_default(),
        workflow_type,
        next_action_time: info
            .and_then(|value| value.future_action_times.first())
            .and_then(proto_datetime),
        recent_action_time: info
            .and_then(|value| value.recent_actions.first())
            .and_then(schedule_action_time),
        state_size_bytes: info.map_or(0, |value| value.state_size_bytes),
    };
    let policies = schedule.policies.as_ref();
    Ok(ScheduleDetails {
        summary,
        workflow_id: start_workflow
            .map(|workflow| workflow.workflow_id.clone())
            .unwrap_or_default(),
        task_queue: start_workflow
            .and_then(|workflow| workflow.task_queue.as_ref())
            .map(|value| value.name.clone())
            .unwrap_or_default(),
        timing: schedule
            .spec
            .as_ref()
            .map_or_else(Vec::new, schedule_timing),
        timezone: schedule
            .spec
            .as_ref()
            .map(|spec| spec.timezone_name.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "UTC".to_string()),
        overlap_policy: policies.map_or_else(
            || "SERVER DEFAULT".to_string(),
            |value| enum_label::<ScheduleOverlapPolicy>(value.overlap_policy),
        ),
        catchup_window: policies
            .and_then(|value| value.catchup_window.as_ref())
            .map_or_else(|| "server default".to_string(), format_duration),
        pause_on_failure: policies.is_some_and(|value| value.pause_on_failure),
        keep_original_workflow_id: policies.is_some_and(|value| value.keep_original_workflow_id),
        limited_actions: state.is_some_and(|value| value.limited_actions),
        remaining_actions: state.map_or(0, |value| value.remaining_actions),
        action_count: info.map_or(0, |value| value.action_count),
        missed_catchup_window: info.map_or(0, |value| value.missed_catchup_window),
        overlap_skipped: info.map_or(0, |value| value.overlap_skipped),
        buffer_dropped: info.map_or(0, |value| value.buffer_dropped),
        buffer_size: info.map_or(0, |value| value.buffer_size),
        running_workflows: info.map_or_else(Vec::new, |value| {
            value
                .running_workflows
                .iter()
                .map(|workflow| WorkflowKey {
                    workflow_id: workflow.workflow_id.clone(),
                    run_id: workflow.run_id.clone(),
                })
                .collect()
        }),
        recent_actions: info.map_or_else(Vec::new, |value| {
            value
                .recent_actions
                .iter()
                .map(schedule_action_result)
                .collect()
        }),
        future_action_times: info.map_or_else(Vec::new, |value| {
            value
                .future_action_times
                .iter()
                .filter_map(proto_datetime)
                .collect()
        }),
        create_time: info
            .and_then(|value| value.create_time.as_ref())
            .and_then(proto_datetime),
        update_time: info
            .and_then(|value| value.update_time.as_ref())
            .and_then(proto_datetime),
        input: start_workflow.map_or_else(Vec::new, |workflow| {
            payload_fields("input", workflow.input.as_ref())
        }),
        memo: response
            .memo
            .as_ref()
            .map_or_else(Vec::new, |memo| payload_map(&memo.fields)),
        search_attributes: response
            .search_attributes
            .as_ref()
            .map_or_else(Vec::new, |attributes| {
                payload_map(&attributes.indexed_fields)
            }),
    })
}

fn schedule_action_time(action: &ProtoScheduleActionResult) -> Option<DateTime<Utc>> {
    action
        .actual_time
        .as_ref()
        .or(action.schedule_time.as_ref())
        .and_then(proto_datetime)
}

fn schedule_action_result(action: &ProtoScheduleActionResult) -> ScheduleActionResult {
    let execution = action.start_workflow_result.as_ref();
    ScheduleActionResult {
        scheduled_time: action.schedule_time.as_ref().and_then(proto_datetime),
        actual_time: action.actual_time.as_ref().and_then(proto_datetime),
        workflow_id: execution
            .map(|value| value.workflow_id.clone())
            .unwrap_or_default(),
        run_id: execution
            .map(|value| value.run_id.clone())
            .unwrap_or_default(),
        workflow_status: enum_label::<WorkflowExecutionStatus>(action.start_workflow_status),
    }
}

fn schedule_timing(spec: &ScheduleSpec) -> Vec<String> {
    let mut timing = spec
        .cron_string
        .iter()
        .map(|cron| format!("cron {cron}"))
        .collect::<Vec<_>>();
    timing.extend(spec.interval.iter().map(format_interval));
    timing.extend(spec.calendar.iter().map(format_calendar));
    timing.extend(
        spec.structured_calendar
            .iter()
            .map(|calendar| format_structured_calendar(calendar, "calendar")),
    );
    timing.extend(
        spec.exclude_structured_calendar
            .iter()
            .map(|calendar| format_structured_calendar(calendar, "exclude")),
    );
    if timing.is_empty() {
        timing.push("no future matching times".to_string());
    }
    timing
}

fn format_interval(interval: &IntervalSpec) -> String {
    let every = interval
        .interval
        .as_ref()
        .map_or_else(|| "missing".to_string(), format_duration);
    let phase = interval
        .phase
        .as_ref()
        .filter(|value| value.seconds != 0 || value.nanos != 0)
        .map(format_duration);
    phase.map_or_else(
        || format!("every {every}"),
        |phase| format!("every {every} / phase {phase}"),
    )
}

fn format_calendar(calendar: &CalendarSpec) -> String {
    format!(
        "calendar sec={} min={} hour={} dom={} month={} dow={} year={}{}",
        calendar.second,
        calendar.minute,
        calendar.hour,
        calendar.day_of_month,
        calendar.month,
        calendar.day_of_week,
        calendar.year,
        if calendar.comment.is_empty() {
            String::new()
        } else {
            format!(" # {}", calendar.comment)
        }
    )
}

fn format_structured_calendar(calendar: &StructuredCalendarSpec, kind: &str) -> String {
    format!(
        "{kind} sec={} min={} hour={} dom={} month={} dow={} year={}{}",
        format_ranges(&calendar.second),
        format_ranges(&calendar.minute),
        format_ranges(&calendar.hour),
        format_ranges(&calendar.day_of_month),
        format_ranges(&calendar.month),
        format_ranges(&calendar.day_of_week),
        format_ranges(&calendar.year),
        if calendar.comment.is_empty() {
            String::new()
        } else {
            format!(" # {}", calendar.comment)
        }
    )
}

fn format_ranges(ranges: &[Range]) -> String {
    if ranges.is_empty() {
        return "*".to_string();
    }
    ranges
        .iter()
        .map(|range| {
            let end = range.end.max(range.start);
            let step = range.step.max(1);
            if end == range.start && step == 1 {
                range.start.to_string()
            } else if step == 1 {
                format!("{}-{end}", range.start)
            } else {
                format!("{}-{end}/{step}", range.start)
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_search_attribute_type(value: &str) -> Result<IndexedValueType, ServiceError> {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', '-'], "");
    match normalized.as_str() {
        "text" => Ok(IndexedValueType::Text),
        "keyword" => Ok(IndexedValueType::Keyword),
        "int" | "integer" => Ok(IndexedValueType::Int),
        "double" | "float" => Ok(IndexedValueType::Double),
        "bool" | "boolean" => Ok(IndexedValueType::Bool),
        "datetime" | "timestamp" => Ok(IndexedValueType::Datetime),
        "keywordlist" => Ok(IndexedValueType::KeywordList),
        _ => Err(ServiceError::Client {
            operation: "add search attribute",
            message: format!(
                "unknown Search Attribute type `{value}`; use Text, Keyword, Int, Double, Bool, \
                 Datetime, or KeywordList"
            ),
        }),
    }
}

fn parse_schedule_overlap_policy(value: &str) -> Result<ScheduleOverlapPolicy, ServiceError> {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', ' '], "-");
    match normalized.as_str() {
        "" | "default" | "unspecified" => Ok(ScheduleOverlapPolicy::Unspecified),
        "skip" => Ok(ScheduleOverlapPolicy::Skip),
        "buffer-one" => Ok(ScheduleOverlapPolicy::BufferOne),
        "buffer-all" => Ok(ScheduleOverlapPolicy::BufferAll),
        "cancel-other" => Ok(ScheduleOverlapPolicy::CancelOther),
        "terminate-other" => Ok(ScheduleOverlapPolicy::TerminateOther),
        "allow-all" => Ok(ScheduleOverlapPolicy::AllowAll),
        _ => Err(ServiceError::Client {
            operation: "backfill schedule",
            message: format!(
                "unknown overlap policy `{value}`; use skip, buffer-one, buffer-all, \
                 cancel-other, terminate-other, or allow-all"
            ),
        }),
    }
}

fn datetime_to_proto(value: DateTime<Utc>) -> prost_wkt_types::Timestamp {
    prost_wkt_types::Timestamp {
        seconds: value.timestamp(),
        nanos: i32::try_from(value.timestamp_subsec_nanos()).unwrap_or_default(),
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

fn validate_deployment_build_id(
    info: Option<&ProtoWorkerDeploymentInfo>,
    build_id: &str,
    operation: &'static str,
) -> Result<(), ServiceError> {
    let info = info.ok_or_else(|| ServiceError::Client {
        operation,
        message: "response did not include Worker Deployment information".to_string(),
    })?;
    if build_id.is_empty()
        || info.version_summaries.iter().any(|summary| {
            summary
                .deployment_version
                .as_ref()
                .is_some_and(|version| version.build_id == build_id)
        })
    {
        return Ok(());
    }
    Err(ServiceError::Client {
        operation,
        message: format!(
            "build ID `{build_id}` is not tracked by Worker Deployment `{}`",
            info.name
        ),
    })
}

fn batch_operation_summary(info: &BatchOperationInfo) -> BatchOperationSummary {
    BatchOperationSummary {
        job_id: info.job_id.clone(),
        state: enum_label::<BatchOperationState>(info.state),
        start_time: info.start_time.as_ref().and_then(proto_datetime),
        close_time: info.close_time.as_ref().and_then(proto_datetime),
    }
}

fn batch_operation_details(response: &DescribeBatchOperationResponse) -> BatchOperationDetails {
    BatchOperationDetails {
        summary: BatchOperationSummary {
            job_id: response.job_id.clone(),
            state: enum_label::<BatchOperationState>(response.state),
            start_time: response.start_time.as_ref().and_then(proto_datetime),
            close_time: response.close_time.as_ref().and_then(proto_datetime),
        },
        operation_type: enum_label::<BatchOperationType>(response.operation_type),
        total_operation_count: response.total_operation_count,
        complete_operation_count: response.complete_operation_count,
        failure_operation_count: response.failure_operation_count,
        identity: response.identity.clone(),
        reason: response.reason.clone(),
    }
}

fn validate_batch_operation_request(request: &BatchOperationRequest) -> Result<(), ServiceError> {
    if request.job_id.trim().is_empty() {
        return Err(ServiceError::Client {
            operation: "start batch operation",
            message: "job ID must not be empty".to_string(),
        });
    }
    if request.visibility_query.trim().is_empty() {
        return Err(ServiceError::Client {
            operation: "start batch operation",
            message: "a non-empty Visibility query is required".to_string(),
        });
    }
    if request.reason.trim().is_empty() {
        return Err(ServiceError::Client {
            operation: "start batch operation",
            message: "reason must not be empty".to_string(),
        });
    }
    if !request.max_operations_per_second.is_finite() || request.max_operations_per_second < 0.0 {
        return Err(ServiceError::Client {
            operation: "start batch operation",
            message: "maximum operations per second must be zero or greater".to_string(),
        });
    }
    if request.kind == BatchOperationKind::Signal && request.signal_name.trim().is_empty() {
        return Err(ServiceError::Client {
            operation: "start batch operation",
            message: "signal batch requires a signal name".to_string(),
        });
    }
    Ok(())
}

fn reported_capability(
    capability: Capability,
    supported: Option<bool>,
    source: &str,
) -> CapabilitySummary {
    match supported {
        Some(true) => capability_summary(
            capability,
            CapabilityAvailability::Available,
            format!("{source} reported support"),
        ),
        Some(false) => capability_summary(
            capability,
            CapabilityAvailability::Unavailable,
            format!("{source} reported the feature disabled"),
        ),
        None => capability_summary(
            capability,
            CapabilityAvailability::Unknown,
            format!("{source} did not include this capability"),
        ),
    }
}

fn reported_and_probed_capability<T>(
    capability: Capability,
    supported: Option<bool>,
    probe: &Result<T, ServiceError>,
    source: &str,
) -> CapabilitySummary {
    if supported == Some(false) {
        return reported_capability(capability, supported, source);
    }
    let mut summary = probed_capability(capability, probe);
    let report = if supported == Some(true) {
        format!("{source} reported support")
    } else {
        format!("{source} omitted the capability")
    };
    summary.detail = format!("{report}; {}", summary.detail);
    summary
}

fn namespace_capability<Getter>(
    capability: Capability,
    namespace_capabilities: &Result<Option<NamespaceCapabilities>, Status>,
    getter: Getter,
) -> CapabilitySummary
where
    Getter: Fn(&NamespaceCapabilities) -> bool,
{
    match namespace_capabilities {
        Ok(Some(capabilities)) => {
            reported_capability(capability, Some(getter(capabilities)), "DescribeNamespace")
        }
        Ok(None) => reported_capability(capability, None, "DescribeNamespace"),
        Err(status) => status_capability(capability, status, "DescribeNamespace capability lookup"),
    }
}

fn namespace_and_probed_capability<T, Getter>(
    capability: Capability,
    namespace_capabilities: &Result<Option<NamespaceCapabilities>, Status>,
    getter: Getter,
    probe: &Result<T, ServiceError>,
) -> CapabilitySummary
where
    Getter: Fn(&NamespaceCapabilities) -> bool,
{
    match namespace_capabilities {
        Ok(Some(capabilities)) if !getter(capabilities) => {
            reported_capability(capability, Some(false), "DescribeNamespace")
        }
        Ok(Some(_)) => {
            reported_and_probed_capability(capability, Some(true), probe, "DescribeNamespace")
        }
        Ok(None) => reported_and_probed_capability(capability, None, probe, "DescribeNamespace"),
        Err(status)
            if matches!(
                status.code(),
                Code::PermissionDenied | Code::Unauthenticated
            ) =>
        {
            status_capability(capability, status, "DescribeNamespace capability lookup")
        }
        Err(_) => probed_capability(capability, probe),
    }
}

fn probed_capability<T>(
    capability: Capability,
    probe: &Result<T, ServiceError>,
) -> CapabilitySummary {
    match probe {
        Ok(_) => capability_summary(
            capability,
            CapabilityAvailability::Available,
            "read-only endpoint probe succeeded",
        ),
        Err(ServiceError::Rpc { source, .. }) => {
            status_capability(capability, source, "read-only endpoint probe")
        }
        Err(error) => capability_summary(
            capability,
            CapabilityAvailability::Unknown,
            bounded_detail(&format!("read-only endpoint probe failed: {error}")),
        ),
    }
}

fn status_capability(capability: Capability, status: &Status, context: &str) -> CapabilitySummary {
    let availability = match status.code() {
        Code::Unimplemented => CapabilityAvailability::Unavailable,
        Code::PermissionDenied | Code::Unauthenticated => CapabilityAvailability::Restricted,
        _ => CapabilityAvailability::Unknown,
    };
    capability_summary(
        capability,
        availability,
        bounded_detail(&format!("{context} returned {}", status.code())),
    )
}

fn capability_summary(
    capability: Capability,
    availability: CapabilityAvailability,
    detail: impl Into<String>,
) -> CapabilitySummary {
    CapabilitySummary {
        capability,
        availability,
        detail: detail.into(),
    }
}

fn bounded_detail(detail: &str) -> String {
    detail.chars().take(160).collect()
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

fn validate_connection_target(
    target: &Url,
    config: &TemporalConnectionConfig,
) -> Result<(), ServiceError> {
    if !matches!(target.scheme(), "http" | "https") {
        return Err(ServiceError::ConnectionConfig(
            "address scheme must be http or https".to_string(),
        ));
    }
    if !target.username().is_empty() || target.password().is_some() {
        return Err(ServiceError::ConnectionConfig(
            "address must not contain credentials".to_string(),
        ));
    }
    if target.path() != "/" || target.query().is_some() || target.fragment().is_some() {
        return Err(ServiceError::ConnectionConfig(
            "address must not contain a path, query, or fragment".to_string(),
        ));
    }
    if config
        .api_key
        .as_deref()
        .is_some_and(|value| value.is_empty() || contains_invalid_metadata_bytes(value))
    {
        return Err(ServiceError::ConnectionConfig(
            "API key is empty or contains invalid bytes".to_string(),
        ));
    }
    for (name, value) in &config.headers {
        HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            ServiceError::ConnectionConfig(format!("gRPC header `{name}` has an invalid name"))
        })?;
        HeaderValue::from_str(value).map_err(|_| {
            ServiceError::ConnectionConfig(format!("gRPC header `{name}` has an invalid value"))
        })?;
    }
    if let Some(server_name) = config
        .tls
        .as_ref()
        .and_then(|tls| tls.server_name.as_deref())
        && (server_name.is_empty() || server_name.chars().any(char::is_control))
    {
        return Err(ServiceError::ConnectionConfig(
            "TLS server name is empty or contains control characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_authenticated_target(target: &Url, allow_insecure: bool) -> Result<(), ServiceError> {
    if target.scheme() == "https" {
        return Ok(());
    }
    if allow_insecure && target.host().is_some_and(|host| host_is_loopback(&host)) {
        return Ok(());
    }
    Err(ServiceError::ConnectionConfig(
        "local login requires TLS; insecure transport is restricted to explicit loopback development"
            .to_string(),
    ))
}

fn host_is_loopback(host: &Host<&str>) -> bool {
    match host {
        Host::Domain(domain) => {
            domain.eq_ignore_ascii_case("localhost")
                || domain
                    .parse::<IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        }
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    }
}

fn contains_invalid_metadata_bytes(value: &str) -> bool {
    value
        .bytes()
        .any(|byte| matches!(byte, b'\0' | b'\r' | b'\n'))
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
        schedule::v1::{ScheduleInfo, ScheduleListInfo},
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
    fn authenticated_targets_require_tls_except_explicit_loopback_development() {
        let remote_plaintext = Url::parse("http://temporal.example:7233").unwrap();
        assert!(validate_authenticated_target(&remote_plaintext, false).is_err());
        assert!(validate_authenticated_target(&remote_plaintext, true).is_err());

        let loopback_plaintext = Url::parse("http://127.0.0.1:7233").unwrap();
        assert!(validate_authenticated_target(&loopback_plaintext, false).is_err());
        validate_authenticated_target(&loopback_plaintext, true).unwrap();

        let remote_tls = Url::parse("https://temporal.example:7233").unwrap();
        validate_authenticated_target(&remote_tls, false).unwrap();
    }

    #[test]
    fn connection_debug_output_redacts_credentials_and_header_values() {
        let config = TemporalConnectionConfig {
            address: "https://temporal.example:7233".to_string(),
            api_key: Some("access-secret".to_string()),
            headers: HashMap::from([(
                "authorization".to_string(),
                "Bearer header-secret".to_string(),
            )]),
            tls: None,
            payload_codec: Some(PayloadCodecConfig {
                endpoint: "https://codec.example".to_string(),
                headers: HashMap::from([(
                    "authorization".to_string(),
                    "Bearer codec-secret".to_string(),
                )]),
            }),
        };

        let rendered = format!("{config:?}");
        assert!(!rendered.contains("access-secret"));
        assert!(!rendered.contains("header-secret"));
        assert!(!rendered.contains("codec-secret"));
        assert!(rendered.contains("authorization"));
    }

    #[test]
    fn rejects_credentials_paths_and_control_bytes_in_connections() {
        let base = TemporalConnectionConfig {
            address: "https://temporal.example:7233".to_string(),
            api_key: None,
            headers: HashMap::new(),
            tls: None,
            payload_codec: None,
        };

        let target = Url::parse("https://operator:secret@temporal.example:7233").unwrap();
        assert!(validate_connection_target(&target, &base).is_err());
        let target = Url::parse("https://temporal.example:7233/unexpected").unwrap();
        assert!(validate_connection_target(&target, &base).is_err());

        let mut invalid_api_key = base.clone();
        invalid_api_key.api_key = Some("secret\r\ninjected".to_string());
        let target = Url::parse(&invalid_api_key.address).unwrap();
        assert!(validate_connection_target(&target, &invalid_api_key).is_err());
    }

    #[test]
    fn maps_forward_compatible_workflow_status() {
        assert_eq!(workflow_status(1), WorkflowStatus::Running);
        assert_eq!(workflow_status(8), WorkflowStatus::Paused);
        assert_eq!(workflow_status(9_999), WorkflowStatus::Unknown(9_999));
    }

    #[test]
    fn search_attribute_types_are_explicit_and_forward_safe() {
        assert_eq!(
            parse_search_attribute_type("KeywordList").unwrap(),
            IndexedValueType::KeywordList
        );
        assert_eq!(
            parse_search_attribute_type("boolean").unwrap(),
            IndexedValueType::Bool
        );
        assert!(parse_search_attribute_type("json").is_err());
        assert_eq!(
            enum_label::<IndexedValueType>(IndexedValueType::Datetime as i32),
            "DATETIME"
        );
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

    #[test]
    fn deployment_mutations_accept_only_tracked_or_unversioned_builds() {
        let info = ProtoWorkerDeploymentInfo {
            name: "payments".to_string(),
            version_summaries: vec![WorkerDeploymentVersionSummary {
                deployment_version: Some(WorkerDeploymentVersion {
                    deployment_name: "payments".to_string(),
                    build_id: "2026.07.28".to_string(),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(
            validate_deployment_build_id(
                Some(&info),
                "2026.07.28",
                "set current Worker Deployment version"
            )
            .is_ok()
        );
        assert!(
            validate_deployment_build_id(Some(&info), "", "set current Worker Deployment version")
                .is_ok()
        );
        assert!(
            validate_deployment_build_id(
                Some(&info),
                "missing",
                "set current Worker Deployment version"
            )
            .is_err()
        );
    }

    #[test]
    fn maps_and_validates_server_side_batch_operations() {
        let info = BatchOperationInfo {
            job_id: "cancel-stale-orders".to_string(),
            state: BatchOperationState::Running as i32,
            start_time: Some(prost_wkt_types::Timestamp {
                seconds: 1_800_000_000,
                nanos: 0,
            }),
            ..Default::default()
        };
        let summary = batch_operation_summary(&info);
        assert_eq!(summary.job_id, "cancel-stale-orders");
        assert_eq!(summary.state, "RUNNING");
        assert!(summary.start_time.is_some());

        let details = batch_operation_details(&DescribeBatchOperationResponse {
            operation_type: BatchOperationType::Cancel as i32,
            job_id: info.job_id,
            state: BatchOperationState::Completed as i32,
            total_operation_count: 12,
            complete_operation_count: 11,
            failure_operation_count: 1,
            identity: "operator".to_string(),
            reason: "stale orders".to_string(),
            ..Default::default()
        });
        assert_eq!(details.operation_type, "CANCEL");
        assert_eq!(details.total_operation_count, 12);
        assert_eq!(details.failure_operation_count, 1);

        let valid = BatchOperationRequest {
            job_id: "job-1".to_string(),
            visibility_query: "WorkflowType = 'OrderWorkflow'".to_string(),
            reason: "operator request".to_string(),
            max_operations_per_second: 10.0,
            kind: BatchOperationKind::Cancel,
            signal_name: String::new(),
            signal_input: Value::Null,
        };
        assert!(validate_batch_operation_request(&valid).is_ok());
        assert!(
            validate_batch_operation_request(&BatchOperationRequest {
                visibility_query: String::new(),
                ..valid.clone()
            })
            .is_err()
        );
        assert!(
            validate_batch_operation_request(&BatchOperationRequest {
                kind: BatchOperationKind::Signal,
                ..valid
            })
            .is_err()
        );
    }

    #[test]
    fn capability_probes_distinguish_absence_permissions_and_transient_unknowns() {
        let available = probed_capability(Capability::BatchOperations, &Ok::<_, ServiceError>(()));
        assert_eq!(available.availability, CapabilityAvailability::Available);

        let unavailable = probed_capability::<()>(
            Capability::WorkerDeployments,
            &Err(ServiceError::Rpc {
                operation: "probe",
                source: Status::unimplemented("not supported"),
            }),
        );
        assert_eq!(
            unavailable.availability,
            CapabilityAvailability::Unavailable
        );

        let restricted = probed_capability::<()>(
            Capability::SearchAttributes,
            &Err(ServiceError::Rpc {
                operation: "probe",
                source: Status::permission_denied("not authorized"),
            }),
        );
        assert_eq!(restricted.availability, CapabilityAvailability::Restricted);

        let unknown = probed_capability::<()>(
            Capability::Schedules,
            &Err(ServiceError::Rpc {
                operation: "probe",
                source: Status::unavailable("temporary outage"),
            }),
        );
        assert_eq!(unknown.availability, CapabilityAvailability::Unknown);
    }

    #[test]
    fn maps_schedule_visibility_definition_and_runtime_state() {
        let next = prost_wkt_types::Timestamp {
            seconds: 1_800_000_000,
            nanos: 0,
        };
        let entry = ScheduleListEntry {
            schedule_id: "hourly-orders".to_string(),
            info: Some(ScheduleListInfo {
                workflow_type: Some(WorkflowType {
                    name: "OrderWorkflow".to_string(),
                }),
                notes: "operator note".to_string(),
                paused: true,
                future_action_times: vec![next],
                state_size_bytes: 1_024,
                ..Default::default()
            }),
            ..Default::default()
        };
        let summary = schedule_summary(&entry);
        assert_eq!(summary.schedule_id, "hourly-orders");
        assert!(summary.paused);
        assert_eq!(summary.workflow_type, "OrderWorkflow");
        assert!(summary.next_action_time.is_some());

        let response = DescribeScheduleResponse {
            schedule: Some(Schedule {
                spec: Some(ScheduleSpec {
                    interval: vec![IntervalSpec {
                        interval: Some(prost_wkt_types::Duration {
                            seconds: 3_600,
                            nanos: 0,
                        }),
                        phase: None,
                    }],
                    timezone_name: "UTC".to_string(),
                    ..Default::default()
                }),
                action: Some(ScheduleAction {
                    action: Some(schedule_action::Action::StartWorkflow(
                        NewWorkflowExecutionInfo {
                            workflow_id: "order-run".to_string(),
                            workflow_type: Some(WorkflowType {
                                name: "OrderWorkflow".to_string(),
                            }),
                            task_queue: Some(ProtoTaskQueue {
                                name: "orders".to_string(),
                                ..Default::default()
                            }),
                            input: Some(Payloads {
                                payloads: vec![Payload {
                                    metadata: HashMap::from([(
                                        "encoding".to_string(),
                                        b"json/plain".to_vec(),
                                    )]),
                                    data: br#"{"region":"eu"}"#.to_vec(),
                                    ..Default::default()
                                }],
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                state: Some(ScheduleState {
                    notes: "operator note".to_string(),
                    paused: true,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            info: Some(ScheduleInfo {
                action_count: 7,
                future_action_times: vec![next],
                running_workflows: vec![WorkflowExecution {
                    workflow_id: "order-run-2026".to_string(),
                    run_id: "run-a".to_string(),
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let details = schedule_details("hourly-orders", &response).unwrap();
        assert_eq!(details.summary.workflow_type, "OrderWorkflow");
        assert_eq!(details.workflow_id, "order-run");
        assert_eq!(details.task_queue, "orders");
        assert_eq!(details.timing, vec!["every 1h"]);
        assert_eq!(details.action_count, 7);
        assert_eq!(details.running_workflows[0].run_id, "run-a");
        assert!(details.input[0].value.contains("\"region\": \"eu\""));
    }

    #[test]
    fn update_outcomes_and_overlap_policy_are_explicit() {
        let success = workflow_update_result(
            "approve",
            "update-1".to_string(),
            UpdateOutcome {
                value: Some(outcome::Value::Success(Payloads {
                    payloads: vec![Payload {
                        metadata: HashMap::from([("encoding".to_string(), b"json/plain".to_vec())]),
                        data: b"true".to_vec(),
                        ..Default::default()
                    }],
                })),
            },
        )
        .unwrap();
        assert_eq!(success.update_id.as_deref(), Some("update-1"));
        assert_eq!(success.fields[0].value, "true");
        assert_eq!(
            parse_schedule_overlap_policy("buffer_all").unwrap(),
            ScheduleOverlapPolicy::BufferAll
        );
        assert!(parse_schedule_overlap_policy("drop-everything").is_err());
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

    #[tokio::test]
    async fn codec_server_retries_one_transient_connection_failure() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "payloads": [{
                "metadata": {
                    "encoding": BASE64_STANDARD.encode("json/plain")
                },
                "data": BASE64_STANDARD.encode("true")
            }]
        })
        .to_string();
        let server = thread::spawn(move || {
            let (first, _) = listener.accept().unwrap();
            drop(first);
            let (mut second, _) = listener.accept().unwrap();
            let _ = read_test_http_request(&mut second);
            write!(
                second,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
                 {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .unwrap();
        });
        let codec = HttpPayloadCodec::new(PayloadCodecConfig {
            endpoint: format!("http://{address}"),
            headers: HashMap::new(),
        })
        .unwrap();
        let decoded = codec
            .transform("default", CodecOperation::Decode, vec![Payload::default()])
            .await
            .unwrap();
        assert_eq!(decoded[0].data, b"true");
        server.join().unwrap();
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
            let request = read_test_http_request(&mut stream);
            sender.send(request).unwrap();
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

    fn read_test_http_request(stream: &mut std::net::TcpStream) -> String {
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
        String::from_utf8(request).unwrap()
    }
}
