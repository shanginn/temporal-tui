use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use url::Url;

use crate::model::{
    BatchOperationDetails, BatchOperationKind, BatchOperationPage, BatchOperationRequest,
    BatchOperationSummary, Capability, CapabilityAvailability, ClusterInfo, HistoryPage,
    NamespaceSummary, ScheduleBackfillRequest, ScheduleCreateRequest, ScheduleDetails,
    SchedulePage, ScheduleSummary, ScheduleUpdateRequest, SearchAttributeSummary,
    ServerCapabilities, TaskQueueSummary, WorkerDeploymentDetails, WorkerDeploymentPage,
    WorkerDeploymentSummary, WorkerDetails, WorkerPage, WorkerSummary, WorkflowCallResult,
    WorkflowCount, WorkflowDetails, WorkflowKey, WorkflowPage, WorkflowStatus, WorkflowSummary,
};

/// Named visibility query loaded from the local config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    pub name: String,
    pub query: String,
}

/// Non-secret profile metadata shown by the in-process cluster switcher.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent profile status flags are rendered as separate switcher columns"
)]
pub struct ProfileSummary {
    pub name: String,
    pub address: String,
    pub namespace: String,
    pub read_only: bool,
    pub auth_enabled: bool,
    pub codec_enabled: bool,
    pub is_default: bool,
}

/// Connection-specific state applied only after a profile reconnect succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileConnectionInfo {
    pub name: String,
    pub address: String,
    pub namespace: String,
    pub read_only: bool,
    pub codec_enabled: bool,
    pub web_ui_url: Option<String>,
}

/// Startup behavior and presentation preferences.
#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent startup policy flags map directly to explicit CLI settings"
)]
pub struct AppConfig {
    pub address: String,
    pub profile_name: Option<String>,
    pub namespace: String,
    pub query: String,
    pub page_size: usize,
    pub refresh_interval: Duration,
    pub auto_refresh: bool,
    pub color: bool,
    pub read_only: bool,
    pub force_read_only: bool,
    pub codec_enabled: bool,
    pub web_ui_url: Option<String>,
    pub saved_queries: Vec<SavedQuery>,
    pub profiles: Vec<ProfileSummary>,
}

/// Which primary pane receives navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Workflows,
    History,
}

/// Top-level diagnostics surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Workflows,
    TaskQueues,
    Workers,
    Deployments,
    Schedules,
    Batches,
}

impl View {
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Workflows => 1,
            Self::TaskQueues => 2,
            Self::Workers => 3,
            Self::Deployments => 4,
            Self::Schedules => 5,
            Self::Batches => 6,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Workflows => "WORKFLOWS",
            Self::TaskQueues => "TASK QUEUES",
            Self::Workers => "WORKERS",
            Self::Deployments => "DEPLOYMENTS",
            Self::Schedules => "SCHEDULES",
            Self::Batches => "BATCHES",
        }
    }

    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Workflows => "WF",
            Self::TaskQueues => "TQ",
            Self::Workers => "WORKERS",
            Self::Deployments => "DEPLOY",
            Self::Schedules => "SCHED",
            Self::Batches => "BATCH",
        }
    }
}

/// User-visible notification severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Success,
    Error,
}

/// Temporary status-bar notification.
#[derive(Debug, Clone)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
    expires_at: Instant,
}

/// Destructive workflow operation requiring confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    Cancel,
    Terminate,
    Pause,
    Unpause,
}

impl ConfirmAction {
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Cancel => "request cancellation of",
            Self::Terminate => "terminate",
            Self::Pause => "pause",
            Self::Unpause => "unpause",
        }
    }
}

/// Workflow handler invocation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowCallKind {
    Query,
    Update,
}

impl WorkflowCallKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Query => "Query",
            Self::Update => "Update",
        }
    }
}

/// Schedule action requiring an exact-ID confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleConfirmAction {
    Trigger,
    Delete,
}

impl ScheduleConfirmAction {
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Trigger => "trigger immediately",
            Self::Delete => "delete permanently",
        }
    }
}

/// Modal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Query(TextInput),
    ScheduleQuery(TextInput),
    TaskQueue(TextInput),
    SavedQueryPicker {
        selected: usize,
    },
    Aggregations {
        selected: usize,
    },
    NamespacePicker {
        selected: usize,
    },
    ProfilePicker {
        selected: usize,
    },
    Capabilities,
    SearchAttributes {
        selected: usize,
    },
    SearchAttributeAdd(SearchAttributeAddForm),
    SearchAttributeRemove {
        name: String,
        input: TextInput,
    },
    DeploymentCurrent(DeploymentCurrentForm),
    DeploymentRamp(DeploymentRampForm),
    BatchCreate(BatchCreateForm),
    BatchConfirm {
        form: BatchCreateForm,
        matched_workflows: i64,
        input: TextInput,
    },
    BatchStop {
        job_id: String,
        input: TextInput,
    },
    Confirm {
        action: ConfirmAction,
        key: WorkflowKey,
        workflow_id: String,
        input: TextInput,
    },
    Signal(SignalForm),
    WorkflowCall {
        kind: WorkflowCallKind,
        form: WorkflowCallForm,
    },
    WorkflowCallResult {
        kind: WorkflowCallKind,
        result: WorkflowCallResult,
        scroll: u16,
    },
    Reset(ResetForm),
    ScheduleCreate(ScheduleCreateForm),
    ScheduleEdit(ScheduleEditForm),
    ScheduleBackfill(ScheduleBackfillForm),
    ScheduleConfirm {
        action: ScheduleConfirmAction,
        schedule_id: String,
        input: TextInput,
    },
    WorkflowChain {
        selected: usize,
    },
    Inspector {
        scroll: u16,
    },
}

/// Editable single-line text with a character-index cursor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    pub value: String,
    pub cursor: usize,
}

impl TextInput {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn insert(&mut self, character: char) {
        let byte_index = self.byte_index(self.cursor);
        self.value.insert(byte_index, character);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let start = self.byte_index(self.cursor - 1);
        let end = self.byte_index(self.cursor);
        self.value.replace_range(start..end, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.value.chars().count() {
            return;
        }
        let start = self.byte_index(self.cursor);
        let end = self.byte_index(self.cursor + 1);
        self.value.replace_range(start..end, "");
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.value.chars().count());
    }

    fn byte_index(&self, char_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_index)
            .map_or(self.value.len(), |(index, _)| index)
    }
}

/// Signal modal data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalForm {
    pub name: TextInput,
    pub input: TextInput,
    pub active_field: SignalField,
}

impl Default for SignalForm {
    fn default() -> Self {
        Self {
            name: TextInput::default(),
            input: TextInput::new("{}"),
            active_field: SignalField::Name,
        }
    }
}

/// Active signal form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalField {
    Name,
    Input,
}

/// Query/Update form data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCallForm {
    pub name: TextInput,
    pub input: TextInput,
    pub active_field: HandlerField,
}

impl Default for WorkflowCallForm {
    fn default() -> Self {
        Self {
            name: TextInput::default(),
            input: TextInput::new("[]"),
            active_field: HandlerField::Name,
        }
    }
}

/// Active Query/Update form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerField {
    Name,
    Input,
}

/// Reset form requiring both an event boundary and an exact Workflow ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResetForm {
    pub key: WorkflowKey,
    pub workflow_id: String,
    pub event_id: TextInput,
    pub confirmation: TextInput,
    pub active_field: ResetField,
}

/// Active reset form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetField {
    EventId,
    Confirmation,
}

/// Add-Search-Attribute form with an explicit type and exact-name confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchAttributeAddForm {
    pub name: TextInput,
    pub value_type: TextInput,
    pub confirmation: TextInput,
    pub active_field: SearchAttributeAddField,
}

impl Default for SearchAttributeAddForm {
    fn default() -> Self {
        Self {
            name: TextInput::default(),
            value_type: TextInput::new("Keyword"),
            confirmation: TextInput::default(),
            active_field: SearchAttributeAddField::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAttributeAddField {
    Name,
    ValueType,
    Confirmation,
}

impl SearchAttributeAddField {
    const ALL: [Self; 3] = [Self::Name, Self::ValueType, Self::Confirmation];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentCurrentForm {
    pub deployment_name: String,
    pub build_id: TextInput,
    pub confirmation: TextInput,
    pub active_field: DeploymentCurrentField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentCurrentField {
    BuildId,
    Confirmation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentRampForm {
    pub deployment_name: String,
    pub build_id: TextInput,
    pub percentage: TextInput,
    pub confirmation: TextInput,
    pub active_field: DeploymentRampField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentRampField {
    BuildId,
    Percentage,
    Confirmation,
}

impl DeploymentRampField {
    const ALL: [Self; 3] = [Self::BuildId, Self::Percentage, Self::Confirmation];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCreateForm {
    pub job_id: TextInput,
    pub operation: TextInput,
    pub visibility_query: TextInput,
    pub reason: TextInput,
    pub max_operations_per_second: TextInput,
    pub signal_name: TextInput,
    pub signal_input: TextInput,
    pub active_field: BatchCreateField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCreateField {
    JobId,
    Operation,
    VisibilityQuery,
    Reason,
    MaxOperationsPerSecond,
    SignalName,
    SignalInput,
}

impl BatchCreateField {
    const ALL: [Self; 7] = [
        Self::JobId,
        Self::Operation,
        Self::VisibilityQuery,
        Self::Reason,
        Self::MaxOperationsPerSecond,
        Self::SignalName,
        Self::SignalInput,
    ];
}

/// Create-Schedule form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleCreateForm {
    pub schedule_id: TextInput,
    pub workflow_id: TextInput,
    pub workflow_type: TextInput,
    pub task_queue: TextInput,
    pub expression: TextInput,
    pub timezone: TextInput,
    pub input: TextInput,
    pub notes: TextInput,
    pub active_field: ScheduleCreateField,
}

impl Default for ScheduleCreateForm {
    fn default() -> Self {
        Self {
            schedule_id: TextInput::default(),
            workflow_id: TextInput::default(),
            workflow_type: TextInput::default(),
            task_queue: TextInput::default(),
            expression: TextInput::new("@every 1h"),
            timezone: TextInput::new("UTC"),
            input: TextInput::new("[]"),
            notes: TextInput::new("Created from temporal-tui"),
            active_field: ScheduleCreateField::ScheduleId,
        }
    }
}

/// Active create-Schedule field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleCreateField {
    ScheduleId,
    WorkflowId,
    WorkflowType,
    TaskQueue,
    Expression,
    Timezone,
    Input,
    Notes,
}

impl ScheduleCreateField {
    const ALL: [Self; 8] = [
        Self::ScheduleId,
        Self::WorkflowId,
        Self::WorkflowType,
        Self::TaskQueue,
        Self::Expression,
        Self::Timezone,
        Self::Input,
        Self::Notes,
    ];
}

/// Edit-Schedule form. A blank expression preserves the current timing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleEditForm {
    pub schedule_id: String,
    pub expression: TextInput,
    pub timezone: TextInput,
    pub notes: TextInput,
    pub active_field: ScheduleEditField,
}

/// Active edit-Schedule field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleEditField {
    Expression,
    Timezone,
    Notes,
}

impl ScheduleEditField {
    const ALL: [Self; 3] = [Self::Expression, Self::Timezone, Self::Notes];
}

/// Schedule backfill form with an exact-ID safety check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleBackfillForm {
    pub schedule_id: String,
    pub start_time: TextInput,
    pub end_time: TextInput,
    pub overlap_policy: TextInput,
    pub confirmation: TextInput,
    pub active_field: ScheduleBackfillField,
}

/// Active Schedule backfill field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleBackfillField {
    Start,
    End,
    Overlap,
    Confirmation,
}

impl ScheduleBackfillField {
    const ALL: [Self; 4] = [Self::Start, Self::End, Self::Overlap, Self::Confirmation];
}

/// Side effects requested by the pure application state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    LoadCluster {
        request_id: u64,
    },
    SwitchProfile {
        request_id: u64,
        profile_name: String,
    },
    LoadCapabilities {
        request_id: u64,
        namespace: String,
    },
    LoadNamespaces {
        request_id: u64,
    },
    LoadWorkflows {
        request_id: u64,
        namespace: String,
        query: String,
        page_size: usize,
        next_page_token: Vec<u8>,
    },
    CountWorkflows {
        request_id: u64,
        namespace: String,
        query: String,
    },
    LoadDetails {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
    },
    LoadHistoryPage {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        next_page_token: Vec<u8>,
    },
    LoadWorkflowChain {
        request_id: u64,
        namespace: String,
        workflow_id: String,
    },
    LoadTaskQueues {
        request_id: u64,
        namespace: String,
        names: Vec<String>,
    },
    LoadWorkers {
        request_id: u64,
        namespace: String,
        query: String,
        page_size: usize,
        next_page_token: Vec<u8>,
    },
    LoadWorkerDetails {
        request_id: u64,
        namespace: String,
        instance_key: String,
    },
    LoadWorkerDeployments {
        request_id: u64,
        namespace: String,
        page_size: usize,
        next_page_token: Vec<u8>,
    },
    LoadWorkerDeploymentDetails {
        request_id: u64,
        namespace: String,
        name: String,
    },
    LoadSchedules {
        request_id: u64,
        namespace: String,
        query: String,
        page_size: usize,
        next_page_token: Vec<u8>,
    },
    LoadScheduleDetails {
        request_id: u64,
        namespace: String,
        schedule_id: String,
    },
    LoadSearchAttributes {
        request_id: u64,
        namespace: String,
    },
    AddSearchAttribute {
        request_id: u64,
        namespace: String,
        name: String,
        value_type: String,
    },
    RemoveSearchAttribute {
        request_id: u64,
        namespace: String,
        name: String,
    },
    SetDeploymentCurrent {
        request_id: u64,
        namespace: String,
        deployment_name: String,
        build_id: String,
    },
    SetDeploymentRamp {
        request_id: u64,
        namespace: String,
        deployment_name: String,
        build_id: String,
        percentage: f32,
    },
    LoadBatchOperations {
        request_id: u64,
        namespace: String,
        page_size: usize,
        next_page_token: Vec<u8>,
    },
    LoadBatchOperationDetails {
        request_id: u64,
        namespace: String,
        job_id: String,
    },
    PreviewBatchOperation {
        request_id: u64,
        namespace: String,
        form: BatchCreateForm,
        request: BatchOperationRequest,
    },
    StartBatchOperation {
        request_id: u64,
        namespace: String,
        request: BatchOperationRequest,
    },
    StopBatchOperation {
        request_id: u64,
        namespace: String,
        job_id: String,
        reason: String,
    },
    QueryWorkflow {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        query_name: String,
        arguments: Vec<Value>,
    },
    UpdateWorkflow {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        update_name: String,
        arguments: Vec<Value>,
    },
    PauseWorkflow {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        reason: String,
    },
    UnpauseWorkflow {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        reason: String,
    },
    ResetWorkflow {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        event_id: i64,
        reason: String,
    },
    Cancel {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        reason: String,
    },
    Terminate {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        reason: String,
    },
    Signal {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
        signal_name: String,
        input: Value,
    },
    CreateSchedule {
        request_id: u64,
        namespace: String,
        request: ScheduleCreateRequest,
    },
    UpdateSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
        request: ScheduleUpdateRequest,
    },
    PauseSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
        note: String,
    },
    UnpauseSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
        note: String,
    },
    TriggerSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
    },
    BackfillSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
        request: ScheduleBackfillRequest,
    },
    DeleteSchedule {
        request_id: u64,
        namespace: String,
        schedule_id: String,
    },
    Copy {
        request_id: u64,
        text: String,
    },
    Export {
        request_id: u64,
        namespace: String,
        cluster: Option<ClusterInfo>,
        details: Box<WorkflowDetails>,
    },
    OpenWeb {
        request_id: u64,
        url: String,
    },
}

/// Result of an asynchronous command.
#[derive(Debug)]
pub enum Message {
    ClusterLoaded {
        request_id: u64,
        result: Result<ClusterInfo, String>,
    },
    ProfileSwitchFinished {
        request_id: u64,
        result: Result<ProfileConnectionInfo, String>,
    },
    CapabilitiesLoaded {
        request_id: u64,
        result: Result<ServerCapabilities, String>,
    },
    NamespacesLoaded {
        request_id: u64,
        result: Result<Vec<NamespaceSummary>, String>,
    },
    WorkflowsLoaded {
        request_id: u64,
        result: Result<WorkflowPage, String>,
    },
    WorkflowCountLoaded {
        request_id: u64,
        result: Result<WorkflowCount, String>,
    },
    DetailsLoaded {
        request_id: u64,
        result: Result<Box<WorkflowDetails>, String>,
    },
    HistoryPageLoaded {
        request_id: u64,
        result: Result<HistoryPage, String>,
    },
    WorkflowChainLoaded {
        request_id: u64,
        result: Result<Vec<WorkflowSummary>, String>,
    },
    TaskQueuesLoaded {
        request_id: u64,
        result: Result<Vec<TaskQueueSummary>, String>,
    },
    WorkersLoaded {
        request_id: u64,
        result: Result<WorkerPage, String>,
    },
    WorkerDetailsLoaded {
        request_id: u64,
        result: Result<Box<WorkerDetails>, String>,
    },
    WorkerDeploymentsLoaded {
        request_id: u64,
        result: Result<WorkerDeploymentPage, String>,
    },
    WorkerDeploymentDetailsLoaded {
        request_id: u64,
        result: Result<Box<WorkerDeploymentDetails>, String>,
    },
    SchedulesLoaded {
        request_id: u64,
        result: Result<SchedulePage, String>,
    },
    ScheduleDetailsLoaded {
        request_id: u64,
        result: Result<Box<ScheduleDetails>, String>,
    },
    SearchAttributesLoaded {
        request_id: u64,
        result: Result<Vec<SearchAttributeSummary>, String>,
    },
    BatchOperationsLoaded {
        request_id: u64,
        result: Result<BatchOperationPage, String>,
    },
    BatchOperationDetailsLoaded {
        request_id: u64,
        result: Result<Box<BatchOperationDetails>, String>,
    },
    BatchOperationPreviewLoaded {
        request_id: u64,
        form: BatchCreateForm,
        result: Result<i64, String>,
    },
    WorkflowCallFinished {
        request_id: u64,
        kind: WorkflowCallKind,
        result: Result<WorkflowCallResult, String>,
    },
    ResetFinished {
        request_id: u64,
        result: Result<String, String>,
    },
    OperationFinished {
        request_id: u64,
        operation: OperationKind,
        result: Result<(), String>,
    },
    UtilityFinished {
        request_id: u64,
        operation: UtilityKind,
        result: Result<String, String>,
    },
}

/// Local desktop side effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilityKind {
    Copy,
    Export,
    OpenWeb,
}

/// Completed mutating operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Cancel,
    Terminate,
    Signal,
    PauseWorkflow,
    UnpauseWorkflow,
    CreateSchedule,
    UpdateSchedule,
    PauseSchedule,
    UnpauseSchedule,
    TriggerSchedule,
    BackfillSchedule,
    DeleteSchedule,
    AddSearchAttribute,
    RemoveSearchAttribute,
    SetDeploymentCurrent,
    SetDeploymentRamp,
    StartBatchOperation,
    StopBatchOperation,
}

