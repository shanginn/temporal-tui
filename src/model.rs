use std::fmt;

use chrono::{DateTime, Utc};

/// Stable identifier for one workflow run.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkflowKey {
    pub workflow_id: String,
    pub run_id: String,
}

/// A workflow row returned by visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Details and history for the selected workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDetails {
    pub summary: WorkflowSummary,
    pub first_run_id: String,
    pub parent_workflow_id: Option<String>,
    pub pending_activities: usize,
    pub pending_children: usize,
    pub pending_nexus_operations: usize,
    pub state_transition_count: i64,
    pub static_summary: Option<String>,
    pub static_details: Option<String>,
    pub events: Vec<HistoryEventSummary>,
}

/// One compact workflow history event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEventSummary {
    pub event_id: i64,
    pub event_type: String,
    pub event_time: Option<DateTime<Utc>>,
    pub detail: String,
}

/// Workflow execution status with forward-compatible unknown values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
