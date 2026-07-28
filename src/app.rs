use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use url::Url;

use crate::model::{
    ClusterInfo, HistoryPage, NamespaceSummary, TaskQueueSummary, WorkerDeploymentDetails,
    WorkerDeploymentPage, WorkerDeploymentSummary, WorkerDetails, WorkerPage, WorkerSummary,
    WorkflowCount, WorkflowDetails, WorkflowKey, WorkflowPage, WorkflowSummary,
};

/// Named visibility query loaded from the local config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQuery {
    pub name: String,
    pub query: String,
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
    pub codec_enabled: bool,
    pub web_ui_url: Option<String>,
    pub saved_queries: Vec<SavedQuery>,
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
}

impl View {
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Workflows => 1,
            Self::TaskQueues => 2,
            Self::Workers => 3,
            Self::Deployments => 4,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Workflows => "WORKFLOWS",
            Self::TaskQueues => "TASK QUEUES",
            Self::Workers => "WORKERS",
            Self::Deployments => "DEPLOYMENTS",
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
}

impl ConfirmAction {
    #[must_use]
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Cancel => "request cancellation of",
            Self::Terminate => "terminate",
        }
    }
}

/// Modal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Query(TextInput),
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
    Confirm {
        action: ConfirmAction,
        key: WorkflowKey,
        workflow_id: String,
        input: TextInput,
    },
    Signal(SignalForm),
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

/// Side effects requested by the pure application state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    LoadCluster,
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
    ClusterLoaded(Result<ClusterInfo, String>),
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
}

impl OperationKind {
    const fn success_message(self) -> &'static str {
        match self {
            Self::Cancel => "Cancellation requested",
            Self::Terminate => "Workflow terminated",
            Self::Signal => "Signal delivered",
        }
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
    pub codec_enabled: bool,
    pub web_ui_url: Option<String>,
    pub saved_queries: Vec<SavedQuery>,
    pub view: View,
    pub cluster: Option<ClusterInfo>,
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
    pub operation_in_flight: bool,
    pub should_quit: bool,
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
            codec_enabled: config.codec_enabled,
            web_ui_url: config.web_ui_url,
            saved_queries: config.saved_queries,
            view: View::Workflows,
            cluster: None,
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
            operation_in_flight: false,
            should_quit: false,
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
            manual_task_queue_names: BTreeSet::new(),
            last_refresh_started: Instant::now(),
        }
    }

    /// Initial data fetches.
    pub fn bootstrap(&mut self) -> Vec<Command> {
        let mut commands = vec![Command::LoadCluster, self.load_namespaces()];
        commands.extend(self.refresh_workflows(true));
        commands
    }

    /// Apply one key event and return requested side effects.
    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<Command> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
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
            Message::ClusterLoaded(result) => {
                match result {
                    Ok(cluster) => self.cluster = Some(cluster),
                    Err(error) => self.show_notice(error, NoticeKind::Error),
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
                        self.refresh_workflows(false)
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

    fn handle_dashboard_key(&mut self, key: KeyEvent) -> Vec<Command> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Char('1') => return self.switch_view(View::Workflows),
            KeyCode::Char('2') => return self.switch_view(View::TaskQueues),
            KeyCode::Char('3') => return self.switch_view(View::Workers),
            KeyCode::Char('4') => return self.switch_view(View::Deployments),
            KeyCode::Char('/') if self.view == View::Workflows => {
                self.overlay = Some(Overlay::Query(TextInput::new(self.query.clone())));
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
                if self
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
            KeyCode::Char('n') => {
                if self.namespaces.is_empty() {
                    self.show_notice("No namespaces loaded", NoticeKind::Info);
                } else {
                    self.overlay = Some(Overlay::NamespacePicker {
                        selected: self.selected_namespace_index(),
                    });
                }
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
            Overlay::Query(input) => match key.code {
                KeyCode::Esc => return Vec::new(),
                KeyCode::Enter => {
                    self.query = input.value.trim().to_string();
                    return self.refresh_workflows(true);
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
                        self.manual_task_queue_names.clear();
                        self.reset_worker_pagination();
                        self.reset_deployment_pagination();
                        return self.refresh_current_view(true);
                    }
                    return Vec::new();
                }
                _ => {}
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
        if !workflow.status.is_running() {
            self.show_notice(
                format!("{} workflows cannot be changed", workflow.status),
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
            View::Workflows | View::Workers | View::Deployments => Vec::new(),
        }
    }

    fn refresh_current_view(&mut self, reset_pagination: bool) -> Vec<Command> {
        match self.view {
            View::Workflows => self.refresh_workflows(reset_pagination),
            View::TaskQueues => self.refresh_task_queues(),
            View::Workers => self.refresh_workers(reset_pagination),
            View::Deployments => self.refresh_worker_deployments(reset_pagination),
        }
    }

    fn current_view_is_loading(&self) -> bool {
        match self.view {
            View::Workflows => self.loading_workflows,
            View::TaskQueues => self.loading_task_queues,
            View::Workers => self.loading_workers,
            View::Deployments => self.loading_worker_deployments,
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

    fn next_page(&mut self) -> Vec<Command> {
        match self.view {
            View::Workflows => self.next_workflow_page(),
            View::Workers => self.next_worker_page(),
            View::Deployments => self.next_deployment_page(),
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

    fn load_namespaces(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_namespace_request = request_id;
        Command::LoadNamespaces { request_id }
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
            codec_enabled: false,
            web_ui_url: Some("http://localhost:8233".to_string()),
            saved_queries: Vec::new(),
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
        assert!(matches!(commands[0], Command::LoadCluster));
        assert!(matches!(commands[1], Command::LoadNamespaces { .. }));
        assert!(matches!(commands[2], Command::LoadWorkflows { .. }));
        assert!(matches!(commands[3], Command::CountWorkflows { .. }));
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
        assert!(matches!(commands[0], Command::LoadWorkflows { .. }));
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
        assert!(app.notice.is_some());
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