impl OperationKind {
    const fn success_message(self) -> &'static str {
        match self {
            Self::Cancel => "Cancellation requested",
            Self::Terminate => "Workflow terminated",
            Self::Signal => "Signal delivered",
            Self::PauseWorkflow => "Workflow paused",
            Self::UnpauseWorkflow => "Workflow unpaused",
            Self::CreateSchedule => "Schedule created",
            Self::UpdateSchedule => "Schedule updated",
            Self::PauseSchedule => "Schedule paused",
            Self::UnpauseSchedule => "Schedule unpaused",
            Self::TriggerSchedule => "Schedule triggered",
            Self::BackfillSchedule => "Schedule backfill requested",
            Self::DeleteSchedule => "Schedule deleted",
            Self::AddSearchAttribute => "Search Attribute registered",
            Self::RemoveSearchAttribute => "Search Attribute removed",
            Self::SetDeploymentCurrent => "Current Worker Deployment version updated",
            Self::SetDeploymentRamp => "Worker Deployment ramp updated",
            Self::StartBatchOperation => "Batch operation started",
            Self::StopBatchOperation => "Batch operation stop requested",
        }
    }

    const fn is_schedule(self) -> bool {
        matches!(
            self,
            Self::CreateSchedule
                | Self::UpdateSchedule
                | Self::PauseSchedule
                | Self::UnpauseSchedule
                | Self::TriggerSchedule
                | Self::BackfillSchedule
                | Self::DeleteSchedule
        )
    }

    const fn is_search_attribute(self) -> bool {
        matches!(self, Self::AddSearchAttribute | Self::RemoveSearchAttribute)
    }

    const fn is_deployment(self) -> bool {
        matches!(self, Self::SetDeploymentCurrent | Self::SetDeploymentRamp)
    }

    const fn is_batch(self) -> bool {
        matches!(self, Self::StartBatchOperation | Self::StopBatchOperation)
    }
}

/// Complete dashboard state.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent UI, loading, and lifecycle flags are clearer than artificial enums"
)]
pub struct App {
    pub address: String,
    pub profile_name: Option<String>,
    pub namespace: String,
    pub query: String,
    pub page_size: usize,
    pub refresh_interval: Duration,
    pub auto_refresh: bool,
    pub color: bool,
    pub read_only: bool,
    pub force_read_only: bool,
    pub codec_enabled: bool,
    pub web_ui_url: Option<String>,
    pub saved_queries: Vec<SavedQuery>,
    pub profiles: Vec<ProfileSummary>,
    pub switching_profile: bool,
    pub pending_profile_name: Option<String>,
    pub view: View,
    pub cluster: Option<ClusterInfo>,
    pub capabilities: Option<ServerCapabilities>,
    pub capabilities_error: Option<String>,
    pub loading_capabilities: bool,
    pub namespaces: Vec<NamespaceSummary>,
    pub workflows: Vec<WorkflowSummary>,
    pub workflow_count: Option<WorkflowCount>,
    pub page_number: usize,
    pub has_previous_page: bool,
    pub has_next_page: bool,
    pub selected_workflow: usize,
    pub details: Option<WorkflowDetails>,
    pub workflow_chain: Vec<WorkflowSummary>,
    pub task_queues: Vec<TaskQueueSummary>,
    pub selected_task_queue: usize,
    pub task_queues_error: Option<String>,
    pub workers: Vec<WorkerSummary>,
    pub selected_worker: usize,
    pub worker_details: Option<WorkerDetails>,
    pub workers_error: Option<String>,
    pub worker_page_number: usize,
    pub worker_has_previous_page: bool,
    pub worker_has_next_page: bool,
    pub worker_deployments: Vec<WorkerDeploymentSummary>,
    pub selected_worker_deployment: usize,
    pub worker_deployment_details: Option<WorkerDeploymentDetails>,
    pub worker_deployments_error: Option<String>,
    pub deployment_page_number: usize,
    pub deployment_has_previous_page: bool,
    pub deployment_has_next_page: bool,
    pub schedule_query: String,
    pub schedules: Vec<ScheduleSummary>,
    pub selected_schedule: usize,
    pub schedule_details: Option<ScheduleDetails>,
    pub schedules_error: Option<String>,
    pub schedule_page_number: usize,
    pub schedule_has_previous_page: bool,
    pub schedule_has_next_page: bool,
    pub batch_operations: Vec<BatchOperationSummary>,
    pub selected_batch_operation: usize,
    pub batch_operation_details: Option<BatchOperationDetails>,
    pub batch_operations_error: Option<String>,
    pub batch_page_number: usize,
    pub batch_has_previous_page: bool,
    pub batch_has_next_page: bool,
    pub search_attributes: Vec<SearchAttributeSummary>,
    pub search_attributes_error: Option<String>,
    pub loading_search_attributes: bool,
    pub selected_event: usize,
    pub focus: Focus,
    pub overlay: Option<Overlay>,
    pub notice: Option<Notice>,
    pub loading_workflows: bool,
    pub loading_details: bool,
    pub loading_history_page: bool,
    pub loading_chain: bool,
    pub loading_task_queues: bool,
    pub loading_workers: bool,
    pub loading_worker_details: bool,
    pub loading_worker_deployments: bool,
    pub loading_worker_deployment_details: bool,
    pub loading_schedules: bool,
    pub loading_schedule_details: bool,
    pub loading_batch_operations: bool,
    pub loading_batch_operation_details: bool,
    pub batch_preview_in_flight: bool,
    pub call_in_flight: bool,
    pub operation_in_flight: bool,
    pub should_quit: bool,
    current_cluster_request: u64,
    current_profile_request: u64,
    current_capabilities_request: u64,
    current_workflow_request: u64,
    current_count_request: u64,
    current_detail_request: u64,
    current_history_request: u64,
    current_chain_request: u64,
    current_task_queues_request: u64,
    current_workers_request: u64,
    current_worker_details_request: u64,
    current_worker_deployments_request: u64,
    current_worker_deployment_details_request: u64,
    current_schedules_request: u64,
    current_schedule_details_request: u64,
    current_search_attributes_request: u64,
    current_batch_operations_request: u64,
    current_batch_operation_details_request: u64,
    current_batch_preview_request: u64,
    current_call_request: u64,
    current_namespace_request: u64,
    current_operation_request: u64,
    current_utility_request: u64,
    next_request_id: u64,
    current_page_token: Vec<u8>,
    next_page_token: Vec<u8>,
    previous_page_tokens: Vec<Vec<u8>>,
    worker_current_page_token: Vec<u8>,
    worker_next_page_token: Vec<u8>,
    worker_previous_page_tokens: Vec<Vec<u8>>,
    deployment_current_page_token: Vec<u8>,
    deployment_next_page_token: Vec<u8>,
    deployment_previous_page_tokens: Vec<Vec<u8>>,
    schedule_current_page_token: Vec<u8>,
    schedule_next_page_token: Vec<u8>,
    schedule_previous_page_tokens: Vec<Vec<u8>>,
    batch_current_page_token: Vec<u8>,
    batch_next_page_token: Vec<u8>,
    batch_previous_page_tokens: Vec<Vec<u8>>,
    manual_task_queue_names: BTreeSet<String>,
    last_refresh_started: Instant,
}

