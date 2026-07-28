use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

use crate::model::{ClusterInfo, NamespaceSummary, WorkflowDetails, WorkflowKey, WorkflowSummary};

/// Startup behavior and presentation preferences.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub address: String,
    pub namespace: String,
    pub query: String,
    pub page_size: usize,
    pub refresh_interval: Duration,
    pub auto_refresh: bool,
    pub color: bool,
}

/// Which primary pane receives navigation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Workflows,
    History,
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
    NamespacePicker {
        selected: usize,
    },
    Confirm {
        action: ConfirmAction,
        key: WorkflowKey,
        workflow_id: String,
    },
    Signal(SignalForm),
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
        limit: usize,
    },
    LoadDetails {
        request_id: u64,
        namespace: String,
        key: WorkflowKey,
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
        result: Result<Vec<WorkflowSummary>, String>,
    },
    DetailsLoaded {
        request_id: u64,
        result: Result<WorkflowDetails, String>,
    },
    OperationFinished {
        request_id: u64,
        operation: OperationKind,
        result: Result<(), String>,
    },
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
    pub namespace: String,
    pub query: String,
    pub page_size: usize,
    pub refresh_interval: Duration,
    pub auto_refresh: bool,
    pub color: bool,
    pub cluster: Option<ClusterInfo>,
    pub namespaces: Vec<NamespaceSummary>,
    pub workflows: Vec<WorkflowSummary>,
    pub selected_workflow: usize,
    pub details: Option<WorkflowDetails>,
    pub selected_event: usize,
    pub focus: Focus,
    pub overlay: Option<Overlay>,
    pub notice: Option<Notice>,
    pub loading_workflows: bool,
    pub loading_details: bool,
    pub operation_in_flight: bool,
    pub should_quit: bool,
    current_workflow_request: u64,
    current_detail_request: u64,
    current_namespace_request: u64,
    current_operation_request: u64,
    next_request_id: u64,
    last_refresh_started: Instant,
}

impl App {
    #[must_use]
    pub fn new(config: AppConfig) -> Self {
        Self {
            address: config.address,
            namespace: config.namespace,
            query: config.query,
            page_size: config.page_size,
            refresh_interval: config.refresh_interval,
            auto_refresh: config.auto_refresh,
            color: config.color,
            cluster: None,
            namespaces: Vec::new(),
            workflows: Vec::new(),
            selected_workflow: 0,
            details: None,
            selected_event: 0,
            focus: Focus::Workflows,
            overlay: None,
            notice: None,
            loading_workflows: false,
            loading_details: false,
            operation_in_flight: false,
            should_quit: false,
            current_workflow_request: 0,
            current_detail_request: 0,
            current_namespace_request: 0,
            current_operation_request: 0,
            next_request_id: 0,
            last_refresh_started: Instant::now(),
        }
    }

