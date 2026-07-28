use std::fmt;

use chrono::{DateTime, Utc};
use serde::Serialize;

/// Stable identifier for one workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct WorkflowKey {
    pub workflow_id: String,
    pub run_id: String,
}

/// A workflow row returned by visibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowSummary {
    pub key: WorkflowKey,
    pub workflow_type: String,
    pub task_queue: String,
    pub status: WorkflowStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub close_time: Option<DateTime<Utc>>,
    pub history_length: i64,
    pub history_size_bytes: i64,
}

/// One cursor-addressable page of workflow executions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPage {
    pub workflows: Vec<WorkflowSummary>,
    pub next_page_token: Vec<u8>,
}

/// Approximate visibility count, optionally grouped by query fields.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowCount {
    pub total: i64,
    pub groups: Vec<WorkflowCountGroup>,
}

/// One grouped visibility count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCountGroup {
    pub values: Vec<String>,
    pub count: i64,
}

/// Details and history for the selected workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkflowDetails {
    pub summary: WorkflowSummary,
    pub first_run_id: String,
    pub parent_workflow_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub root_workflow_id: Option<String>,
    pub root_run_id: Option<String>,
    pub reset_run_id: Option<String>,
    pub cancel_requested: bool,
    pub pending_activities: usize,
    pub pending_activity_details: Vec<PendingActivitySummary>,
    pub pending_children: usize,
    pub pending_nexus_operations: usize,
    pub state_transition_count: i64,
    pub static_summary: Option<String>,
    pub static_details: Option<String>,
    pub memo: Vec<StructuredField>,
    pub search_attributes: Vec<StructuredField>,
    pub events: Vec<HistoryEventSummary>,
    #[serde(skip)]
    pub history_next_page_token: Vec<u8>,
    pub history_archived: bool,
}

/// One additional page of reverse-ordered history, normalized chronologically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryPage {
    pub events: Vec<HistoryEventSummary>,
    pub next_page_token: Vec<u8>,
    pub archived: bool,
}

/// One compact workflow history event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEventSummary {
    pub event_id: i64,
    pub event_type: String,
    pub event_time: Option<DateTime<Utc>>,
    pub detail: String,
    pub fields: Vec<StructuredField>,
    pub failure: Option<FailureSummary>,
}

/// A decoded, size-bounded, and possibly redacted payload field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructuredField {
    pub name: String,
    pub encoding: String,
    pub value: String,
    pub size_bytes: usize,
    pub redacted: bool,
}

/// Safe failure tree extracted from Temporal failure payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FailureSummary {
    pub message: String,
    pub source: String,
    pub kind: String,
    pub stack_trace: String,
    pub encoded_attributes: Option<StructuredField>,
    pub cause: Option<Box<Self>>,
}

/// Pending Activity diagnostics for a running Workflow Execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PendingActivitySummary {
    pub activity_id: String,
    pub activity_type: String,
    pub state: String,
    pub attempt: i32,
    pub maximum_attempts: i32,
    pub last_worker_identity: String,
    pub paused: bool,
    pub last_failure: Option<FailureSummary>,
}

/// Workflow execution status with forward-compatible unknown values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum WorkflowStatus {
    Running,
    Completed,
    Failed,
    Canceled,
    Terminated,
    ContinuedAsNew,
    TimedOut,
    Paused,
    Unspecified,
    Unknown(i32),
}

impl WorkflowStatus {
    #[must_use]
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Canceled => "CANCELED",
            Self::Terminated => "TERMINATED",
            Self::ContinuedAsNew => "CONTINUED",
            Self::TimedOut => "TIMED OUT",
            Self::Paused => "PAUSED",
            Self::Unspecified | Self::Unknown(_) => "UNKNOWN",
        }
    }
}

impl fmt::Display for WorkflowStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

/// Namespace information shown by the picker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NamespaceSummary {
    pub name: String,
    pub id: String,
    pub description: String,
    pub state: String,
    pub retention: String,
    pub active_cluster: String,
    pub is_global: bool,
}

/// Connected Temporal cluster metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ClusterInfo {
    pub cluster_name: String,
    pub cluster_id: String,
    pub server_version: String,
    pub persistence_store: String,
    pub visibility_store: String,
    pub history_shard_count: i32,
}

/// Workflow or Activity side of a Temporal Task Queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum TaskQueueType {
    Workflow,
    Activity,
}