impl App {
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        Self {
            address: config.address,
            profile_name: config.profile_name,
            namespace: config.namespace,
            query: config.query,
            page_size: config.page_size,
            refresh_interval: config.refresh_interval,
            auto_refresh: config.auto_refresh,
            color: config.color,
            read_only: config.read_only,
            force_read_only: config.force_read_only,
            codec_enabled: config.codec_enabled,
            web_ui_url: config.web_ui_url,
            saved_queries: config.saved_queries,
            profiles: config.profiles,
            switching_profile: false,
            pending_profile_name: None,
            view: View::Workflows,
            cluster: None,
            capabilities: None,
            capabilities_error: None,
            loading_capabilities: false,
            namespaces: Vec::new(),
            workflows: Vec::new(),
            workflow_count: None,
            page_number: 1,
            has_previous_page: false,
            has_next_page: false,
            selected_workflow: 0,
            details: None,
            workflow_chain: Vec::new(),
            task_queues: Vec::new(),
            selected_task_queue: 0,
            task_queues_error: None,
            workers: Vec::new(),
            selected_worker: 0,
            worker_details: None,
            workers_error: None,
            worker_page_number: 1,
            worker_has_previous_page: false,
            worker_has_next_page: false,
            worker_deployments: Vec::new(),
            selected_worker_deployment: 0,
            worker_deployment_details: None,
            worker_deployments_error: None,
            deployment_page_number: 1,
            deployment_has_previous_page: false,
            deployment_has_next_page: false,
            schedule_query: String::new(),
            schedules: Vec::new(),
            selected_schedule: 0,
            schedule_details: None,
            schedules_error: None,
            schedule_page_number: 1,
            schedule_has_previous_page: false,
            schedule_has_next_page: false,
            batch_operations: Vec::new(),
            selected_batch_operation: 0,
            batch_operation_details: None,
            batch_operations_error: None,
            batch_page_number: 1,
            batch_has_previous_page: false,
            batch_has_next_page: false,
            search_attributes: Vec::new(),
            search_attributes_error: None,
            loading_search_attributes: false,
            selected_event: 0,
            focus: Focus::Workflows,
            overlay: None,
            notice: None,
            loading_workflows: false,
            loading_details: false,
            loading_history_page: false,
            loading_chain: false,
            loading_task_queues: false,
            loading_workers: false,
            loading_worker_details: false,
            loading_worker_deployments: false,
            loading_worker_deployment_details: false,
            loading_schedules: false,
            loading_schedule_details: false,
            loading_batch_operations: false,
            loading_batch_operation_details: false,
            batch_preview_in_flight: false,
            call_in_flight: false,
            operation_in_flight: false,
            should_quit: false,
            current_cluster_request: 0,
            current_profile_request: 0,
            current_capabilities_request: 0,
            current_workflow_request: 0,
            current_count_request: 0,
            current_detail_request: 0,
            current_history_request: 0,
            current_chain_request: 0,
            current_task_queues_request: 0,
            current_workers_request: 0,
            current_worker_details_request: 0,
            current_worker_deployments_request: 0,
            current_worker_deployment_details_request: 0,
            current_schedules_request: 0,
            current_schedule_details_request: 0,
            current_search_attributes_request: 0,
            current_batch_operations_request: 0,
            current_batch_operation_details_request: 0,
            current_batch_preview_request: 0,
            current_call_request: 0,
            current_namespace_request: 0,
            current_operation_request: 0,
            current_utility_request: 0,
            next_request_id: 0,
            current_page_token: Vec::new(),
            next_page_token: Vec::new(),
            previous_page_tokens: Vec::new(),
            worker_current_page_token: Vec::new(),
            worker_next_page_token: Vec::new(),
            worker_previous_page_tokens: Vec::new(),
            deployment_current_page_token: Vec::new(),
            deployment_next_page_token: Vec::new(),
            deployment_previous_page_tokens: Vec::new(),
            schedule_current_page_token: Vec::new(),
            schedule_next_page_token: Vec::new(),
            schedule_previous_page_tokens: Vec::new(),
            batch_current_page_token: Vec::new(),
            batch_next_page_token: Vec::new(),
            batch_previous_page_tokens: Vec::new(),
            manual_task_queue_names: BTreeSet::new(),
            last_refresh_started: Instant::now(),
        }
    }

    /// Initial data fetches.
    pub fn bootstrap(&mut self) -> Vec<Command> {
        let mut commands = vec![
            self.load_cluster(),
            self.load_capabilities(),
            self.load_namespaces(),
        ];
        commands.extend(self.refresh_current_view(true));
        commands
    }

    /// Apply one key event and return requested side effects.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Vec::new();
        }
        if self.switching_profile {
            if key.code == KeyCode::Char('q') {
                self.should_quit = true;
            }
            return Vec::new();
        }
        if self.overlay.is_some() {
            return self.handle_overlay_key(key);
        }
        self.handle_dashboard_key(key)
    }

    /// Apply a service response, dropping stale responses by request ID.
    pub fn handle_message(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ClusterLoaded { request_id, result } => {
                if request_id != self.current_cluster_request {
                    return Vec::new();
                }
                match result {
                    Ok(cluster) => self.cluster = Some(cluster),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::ProfileSwitchFinished { request_id, result } => {
                if request_id != self.current_profile_request {
                    return Vec::new();
                }
                self.switching_profile = false;
                self.pending_profile_name = None;
                match result {
                    Ok(profile) => {
                        self.invalidate_pending_requests();
                        self.address = profile.address;
                        self.profile_name = Some(profile.name.clone());
                        self.namespace = profile.namespace;
                        self.read_only = self.force_read_only || profile.read_only;
                        self.codec_enabled = profile.codec_enabled;
                        self.web_ui_url = profile.web_ui_url;
                        self.clear_connected_state();
                        self.show_notice(
                            format!("Connected to profile/{}", profile.name),
                            NoticeKind::Success,
                        );
                        self.bootstrap()
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::CapabilitiesLoaded { request_id, result } => {
                if request_id != self.current_capabilities_request {
                    return Vec::new();
                }
                self.loading_capabilities = false;
                match result {
                    Ok(capabilities) => {
                        self.capabilities = Some(capabilities);
                        self.capabilities_error = None;
                        self.apply_capability_degradation();
                    }
                    Err(error) => {
                        self.capabilities_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                    }
                }
                Vec::new()
            }
            Message::NamespacesLoaded { request_id, result } => {
                if request_id != self.current_namespace_request {
                    return Vec::new();
                }
                match result {
                    Ok(namespaces) => self.namespaces = namespaces,
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::WorkflowsLoaded { request_id, result } => {
                if request_id != self.current_workflow_request {
                    return Vec::new();
                }
                self.loading_workflows = false;
                match result {
                    Ok(page) => {
                        let previous_key = self
                            .selected_workflow()
                            .map(|workflow| workflow.key.clone());
                        self.next_page_token = page.next_page_token;
                        self.has_previous_page = !self.previous_page_tokens.is_empty();
                        self.has_next_page = !self.next_page_token.is_empty();
                        self.page_number = self.previous_page_tokens.len() + 1;
                        self.workflows = page.workflows;
                        self.selected_workflow = previous_key
                            .and_then(|key| {
                                self.workflows
                                    .iter()
                                    .position(|workflow| workflow.key == key)
                            })
                            .unwrap_or(0)
                            .min(self.workflows.len().saturating_sub(1));
                        self.details = None;
                        self.selected_event = 0;
                        self.load_selected_details().into_iter().collect()
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::WorkflowCountLoaded { request_id, result } => {
                if request_id != self.current_count_request {
                    return Vec::new();
                }
                match result {
                    Ok(count) => self.workflow_count = Some(count),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::DetailsLoaded { request_id, result } => {
                if request_id != self.current_detail_request {
                    return Vec::new();
                }
                self.loading_details = false;
                match result {
                    Ok(details) => {
                        self.selected_event = details.events.len().saturating_sub(1);
                        self.details = Some(*details);
                    }
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::HistoryPageLoaded { request_id, result } => {
                if request_id != self.current_history_request {
                    return Vec::new();
                }
                self.loading_history_page = false;
                match result {
                    Ok(page) => {
                        if let Some(details) = &mut self.details {
                            let selected_event_id = details
                                .events
                                .get(self.selected_event)
                                .map(|event| event.event_id);
                            let mut events = page.events;
                            events.append(&mut details.events);
                            events.sort_by_key(|event| event.event_id);
                            events.dedup_by_key(|event| event.event_id);
                            details.events = events;
                            details.history_next_page_token = page.next_page_token;
                            details.history_archived |= page.archived;
                            self.selected_event = selected_event_id
                                .and_then(|event_id| {
                                    details
                                        .events
                                        .iter()
                                        .position(|event| event.event_id == event_id)
                                })
                                .unwrap_or(0);
                        }
                    }
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::WorkflowChainLoaded { request_id, result } => {
                if request_id != self.current_chain_request {
                    return Vec::new();
                }
                self.loading_chain = false;
                match result {
                    Ok(chain) => {
                        self.workflow_chain = chain;
                        self.overlay = Some(Overlay::WorkflowChain { selected: 0 });
                    }
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::TaskQueuesLoaded { request_id, result } => {
                if request_id != self.current_task_queues_request {
                    return Vec::new();
                }
                self.loading_task_queues = false;
                match result {
                    Ok(task_queues) => {
                        let previous = self
                            .task_queues
                            .get(self.selected_task_queue)
                            .map(|queue| (queue.name.clone(), queue.queue_type));
                        self.task_queues = task_queues;
                        self.selected_task_queue = previous
                            .and_then(|key| {
                                self.task_queues
                                    .iter()
                                    .position(|queue| (queue.name.clone(), queue.queue_type) == key)
                            })
                            .unwrap_or(0)
                            .min(self.task_queues.len().saturating_sub(1));
                        self.task_queues_error = None;
                    }
                    Err(error) => {
                        self.task_queues_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                    }
                }
                Vec::new()
            }
            Message::WorkersLoaded { request_id, result } => {
                if request_id != self.current_workers_request {
                    return Vec::new();
                }
                self.loading_workers = false;
                match result {
                    Ok(page) => {
                        let previous = self
                            .workers
                            .get(self.selected_worker)
                            .map(|worker| worker.instance_key.clone());
                        self.worker_next_page_token = page.next_page_token;
                        self.worker_has_previous_page =
                            !self.worker_previous_page_tokens.is_empty();
                        self.worker_has_next_page = !self.worker_next_page_token.is_empty();
                        self.worker_page_number = self.worker_previous_page_tokens.len() + 1;
                        self.workers = page.workers;
                        self.selected_worker = previous
                            .and_then(|key| {
                                self.workers
                                    .iter()
                                    .position(|worker| worker.instance_key == key)
                            })
                            .unwrap_or(0)
                            .min(self.workers.len().saturating_sub(1));
                        self.worker_details = None;
                        self.workers_error = None;
                        self.load_selected_worker_details().into_iter().collect()
                    }
                    Err(error) => {
                        self.workers_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::WorkerDetailsLoaded { request_id, result } => {
                if request_id != self.current_worker_details_request {
                    return Vec::new();
                }
                self.loading_worker_details = false;
                match result {
                    Ok(details) => self.worker_details = Some(*details),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::WorkerDeploymentsLoaded { request_id, result } => {
                if request_id != self.current_worker_deployments_request {
                    return Vec::new();
                }
                self.loading_worker_deployments = false;
                match result {
                    Ok(page) => {
                        let previous = self
                            .worker_deployments
                            .get(self.selected_worker_deployment)
                            .map(|deployment| deployment.name.clone());
                        self.deployment_next_page_token = page.next_page_token;
                        self.deployment_has_previous_page =
                            !self.deployment_previous_page_tokens.is_empty();
                        self.deployment_has_next_page = !self.deployment_next_page_token.is_empty();
                        self.deployment_page_number =
                            self.deployment_previous_page_tokens.len() + 1;
                        self.worker_deployments = page.deployments;
                        self.selected_worker_deployment = previous
                            .and_then(|name| {
                                self.worker_deployments
                                    .iter()
                                    .position(|deployment| deployment.name == name)
                            })
                            .unwrap_or(0)
                            .min(self.worker_deployments.len().saturating_sub(1));
                        self.worker_deployment_details = None;
                        self.worker_deployments_error = None;
                        self.load_selected_worker_deployment_details()
                            .into_iter()
                            .collect()
                    }
                    Err(error) => {
                        self.worker_deployments_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::WorkerDeploymentDetailsLoaded { request_id, result } => {
                if request_id != self.current_worker_deployment_details_request {
                    return Vec::new();
                }
                self.loading_worker_deployment_details = false;
                match result {
                    Ok(details) => self.worker_deployment_details = Some(*details),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::SchedulesLoaded { request_id, result } => {
                if request_id != self.current_schedules_request {
                    return Vec::new();
                }
                self.loading_schedules = false;
                match result {
                    Ok(page) => {
                        let previous = self
                            .schedules
                            .get(self.selected_schedule)
                            .map(|schedule| schedule.schedule_id.clone());
                        self.schedule_next_page_token = page.next_page_token;
                        self.schedule_has_previous_page =
                            !self.schedule_previous_page_tokens.is_empty();
                        self.schedule_has_next_page = !self.schedule_next_page_token.is_empty();
                        self.schedule_page_number = self.schedule_previous_page_tokens.len() + 1;
                        self.schedules = page.schedules;
                        self.selected_schedule = previous
                            .and_then(|id| {
                                self.schedules
                                    .iter()
                                    .position(|schedule| schedule.schedule_id == id)
                            })
                            .unwrap_or(0)
                            .min(self.schedules.len().saturating_sub(1));
                        self.schedule_details = None;
                        self.schedules_error = None;
                        self.load_selected_schedule_details().into_iter().collect()
                    }
                    Err(error) => {
                        self.schedules_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::ScheduleDetailsLoaded { request_id, result } => {
                if request_id != self.current_schedule_details_request {
                    return Vec::new();
                }
                self.loading_schedule_details = false;
                match result {
                    Ok(details) => self.schedule_details = Some(*details),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::SearchAttributesLoaded { request_id, result } => {
                if request_id != self.current_search_attributes_request {
                    return Vec::new();
                }
                self.loading_search_attributes = false;
                match result {
                    Ok(attributes) => {
                        self.search_attributes = attributes;
                        self.search_attributes_error = None;
                    }
                    Err(error) => {
                        self.search_attributes_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                    }
                }
                Vec::new()
            }
            Message::BatchOperationsLoaded { request_id, result } => {
                if request_id != self.current_batch_operations_request {
                    return Vec::new();
                }
                self.loading_batch_operations = false;
                match result {
                    Ok(page) => {
                        let previous_job = self
                            .batch_operations
                            .get(self.selected_batch_operation)
                            .map(|operation| operation.job_id.clone());
                        self.batch_next_page_token = page.next_page_token;
                        self.batch_has_previous_page = !self.batch_previous_page_tokens.is_empty();
                        self.batch_has_next_page = !self.batch_next_page_token.is_empty();
                        self.batch_page_number = self.batch_previous_page_tokens.len() + 1;
                        self.batch_operations = page.operations;
                        self.selected_batch_operation = previous_job
                            .and_then(|job_id| {
                                self.batch_operations
                                    .iter()
                                    .position(|operation| operation.job_id == job_id)
                            })
                            .unwrap_or(0)
                            .min(self.batch_operations.len().saturating_sub(1));
                        self.batch_operation_details = None;
                        self.batch_operations_error = None;
                        self.load_selected_batch_operation_details()
                            .into_iter()
                            .collect()
                    }
                    Err(error) => {
                        self.batch_operations_error = Some(error.clone());
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::BatchOperationDetailsLoaded { request_id, result } => {
                if request_id != self.current_batch_operation_details_request {
                    return Vec::new();
                }
                self.loading_batch_operation_details = false;
                match result {
                    Ok(details) => self.batch_operation_details = Some(*details),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::BatchOperationPreviewLoaded {
                request_id,
                form,
                result,
            } => {
                if request_id != self.current_batch_preview_request {
                    return Vec::new();
                }
                self.batch_preview_in_flight = false;
                match result {
                    Ok(matched_workflows) => {
                        self.overlay = Some(Overlay::BatchConfirm {
                            form,
                            matched_workflows,
                            input: TextInput::default(),
                        });
                    }
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
            Message::WorkflowCallFinished {
                request_id,
                kind,
                result,
            } => {
                if request_id != self.current_call_request {
                    return Vec::new();
                }
                self.call_in_flight = false;
                self.operation_in_flight = false;
                match result {
                    Ok(result) => {
                        self.overlay = Some(Overlay::WorkflowCallResult {
                            kind,
                            result,
                            scroll: 0,
                        });
                        if kind == WorkflowCallKind::Update {
                            self.refresh_workflows(false)
                        } else {
                            Vec::new()
                        }
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::ResetFinished { request_id, result } => {
                if request_id != self.current_operation_request {
                    return Vec::new();
                }
                self.operation_in_flight = false;
                match result {
                    Ok(run_id) => {
                        self.show_notice(
                            format!("Workflow reset into run {run_id}"),
                            NoticeKind::Success,
                        );
                        self.refresh_workflows(false)
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::OperationFinished {
                request_id,
                operation,
                result,
            } => {
                if request_id != self.current_operation_request {
                    return Vec::new();
                }
                self.operation_in_flight = false;
                match result {
                    Ok(()) => {
                        self.show_notice(operation.success_message(), NoticeKind::Success);
                        if operation.is_schedule() {
                            self.refresh_schedules(false)
                        } else if operation.is_search_attribute() {
                            self.overlay = Some(Overlay::SearchAttributes { selected: 0 });
                            vec![self.load_search_attributes()]
                        } else if operation.is_deployment() {
                            self.refresh_worker_deployments(false)
                        } else if operation.is_batch() {
                            self.refresh_batch_operations(false)
                        } else {
                            self.refresh_workflows(false)
                        }
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
            }
            Message::UtilityFinished {
                request_id,
                operation,
                result,
            } => {
                if request_id != self.current_utility_request {
                    return Vec::new();
                }
                match result {
                    Ok(detail) => {
                        let message = match operation {
                            UtilityKind::Copy => "Copied workflow identity".to_string(),
                            UtilityKind::Export => format!("Exported to {detail}"),
                            UtilityKind::OpenWeb => "Opened in Temporal Web UI".to_string(),
                        };
                        self.show_notice(message, NoticeKind::Success);
                    }
                    Err(error) => self.show_notice(error, NoticeKind::Error),
                }
                Vec::new()
            }
        }
    }

    /// Timer hook for notice expiry and automatic refresh.
    pub fn on_tick(&mut self, now: Instant) -> Vec<Command> {
        if self
            .notice
            .as_ref()
            .is_some_and(|notice| now >= notice.expires_at)
        {
            self.notice = None;
        }
        if self.switching_profile {
            return Vec::new();
        }
        if self.auto_refresh
            && !self.current_view_is_loading()
            && now.duration_since(self.last_refresh_started) >= self.refresh_interval
        {
            return self.refresh_current_view(false);
        }
        Vec::new()
    }

    #[must_use]
    pub fn selected_workflow(&self) -> Option<&WorkflowSummary> {
        self.workflows.get(self.selected_workflow)
    }

    #[must_use]
    pub fn selected_namespace_index(&self) -> usize {
        self.namespaces
            .iter()
            .position(|namespace| namespace.name == self.namespace)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn selected_profile_index(&self) -> usize {
        self.profile_name
            .as_ref()
            .and_then(|name| {
                self.profiles
                    .iter()
                    .position(|profile| &profile.name == name)
            })
            .unwrap_or(0)
    }

    #[must_use]
    pub(crate) fn expects_profile_switch(&self, request_id: u64) -> bool {
        self.switching_profile && request_id == self.current_profile_request
    }

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> Vec<Command> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Char('1') => return self.switch_view(View::Workflows),
            KeyCode::Char('2') => return self.switch_view(View::TaskQueues),
            KeyCode::Char('3') => return self.switch_view(View::Workers),
            KeyCode::Char('4') => return self.switch_view(View::Deployments),
            KeyCode::Char('5') => return self.switch_view(View::Schedules),
            KeyCode::Char('6') => return self.switch_view(View::Batches),
            KeyCode::Char('/') if self.view == View::Workflows => {
                self.overlay = Some(Overlay::Query(TextInput::new(self.query.clone())));
            }
            KeyCode::Char('/') if self.view == View::Schedules => {
                self.overlay = Some(Overlay::ScheduleQuery(TextInput::new(
                    self.schedule_query.clone(),
                )));
            }
            KeyCode::Char('/') if self.view == View::TaskQueues => {
                self.overlay = Some(Overlay::TaskQueue(TextInput::default()));
            }
            KeyCode::Char('f') if self.view == View::Workflows => {
                if self.saved_queries.is_empty() {
                    self.show_notice("No saved visibility queries", NoticeKind::Info);
                } else {
                    self.overlay = Some(Overlay::SavedQueryPicker { selected: 0 });
                }
            }
            KeyCode::Char('#') if self.view == View::Workflows => {
                if let Some(message) = self.blocked_capability(Capability::VisibilityAggregations) {
                    self.show_notice(message, NoticeKind::Info);
                } else if self
                    .workflow_count
                    .as_ref()
                    .is_none_or(|count| count.groups.is_empty())
                {
                    self.show_notice(
                        "The current query has no GROUP BY aggregation",
                        NoticeKind::Info,
                    );
                } else {
                    self.overlay = Some(Overlay::Aggregations { selected: 0 });
                }
            }
            KeyCode::Char('P') => {
                if self.operation_in_flight || self.call_in_flight || self.batch_preview_in_flight {
                    self.show_notice(
                        "Wait for the active operation before switching profiles",
                        NoticeKind::Info,
                    );
                } else if self.profiles.is_empty() {
                    self.show_notice("No connection profiles configured", NoticeKind::Info);
                } else {
                    self.overlay = Some(Overlay::ProfilePicker {
                        selected: self.selected_profile_index(),
                    });
                }
            }
            KeyCode::Char('K') => {
                self.overlay = Some(Overlay::Capabilities);
                if self.loading_capabilities {
                    return Vec::new();
                }
                return vec![self.load_capabilities()];
            }
            KeyCode::Char('n') => {
                if self.namespaces.is_empty() {
                    self.show_notice("No namespaces loaded", NoticeKind::Info);
                } else {
                    self.overlay = Some(Overlay::NamespacePicker {
                        selected: self.selected_namespace_index(),
                    });
                }
            }
            KeyCode::Char('A') => {
                if let Some(message) = self.blocked_capability(Capability::SearchAttributes) {
                    self.show_notice(message, NoticeKind::Info);
                    return Vec::new();
                }
                self.overlay = Some(Overlay::SearchAttributes { selected: 0 });
                return vec![self.load_search_attributes()];
            }
            KeyCode::Char('r') => return self.refresh_current_view(false),
            KeyCode::Char('a') => {
                self.auto_refresh = !self.auto_refresh;
                let state = if self.auto_refresh {
                    "enabled"
                } else {
                    "disabled"
                };
                self.show_notice(format!("Auto-refresh {state}"), NoticeKind::Info);
            }
            KeyCode::Char('c') if self.view == View::Workflows => {
                self.open_confirmation(ConfirmAction::Cancel);
            }
            KeyCode::Char('x') if self.view == View::Workflows => {
                self.open_confirmation(ConfirmAction::Terminate);
            }
            KeyCode::Char('s') if self.view == View::Workflows => self.open_signal(),
            KeyCode::Char('Q') if self.view == View::Workflows => {
                self.open_workflow_call(WorkflowCallKind::Query);
            }
            KeyCode::Char('U') if self.view == View::Workflows => {
                self.open_workflow_call(WorkflowCallKind::Update);
            }
            KeyCode::Char('p') if self.view == View::Workflows => self.open_pause_toggle(),
            KeyCode::Char('R') if self.view == View::Workflows => self.open_reset(),
            KeyCode::Char('C') if self.view == View::Deployments => {
                self.open_deployment_current();
            }
            KeyCode::Char('R') if self.view == View::Deployments => {
                self.open_deployment_ramp();
            }
            KeyCode::Char('N') if self.view == View::Schedules => self.open_schedule_create(),
            KeyCode::Char('E') if self.view == View::Schedules => self.open_schedule_edit(),
            KeyCode::Char('p') if self.view == View::Schedules => {
                return self.toggle_schedule_pause();
            }
            KeyCode::Char('t') if self.view == View::Schedules => {
                self.open_schedule_confirmation(ScheduleConfirmAction::Trigger);
            }
            KeyCode::Char('b') if self.view == View::Schedules => self.open_schedule_backfill(),
            KeyCode::Char('d') if self.view == View::Schedules => {
                self.open_schedule_confirmation(ScheduleConfirmAction::Delete);
            }
            KeyCode::Char('N') if self.view == View::Batches => self.open_batch_create(),
            KeyCode::Char('s') if self.view == View::Batches => self.open_batch_stop(),
            KeyCode::Char('[') => return self.previous_page(),
            KeyCode::Char(']') => return self.next_page(),
            KeyCode::Char('H') if self.view == View::Workflows => {
                return self.load_older_history();
            }
            KeyCode::Char('C') if self.view == View::Workflows => {
                return self.load_workflow_chain();
            }
            KeyCode::Char('v') if self.view == View::Workflows => {
                if self.details.is_some() {
                    self.overlay = Some(Overlay::Inspector { scroll: 0 });
                }
            }
            KeyCode::Char('y') if self.view == View::Workflows => {
                return self.copy_workflow_identity();
            }
            KeyCode::Char('e') if self.view == View::Workflows => return self.export_workflow(),
            KeyCode::Char('o') if self.view == View::Workflows => {
                return self.open_in_web_ui();
            }
            KeyCode::Tab | KeyCode::Enter if self.view == View::Workflows => {
                self.focus = match self.focus {
                    Focus::Workflows if self.details.is_some() => Focus::History,
                    _ => Focus::Workflows,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => return self.move_up(1),
            KeyCode::Down | KeyCode::Char('j') => return self.move_down(1),
            KeyCode::PageUp => return self.move_up(10),
            KeyCode::PageDown => return self.move_down(10),
            KeyCode::Home | KeyCode::Char('g') => return self.move_first(),
            KeyCode::End | KeyCode::Char('G') => return self.move_last(),
            _ => {}
        }
        Vec::new()
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) -> Vec<Command> {
        let Some(mut overlay) = self.overlay.take() else {
            return Vec::new();
        };

        match &mut overlay {
            Overlay::Help => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Char('q' | '?')) {
                    return Vec::new();
                }
            }
            Overlay::Capabilities => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'K') => return Vec::new(),
                KeyCode::Char('r') => return vec![self.load_capabilities()],
                _ => {}
            },
            Overlay::Query(input) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    self.query = input.value.trim().to_string();
                    return self.refresh_workflows(true);
                }
                _ => edit_text(input, key),
            },
            Overlay::ScheduleQuery(input) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    self.schedule_query = input.value.trim().to_string();
                    return self.refresh_schedules(true);
                }
                _ => edit_text(input, key),
            },
            Overlay::TaskQueue(input) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    let name = input.value.trim();
                    if name.is_empty() {
                        self.show_notice("Task Queue name must not be empty", NoticeKind::Error);
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    self.manual_task_queue_names.insert(name.to_string());
                    return self.refresh_task_queues();
                }
                _ => edit_text(input, key),
            },
            Overlay::SavedQueryPicker { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'f') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(self.saved_queries.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self.saved_queries.len().saturating_sub(1);
                }
                KeyCode::Enter => {
                    if let Some(filter) = self.saved_queries.get(*selected) {
                        self.query.clone_from(&filter.query);
                        return self.refresh_workflows(true);
                    }
                    return Vec::new();
                }
                _ => {}
            },
            Overlay::Aggregations { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | '#') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let last = self
                        .workflow_count
                        .as_ref()
                        .map_or(0, |count| count.groups.len().saturating_sub(1));
                    *selected = (*selected + 1).min(last);
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self
                        .workflow_count
                        .as_ref()
                        .map_or(0, |count| count.groups.len().saturating_sub(1));
                }
                _ => {}
            },
            Overlay::ProfilePicker { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'P') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(self.profiles.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self.profiles.len().saturating_sub(1);
                }
                KeyCode::Enter => {
                    let Some(profile) = self.profiles.get(*selected).cloned() else {
                        return Vec::new();
                    };
                    if self.profile_name.as_deref() == Some(profile.name.as_str()) {
                        self.show_notice(
                            format!("Already connected to profile/{}", profile.name),
                            NoticeKind::Info,
                        );
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_profile_request = request_id;
                    self.switching_profile = true;
                    self.pending_profile_name = Some(profile.name.clone());
                    self.show_notice(
                        format!("Connecting to profile/{}…", profile.name),
                        NoticeKind::Info,
                    );
                    return vec![Command::SwitchProfile {
                        request_id,
                        profile_name: profile.name,
                    }];
                }
                _ => {}
            },
            Overlay::NamespacePicker { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(self.namespaces.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self.namespaces.len().saturating_sub(1);
                }
                KeyCode::Enter => {
                    if let Some(namespace) = self.namespaces.get(*selected) {
                        self.namespace.clone_from(&namespace.name);
                        self.workflows.clear();
                        self.details = None;
                        self.selected_workflow = 0;
                        self.selected_event = 0;
                        self.task_queues.clear();
                        self.workers.clear();
                        self.worker_details = None;
                        self.worker_deployments.clear();
                        self.worker_deployment_details = None;
                        self.schedules.clear();
                        self.schedule_details = None;
                        self.batch_operations.clear();
                        self.batch_operation_details = None;
                        self.search_attributes.clear();
                        self.search_attributes_error = None;
                        self.capabilities = None;
                        self.capabilities_error = None;
                        self.manual_task_queue_names.clear();
                        self.reset_worker_pagination();
                        self.reset_deployment_pagination();
                        self.reset_schedule_pagination();
                        self.reset_batch_pagination();
                        let mut commands = vec![self.load_capabilities()];
                        commands.extend(self.refresh_current_view(true));
                        return commands;
                    }
                    return Vec::new();
                }
                _ => {}
            },
            Overlay::SearchAttributes { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'A') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(self.search_attributes.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self.search_attributes.len().saturating_sub(1);
                }
                KeyCode::Char('r') => {
                    self.overlay = Some(overlay);
                    return vec![self.load_search_attributes()];
                }
                KeyCode::Char('a') => {
                    if self.read_only {
                        self.show_notice(
                            "Search Attribute registration is blocked by read-only mode",
                            NoticeKind::Error,
                        );
                    } else {
                        self.overlay = Some(Overlay::SearchAttributeAdd(
                            SearchAttributeAddForm::default(),
                        ));
                        return Vec::new();
                    }
                }
                KeyCode::Char('d') => {
                    if self.read_only {
                        self.show_notice(
                            "Search Attribute removal is blocked by read-only mode",
                            NoticeKind::Error,
                        );
                    } else if let Some(attribute) = self.search_attributes.get(*selected) {
                        if attribute.custom {
                            self.overlay = Some(Overlay::SearchAttributeRemove {
                                name: attribute.name.clone(),
                                input: TextInput::default(),
                            });
                            return Vec::new();
                        }
                        self.show_notice(
                            "System Search Attributes cannot be removed",
                            NoticeKind::Info,
                        );
                    }
                }
                _ => {}
            },
            Overlay::SearchAttributeAdd(form) => match key.code {
                KeyCode::Esc => {
                    self.overlay = Some(Overlay::SearchAttributes { selected: 0 });
                    return Vec::new();
                }
                KeyCode::Tab => {
                    form.active_field =
                        next_field(form.active_field, &SearchAttributeAddField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field =
                        previous_field(form.active_field, &SearchAttributeAddField::ALL);
                }
                KeyCode::Enter if form.active_field != SearchAttributeAddField::Confirmation => {
                    form.active_field =
                        next_field(form.active_field, &SearchAttributeAddField::ALL);
                }
                KeyCode::Enter => {
                    let name = form.name.value.trim().to_string();
                    if name.is_empty() {
                        self.show_notice(
                            "Search Attribute name must not be empty",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if name.to_ascii_lowercase().starts_with("temporal") {
                        self.show_notice(
                            "Search Attribute names starting with Temporal are reserved by the \
                             server",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let Some(value_type) = canonical_search_attribute_type(&form.value_type.value)
                    else {
                        self.show_notice(
                            "Type must be Text, Keyword, Int, Double, Bool, Datetime, or \
                             KeywordList",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    };
                    if form.confirmation.value != name {
                        self.show_notice(
                            "Confirmation must exactly match the Search Attribute name",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::AddSearchAttribute {
                        request_id,
                        namespace: self.namespace.clone(),
                        name,
                        value_type: value_type.to_string(),
                    }];
                }
                _ => edit_search_attribute_add_field(form, key),
            },
            Overlay::SearchAttributeRemove { name, input } => match key.code {
                KeyCode::Esc => {
                    self.overlay = Some(Overlay::SearchAttributes { selected: 0 });
                    return Vec::new();
                }
                KeyCode::Enter => {
                    if input.value != *name {
                        self.show_notice(
                            "Confirmation must exactly match the Search Attribute name",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::RemoveSearchAttribute {
                        request_id,
                        namespace: self.namespace.clone(),
                        name: name.clone(),
                    }];
                }
                _ => edit_text(input, key),
            },
            Overlay::DeploymentCurrent(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab | KeyCode::BackTab => {
                    form.active_field = match form.active_field {
                        DeploymentCurrentField::BuildId => DeploymentCurrentField::Confirmation,
                        DeploymentCurrentField::Confirmation => DeploymentCurrentField::BuildId,
                    };
                }
                KeyCode::Enter if form.active_field == DeploymentCurrentField::BuildId => {
                    form.active_field = DeploymentCurrentField::Confirmation;
                }
                KeyCode::Enter => {
                    if form.confirmation.value != form.deployment_name {
                        self.show_notice(
                            "Confirmation must exactly match the Worker Deployment name",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::SetDeploymentCurrent {
                        request_id,
                        namespace: self.namespace.clone(),
                        deployment_name: form.deployment_name.clone(),
                        build_id: form.build_id.value.trim().to_string(),
                    }];
                }
                _ => match form.active_field {
                    DeploymentCurrentField::BuildId => edit_text(&mut form.build_id, key),
                    DeploymentCurrentField::Confirmation => {
                        edit_text(&mut form.confirmation, key);
                    }
                },
            },
            Overlay::DeploymentRamp(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = next_field(form.active_field, &DeploymentRampField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field =
                        previous_field(form.active_field, &DeploymentRampField::ALL);
                }
                KeyCode::Enter if form.active_field != DeploymentRampField::Confirmation => {
                    form.active_field = next_field(form.active_field, &DeploymentRampField::ALL);
                }
                KeyCode::Enter => {
                    let build_id = form.build_id.value.trim().to_string();
                    let percentage = match form.percentage.value.trim().parse::<f32>() {
                        Ok(percentage)
                            if percentage.is_finite() && (0.0..=100.0).contains(&percentage) =>
                        {
                            percentage
                        }
                        _ => {
                            self.show_notice(
                                "Ramp percentage must be a number from 0 through 100",
                                NoticeKind::Error,
                            );
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    if build_id.is_empty() && percentage != 0.0 {
                        self.show_notice(
                            "Clearing the ramping version requires a 0% ramp",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if form.confirmation.value != form.deployment_name {
                        self.show_notice(
                            "Confirmation must exactly match the Worker Deployment name",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::SetDeploymentRamp {
                        request_id,
                        namespace: self.namespace.clone(),
                        deployment_name: form.deployment_name.clone(),
                        build_id,
                        percentage,
                    }];
                }
                _ => edit_deployment_ramp_field(form, key),
            },
            Overlay::BatchCreate(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = next_field(form.active_field, &BatchCreateField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field = previous_field(form.active_field, &BatchCreateField::ALL);
                }
                KeyCode::Enter if form.active_field != BatchCreateField::SignalInput => {
                    form.active_field = next_field(form.active_field, &BatchCreateField::ALL);
                }
                KeyCode::Enter => {
                    let request = match batch_request_from_form(form) {
                        Ok(request) => request,
                        Err(error) => {
                            self.show_notice(error, NoticeKind::Error);
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    if self.batch_preview_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_batch_preview_request = request_id;
                    self.batch_preview_in_flight = true;
                    return vec![Command::PreviewBatchOperation {
                        request_id,
                        namespace: self.namespace.clone(),
                        form: form.clone(),
                        request,
                    }];
                }
                _ => edit_batch_create_field(form, key),
            },
            Overlay::BatchConfirm {
                form,
                matched_workflows: _,
                input,
            } => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    if input.value != form.job_id.value.trim() {
                        self.show_notice(
                            "Confirmation must exactly match the batch Job ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request = match batch_request_from_form(form) {
                        Ok(request) => request,
                        Err(error) => {
                            self.show_notice(error, NoticeKind::Error);
                            return Vec::new();
                        }
                    };
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::StartBatchOperation {
                        request_id,
                        namespace: self.namespace.clone(),
                        request,
                    }];
                }
                _ => edit_text(input, key),
            },
            Overlay::BatchStop { job_id, input } => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    if input.value != *job_id {
                        self.show_notice(
                            "Confirmation must exactly match the batch Job ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::StopBatchOperation {
                        request_id,
                        namespace: self.namespace.clone(),
                        job_id: job_id.clone(),
                        reason: "Stopped from temporal-tui".to_string(),
                    }];
                }
                _ => edit_text(input, key),
            },
            Overlay::Confirm {
                action,
                key: workflow_key,
                workflow_id,
                input,
            } => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    if input.value != *workflow_id {
                        self.show_notice(
                            "Confirmation must exactly match the Workflow ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    if self.operation_in_flight {
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    self.operation_in_flight = true;
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    let reason = "Requested from temporal-tui".to_string();
                    return vec![match action {
                        ConfirmAction::Cancel => Command::Cancel {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key.clone(),
                            reason,
                        },
                        ConfirmAction::Terminate => Command::Terminate {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key.clone(),
                            reason,
                        },
                        ConfirmAction::Pause => Command::PauseWorkflow {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key.clone(),
                            reason,
                        },
                        ConfirmAction::Unpause => Command::UnpauseWorkflow {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key.clone(),
                            reason,
                        },
                    }];
                }
                _ => edit_text(input, key),
            },
            Overlay::Signal(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = match form.active_field {
                        SignalField::Name => SignalField::Input,
                        SignalField::Input => SignalField::Name,
                    };
                }
                KeyCode::Enter if form.active_field == SignalField::Name => {
                    form.active_field = SignalField::Input;
                }
                KeyCode::Enter => {
                    let signal_name = form.name.value.trim().to_string();
                    if signal_name.is_empty() {
                        self.show_notice("Signal name must not be empty", NoticeKind::Error);
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let input = match serde_json::from_str::<Value>(&form.input.value) {
                        Ok(input) => input,
                        Err(error) => {
                            self.show_notice(
                                format!("Signal input is not valid JSON: {error}"),
                                NoticeKind::Error,
                            );
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    let Some(key) = self
                        .selected_workflow()
                        .map(|workflow| workflow.key.clone())
                    else {
                        return Vec::new();
                    };
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::Signal {
                        request_id,
                        namespace: self.namespace.clone(),
                        key,
                        signal_name,
                        input,
                    }];
                }
                _ => match form.active_field {
                    SignalField::Name => edit_text(&mut form.name, key),
                    SignalField::Input => edit_text(&mut form.input, key),
                },
            },
            Overlay::WorkflowCall { kind, form } => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab | KeyCode::BackTab => {
                    form.active_field = match form.active_field {
                        HandlerField::Name => HandlerField::Input,
                        HandlerField::Input => HandlerField::Name,
                    };
                }
                KeyCode::Enter if form.active_field == HandlerField::Name => {
                    form.active_field = HandlerField::Input;
                }
                KeyCode::Enter => {
                    let name = form.name.value.trim().to_string();
                    if name.is_empty() {
                        self.show_notice(
                            format!("{} handler name must not be empty", kind.label()),
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let arguments = match parse_json_arguments(&form.input.value) {
                        Ok(arguments) => arguments,
                        Err(error) => {
                            self.show_notice(
                                format!("{} arguments are invalid: {error}", kind.label()),
                                NoticeKind::Error,
                            );
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    let Some(workflow_key) = self
                        .selected_workflow()
                        .map(|workflow| workflow.key.clone())
                    else {
                        return Vec::new();
                    };
                    let request_id = self.next_request_id();
                    self.current_call_request = request_id;
                    self.call_in_flight = true;
                    if *kind == WorkflowCallKind::Update {
                        self.operation_in_flight = true;
                    }
                    return vec![match kind {
                        WorkflowCallKind::Query => Command::QueryWorkflow {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key,
                            query_name: name,
                            arguments,
                        },
                        WorkflowCallKind::Update => Command::UpdateWorkflow {
                            request_id,
                            namespace: self.namespace.clone(),
                            key: workflow_key,
                            update_name: name,
                            arguments,
                        },
                    }];
                }
                _ => match form.active_field {
                    HandlerField::Name => edit_text(&mut form.name, key),
                    HandlerField::Input => edit_text(&mut form.input, key),
                },
            },
            Overlay::WorkflowCallResult { scroll, .. } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                KeyCode::Home | KeyCode::Char('g') => *scroll = 0,
                _ => {}
            },
            Overlay::Reset(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab | KeyCode::BackTab => {
                    form.active_field = match form.active_field {
                        ResetField::EventId => ResetField::Confirmation,
                        ResetField::Confirmation => ResetField::EventId,
                    };
                }
                KeyCode::Enter if form.active_field == ResetField::EventId => {
                    form.active_field = ResetField::Confirmation;
                }
                KeyCode::Enter => {
                    if form.confirmation.value != form.workflow_id {
                        self.show_notice(
                            "Confirmation must exactly match the Workflow ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let event_id = match form.event_id.value.trim().parse::<i64>() {
                        Ok(event_id) if event_id > 0 => event_id,
                        _ => {
                            self.show_notice(
                                "Reset event ID must be a positive integer",
                                NoticeKind::Error,
                            );
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::ResetWorkflow {
                        request_id,
                        namespace: self.namespace.clone(),
                        key: form.key.clone(),
                        event_id,
                        reason: "Requested from temporal-tui".to_string(),
                    }];
                }
                _ => match form.active_field {
                    ResetField::EventId => edit_text(&mut form.event_id, key),
                    ResetField::Confirmation => edit_text(&mut form.confirmation, key),
                },
            },
            Overlay::ScheduleCreate(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = next_field(form.active_field, &ScheduleCreateField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field =
                        previous_field(form.active_field, &ScheduleCreateField::ALL);
                }
                KeyCode::Enter if form.active_field != ScheduleCreateField::Notes => {
                    form.active_field = next_field(form.active_field, &ScheduleCreateField::ALL);
                }
                KeyCode::Enter => {
                    let required = [
                        ("Schedule ID", form.schedule_id.value.trim()),
                        ("Workflow ID", form.workflow_id.value.trim()),
                        ("Workflow type", form.workflow_type.value.trim()),
                        ("Task Queue", form.task_queue.value.trim()),
                        ("Schedule expression", form.expression.value.trim()),
                    ];
                    if let Some((label, _)) = required.iter().find(|(_, value)| value.is_empty()) {
                        self.show_notice(format!("{label} must not be empty"), NoticeKind::Error);
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let arguments = match parse_json_arguments(&form.input.value) {
                        Ok(arguments) => arguments,
                        Err(error) => {
                            self.show_notice(
                                format!("Schedule arguments are invalid: {error}"),
                                NoticeKind::Error,
                            );
                            self.overlay = Some(overlay);
                            return Vec::new();
                        }
                    };
                    let request = ScheduleCreateRequest {
                        schedule_id: form.schedule_id.value.trim().to_string(),
                        workflow_id: form.workflow_id.value.trim().to_string(),
                        workflow_type: form.workflow_type.value.trim().to_string(),
                        task_queue: form.task_queue.value.trim().to_string(),
                        schedule_expression: form.expression.value.trim().to_string(),
                        timezone: form.timezone.value.trim().to_string(),
                        arguments,
                        paused: false,
                        notes: form.notes.value.trim().to_string(),
                    };
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::CreateSchedule {
                        request_id,
                        namespace: self.namespace.clone(),
                        request,
                    }];
                }
                _ => edit_schedule_create_field(form, key),
            },
            Overlay::ScheduleEdit(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = next_field(form.active_field, &ScheduleEditField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field = previous_field(form.active_field, &ScheduleEditField::ALL);
                }
                KeyCode::Enter if form.active_field != ScheduleEditField::Notes => {
                    form.active_field = next_field(form.active_field, &ScheduleEditField::ALL);
                }
                KeyCode::Enter => {
                    let expression = form.expression.value.trim();
                    let timezone = form.timezone.value.trim();
                    let request = ScheduleUpdateRequest {
                        schedule_expression: (!expression.is_empty())
                            .then(|| expression.to_string()),
                        timezone: Some(timezone.to_string()),
                        notes: form.notes.value.trim().to_string(),
                    };
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::UpdateSchedule {
                        request_id,
                        namespace: self.namespace.clone(),
                        schedule_id: form.schedule_id.clone(),
                        request,
                    }];
                }
                _ => edit_schedule_edit_field(form, key),
            },
            Overlay::ScheduleBackfill(form) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Tab => {
                    form.active_field = next_field(form.active_field, &ScheduleBackfillField::ALL);
                }
                KeyCode::BackTab => {
                    form.active_field =
                        previous_field(form.active_field, &ScheduleBackfillField::ALL);
                }
                KeyCode::Enter if form.active_field != ScheduleBackfillField::Confirmation => {
                    form.active_field = next_field(form.active_field, &ScheduleBackfillField::ALL);
                }
                KeyCode::Enter => {
                    if form.confirmation.value != form.schedule_id {
                        self.show_notice(
                            "Confirmation must exactly match the Schedule ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let start_time =
                        match chrono::DateTime::parse_from_rfc3339(form.start_time.value.trim()) {
                            Ok(value) => value.with_timezone(&Utc),
                            Err(error) => {
                                self.show_notice(
                                    format!("Backfill start time is invalid: {error}"),
                                    NoticeKind::Error,
                                );
                                self.overlay = Some(overlay);
                                return Vec::new();
                            }
                        };
                    let end_time =
                        match chrono::DateTime::parse_from_rfc3339(form.end_time.value.trim()) {
                            Ok(value) => value.with_timezone(&Utc),
                            Err(error) => {
                                self.show_notice(
                                    format!("Backfill end time is invalid: {error}"),
                                    NoticeKind::Error,
                                );
                                self.overlay = Some(overlay);
                                return Vec::new();
                            }
                        };
                    if start_time >= end_time {
                        self.show_notice(
                            "Backfill start time must be before end time",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![Command::BackfillSchedule {
                        request_id,
                        namespace: self.namespace.clone(),
                        schedule_id: form.schedule_id.clone(),
                        request: ScheduleBackfillRequest {
                            start_time,
                            end_time,
                            overlap_policy: form.overlap_policy.value.trim().to_string(),
                        },
                    }];
                }
                _ => edit_schedule_backfill_field(form, key),
            },
            Overlay::ScheduleConfirm {
                action,
                schedule_id,
                input,
            } => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    if input.value != *schedule_id {
                        self.show_notice(
                            "Confirmation must exactly match the Schedule ID",
                            NoticeKind::Error,
                        );
                        self.overlay = Some(overlay);
                        return Vec::new();
                    }
                    let request_id = self.next_request_id();
                    self.current_operation_request = request_id;
                    self.operation_in_flight = true;
                    return vec![match action {
                        ScheduleConfirmAction::Trigger => Command::TriggerSchedule {
                            request_id,
                            namespace: self.namespace.clone(),
                            schedule_id: schedule_id.clone(),
                        },
                        ScheduleConfirmAction::Delete => Command::DeleteSchedule {
                            request_id,
                            namespace: self.namespace.clone(),
                            schedule_id: schedule_id.clone(),
                        },
                    }];
                }
                _ => edit_text(input, key),
            },
            Overlay::WorkflowChain { selected } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'C') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => {
                    *selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    *selected = (*selected + 1).min(self.workflow_chain.len().saturating_sub(1));
                }
                KeyCode::Home | KeyCode::Char('g') => *selected = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    *selected = self.workflow_chain.len().saturating_sub(1);
                }
                _ => {}
            },
            Overlay::Inspector { scroll } => match key.code {
                KeyCode::Esc | KeyCode::Char('q' | 'v') => return Vec::new(),
                KeyCode::Up | KeyCode::Char('k') => *scroll = scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *scroll = scroll.saturating_add(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(10),
                KeyCode::PageDown => *scroll = scroll.saturating_add(10),
                KeyCode::Home | KeyCode::Char('g') => *scroll = 0,
                _ => {}
            },
        }

        self.overlay = Some(overlay);
        Vec::new()
    }

    fn move_up(&mut self, amount: usize) -> Vec<Command> {
        match self.view {
            View::Workflows => match self.focus {
                Focus::Workflows => {
                    let next = self.selected_workflow.saturating_sub(amount);
                    self.select_workflow(next)
                }
                Focus::History => {
                    self.selected_event = self.selected_event.saturating_sub(amount);
                    Vec::new()
                }
            },
            View::TaskQueues => {
                self.selected_task_queue = self.selected_task_queue.saturating_sub(amount);
                Vec::new()
            }
            View::Workers => {
                let next = self.selected_worker.saturating_sub(amount);
                self.select_worker(next)
            }
            View::Deployments => {
                let next = self.selected_worker_deployment.saturating_sub(amount);
                self.select_worker_deployment(next)
            }
            View::Schedules => {
                let next = self.selected_schedule.saturating_sub(amount);
                self.select_schedule(next)
            }
            View::Batches => {
                let next = self.selected_batch_operation.saturating_sub(amount);
                self.select_batch_operation(next)
            }
        }
    }

    fn move_down(&mut self, amount: usize) -> Vec<Command> {
        match self.view {
            View::Workflows => match self.focus {
                Focus::Workflows => {
                    let next = (self.selected_workflow + amount)
                        .min(self.workflows.len().saturating_sub(1));
                    self.select_workflow(next)
                }
                Focus::History => {
                    let last = self
                        .details
                        .as_ref()
                        .map_or(0, |details| details.events.len().saturating_sub(1));
                    self.selected_event = (self.selected_event + amount).min(last);
                    Vec::new()
                }
            },
            View::TaskQueues => {
                self.selected_task_queue = (self.selected_task_queue + amount)
                    .min(self.task_queues.len().saturating_sub(1));
                Vec::new()
            }
            View::Workers => {
                let next =
                    (self.selected_worker + amount).min(self.workers.len().saturating_sub(1));
                self.select_worker(next)
            }
            View::Deployments => {
                let next = (self.selected_worker_deployment + amount)
                    .min(self.worker_deployments.len().saturating_sub(1));
                self.select_worker_deployment(next)
            }
            View::Schedules => {
                let next =
                    (self.selected_schedule + amount).min(self.schedules.len().saturating_sub(1));
                self.select_schedule(next)
            }
            View::Batches => {
                let next = (self.selected_batch_operation + amount)
                    .min(self.batch_operations.len().saturating_sub(1));
                self.select_batch_operation(next)
            }
        }
    }

    fn move_first(&mut self) -> Vec<Command> {
        match self.view {
            View::Workflows => match self.focus {
                Focus::Workflows => self.select_workflow(0),
                Focus::History => {
                    self.selected_event = 0;
                    Vec::new()
                }
            },
            View::TaskQueues => {
                self.selected_task_queue = 0;
                Vec::new()
            }
            View::Workers => self.select_worker(0),
            View::Deployments => self.select_worker_deployment(0),
            View::Schedules => self.select_schedule(0),
            View::Batches => self.select_batch_operation(0),
        }
    }

    fn move_last(&mut self) -> Vec<Command> {
        match self.view {
            View::Workflows => match self.focus {
                Focus::Workflows => self.select_workflow(self.workflows.len().saturating_sub(1)),
                Focus::History => {
                    self.selected_event = self
                        .details
                        .as_ref()
                        .map_or(0, |details| details.events.len().saturating_sub(1));
                    Vec::new()
                }
            },
            View::TaskQueues => {
                self.selected_task_queue = self.task_queues.len().saturating_sub(1);
                Vec::new()
            }
            View::Workers => self.select_worker(self.workers.len().saturating_sub(1)),
            View::Deployments => {
                self.select_worker_deployment(self.worker_deployments.len().saturating_sub(1))
            }
            View::Schedules => self.select_schedule(self.schedules.len().saturating_sub(1)),
            View::Batches => {
                self.select_batch_operation(self.batch_operations.len().saturating_sub(1))
            }
        }
    }

    fn select_workflow(&mut self, index: usize) -> Vec<Command> {
        if self.workflows.is_empty() || index == self.selected_workflow {
            return Vec::new();
        }
        self.selected_workflow = index;
        self.details = None;
        self.selected_event = 0;
        self.load_selected_details().into_iter().collect()
    }

    fn select_worker(&mut self, index: usize) -> Vec<Command> {
        if self.workers.is_empty() || index == self.selected_worker {
            return Vec::new();
        }
        self.selected_worker = index;
        self.worker_details = None;
        self.load_selected_worker_details().into_iter().collect()
    }

    fn select_worker_deployment(&mut self, index: usize) -> Vec<Command> {
        if self.worker_deployments.is_empty() || index == self.selected_worker_deployment {
            return Vec::new();
        }
        self.selected_worker_deployment = index;
        self.worker_deployment_details = None;
        self.load_selected_worker_deployment_details()
            .into_iter()
            .collect()
    }

    fn select_schedule(&mut self, index: usize) -> Vec<Command> {
        if self.schedules.is_empty() || index == self.selected_schedule {
            return Vec::new();
        }
        self.selected_schedule = index;
        self.schedule_details = None;
        self.load_selected_schedule_details().into_iter().collect()
    }

    fn select_batch_operation(&mut self, index: usize) -> Vec<Command> {
        if self.batch_operations.is_empty() || index == self.selected_batch_operation {
            return Vec::new();
        }
        self.selected_batch_operation = index;
        self.batch_operation_details = None;
        self.load_selected_batch_operation_details()
            .into_iter()
            .collect()
    }

    fn open_confirmation(&mut self, action: ConfirmAction) {
        if self.read_only {
            self.show_notice("Read-only mode blocks workflow mutations", NoticeKind::Info);
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A workflow operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(workflow) = self.selected_workflow() else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return;
        };
        let valid_status = match action {
            ConfirmAction::Pause => workflow.status == WorkflowStatus::Running,
            ConfirmAction::Unpause => workflow.status == WorkflowStatus::Paused,
            ConfirmAction::Cancel | ConfirmAction::Terminate => workflow.status.is_running(),
        };
        if !valid_status {
            self.show_notice(
                format!(
                    "{} cannot be applied to a {} workflow",
                    action.verb(),
                    workflow.status
                ),
                NoticeKind::Info,
            );
            return;
        }
        self.overlay = Some(Overlay::Confirm {
            action,
            key: workflow.key.clone(),
            workflow_id: workflow.key.workflow_id.clone(),
            input: TextInput::default(),
        });
    }

    fn open_signal(&mut self) {
        if self.read_only {
            self.show_notice("Read-only mode blocks workflow mutations", NoticeKind::Info);
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A workflow operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(workflow) = self.selected_workflow() else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return;
        };
        if !workflow.status.is_running() {
            self.show_notice(
                format!("{} workflows cannot receive signals", workflow.status),
                NoticeKind::Info,
            );
            return;
        }
        self.overlay = Some(Overlay::Signal(SignalForm::default()));
    }

    fn open_workflow_call(&mut self, kind: WorkflowCallKind) {
        if self.call_in_flight {
            self.show_notice(
                "A workflow handler call is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        if kind == WorkflowCallKind::Update {
            if let Some(message) = self.blocked_capability(Capability::WorkflowUpdate) {
                self.show_notice(message, NoticeKind::Info);
                return;
            }
            if self.read_only {
                self.show_notice("Read-only mode blocks Workflow Updates", NoticeKind::Info);
                return;
            }
            if self.operation_in_flight {
                self.show_notice(
                    "A workflow operation is already in progress",
                    NoticeKind::Info,
                );
                return;
            }
            let Some(workflow) = self.selected_workflow() else {
                self.show_notice("No workflow selected", NoticeKind::Info);
                return;
            };
            if !workflow.status.is_running() {
                self.show_notice(
                    format!("{} workflows cannot receive Updates", workflow.status),
                    NoticeKind::Info,
                );
                return;
            }
        } else if self.selected_workflow().is_none() {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return;
        }
        self.overlay = Some(Overlay::WorkflowCall {
            kind,
            form: WorkflowCallForm::default(),
        });
    }

    fn open_pause_toggle(&mut self) {
        if let Some(message) = self.blocked_capability(Capability::WorkflowPause) {
            self.show_notice(message, NoticeKind::Info);
            return;
        }
        let Some(status) = self.selected_workflow().map(|workflow| workflow.status) else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return;
        };
        self.open_confirmation(if status == WorkflowStatus::Paused {
            ConfirmAction::Unpause
        } else {
            ConfirmAction::Pause
        });
    }

    fn open_reset(&mut self) {
        if self.read_only {
            self.show_notice("Read-only mode blocks Workflow Reset", NoticeKind::Info);
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A workflow operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(workflow) = self.selected_workflow() else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return;
        };
        let event_id = self
            .details
            .as_ref()
            .and_then(|details| details.events.get(self.selected_event))
            .map_or_else(|| "1".to_string(), |event| event.event_id.to_string());
        self.overlay = Some(Overlay::Reset(ResetForm {
            key: workflow.key.clone(),
            workflow_id: workflow.key.workflow_id.clone(),
            event_id: TextInput::new(event_id),
            confirmation: TextInput::default(),
            active_field: ResetField::EventId,
        }));
    }

    fn open_deployment_current(&mut self) {
        if let Some(message) = self.blocked_capability(Capability::WorkerDeployments) {
            self.show_notice(message, NoticeKind::Info);
            return;
        }
        if self.read_only {
            self.show_notice(
                "Read-only mode blocks Worker Deployment changes",
                NoticeKind::Info,
            );
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A control-plane operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(details) = &self.worker_deployment_details else {
            self.show_notice("No Worker Deployment details loaded", NoticeKind::Info);
            return;
        };
        let build_id = details
            .summary
            .ramping_version
            .as_ref()
            .or(details.summary.latest_version.as_ref())
            .or(details.summary.current_version.as_ref())
            .map(|version| version.build_id.clone())
            .unwrap_or_default();
        self.overlay = Some(Overlay::DeploymentCurrent(DeploymentCurrentForm {
            deployment_name: details.summary.name.clone(),
            build_id: TextInput::new(build_id),
            confirmation: TextInput::default(),
            active_field: DeploymentCurrentField::BuildId,
        }));
    }

    fn open_deployment_ramp(&mut self) {
        if let Some(message) = self.blocked_capability(Capability::WorkerDeployments) {
            self.show_notice(message, NoticeKind::Info);
            return;
        }
        if self.read_only {
            self.show_notice(
                "Read-only mode blocks Worker Deployment changes",
                NoticeKind::Info,
            );
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A control-plane operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(details) = &self.worker_deployment_details else {
            self.show_notice("No Worker Deployment details loaded", NoticeKind::Info);
            return;
        };
        let build_id = details
            .summary
            .ramping_version
            .as_ref()
            .or(details.summary.latest_version.as_ref())
            .map(|version| version.build_id.clone())
            .unwrap_or_default();
        let percentage = if details.summary.ramping_version.is_some() {
            details.summary.ramping_percentage
        } else {
            10.0
        };
        let percentage = if percentage.fract() == 0.0 {
            format!("{percentage:.0}")
        } else {
            percentage.to_string()
        };
        self.overlay = Some(Overlay::DeploymentRamp(DeploymentRampForm {
            deployment_name: details.summary.name.clone(),
            build_id: TextInput::new(build_id),
            percentage: TextInput::new(percentage),
            confirmation: TextInput::default(),
            active_field: DeploymentRampField::BuildId,
        }));
    }

    fn open_batch_create(&mut self) {
        if let Some(message) = self.blocked_capability(Capability::BatchOperations) {
            self.show_notice(message, NoticeKind::Info);
            return;
        }
        if self.read_only {
            self.show_notice("Read-only mode blocks batch operations", NoticeKind::Info);
            return;
        }
        if self.operation_in_flight || self.batch_preview_in_flight {
            self.show_notice(
                "A control-plane operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        self.overlay = Some(Overlay::BatchCreate(BatchCreateForm {
            job_id: TextInput::new(format!(
                "temporal-tui-{}",
                Utc::now().format("%Y%m%d-%H%M%S")
            )),
            operation: TextInput::new("cancel"),
            visibility_query: TextInput::new(self.query.clone()),
            reason: TextInput::new("Requested from temporal-tui"),
            max_operations_per_second: TextInput::new("10"),
            signal_name: TextInput::default(),
            signal_input: TextInput::new("{}"),
            active_field: BatchCreateField::JobId,
        }));
    }

    fn open_batch_stop(&mut self) {
        if self.read_only {
            self.show_notice("Read-only mode blocks batch operations", NoticeKind::Info);
            return;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A control-plane operation is already in progress",
                NoticeKind::Info,
            );
            return;
        }
        let Some(operation) = self.batch_operations.get(self.selected_batch_operation) else {
            self.show_notice("No batch operation selected", NoticeKind::Info);
            return;
        };
        if operation.state != "RUNNING" {
            self.show_notice(
                "Only a running batch operation can be stopped",
                NoticeKind::Info,
            );
            return;
        }
        self.overlay = Some(Overlay::BatchStop {
            job_id: operation.job_id.clone(),
            input: TextInput::default(),
        });
    }

    fn open_schedule_create(&mut self) {
        if !self.schedule_mutation_available() {
            return;
        }
        self.overlay = Some(Overlay::ScheduleCreate(ScheduleCreateForm::default()));
    }

    fn open_schedule_edit(&mut self) {
        if !self.schedule_mutation_available() {
            return;
        }
        let Some(details) = &self.schedule_details else {
            self.show_notice("Schedule details are still loading", NoticeKind::Info);
            return;
        };
        self.overlay = Some(Overlay::ScheduleEdit(ScheduleEditForm {
            schedule_id: details.summary.schedule_id.clone(),
            expression: TextInput::default(),
            timezone: TextInput::new(details.timezone.clone()),
            notes: TextInput::new(details.summary.notes.clone()),
            active_field: ScheduleEditField::Expression,
        }));
    }

    fn toggle_schedule_pause(&mut self) -> Vec<Command> {
        if !self.schedule_mutation_available() {
            return Vec::new();
        }
        let Some(schedule) = self.schedules.get(self.selected_schedule) else {
            self.show_notice("No Schedule selected", NoticeKind::Info);
            return Vec::new();
        };
        let schedule_id = schedule.schedule_id.clone();
        let paused = schedule.paused;
        let request_id = self.next_request_id();
        self.current_operation_request = request_id;
        self.operation_in_flight = true;
        vec![if paused {
            Command::UnpauseSchedule {
                request_id,
                namespace: self.namespace.clone(),
                schedule_id,
                note: "Unpaused from temporal-tui".to_string(),
            }
        } else {
            Command::PauseSchedule {
                request_id,
                namespace: self.namespace.clone(),
                schedule_id,
                note: "Paused from temporal-tui".to_string(),
            }
        }]
    }

    fn open_schedule_confirmation(&mut self, action: ScheduleConfirmAction) {
        if !self.schedule_mutation_available() {
            return;
        }
        let Some(schedule) = self.schedules.get(self.selected_schedule) else {
            self.show_notice("No Schedule selected", NoticeKind::Info);
            return;
        };
        self.overlay = Some(Overlay::ScheduleConfirm {
            action,
            schedule_id: schedule.schedule_id.clone(),
            input: TextInput::default(),
        });
    }

    fn open_schedule_backfill(&mut self) {
        if !self.schedule_mutation_available() {
            return;
        }
        let Some(schedule_id) = self
            .schedules
            .get(self.selected_schedule)
            .map(|schedule| schedule.schedule_id.clone())
        else {
            self.show_notice("No Schedule selected", NoticeKind::Info);
            return;
        };
        let end = Utc::now();
        let start = end - chrono::Duration::hours(1);
        self.overlay = Some(Overlay::ScheduleBackfill(ScheduleBackfillForm {
            schedule_id,
            start_time: TextInput::new(start.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            end_time: TextInput::new(end.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)),
            overlap_policy: TextInput::new("skip"),
            confirmation: TextInput::default(),
            active_field: ScheduleBackfillField::Start,
        }));
    }

    fn schedule_mutation_available(&mut self) -> bool {
        if let Some(message) = self.blocked_capability(Capability::Schedules) {
            self.show_notice(message, NoticeKind::Info);
            return false;
        }
        if self.read_only {
            self.show_notice("Read-only mode blocks Schedule mutations", NoticeKind::Info);
            return false;
        }
        if self.operation_in_flight {
            self.show_notice(
                "A control-plane operation is already in progress",
                NoticeKind::Info,
            );
            return false;
        }
        true
    }

    fn blocked_capability(&self, capability: Capability) -> Option<String> {
        let summary = self.capabilities.as_ref()?.get(capability)?;
        matches!(
            summary.availability,
            CapabilityAvailability::Unavailable | CapabilityAvailability::Restricted
        )
        .then(|| {
            format!(
                "{} is {}: {}",
                capability.label(),
                summary.availability.label().to_ascii_lowercase(),
                summary.detail
            )
        })
    }

    fn apply_capability_degradation(&mut self) {
        if let Some(message) = self.blocked_capability(Capability::WorkerHeartbeats) {
            self.workers.clear();
            self.worker_details = None;
            self.workers_error = Some(message);
            self.loading_workers = false;
            self.loading_worker_details = false;
        }
        if let Some(message) = self.blocked_capability(Capability::WorkerDeployments) {
            self.worker_deployments.clear();
            self.worker_deployment_details = None;
            self.worker_deployments_error = Some(message);
            self.loading_worker_deployments = false;
            self.loading_worker_deployment_details = false;
        }
        if let Some(message) = self.blocked_capability(Capability::Schedules) {
            self.schedules.clear();
            self.schedule_details = None;
            self.schedules_error = Some(message);
            self.loading_schedules = false;
            self.loading_schedule_details = false;
        }
        if let Some(message) = self.blocked_capability(Capability::BatchOperations) {
            self.batch_operations.clear();
            self.batch_operation_details = None;
            self.batch_operations_error = Some(message);
            self.loading_batch_operations = false;
            self.loading_batch_operation_details = false;
        }
        if let Some(message) = self.blocked_capability(Capability::SearchAttributes) {
            self.search_attributes.clear();
            self.search_attributes_error = Some(message);
            self.loading_search_attributes = false;
        }
    }

    fn switch_view(&mut self, view: View) -> Vec<Command> {
        if self.view == view {
            return Vec::new();
        }
        self.view = view;
        self.overlay = None;
        match view {
            View::Workflows if self.workflows.is_empty() => self.refresh_workflows(false),
            View::TaskQueues => {
                let mut commands = self.refresh_task_queues();
                if self.workers.is_empty() && !self.loading_workers {
                    commands.extend(self.refresh_workers(true));
                }
                commands
            }
            View::Workers if self.workers.is_empty() => self.refresh_workers(true),
            View::Deployments if self.worker_deployments.is_empty() => {
                self.refresh_worker_deployments(true)
            }
            View::Schedules if self.schedules.is_empty() => self.refresh_schedules(true),
            View::Batches if self.batch_operations.is_empty() => {
                self.refresh_batch_operations(true)
            }
            View::Workflows
            | View::Workers
            | View::Deployments
            | View::Schedules
            | View::Batches => Vec::new(),
        }
    }

    fn refresh_current_view(&mut self, reset_pagination: bool) -> Vec<Command> {
        match self.view {
            View::Workflows => self.refresh_workflows(reset_pagination),
            View::TaskQueues => self.refresh_task_queues(),
            View::Workers => self.refresh_workers(reset_pagination),
            View::Deployments => self.refresh_worker_deployments(reset_pagination),
            View::Schedules => self.refresh_schedules(reset_pagination),
            View::Batches => self.refresh_batch_operations(reset_pagination),
        }
    }

    fn current_view_is_loading(&self) -> bool {
        match self.view {
            View::Workflows => self.loading_workflows,
            View::TaskQueues => self.loading_task_queues,
            View::Workers => self.loading_workers,
            View::Deployments => self.loading_worker_deployments,
            View::Schedules => self.loading_schedules,
            View::Batches => self.loading_batch_operations,
        }
    }

    fn known_task_queue_names(&self) -> Vec<String> {
        self.workflows
            .iter()
            .map(|workflow| workflow.task_queue.clone())
            .chain(self.workers.iter().map(|worker| worker.task_queue.clone()))
            .filter(|name| !name.is_empty())
            .chain(self.manual_task_queue_names.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn refresh_task_queues(&mut self) -> Vec<Command> {
        let names = self.known_task_queue_names();
        if names.is_empty() {
            self.show_notice(
                "No Task Queue names discovered; load Workflows or Workers first",
                NoticeKind::Info,
            );
            return Vec::new();
        }
        let request_id = self.next_request_id();
        self.current_task_queues_request = request_id;
        self.loading_task_queues = true;
        self.last_refresh_started = Instant::now();
        vec![Command::LoadTaskQueues {
            request_id,
            namespace: self.namespace.clone(),
            names,
        }]
    }

    fn refresh_workers(&mut self, reset_pagination: bool) -> Vec<Command> {
        if let Some(message) = self.blocked_capability(Capability::WorkerHeartbeats) {
            self.workers.clear();
            self.worker_details = None;
            self.workers_error = Some(message);
            self.loading_workers = false;
            self.loading_worker_details = false;
            return Vec::new();
        }
        if reset_pagination {
            self.reset_worker_pagination();
        }
        let request_id = self.next_request_id();
        self.current_workers_request = request_id;
        self.loading_workers = true;
        self.last_refresh_started = Instant::now();
        vec![Command::LoadWorkers {
            request_id,
            namespace: self.namespace.clone(),
            query: String::new(),
            page_size: self.page_size,
            next_page_token: self.worker_current_page_token.clone(),
        }]
    }

    fn refresh_worker_deployments(&mut self, reset_pagination: bool) -> Vec<Command> {
        if let Some(message) = self.blocked_capability(Capability::WorkerDeployments) {
            self.worker_deployments.clear();
            self.worker_deployment_details = None;
            self.worker_deployments_error = Some(message);
            self.loading_worker_deployments = false;
            self.loading_worker_deployment_details = false;
            return Vec::new();
        }
        if reset_pagination {
            self.reset_deployment_pagination();
        }
        let request_id = self.next_request_id();
        self.current_worker_deployments_request = request_id;
        self.loading_worker_deployments = true;
        self.last_refresh_started = Instant::now();
        vec![Command::LoadWorkerDeployments {
            request_id,
            namespace: self.namespace.clone(),
            page_size: self.page_size,
            next_page_token: self.deployment_current_page_token.clone(),
        }]
    }

    fn refresh_batch_operations(&mut self, reset_pagination: bool) -> Vec<Command> {
        if let Some(message) = self.blocked_capability(Capability::BatchOperations) {
            self.batch_operations.clear();
            self.batch_operation_details = None;
            self.batch_operations_error = Some(message);
            self.loading_batch_operations = false;
            self.loading_batch_operation_details = false;
            return Vec::new();
        }
        if reset_pagination {
            self.reset_batch_pagination();
        }
        let request_id = self.next_request_id();
        self.current_batch_operations_request = request_id;
        self.loading_batch_operations = true;
        self.last_refresh_started = Instant::now();
        vec![Command::LoadBatchOperations {
            request_id,
            namespace: self.namespace.clone(),
            page_size: self.page_size,
            next_page_token: self.batch_current_page_token.clone(),
        }]
    }

    fn next_page(&mut self) -> Vec<Command> {
        match self.view {
            View::Workflows => self.next_workflow_page(),
            View::Workers => self.next_worker_page(),
            View::Deployments => self.next_deployment_page(),
            View::Schedules => self.next_schedule_page(),
            View::Batches => self.next_batch_page(),
            View::TaskQueues => {
                self.show_notice("Task Queue diagnostics are not paginated", NoticeKind::Info);
                Vec::new()
            }
        }
    }

    fn previous_page(&mut self) -> Vec<Command> {
        match self.view {
            View::Workflows => self.previous_workflow_page(),
            View::Workers => self.previous_worker_page(),
            View::Deployments => self.previous_deployment_page(),
            View::Schedules => self.previous_schedule_page(),
            View::Batches => self.previous_batch_page(),
            View::TaskQueues => {
                self.show_notice("Task Queue diagnostics are not paginated", NoticeKind::Info);
                Vec::new()
            }
        }
    }

    fn next_worker_page(&mut self) -> Vec<Command> {
        if self.loading_workers {
            self.show_notice("Worker page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        if self.worker_next_page_token.is_empty() {
            self.show_notice("Already on the last Worker page", NoticeKind::Info);
            return Vec::new();
        }
        self.worker_previous_page_tokens
            .push(self.worker_current_page_token.clone());
        self.worker_current_page_token = std::mem::take(&mut self.worker_next_page_token);
        self.refresh_workers(false)
    }

    fn previous_worker_page(&mut self) -> Vec<Command> {
        if self.loading_workers {
            self.show_notice("Worker page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(previous) = self.worker_previous_page_tokens.pop() else {
            self.show_notice("Already on the first Worker page", NoticeKind::Info);
            return Vec::new();
        };
        self.worker_current_page_token = previous;
        self.refresh_workers(false)
    }

    fn next_deployment_page(&mut self) -> Vec<Command> {
        if self.loading_worker_deployments {
            self.show_notice("Deployment page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        if self.deployment_next_page_token.is_empty() {
            self.show_notice("Already on the last Deployment page", NoticeKind::Info);
            return Vec::new();
        }
        self.deployment_previous_page_tokens
            .push(self.deployment_current_page_token.clone());
        self.deployment_current_page_token = std::mem::take(&mut self.deployment_next_page_token);
        self.refresh_worker_deployments(false)
    }

    fn previous_deployment_page(&mut self) -> Vec<Command> {
        if self.loading_worker_deployments {
            self.show_notice("Deployment page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(previous) = self.deployment_previous_page_tokens.pop() else {
            self.show_notice("Already on the first Deployment page", NoticeKind::Info);
            return Vec::new();
        };
        self.deployment_current_page_token = previous;
        self.refresh_worker_deployments(false)
    }

    fn next_schedule_page(&mut self) -> Vec<Command> {
        if self.loading_schedules {
            self.show_notice("Schedule page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        if self.schedule_next_page_token.is_empty() {
            self.show_notice("Already on the last Schedule page", NoticeKind::Info);
            return Vec::new();
        }
        self.schedule_previous_page_tokens
            .push(self.schedule_current_page_token.clone());
        self.schedule_current_page_token = std::mem::take(&mut self.schedule_next_page_token);
        self.refresh_schedules(false)
    }

    fn previous_schedule_page(&mut self) -> Vec<Command> {
        if self.loading_schedules {
            self.show_notice("Schedule page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(previous) = self.schedule_previous_page_tokens.pop() else {
            self.show_notice("Already on the first Schedule page", NoticeKind::Info);
            return Vec::new();
        };
        self.schedule_current_page_token = previous;
        self.refresh_schedules(false)
    }

    fn next_batch_page(&mut self) -> Vec<Command> {
        if self.loading_batch_operations {
            self.show_notice("Batch operation page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        if self.batch_next_page_token.is_empty() {
            self.show_notice("Already on the last Batch operation page", NoticeKind::Info);
            return Vec::new();
        }
        self.batch_previous_page_tokens
            .push(self.batch_current_page_token.clone());
        self.batch_current_page_token = std::mem::take(&mut self.batch_next_page_token);
        self.refresh_batch_operations(false)
    }

    fn previous_batch_page(&mut self) -> Vec<Command> {
        if self.loading_batch_operations {
            self.show_notice("Batch operation page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(previous) = self.batch_previous_page_tokens.pop() else {
            self.show_notice(
                "Already on the first Batch operation page",
                NoticeKind::Info,
            );
            return Vec::new();
        };
        self.batch_current_page_token = previous;
        self.refresh_batch_operations(false)
    }

    fn reset_worker_pagination(&mut self) {
        self.worker_current_page_token.clear();
        self.worker_next_page_token.clear();
        self.worker_previous_page_tokens.clear();
        self.worker_page_number = 1;
        self.worker_has_previous_page = false;
        self.worker_has_next_page = false;
    }

    fn reset_deployment_pagination(&mut self) {
        self.deployment_current_page_token.clear();
        self.deployment_next_page_token.clear();
        self.deployment_previous_page_tokens.clear();
        self.deployment_page_number = 1;
        self.deployment_has_previous_page = false;
        self.deployment_has_next_page = false;
    }

    fn reset_schedule_pagination(&mut self) {
        self.schedule_current_page_token.clear();
        self.schedule_next_page_token.clear();
        self.schedule_previous_page_tokens.clear();
        self.schedule_page_number = 1;
        self.schedule_has_previous_page = false;
        self.schedule_has_next_page = false;
    }

    fn reset_batch_pagination(&mut self) {
        self.batch_current_page_token.clear();
        self.batch_next_page_token.clear();
        self.batch_previous_page_tokens.clear();
        self.batch_page_number = 1;
        self.batch_has_previous_page = false;
        self.batch_has_next_page = false;
    }

    fn refresh_schedules(&mut self, reset_pagination: bool) -> Vec<Command> {
        if let Some(message) = self.blocked_capability(Capability::Schedules) {
            self.schedules.clear();
            self.schedule_details = None;
            self.schedules_error = Some(message);
            self.loading_schedules = false;
            self.loading_schedule_details = false;
            return Vec::new();
        }
        if reset_pagination {
            self.reset_schedule_pagination();
        }
        let request_id = self.next_request_id();
        self.current_schedules_request = request_id;
        self.loading_schedules = true;
        self.last_refresh_started = Instant::now();
        vec![Command::LoadSchedules {
            request_id,
            namespace: self.namespace.clone(),
            query: self.schedule_query.clone(),
            page_size: self.page_size,
            next_page_token: self.schedule_current_page_token.clone(),
        }]
    }

    fn refresh_workflows(&mut self, reset_pagination: bool) -> Vec<Command> {
        if reset_pagination {
            self.current_page_token.clear();
            self.next_page_token.clear();
            self.previous_page_tokens.clear();
            self.page_number = 1;
            self.has_previous_page = false;
            self.has_next_page = false;
            self.workflow_count = None;
        }
        let request_id = self.next_request_id();
        self.current_workflow_request = request_id;
        self.loading_workflows = true;
        self.last_refresh_started = Instant::now();
        let count_request_id = self.next_request_id();
        self.current_count_request = count_request_id;
        vec![
            Command::LoadWorkflows {
                request_id,
                namespace: self.namespace.clone(),
                query: self.query.clone(),
                page_size: self.page_size,
                next_page_token: self.current_page_token.clone(),
            },
            Command::CountWorkflows {
                request_id: count_request_id,
                namespace: self.namespace.clone(),
                query: self.query.clone(),
            },
        ]
    }

    fn next_workflow_page(&mut self) -> Vec<Command> {
        if self.loading_workflows {
            self.show_notice("Workflow page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        if self.next_page_token.is_empty() {
            self.show_notice("Already on the last workflow page", NoticeKind::Info);
            return Vec::new();
        }
        self.previous_page_tokens
            .push(self.current_page_token.clone());
        self.current_page_token = std::mem::take(&mut self.next_page_token);
        self.refresh_workflows(false)
    }

    fn previous_workflow_page(&mut self) -> Vec<Command> {
        if self.loading_workflows {
            self.show_notice("Workflow page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(previous) = self.previous_page_tokens.pop() else {
            self.show_notice("Already on the first workflow page", NoticeKind::Info);
            return Vec::new();
        };
        self.current_page_token = previous;
        self.refresh_workflows(false)
    }

    fn load_older_history(&mut self) -> Vec<Command> {
        if self.loading_history_page {
            self.show_notice("History page is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(details) = &self.details else {
            self.show_notice("No workflow details loaded", NoticeKind::Info);
            return Vec::new();
        };
        if details.history_next_page_token.is_empty() {
            self.show_notice("Complete workflow history is loaded", NoticeKind::Info);
            return Vec::new();
        }
        let key = details.summary.key.clone();
        let next_page_token = details.history_next_page_token.clone();
        let request_id = self.next_request_id();
        self.current_history_request = request_id;
        self.loading_history_page = true;
        vec![Command::LoadHistoryPage {
            request_id,
            namespace: self.namespace.clone(),
            key,
            next_page_token,
        }]
    }

    fn load_workflow_chain(&mut self) -> Vec<Command> {
        if self.loading_chain {
            self.show_notice("Workflow chain is still loading", NoticeKind::Info);
            return Vec::new();
        }
        let Some(workflow_id) = self
            .selected_workflow()
            .map(|workflow| workflow.key.workflow_id.clone())
        else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return Vec::new();
        };
        let request_id = self.next_request_id();
        self.current_chain_request = request_id;
        self.loading_chain = true;
        vec![Command::LoadWorkflowChain {
            request_id,
            namespace: self.namespace.clone(),
            workflow_id,
        }]
    }

    fn copy_workflow_identity(&mut self) -> Vec<Command> {
        let Some(key) = self
            .selected_workflow()
            .map(|workflow| workflow.key.clone())
        else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return Vec::new();
        };
        let request_id = self.next_request_id();
        self.current_utility_request = request_id;
        vec![Command::Copy {
            request_id,
            text: format!("{}\n{}", key.workflow_id, key.run_id),
        }]
    }

    fn export_workflow(&mut self) -> Vec<Command> {
        let Some(details) = self.details.clone() else {
            self.show_notice("No workflow details loaded", NoticeKind::Info);
            return Vec::new();
        };
        let request_id = self.next_request_id();
        self.current_utility_request = request_id;
        vec![Command::Export {
            request_id,
            namespace: self.namespace.clone(),
            cluster: self.cluster.clone(),
            details: Box::new(details),
        }]
    }

    fn open_in_web_ui(&mut self) -> Vec<Command> {
        let Some(base_url) = self.web_ui_url.as_deref() else {
            self.show_notice("No Temporal Web UI URL configured", NoticeKind::Info);
            return Vec::new();
        };
        let Some(key) = self
            .selected_workflow()
            .map(|workflow| workflow.key.clone())
        else {
            self.show_notice("No workflow selected", NoticeKind::Info);
            return Vec::new();
        };
        let url = match workflow_web_url(base_url, &self.namespace, &key) {
            Ok(url) => url,
            Err(error) => {
                self.show_notice(error, NoticeKind::Error);
                return Vec::new();
            }
        };
        let request_id = self.next_request_id();
        self.current_utility_request = request_id;
        vec![Command::OpenWeb { request_id, url }]
    }

    fn load_selected_details(&mut self) -> Option<Command> {
        let key = self.selected_workflow()?.key.clone();
        let request_id = self.next_request_id();
        self.current_detail_request = request_id;
        self.loading_details = true;
        Some(Command::LoadDetails {
            request_id,
            namespace: self.namespace.clone(),
            key,
        })
    }

    fn load_selected_worker_details(&mut self) -> Option<Command> {
        let instance_key = self.workers.get(self.selected_worker)?.instance_key.clone();
        let request_id = self.next_request_id();
        self.current_worker_details_request = request_id;
        self.loading_worker_details = true;
        Some(Command::LoadWorkerDetails {
            request_id,
            namespace: self.namespace.clone(),
            instance_key,
        })
    }

    fn load_selected_worker_deployment_details(&mut self) -> Option<Command> {
        let name = self
            .worker_deployments
            .get(self.selected_worker_deployment)?
            .name
            .clone();
        let request_id = self.next_request_id();
        self.current_worker_deployment_details_request = request_id;
        self.loading_worker_deployment_details = true;
        Some(Command::LoadWorkerDeploymentDetails {
            request_id,
            namespace: self.namespace.clone(),
            name,
        })
    }

    fn load_selected_schedule_details(&mut self) -> Option<Command> {
        let schedule_id = self
            .schedules
            .get(self.selected_schedule)?
            .schedule_id
            .clone();
        let request_id = self.next_request_id();
        self.current_schedule_details_request = request_id;
        self.loading_schedule_details = true;
        Some(Command::LoadScheduleDetails {
            request_id,
            namespace: self.namespace.clone(),
            schedule_id,
        })
    }

    fn load_selected_batch_operation_details(&mut self) -> Option<Command> {
        let job_id = self
            .batch_operations
            .get(self.selected_batch_operation)?
            .job_id
            .clone();
        let request_id = self.next_request_id();
        self.current_batch_operation_details_request = request_id;
        self.loading_batch_operation_details = true;
        Some(Command::LoadBatchOperationDetails {
            request_id,
            namespace: self.namespace.clone(),
            job_id,
        })
    }

    fn load_cluster(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_cluster_request = request_id;
        Command::LoadCluster { request_id }
    }

    fn load_capabilities(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_capabilities_request = request_id;
        self.loading_capabilities = true;
        Command::LoadCapabilities {
            request_id,
            namespace: self.namespace.clone(),
        }
    }

    fn load_namespaces(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_namespace_request = request_id;
        Command::LoadNamespaces { request_id }
    }

    fn load_search_attributes(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_search_attributes_request = request_id;
        self.loading_search_attributes = true;
        Command::LoadSearchAttributes {
            request_id,
            namespace: self.namespace.clone(),
        }
    }

    fn invalidate_pending_requests(&mut self) {
        let barrier = self.next_request_id();
        self.current_cluster_request = barrier;
        self.current_profile_request = barrier;
        self.current_capabilities_request = barrier;
        self.current_workflow_request = barrier;
        self.current_count_request = barrier;
        self.current_detail_request = barrier;
        self.current_history_request = barrier;
        self.current_chain_request = barrier;
        self.current_task_queues_request = barrier;
        self.current_workers_request = barrier;
        self.current_worker_details_request = barrier;
        self.current_worker_deployments_request = barrier;
        self.current_worker_deployment_details_request = barrier;
        self.current_schedules_request = barrier;
        self.current_schedule_details_request = barrier;
        self.current_search_attributes_request = barrier;
        self.current_batch_operations_request = barrier;
        self.current_batch_operation_details_request = barrier;
        self.current_batch_preview_request = barrier;
        self.current_call_request = barrier;
        self.current_namespace_request = barrier;
        self.current_operation_request = barrier;
        self.current_utility_request = barrier;
    }

    fn clear_connected_state(&mut self) {
        self.cluster = None;
        self.capabilities = None;
        self.capabilities_error = None;
        self.loading_capabilities = false;
        self.namespaces.clear();
        self.workflows.clear();
        self.workflow_count = None;
        self.details = None;
        self.workflow_chain.clear();
        self.task_queues.clear();
        self.task_queues_error = None;
        self.workers.clear();
        self.worker_details = None;
        self.workers_error = None;
        self.worker_deployments.clear();
        self.worker_deployment_details = None;
        self.worker_deployments_error = None;
        self.schedules.clear();
        self.schedule_details = None;
        self.schedules_error = None;
        self.batch_operations.clear();
        self.batch_operation_details = None;
        self.batch_operations_error = None;
        self.search_attributes.clear();
        self.search_attributes_error = None;
        self.selected_workflow = 0;
        self.selected_event = 0;
        self.selected_task_queue = 0;
        self.selected_worker = 0;
        self.selected_worker_deployment = 0;
        self.selected_schedule = 0;
        self.selected_batch_operation = 0;
        self.focus = Focus::Workflows;
        self.overlay = None;
        self.loading_workflows = false;
        self.loading_details = false;
        self.loading_history_page = false;
        self.loading_chain = false;
        self.loading_task_queues = false;
        self.loading_workers = false;
        self.loading_worker_details = false;
        self.loading_worker_deployments = false;
        self.loading_worker_deployment_details = false;
        self.loading_schedules = false;
        self.loading_schedule_details = false;
        self.loading_batch_operations = false;
        self.loading_batch_operation_details = false;
        self.loading_search_attributes = false;
        self.batch_preview_in_flight = false;
        self.call_in_flight = false;
        self.operation_in_flight = false;
        self.current_page_token.clear();
        self.next_page_token.clear();
        self.previous_page_tokens.clear();
        self.page_number = 1;
        self.has_previous_page = false;
        self.has_next_page = false;
        self.reset_worker_pagination();
        self.reset_deployment_pagination();
        self.reset_schedule_pagination();
        self.reset_batch_pagination();
        self.manual_task_queue_names.clear();
        self.last_refresh_started = Instant::now();
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_request_id = self.next_request_id.wrapping_add(1);
        self.next_request_id
    }

    fn show_notice(&mut self, text: impl Into<String>, kind: NoticeKind) {
        self.notice = Some(Notice {
            text: text.into(),
            kind,
            expires_at: Instant::now() + Duration::from_secs(6),
        });
    }
}

fn next_field<Field: Copy + PartialEq>(current: Field, fields: &[Field]) -> Field {
    let index = fields
        .iter()
        .position(|field| *field == current)
        .unwrap_or_default();
    fields[(index + 1) % fields.len()]
}

fn previous_field<Field: Copy + PartialEq>(current: Field, fields: &[Field]) -> Field {
    let index = fields
        .iter()
        .position(|field| *field == current)
        .unwrap_or_default();
    fields[(index + fields.len() - 1) % fields.len()]
}

fn parse_json_arguments(input: &str) -> Result<Vec<Value>, String> {
    match serde_json::from_str::<Value>(input) {
        Ok(Value::Array(arguments)) => Ok(arguments),
        Ok(_) => Err(
            "expected a JSON array; use [] for no arguments or [value] for one argument"
                .to_string(),
        ),
        Err(error) => Err(format!("invalid JSON: {error}")),
    }
}

fn canonical_search_attribute_type(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_ascii_lowercase().replace(['_', '-'], "");
    match normalized.as_str() {
        "text" => Some("Text"),
        "keyword" => Some("Keyword"),
        "int" | "integer" => Some("Int"),
        "double" | "float" => Some("Double"),
        "bool" | "boolean" => Some("Bool"),
        "datetime" | "timestamp" => Some("Datetime"),
        "keywordlist" => Some("KeywordList"),
        _ => None,
    }
}

fn batch_request_from_form(form: &BatchCreateForm) -> Result<BatchOperationRequest, String> {
    let job_id = form.job_id.value.trim().to_string();
    if job_id.is_empty() {
        return Err("Batch Job ID must not be empty".to_string());
    }
    let visibility_query = form.visibility_query.value.trim().to_string();
    if visibility_query.is_empty() {
        return Err("A non-empty Visibility query is required".to_string());
    }
    let reason = form.reason.value.trim().to_string();
    if reason.is_empty() {
        return Err("Batch reason must not be empty".to_string());
    }
    let kind = match form.operation.value.trim().to_ascii_lowercase().as_str() {
        "cancel" => BatchOperationKind::Cancel,
        "terminate" => BatchOperationKind::Terminate,
        "signal" => BatchOperationKind::Signal,
        "delete" => BatchOperationKind::Delete,
        _ => return Err("Operation must be cancel, terminate, signal, or delete".to_string()),
    };
    let max_operations_per_second = form
        .max_operations_per_second
        .value
        .trim()
        .parse::<f32>()
        .map_err(|_| "Maximum operations per second must be a number".to_string())?;
    if !max_operations_per_second.is_finite() || max_operations_per_second < 0.0 {
        return Err("Maximum operations per second must be zero or greater".to_string());
    }
    let signal_name = form.signal_name.value.trim().to_string();
    if kind == BatchOperationKind::Signal && signal_name.is_empty() {
        return Err("A signal batch requires a signal name".to_string());
    }
    let signal_input = if kind == BatchOperationKind::Signal {
        serde_json::from_str::<Value>(&form.signal_input.value)
            .map_err(|error| format!("Signal input is not valid JSON: {error}"))?
    } else {
        Value::Null
    };
    Ok(BatchOperationRequest {
        job_id,
        visibility_query,
        reason,
        max_operations_per_second,
        kind,
        signal_name,
        signal_input,
    })
}

fn edit_batch_create_field(form: &mut BatchCreateForm, key: KeyEvent) {
    let input = match form.active_field {
        BatchCreateField::JobId => &mut form.job_id,
        BatchCreateField::Operation => &mut form.operation,
        BatchCreateField::VisibilityQuery => &mut form.visibility_query,
        BatchCreateField::Reason => &mut form.reason,
        BatchCreateField::MaxOperationsPerSecond => &mut form.max_operations_per_second,
        BatchCreateField::SignalName => &mut form.signal_name,
        BatchCreateField::SignalInput => &mut form.signal_input,
    };
    edit_text(input, key);
}

fn edit_search_attribute_add_field(form: &mut SearchAttributeAddForm, key: KeyEvent) {
    let input = match form.active_field {
        SearchAttributeAddField::Name => &mut form.name,
        SearchAttributeAddField::ValueType => &mut form.value_type,
        SearchAttributeAddField::Confirmation => &mut form.confirmation,
    };
    edit_text(input, key);
}

fn edit_deployment_ramp_field(form: &mut DeploymentRampForm, key: KeyEvent) {
    let input = match form.active_field {
        DeploymentRampField::BuildId => &mut form.build_id,
        DeploymentRampField::Percentage => &mut form.percentage,
        DeploymentRampField::Confirmation => &mut form.confirmation,
    };
    edit_text(input, key);
}

fn edit_schedule_create_field(form: &mut ScheduleCreateForm, key: KeyEvent) {
    let input = match form.active_field {
        ScheduleCreateField::ScheduleId => &mut form.schedule_id,
        ScheduleCreateField::WorkflowId => &mut form.workflow_id,
        ScheduleCreateField::WorkflowType => &mut form.workflow_type,
        ScheduleCreateField::TaskQueue => &mut form.task_queue,
        ScheduleCreateField::Expression => &mut form.expression,
        ScheduleCreateField::Timezone => &mut form.timezone,
        ScheduleCreateField::Input => &mut form.input,
        ScheduleCreateField::Notes => &mut form.notes,
    };
    edit_text(input, key);
}

fn edit_schedule_edit_field(form: &mut ScheduleEditForm, key: KeyEvent) {
    let input = match form.active_field {
        ScheduleEditField::Expression => &mut form.expression,
        ScheduleEditField::Timezone => &mut form.timezone,
        ScheduleEditField::Notes => &mut form.notes,
    };
    edit_text(input, key);
}

fn edit_schedule_backfill_field(form: &mut ScheduleBackfillForm, key: KeyEvent) {
    let input = match form.active_field {
        ScheduleBackfillField::Start => &mut form.start_time,
        ScheduleBackfillField::End => &mut form.end_time,
        ScheduleBackfillField::Overlap => &mut form.overlap_policy,
        ScheduleBackfillField::Confirmation => &mut form.confirmation,
    };
    edit_text(input, key);
}

fn edit_text(input: &mut TextInput, key: KeyEvent) {
    match key.code {
        KeyCode::Char(character)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            input.insert(character);
        }
        KeyCode::Backspace => input.backspace(),
        KeyCode::Delete => input.delete(),
        KeyCode::Left => input.move_left(),
        KeyCode::Right => input.move_right(),
        KeyCode::Home => input.cursor = 0,
        KeyCode::End => input.cursor = input.value.chars().count(),
        _ => {}
    }
}

fn workflow_web_url(base_url: &str, namespace: &str, key: &WorkflowKey) -> Result<String, String> {
    let mut url =
        Url::parse(base_url).map_err(|error| format!("Temporal Web UI URL is invalid: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Temporal Web UI URL must use http or https".to_string());
    }
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|()| "Temporal Web UI URL cannot be used as a base URL".to_string())?;
        segments.pop_if_empty();
        segments.extend([
            "namespaces",
            namespace,
            "workflows",
            &key.workflow_id,
            &key.run_id,
            "history",
        ]);
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use crossterm::event::KeyEvent;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::model::{WorkflowKey, WorkflowStatus};

    fn app() -> App {
        App::new(AppConfig {
            address: "localhost:7233".to_string(),
            profile_name: None,
            namespace: "default".to_string(),
            query: String::new(),
            page_size: 50,
            refresh_interval: Duration::from_secs(5),
            auto_refresh: true,
            color: true,
            read_only: false,
            force_read_only: false,
            codec_enabled: false,
            web_ui_url: Some("http://localhost:8233".to_string()),
            saved_queries: Vec::new(),
            profiles: Vec::new(),
        })
    }

    fn workflow(id: &str, status: WorkflowStatus) -> WorkflowSummary {
        WorkflowSummary {
            key: WorkflowKey {
                workflow_id: id.to_string(),
                run_id: format!("{id}-run"),
            },
            workflow_type: "OrderWorkflow".to_string(),
            task_queue: "orders".to_string(),
            status,
            start_time: Some(Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap()),
            close_time: None,
            history_length: 10,
            history_size_bytes: 1000,
        }
    }

    fn schedule(id: &str, paused: bool) -> ScheduleSummary {
        ScheduleSummary {
            schedule_id: id.to_string(),
            paused,
            notes: String::new(),
            workflow_type: "OrderWorkflow".to_string(),
            next_action_time: None,
            recent_action_time: None,
            state_size_bytes: 1_024,
        }
    }

    fn profile(name: &str, address: &str, namespace: &str) -> ProfileSummary {
        ProfileSummary {
            name: name.to_string(),
            address: address.to_string(),
            namespace: namespace.to_string(),
            read_only: false,
            auth_enabled: false,
            codec_enabled: false,
            is_default: name == "dev",
        }
    }

    fn capability(
        capability: Capability,
        availability: CapabilityAvailability,
    ) -> crate::model::CapabilitySummary {
        crate::model::CapabilitySummary {
            capability,
            availability,
            detail: "test negotiation evidence".to_string(),
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn load_request_id(command: &Command) -> u64 {
        match command {
            Command::LoadWorkflows { request_id, .. } => *request_id,
            _ => panic!("expected workflow load"),
        }
    }

    #[test]
    fn bootstrap_requests_all_dashboard_data() {
        let commands = app().bootstrap();
        assert!(matches!(commands[0], Command::LoadCluster { .. }));
        assert!(matches!(commands[1], Command::LoadCapabilities { .. }));
        assert!(matches!(commands[2], Command::LoadNamespaces { .. }));
        assert!(matches!(commands[3], Command::LoadWorkflows { .. }));
        assert!(matches!(commands[4], Command::CountWorkflows { .. }));
    }

    #[test]
    fn stale_workflow_results_are_ignored() {
        let mut app = app();
        let first = app.refresh_workflows(false);
        let second = app.refresh_workflows(false);
        let commands = app.handle_message(Message::WorkflowsLoaded {
            request_id: load_request_id(&first[0]),
            result: Ok(WorkflowPage {
                workflows: vec![workflow("stale", WorkflowStatus::Running)],
                next_page_token: Vec::new(),
            }),
        });
        assert!(commands.is_empty());
        assert!(app.workflows.is_empty());

        app.handle_message(Message::WorkflowsLoaded {
            request_id: load_request_id(&second[0]),
            result: Ok(WorkflowPage {
                workflows: vec![workflow("fresh", WorkflowStatus::Running)],
                next_page_token: Vec::new(),
            }),
        });
        assert_eq!(app.workflows[0].key.workflow_id, "fresh");
    }

    #[test]
    fn moving_selection_loads_new_details() {
        let mut app = app();
        app.workflows = vec![
            workflow("one", WorkflowStatus::Running),
            workflow("two", WorkflowStatus::Running),
        ];
        let commands = app.handle_key(key(KeyCode::Down));
        assert_eq!(app.selected_workflow, 1);
        assert!(matches!(
            &commands[0],
            Command::LoadDetails { key, .. } if key.workflow_id == "two"
        ));
    }

    #[test]
    fn query_editor_handles_unicode_and_refreshes() {
        let mut app = app();
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(key(KeyCode::Char('т')));
        app.handle_key(key(KeyCode::Char('е')));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.query, "те");
        assert!(matches!(commands[0], Command::LoadWorkflows { .. }));
        assert!(app.overlay.is_none());
    }

    #[test]
    fn view_switching_loads_each_observability_surface() {
        let mut task_queues = app();
        task_queues.workflows = vec![workflow("one", WorkflowStatus::Running)];
        let commands = task_queues.handle_key(key(KeyCode::Char('2')));
        assert_eq!(task_queues.view, View::TaskQueues);
        assert!(matches!(
            &commands[0],
            Command::LoadTaskQueues { names, .. } if names == &["orders".to_string()]
        ));
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::LoadWorkers { .. }))
        );

        let mut workers = app();
        let commands = workers.handle_key(key(KeyCode::Char('3')));
        assert_eq!(workers.view, View::Workers);
        assert!(matches!(commands[0], Command::LoadWorkers { .. }));

        let mut deployments = app();
        let commands = deployments.handle_key(key(KeyCode::Char('4')));
        assert_eq!(deployments.view, View::Deployments);
        assert!(matches!(commands[0], Command::LoadWorkerDeployments { .. }));

        let mut schedules = app();
        let commands = schedules.handle_key(key(KeyCode::Char('5')));
        assert_eq!(schedules.view, View::Schedules);
        assert!(matches!(commands[0], Command::LoadSchedules { .. }));

        let mut batches = app();
        let commands = batches.handle_key(key(KeyCode::Char('6')));
        assert_eq!(batches.view, View::Batches);
        assert!(matches!(commands[0], Command::LoadBatchOperations { .. }));
    }

    #[test]
    fn manual_task_queue_name_is_trimmed_and_loaded() {
        let mut app = app();
        app.handle_key(key(KeyCode::Char('2')));
        app.handle_key(key(KeyCode::Char('/')));
        for character in "  payments  ".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(app.overlay.is_none());
        assert!(matches!(
            &commands[0],
            Command::LoadTaskQueues { names, .. } if names == &["payments".to_string()]
        ));
    }

    #[test]
    fn workflow_mutations_are_not_bound_outside_workflow_view() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('3')));
        assert!(app.handle_key(key(KeyCode::Char('c'))).is_empty());
        assert!(app.handle_key(key(KeyCode::Char('x'))).is_empty());
        assert!(app.handle_key(key(KeyCode::Char('s'))).is_empty());
        assert!(app.overlay.is_none());
    }

    #[test]
    fn cancel_requires_confirmation() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        assert!(app.handle_key(key(KeyCode::Char('c'))).is_empty());
        assert!(matches!(
            app.overlay,
            Some(Overlay::Confirm {
                action: ConfirmAction::Cancel,
                ..
            })
        ));
        for character in "order-42".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::Cancel { key, .. } if key.workflow_id == "order-42"
        ));
    }

    #[test]
    fn destructive_confirmation_rejects_mismatched_workflow_id() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('x')));
        for character in "order-41".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(commands.is_empty());
        assert!(matches!(app.overlay, Some(Overlay::Confirm { .. })));
        assert_eq!(app.notice.as_ref().unwrap().kind, NoticeKind::Error);
    }

    #[test]
    fn closed_workflow_actions_are_blocked() {
        let mut app = app();
        app.workflows = vec![workflow("done", WorkflowStatus::Completed)];
        assert!(app.handle_key(key(KeyCode::Char('x'))).is_empty());
        assert!(app.overlay.is_none());
        assert_eq!(app.notice.as_ref().unwrap().kind, NoticeKind::Info);
    }

    #[test]
    fn concurrent_workflow_operations_are_blocked() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('c')));
        for character in "order-42".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(commands[0], Command::Cancel { .. }));
        assert!(app.operation_in_flight);

        assert!(app.handle_key(key(KeyCode::Char('s'))).is_empty());
        assert!(app.overlay.is_none());
        assert_eq!(app.notice.as_ref().unwrap().kind, NoticeKind::Info);
    }

    #[test]
    fn signal_form_validates_json_before_dispatch() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('s')));
        app.handle_key(key(KeyCode::Char('p')));
        app.handle_key(key(KeyCode::Enter));
        for _ in 0..2 {
            app.handle_key(key(KeyCode::Backspace));
        }
        app.handle_key(key(KeyCode::Char('{')));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(commands.is_empty());
        assert!(matches!(app.overlay, Some(Overlay::Signal(_))));
        assert_eq!(app.notice.as_ref().unwrap().kind, NoticeKind::Error);
    }

    #[test]
    fn namespace_picker_switches_and_refreshes() {
        let mut app = app();
        app.namespaces = vec![
            NamespaceSummary {
                name: "default".to_string(),
                id: "1".to_string(),
                description: String::new(),
                state: "REGISTERED".to_string(),
                retention: "3d".to_string(),
                active_cluster: "active".to_string(),
                is_global: false,
            },
            NamespaceSummary {
                name: "payments".to_string(),
                id: "2".to_string(),
                description: String::new(),
                state: "REGISTERED".to_string(),
                retention: "7d".to_string(),
                active_cluster: "active".to_string(),
                is_global: false,
            },
        ];
        app.handle_key(key(KeyCode::Char('n')));
        app.handle_key(key(KeyCode::Down));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.namespace, "payments");
        assert!(matches!(commands[0], Command::LoadCapabilities { .. }));
        assert!(matches!(commands[1], Command::LoadWorkflows { .. }));
    }

    #[test]
    fn profile_switch_is_atomic_invalidates_old_results_and_refreshes_current_view() {
        let mut app = app();
        app.profile_name = Some("dev".to_string());
        app.profiles = vec![
            profile("dev", "dev.example:7233", "development"),
            ProfileSummary {
                read_only: true,
                codec_enabled: true,
                ..profile("prod", "prod.example:7233", "production")
            },
        ];
        app.force_read_only = true;
        app.view = View::Batches;
        app.workflows = vec![workflow("old-cluster", WorkflowStatus::Running)];
        app.batch_operations = vec![BatchOperationSummary {
            job_id: "old-job".to_string(),
            state: "RUNNING".to_string(),
            start_time: None,
            close_time: None,
        }];
        let bootstrap = app.bootstrap();
        let Command::LoadCluster {
            request_id: stale_cluster_request,
        } = &bootstrap[0]
        else {
            panic!("expected cluster load");
        };
        let stale_cluster_request = *stale_cluster_request;

        app.handle_key(key(KeyCode::Char('P')));
        assert!(matches!(app.overlay, Some(Overlay::ProfilePicker { .. })));
        app.handle_key(key(KeyCode::Down));
        let switch = app.handle_key(key(KeyCode::Enter));
        let Command::SwitchProfile {
            request_id,
            profile_name,
        } = &switch[0]
        else {
            panic!("expected profile switch");
        };
        assert_eq!(profile_name, "prod");
        assert!(app.switching_profile);
        assert_eq!(app.pending_profile_name.as_deref(), Some("prod"));
        assert!(app.handle_key(key(KeyCode::Char('N'))).is_empty());

        let refresh = app.handle_message(Message::ProfileSwitchFinished {
            request_id: *request_id,
            result: Ok(ProfileConnectionInfo {
                name: "prod".to_string(),
                address: "prod.example:7233".to_string(),
                namespace: "production".to_string(),
                read_only: false,
                codec_enabled: true,
                web_ui_url: Some("https://temporal.example".to_string()),
            }),
        });
        assert!(!app.switching_profile);
        assert_eq!(app.profile_name.as_deref(), Some("prod"));
        assert_eq!(app.address, "prod.example:7233");
        assert_eq!(app.namespace, "production");
        assert!(
            app.read_only,
            "global read-only must survive profile switches"
        );
        assert!(app.codec_enabled);
        assert!(app.workflows.is_empty());
        assert!(app.batch_operations.is_empty());
        assert!(matches!(refresh[0], Command::LoadCluster { .. }));
        assert!(matches!(refresh[1], Command::LoadCapabilities { .. }));
        assert!(matches!(refresh[2], Command::LoadNamespaces { .. }));
        assert!(matches!(refresh[3], Command::LoadBatchOperations { .. }));

        app.handle_message(Message::ClusterLoaded {
            request_id: stale_cluster_request,
            result: Ok(ClusterInfo {
                cluster_name: "stale".to_string(),
                ..ClusterInfo::default()
            }),
        });
        assert!(
            app.cluster.is_none(),
            "old-cluster result must be discarded"
        );
    }

    #[test]
    fn failed_profile_switch_keeps_current_connection_state() {
        let mut app = app();
        app.profile_name = Some("dev".to_string());
        app.address = "dev.example:7233".to_string();
        app.namespace = "development".to_string();
        app.profiles = vec![
            profile("dev", "dev.example:7233", "development"),
            profile("broken", "broken.example:7233", "production"),
        ];
        app.workflows = vec![workflow("dev-workflow", WorkflowStatus::Running)];

        app.handle_key(key(KeyCode::Char('P')));
        app.handle_key(key(KeyCode::Down));
        let switch = app.handle_key(key(KeyCode::Enter));
        let Command::SwitchProfile { request_id, .. } = &switch[0] else {
            panic!("expected profile switch");
        };
        let commands = app.handle_message(Message::ProfileSwitchFinished {
            request_id: *request_id,
            result: Err("connection refused".to_string()),
        });
        assert!(commands.is_empty());
        assert!(!app.switching_profile);
        assert_eq!(app.profile_name.as_deref(), Some("dev"));
        assert_eq!(app.address, "dev.example:7233");
        assert_eq!(app.namespace, "development");
        assert_eq!(app.workflows[0].key.workflow_id, "dev-workflow");
        assert_eq!(app.notice.as_ref().unwrap().kind, NoticeKind::Error);
    }

    #[test]
    fn capability_negotiation_degrades_only_unsupported_surfaces() {
        let mut app = app();
        app.workflows = vec![workflow("still-visible", WorkflowStatus::Running)];
        let load = app.handle_key(key(KeyCode::Char('K')));
        let Command::LoadCapabilities { request_id, .. } = &load[0] else {
            panic!("expected capability negotiation");
        };
        app.handle_message(Message::CapabilitiesLoaded {
            request_id: *request_id,
            result: Ok(ServerCapabilities {
                server_version: "1.31.2".to_string(),
                namespace: "default".to_string(),
                features: vec![
                    capability(
                        Capability::WorkerHeartbeats,
                        CapabilityAvailability::Unavailable,
                    ),
                    capability(
                        Capability::WorkflowUpdate,
                        CapabilityAvailability::Restricted,
                    ),
                    capability(Capability::Schedules, CapabilityAvailability::Unknown),
                ],
            }),
        });
        app.handle_key(key(KeyCode::Esc));

        let workers = app.handle_key(key(KeyCode::Char('3')));
        assert!(workers.is_empty());
        assert_eq!(app.view, View::Workers);
        assert!(
            app.workers_error
                .as_deref()
                .is_some_and(|error| error.contains("unavailable"))
        );

        app.handle_key(key(KeyCode::Char('1')));
        assert_eq!(app.workflows[0].key.workflow_id, "still-visible");
        assert!(app.handle_key(key(KeyCode::Char('U'))).is_empty());
        assert!(
            app.notice
                .as_ref()
                .is_some_and(|notice| notice.text.contains("restricted"))
        );

        let schedules = app.handle_key(key(KeyCode::Char('5')));
        assert!(
            schedules
                .iter()
                .any(|command| matches!(command, Command::LoadSchedules { .. })),
            "unknown capability status must remain optimistic"
        );
    }

    #[test]
    fn saved_query_picker_applies_query_and_resets_cursor() {
        let mut app = app();
        app.saved_queries = vec![SavedQuery {
            name: "failures".to_string(),
            query: "ExecutionStatus = 'Failed'".to_string(),
        }];
        app.current_page_token = vec![9];
        app.previous_page_tokens = vec![Vec::new()];
        app.handle_key(key(KeyCode::Char('f')));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.query, "ExecutionStatus = 'Failed'");
        assert!(matches!(
            &commands[0],
            Command::LoadWorkflows {
                next_page_token,
                ..
            } if next_page_token.is_empty()
        ));
        assert!(app.previous_page_tokens.is_empty());
    }

    #[test]
    fn text_input_cursor_is_unicode_safe() {
        let mut input = TextInput::new("a🦀b");
        input.move_left();
        input.backspace();
        assert_eq!(input.value, "ab");
        assert_eq!(input.cursor, 1);
    }

    #[test]
    fn read_only_mode_blocks_all_mutations() {
        let mut app = app();
        app.read_only = true;
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        assert!(app.handle_key(key(KeyCode::Char('c'))).is_empty());
        assert!(app.overlay.is_none());
        assert!(app.handle_key(key(KeyCode::Char('s'))).is_empty());
        assert!(app.overlay.is_none());
        assert!(app.handle_key(key(KeyCode::Char('U'))).is_empty());
        assert!(app.overlay.is_none());
        assert!(app.handle_key(key(KeyCode::Char('R'))).is_empty());
        assert!(app.overlay.is_none());
        app.view = View::Schedules;
        app.schedules = vec![schedule("hourly-orders", false)];
        assert!(app.handle_key(key(KeyCode::Char('p'))).is_empty());
        assert!(app.handle_key(key(KeyCode::Char('N'))).is_empty());
        assert!(app.overlay.is_none());
        app.view = View::Batches;
        assert!(app.handle_key(key(KeyCode::Char('N'))).is_empty());
        assert!(app.overlay.is_none());
        assert!(app.notice.is_some());
    }

    #[test]
    fn workflow_query_and_update_forms_dispatch_typed_commands() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('Q')));
        for character in "state".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        app.handle_key(key(KeyCode::Enter));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::QueryWorkflow {
                query_name,
                key,
                arguments,
                ..
            } if query_name == "state" && key.workflow_id == "order-42" && arguments.is_empty()
        ));

        app.call_in_flight = false;
        app.handle_key(key(KeyCode::Char('U')));
        for character in "approve".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        app.handle_key(key(KeyCode::Enter));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::UpdateWorkflow { update_name, .. } if update_name == "approve"
        ));
        assert!(app.operation_in_flight);
    }

    #[test]
    fn reset_requires_exact_workflow_id_and_positive_event() {
        let mut app = app();
        app.workflows = vec![workflow("order-42", WorkflowStatus::Running)];
        app.handle_key(key(KeyCode::Char('R')));
        assert!(matches!(app.overlay, Some(Overlay::Reset(_))));
        if let Some(Overlay::Reset(form)) = &mut app.overlay {
            form.event_id = TextInput::new("7");
            form.confirmation = TextInput::new("order-42");
            form.active_field = ResetField::Confirmation;
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::ResetWorkflow { event_id: 7, .. }
        ));
    }

    #[test]
    fn schedule_controls_use_cursor_pagination_and_exact_confirmation() {
        let mut app = app();
        app.view = View::Schedules;
        app.schedules = vec![schedule("hourly-orders", false)];
        app.schedule_next_page_token = vec![9];
        let commands = app.handle_key(key(KeyCode::Char(']')));
        assert!(matches!(
            &commands[0],
            Command::LoadSchedules {
                next_page_token,
                ..
            } if next_page_token == &[9]
        ));

        app.loading_schedules = false;
        app.schedule_current_page_token.clear();
        app.schedule_next_page_token.clear();
        app.operation_in_flight = false;
        app.handle_key(key(KeyCode::Char('t')));
        assert!(matches!(
            app.overlay,
            Some(Overlay::ScheduleConfirm {
                action: ScheduleConfirmAction::Trigger,
                ..
            })
        ));
        for character in "hourly-orders".chars() {
            app.handle_key(key(KeyCode::Char(character)));
        }
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::TriggerSchedule { schedule_id, .. } if schedule_id == "hourly-orders"
        ));
    }

    #[test]
    fn create_schedule_form_validates_and_dispatches_complete_definition() {
        let mut app = app();
        app.view = View::Schedules;
        let form = ScheduleCreateForm {
            schedule_id: TextInput::new("hourly-orders"),
            workflow_id: TextInput::new("scheduled-order"),
            workflow_type: TextInput::new("OrderWorkflow"),
            task_queue: TextInput::new("orders"),
            expression: TextInput::new("@every 1h"),
            input: TextInput::new(r#"[{"region":"eu"}]"#),
            active_field: ScheduleCreateField::Notes,
            ..ScheduleCreateForm::default()
        };
        app.overlay = Some(Overlay::ScheduleCreate(form));
        let commands = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::CreateSchedule { request, .. }
                if request.schedule_id == "hourly-orders"
                    && request.schedule_expression == "@every 1h"
                    && request.arguments[0]["region"] == "eu"
        ));
    }

    #[test]
    fn workflow_arguments_require_an_explicit_json_array() {
        assert_eq!(parse_json_arguments("[]").unwrap(), Vec::<Value>::new());
        assert_eq!(
            parse_json_arguments(r#"[41, {"approved":true}]"#).unwrap(),
            vec![serde_json::json!(41), serde_json::json!({"approved": true})]
        );
        assert!(parse_json_arguments(r#"{"not":"an argument list"}"#).is_err());
        assert!(parse_json_arguments("[broken").is_err());
    }

    #[test]
    fn search_attribute_registry_requires_type_and_exact_confirmation() {
        let mut app = app();
        let load = app.handle_key(key(KeyCode::Char('A')));
        let Command::LoadSearchAttributes { request_id, .. } = &load[0] else {
            panic!("expected Search Attribute load");
        };
        app.handle_message(Message::SearchAttributesLoaded {
            request_id: *request_id,
            result: Ok(vec![SearchAttributeSummary {
                name: "CustomerTier".to_string(),
                value_type: "KEYWORD".to_string(),
                storage_type: "keyword".to_string(),
                custom: true,
            }]),
        });
        app.handle_key(key(KeyCode::Char('a')));
        app.overlay = Some(Overlay::SearchAttributeAdd(SearchAttributeAddForm {
            name: TextInput::new("Region"),
            value_type: TextInput::new("Keyword"),
            confirmation: TextInput::new("Region"),
            active_field: SearchAttributeAddField::Confirmation,
        }));
        let command = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &command[0],
            Command::AddSearchAttribute {
                name,
                value_type,
                ..
            } if name == "Region" && value_type == "Keyword"
        ));
    }

    #[test]
    fn deployment_rollout_forms_preserve_server_safety_checks() {
        let mut app = app();
        app.view = View::Deployments;
        let version = crate::model::DeploymentVersion {
            deployment_name: "payments".to_string(),
            build_id: "v2".to_string(),
        };
        let summary = WorkerDeploymentSummary {
            name: "payments".to_string(),
            create_time: None,
            current_version: None,
            ramping_version: Some(version.clone()),
            ramping_percentage: 25.0,
            latest_version: Some(version.clone()),
        };
        app.worker_deployment_details = Some(WorkerDeploymentDetails {
            summary,
            versions: vec![crate::model::DeploymentVersionSummary {
                version,
                status: "RAMPING".to_string(),
                create_time: None,
                is_current: false,
                is_ramping: true,
                ramp_percentage: 25.0,
                drainage_status: String::new(),
                drainage_last_checked: None,
            }],
            manager_identity: String::new(),
            last_modifier_identity: String::new(),
            routing_update_state: "COMPLETED".to_string(),
        });

        app.handle_key(key(KeyCode::Char('C')));
        app.overlay = Some(Overlay::DeploymentCurrent(DeploymentCurrentForm {
            deployment_name: "payments".to_string(),
            build_id: TextInput::new("v2"),
            confirmation: TextInput::new("payments"),
            active_field: DeploymentCurrentField::Confirmation,
        }));
        let command = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &command[0],
            Command::SetDeploymentCurrent {
                deployment_name,
                build_id,
                ..
            } if deployment_name == "payments" && build_id == "v2"
        ));

        app.operation_in_flight = false;
        app.overlay = Some(Overlay::DeploymentRamp(DeploymentRampForm {
            deployment_name: "payments".to_string(),
            build_id: TextInput::new("v2"),
            percentage: TextInput::new("37.5"),
            confirmation: TextInput::new("payments"),
            active_field: DeploymentRampField::Confirmation,
        }));
        let command = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &command[0],
            Command::SetDeploymentRamp { percentage, .. }
                if (*percentage - 37.5).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn batch_operation_preview_freezes_query_and_requires_exact_job_id() {
        let mut app = app();
        app.view = View::Batches;
        let form = BatchCreateForm {
            job_id: TextInput::new("cancel-stale-orders"),
            operation: TextInput::new("cancel"),
            visibility_query: TextInput::new(
                "WorkflowType = 'OrderWorkflow' AND ExecutionStatus = 'Running'",
            ),
            reason: TextInput::new("stale order cleanup"),
            max_operations_per_second: TextInput::new("12.5"),
            signal_name: TextInput::default(),
            signal_input: TextInput::new("not JSON, but irrelevant for cancellation"),
            active_field: BatchCreateField::SignalInput,
        };
        app.overlay = Some(Overlay::BatchCreate(form.clone()));
        let preview = app.handle_key(key(KeyCode::Enter));
        let Command::PreviewBatchOperation {
            request_id,
            namespace,
            form: frozen_form,
            request,
        } = &preview[0]
        else {
            panic!("expected Batch Operation preview");
        };
        assert_eq!(namespace, "default");
        assert_eq!(frozen_form, &form);
        assert_eq!(request.job_id, "cancel-stale-orders");
        assert_eq!(
            request.visibility_query,
            "WorkflowType = 'OrderWorkflow' AND ExecutionStatus = 'Running'"
        );
        assert!((request.max_operations_per_second - 12.5).abs() < f32::EPSILON);
        assert_eq!(request.signal_input, Value::Null);

        app.handle_message(Message::BatchOperationPreviewLoaded {
            request_id: *request_id,
            form: frozen_form.clone(),
            result: Ok(3),
        });
        let Some(Overlay::BatchConfirm {
            matched_workflows,
            input,
            ..
        }) = &mut app.overlay
        else {
            panic!("expected Batch Operation confirmation");
        };
        assert_eq!(*matched_workflows, 3);
        *input = TextInput::new("cancel-stale-orders");
        let start = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &start[0],
            Command::StartBatchOperation {
                namespace,
                request,
                ..
            } if namespace == "default"
                && request.job_id == "cancel-stale-orders"
                && request.kind == BatchOperationKind::Cancel
                && request.visibility_query
                    == "WorkflowType = 'OrderWorkflow' AND ExecutionStatus = 'Running'"
        ));
    }

    #[test]
    fn running_batch_operation_stop_requires_exact_job_id() {
        let mut app = app();
        app.view = View::Batches;
        app.batch_operations = vec![BatchOperationSummary {
            job_id: "terminate-test-orders".to_string(),
            state: "RUNNING".to_string(),
            start_time: None,
            close_time: None,
        }];
        app.handle_key(key(KeyCode::Char('s')));
        assert!(matches!(app.overlay, Some(Overlay::BatchStop { .. })));

        if let Some(Overlay::BatchStop { input, .. }) = &mut app.overlay {
            *input = TextInput::new("wrong-job");
        }
        assert!(app.handle_key(key(KeyCode::Enter)).is_empty());
        assert!(matches!(app.overlay, Some(Overlay::BatchStop { .. })));

        if let Some(Overlay::BatchStop { input, .. }) = &mut app.overlay {
            *input = TextInput::new("terminate-test-orders");
        }
        let stop = app.handle_key(key(KeyCode::Enter));
        assert!(matches!(
            &stop[0],
            Command::StopBatchOperation {
                namespace,
                job_id,
                ..
            } if namespace == "default" && job_id == "terminate-test-orders"
        ));
    }

    #[test]
    fn cursor_navigation_preserves_previous_page_token() {
        let mut app = app();
        app.next_page_token = vec![2];
        let next = app.handle_key(key(KeyCode::Char(']')));
        assert!(matches!(
            &next[0],
            Command::LoadWorkflows {
                next_page_token,
                ..
            } if next_page_token == &[2]
        ));
        assert_eq!(app.page_number, 1);
        app.handle_message(Message::WorkflowsLoaded {
            request_id: load_request_id(&next[0]),
            result: Ok(WorkflowPage {
                workflows: vec![workflow("page-2", WorkflowStatus::Running)],
                next_page_token: Vec::new(),
            }),
        });
        assert_eq!(app.page_number, 2);
        let previous = app.handle_key(key(KeyCode::Char('[')));
        assert!(matches!(
            &previous[0],
            Command::LoadWorkflows {
                next_page_token,
                ..
            } if next_page_token.is_empty()
        ));
    }

    #[test]
    fn web_url_percent_encodes_identity_segments() {
        let key = WorkflowKey {
            workflow_id: "order/42".to_string(),
            run_id: "run id".to_string(),
        };
        let url = workflow_web_url("http://localhost:8233", "team one", &key).unwrap();
        assert!(url.contains("team%20one"));
        assert!(url.contains("order%2F42"));
        assert!(url.contains("run%20id"));
    }
}