    /// Initial data fetches.
    pub fn bootstrap(&mut self) -> Vec<Command> {
        let mut commands = vec![Command::LoadCluster, self.load_namespaces()];
        commands.push(self.refresh_workflows());
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
                    Ok(workflows) => {
                        let previous_key = self
                            .selected_workflow()
                            .map(|workflow| workflow.key.clone());
                        self.workflows = workflows;
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
            Message::DetailsLoaded { request_id, result } => {
                if request_id != self.current_detail_request {
                    return Vec::new();
                }
                self.loading_details = false;
                match result {
                    Ok(details) => {
                        self.selected_event = details.events.len().saturating_sub(1);
                        self.details = Some(details);
                    }
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
                        vec![self.refresh_workflows()]
                    }
                    Err(error) => {
                        self.show_notice(error, NoticeKind::Error);
                        Vec::new()
                    }
                }
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
            && !self.loading_workflows
            && now.duration_since(self.last_refresh_started) >= self.refresh_interval
        {
            return vec![self.refresh_workflows()];
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
            KeyCode::Char('/') => {
                self.overlay = Some(Overlay::Query(TextInput::new(self.query.clone())));
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
            KeyCode::Char('r') => return vec![self.refresh_workflows()],
            KeyCode::Char('a') => {
                self.auto_refresh = !self.auto_refresh;
                let state = if self.auto_refresh {
                    "enabled"
                } else {
                    "disabled"
                };
                self.show_notice(format!("Auto-refresh {state}"), NoticeKind::Info);
            }
            KeyCode::Char('c') => self.open_confirmation(ConfirmAction::Cancel),
            KeyCode::Char('x') => self.open_confirmation(ConfirmAction::Terminate),
            KeyCode::Char('s') => self.open_signal(),
            KeyCode::Tab | KeyCode::Enter => {
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
                    return vec![self.refresh_workflows()];
                }
                _ => edit_text(input, key),
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
                        return vec![self.refresh_workflows()];
                    }
                    return Vec::new();
                }
                _ => {}
            },
            Overlay::Confirm {
                action,
                key: workflow_key,
                ..
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('n' | 'N') => return Vec::new(),
                KeyCode::Char('y' | 'Y') => {
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
                _ => {}
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
        }

        self.overlay = Some(overlay);
        Vec::new()
    }

    fn move_up(&mut self, amount: usize) -> Vec<Command> {
        match self.focus {
            Focus::Workflows => {
                let next = self.selected_workflow.saturating_sub(amount);
                self.select_workflow(next)
            }
            Focus::History => {
                self.selected_event = self.selected_event.saturating_sub(amount);
                Vec::new()
            }
        }
    }

    fn move_down(&mut self, amount: usize) -> Vec<Command> {
        match self.focus {
            Focus::Workflows => {
                let next =
                    (self.selected_workflow + amount).min(self.workflows.len().saturating_sub(1));
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
        }
    }

    fn move_first(&mut self) -> Vec<Command> {
        match self.focus {
            Focus::Workflows => self.select_workflow(0),
            Focus::History => {
                self.selected_event = 0;
                Vec::new()
            }
        }
    }

    fn move_last(&mut self) -> Vec<Command> {
        match self.focus {
            Focus::Workflows => self.select_workflow(self.workflows.len().saturating_sub(1)),
            Focus::History => {
                self.selected_event = self
                    .details
                    .as_ref()
                    .map_or(0, |details| details.events.len().saturating_sub(1));
                Vec::new()
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

    fn open_confirmation(&mut self, action: ConfirmAction) {
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
        });
    }

    fn open_signal(&mut self) {
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

    fn refresh_workflows(&mut self) -> Command {
        let request_id = self.next_request_id();
        self.current_workflow_request = request_id;
        self.loading_workflows = true;
        self.last_refresh_started = Instant::now();
        Command::LoadWorkflows {
            request_id,
            namespace: self.namespace.clone(),
            query: self.query.clone(),
            limit: self.page_size,
        }
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
            namespace: "default".to_string(),
            query: String::new(),
            page_size: 50,
            refresh_interval: Duration::from_secs(5),
            auto_refresh: true,
            color: true,
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
    }

    #[test]
    fn stale_workflow_results_are_ignored() {
        let mut app = app();
        let first = app.refresh_workflows();
        let second = app.refresh_workflows();
        let commands = app.handle_message(Message::WorkflowsLoaded {
            request_id: load_request_id(&first),
            result: Ok(vec![workflow("stale", WorkflowStatus::Running)]),
        });
        assert!(commands.is_empty());
        assert!(app.workflows.is_empty());

        app.handle_message(Message::WorkflowsLoaded {
            request_id: load_request_id(&second),
            result: Ok(vec![workflow("fresh", WorkflowStatus::Running)]),
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
        let commands = app.handle_key(key(KeyCode::Char('y')));
        assert!(matches!(
            &commands[0],
            Command::Cancel { key, .. } if key.workflow_id == "order-42"
        ));
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
        let commands = app.handle_key(key(KeyCode::Char('y')));
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
    fn text_input_cursor_is_unicode_safe() {
        let mut input = TextInput::new("a🦀b");
        input.move_left();
        input.backspace();
        assert_eq!(input.value, "ab");
        assert_eq!(input.cursor, 1);
    }
}
