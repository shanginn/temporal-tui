use std::{collections::HashMap, path::PathBuf};

use async_trait::async_trait;
use base64::prelude::*;
use chrono::{DateTime, Utc};
use serde_json::Value;
use temporalio_client::{
    Client, ClientOptions, ClientTlsOptions, Connection, ConnectionOptions, TlsOptions,
    UntypedSignal, WorkflowCancelOptions, WorkflowDescribeOptions, WorkflowExecutionInfo,
    WorkflowHandle, WorkflowSignalOptions, WorkflowTerminateOptions,
    tonic::{IntoRequest, Request, Status},
};
use temporalio_common::{
    UntypedWorkflow,
    data_converters::{PayloadConverter, RawValue},
    protos::{
        proto_ts_to_system_time,
        temporal::api::{
            common::v1::{Payload, Payloads, WorkflowExecution as ProtoWorkflowExecution},
            enums::v1::{EventType, PendingActivityState, WorkflowExecutionStatus},
            failure::v1::{Failure, failure::FailureInfo},
            history::v1::{HistoryEvent, history_event::Attributes},
            workflow::v1::PendingActivityInfo,
            workflow::v1::WorkflowExecutionInfo as ProtoWorkflowExecutionInfo,
            workflowservice::v1::{
                CountWorkflowExecutionsRequest, DescribeNamespaceResponse, GetClusterInfoRequest,
                GetWorkflowExecutionHistoryReverseRequest, ListNamespacesRequest,
                ListWorkflowExecutionsRequest,
            },
        },
    },
};
use thiserror::Error;
use url::Url;

use crate::model::{
    ClusterInfo, FailureSummary, HistoryEventSummary, HistoryPage, NamespaceSummary,
    PendingActivitySummary, StructuredField, WorkflowCount, WorkflowCountGroup, WorkflowDetails,
    WorkflowKey, WorkflowPage, WorkflowStatus, WorkflowSummary,
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
}

impl GrpcTemporalService {
    /// Connect and verify the Temporal frontend with `GetSystemInfo`.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid address, unreadable TLS material, invalid
    /// client configuration, or an unreachable Temporal frontend.
    pub async fn connect(config: TemporalConnectionConfig) -> Result<Self, ServiceError> {
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
        Ok(Self { connection })
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

    async fn recent_history(
        &self,
        namespace: &str,
        key: &WorkflowKey,
        next_page_token: Vec<u8>,
    ) -> Result<HistoryPage, ServiceError> {
        let mut service = self.connection.workflow_service();
        let response = service
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
        let response = service
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

        let raw = description.raw();
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
            static_summary: description.static_summary().map(ToOwned::to_owned),
            static_details: description.static_details().map(ToOwned::to_owned),
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
        self.workflow_handle(namespace, key)?
            .signal(
                UntypedSignal::<UntypedWorkflow>::new(signal_name),
                input,
                WorkflowSignalOptions::default(),
            )
            .await
            .map_err(|error| ServiceError::Client {
                operation: "signal workflow",
                message: error.to_string(),
            })
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
    use super::*;
    use prost_wkt_types;
    use temporalio_common::protos::temporal::api::{
        common::v1::{ActivityType, WorkflowExecution, WorkflowType},
        history::v1::ActivityTaskScheduledEventAttributes,
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
}