impl TaskQueueType {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Workflow => "WORKFLOW",
            Self::Activity => "ACTIVITY",
        }
    }
}

/// Backlog and throughput estimates for one Task Queue type.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TaskQueueStats {
    pub approximate_backlog_count: i64,
    pub approximate_backlog_age_seconds: f64,
    pub tasks_add_rate: f32,
    pub tasks_dispatch_rate: f32,
}

/// One Worker polling a Task Queue through the classic poller API.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PollerSummary {
    pub identity: String,
    pub last_access_time: Option<DateTime<Utc>>,
    pub rate_per_second: f64,
    pub deployment_name: String,
    pub build_id: String,
}

/// Diagnostics for one Task Queue type.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TaskQueueSummary {
    pub name: String,
    pub queue_type: TaskQueueType,
    pub pollers: Vec<PollerSummary>,
    pub stats: TaskQueueStats,
    pub current_deployment: Option<DeploymentVersion>,
    pub ramping_deployment: Option<DeploymentVersion>,
    pub ramping_percentage: f32,
    pub effective_rate_limit: Option<f32>,
}

/// Stable Worker Deployment Version identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeploymentVersion {
    pub deployment_name: String,
    pub build_id: String,
}

impl DeploymentVersion {
    #[must_use]
    pub fn label(&self) -> String {
        if self.deployment_name.is_empty() {
            self.build_id.clone()
        } else {
            format!("{}:{}", self.deployment_name, self.build_id)
        }
    }
}

/// Lightweight heartbeat-backed Worker row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkerSummary {
    pub instance_key: String,
    pub identity: String,
    pub task_queue: String,
    pub deployment: Option<DeploymentVersion>,
    pub sdk_name: String,
    pub sdk_version: String,
    pub status: String,
    pub start_time: Option<DateTime<Utc>>,
    pub host_name: String,
    pub process_id: String,
    pub plugins: Vec<String>,
}

/// Cursor page returned by the experimental Worker observability API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerPage {
    pub workers: Vec<WorkerSummary>,
    pub next_page_token: Vec<u8>,
}

/// Slot usage and outcome counters for a Worker task category.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct WorkerSlots {
    pub available: i32,
    pub used: i32,
    pub supplier: String,
    pub processed: i32,
    pub failed: i32,
}

/// Full Worker heartbeat diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkerDetails {
    pub summary: WorkerSummary,
    pub heartbeat_time: Option<DateTime<Utc>>,
    pub elapsed_since_heartbeat_seconds: f64,
    pub host_cpu_usage: f32,
    pub host_memory_usage: f32,
    pub workflow_slots: WorkerSlots,
    pub activity_slots: WorkerSlots,
    pub local_activity_slots: WorkerSlots,
    pub nexus_slots: WorkerSlots,
    pub workflow_pollers: i32,
    pub activity_pollers: i32,
    pub nexus_pollers: i32,
    pub sticky_cache_hits: i32,
    pub sticky_cache_misses: i32,
    pub sticky_cache_size: i32,
}

/// One Worker Deployment version and its drainage state.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeploymentVersionSummary {
    pub version: DeploymentVersion,
    pub status: String,
    pub create_time: Option<DateTime<Utc>>,
    pub is_current: bool,
    pub is_ramping: bool,
    pub ramp_percentage: f32,
    pub drainage_status: String,
    pub drainage_last_checked: Option<DateTime<Utc>>,
}

/// Lightweight Worker Deployment row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkerDeploymentSummary {
    pub name: String,
    pub create_time: Option<DateTime<Utc>>,
    pub current_version: Option<DeploymentVersion>,
    pub ramping_version: Option<DeploymentVersion>,
    pub ramping_percentage: f32,
    pub latest_version: Option<DeploymentVersion>,
}

/// Cursor page of Worker Deployments.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkerDeploymentPage {
    pub deployments: Vec<WorkerDeploymentSummary>,
    pub next_page_token: Vec<u8>,
}

/// Full GA Worker Deployment routing and drainage diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkerDeploymentDetails {
    pub summary: WorkerDeploymentSummary,
    pub versions: Vec<DeploymentVersionSummary>,
    pub manager_identity: String,
    pub last_modifier_identity: String,
    pub routing_update_state: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_open_statuses_are_actionable() {
        assert!(WorkflowStatus::Running.is_running());
        assert!(WorkflowStatus::Paused.is_running());
        assert!(!WorkflowStatus::Completed.is_running());
        assert!(!WorkflowStatus::Failed.is_running());
    }
}
