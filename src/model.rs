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
