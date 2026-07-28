use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
    },
};
use unicode_width::UnicodeWidthStr;

use crate::{
    app::{App, ConfirmAction, Focus, NoticeKind, Overlay, SignalField, SignalForm, TextInput},
    model::WorkflowStatus,
};

const MIN_WIDTH: u16 = 58;
const MIN_HEIGHT: u16 = 15;

/// Draw the complete dashboard.
pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    let theme = Theme::new(app.color);
    let vertical = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .split(area);
    render_header(frame, app, vertical[0], theme);
    render_dashboard(frame, app, vertical[1], theme);
    render_footer(frame, app, vertical[2], theme);

    if let Some(overlay) = &app.overlay {
        match overlay {
            Overlay::Help => render_help(frame, area, theme),
            Overlay::Query(input) => render_query(frame, area, input, theme),
            Overlay::NamespacePicker { selected } => {
                render_namespaces(frame, area, app, *selected, theme);
            }
            Overlay::Confirm {
                action,
                workflow_id,
                ..
            } => render_confirmation(frame, area, *action, workflow_id, theme),
            Overlay::Signal(form) => render_signal(frame, area, form, theme),
        }
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let cluster = app.cluster.as_ref().map_or_else(
        || "connecting…".to_string(),
        |cluster| {
            let name = if cluster.cluster_name.is_empty() {
                "Temporal"
            } else {
                cluster.cluster_name.as_str()
            };
            format!("{name} {}", cluster.server_version)
        },
    );
    let refresh = if app.auto_refresh {
        format!("auto {}s", app.refresh_interval.as_secs())
    } else {
        "manual".to_string()
    };
    let query = if app.query.is_empty() {
        "all workflows".to_string()
    } else {
        format!("query: {}", app.query)
    };
    let line = Line::from(vec![
        Span::styled(" ● ", theme.success()),
        Span::styled(cluster, theme.strong()),
        Span::raw("  "),
        Span::styled(format!("ns/{}", app.namespace), theme.accent()),
        Span::raw("  "),
        Span::raw(refresh),
        Span::raw("  "),
        Span::styled(query, theme.muted()),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(false))
        .title(Span::styled(" temporal-tui ", theme.title()))
        .title_bottom(
            Line::from(format!(" {} ", app.address))
                .alignment(Alignment::Right)
                .style(theme.muted()),
        );
    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    if area.width >= 106 {
        let horizontal =
            Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .split(area);
        render_workflows(frame, app, horizontal[0], theme);
        render_details(frame, app, horizontal[1], theme);
    } else {
        let vertical =
            Layout::vertical([Constraint::Percentage(54), Constraint::Percentage(46)]).split(area);
        render_workflows(frame, app, vertical[0], theme);
        render_details(frame, app, vertical[1], theme);
    }
}

fn render_workflows(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let title = if app.loading_workflows {
        format!(" Workflows ({}) ⟳ ", app.workflows.len())
    } else {
        format!(" Workflows ({}) ", app.workflows.len())
    };
    let rows = app.workflows.iter().map(|workflow| {
        Row::new(vec![
            Cell::from(workflow.status.label()).style(theme.workflow_status(workflow.status)),
            Cell::from(workflow.key.workflow_id.clone()),
            Cell::from(workflow.workflow_type.clone()),
            Cell::from(format_time(workflow.start_time.as_ref())),
            Cell::from(format_count(workflow.history_length)),
        ])
    });
    let header = Row::new(["STATUS", "WORKFLOW ID", "TYPE", "STARTED", "EVENTS"])
        .style(theme.table_header())
        .bottom_margin(1);
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(17),
            Constraint::Length(7),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border(app.focus == Focus::Workflows))
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.workflows.is_empty()).then_some(app.selected_workflow));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_details(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let Some(details) = &app.details else {
        let message = if app.loading_details {
            "Loading workflow details…"
        } else if app.workflows.is_empty() {
            "No workflows match this query"
        } else {
            "Select a workflow"
        };
        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .style(theme.muted())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme.border(false))
                        .title(Span::styled(" Details ", theme.title()))
                        .padding(Padding::vertical(1)),
                ),
            area,
        );
        return;
    };

    let metadata_height = area.height.min(10);
    let vertical =
        Layout::vertical([Constraint::Length(metadata_height), Constraint::Min(4)]).split(area);
    let workflow = &details.summary;
    let pending = format!(
        "{} activities · {} children · {} nexus",
        details.pending_activities, details.pending_children, details.pending_nexus_operations
    );
    let history = format!(
        "{} events · {} · {} transitions",
        workflow.history_length,
        format_bytes(workflow.history_size_bytes),
        details.state_transition_count
    );
    let mut lines = vec![
        field_line("ID", &workflow.key.workflow_id, theme),
        field_line("Run", &workflow.key.run_id, theme),
        Line::from(vec![
            Span::styled("Type       ", theme.muted()),
            Span::raw(&workflow.workflow_type),
            Span::raw("  "),
            Span::styled(
                workflow.status.label(),
                theme.workflow_status(workflow.status),
            ),
        ]),
        field_line("Task queue", &workflow.task_queue, theme),
        field_line("History", &history, theme),
        field_line("Pending", &pending, theme),
    ];
    if let Some(summary) = details.static_summary.as_deref() {
        lines.push(field_line("Summary", summary, theme));
    }
    let metadata = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border(false))
            .title(Span::styled(" Workflow ", theme.title())),
    );
    frame.render_widget(metadata, vertical[0]);

    let history_rows = details.events.iter().map(|event| {
        Row::new(vec![
            Cell::from(event.event_id.to_string()),
            Cell::from(format_clock(event.event_time.as_ref())),
            Cell::from(event.event_type.clone()),
            Cell::from(event.detail.clone()).style(theme.muted()),
        ])
    });
    let history_table = Table::new(
        history_rows,
        [
            Constraint::Length(7),
            Constraint::Length(9),
            Constraint::Fill(3),
            Constraint::Fill(2),
        ],
    )
    .header(Row::new(["EVENT", "TIME", "TYPE", "DETAIL"]).style(theme.table_header()))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.border(app.focus == Focus::History))
            .title(Span::styled(
                if usize::try_from(details.summary.history_length)
                    .is_ok_and(|total| total > details.events.len())
                {
                    format!(
                        " History (latest {} of {}) ",
                        details.events.len(),
                        details.summary.history_length
                    )
                } else {
                    format!(" History ({}) ", details.events.len())
                },
                theme.title(),
            )),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut table_state = TableState::default()
        .with_selected((!details.events.is_empty()).then_some(app.selected_event));
    frame.render_stateful_widget(history_table, vertical[1], &mut table_state);

    if details.events.len() > vertical[1].height.saturating_sub(4) as usize {
        let mut scrollbar_state = ScrollbarState::new(details.events.len())
            .position(app.selected_event)
            .viewport_content_length(vertical[1].height.saturating_sub(4) as usize);
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None)
                .style(theme.muted()),
            vertical[1].inner(ratatui::layout::Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut scrollbar_state,
        );
    }
}

fn render_footer(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let line = if let Some(notice) = &app.notice {
        Line::from(vec![
            Span::styled(
                match notice.kind {
                    NoticeKind::Info => " INFO ",
                    NoticeKind::Success => " OK ",
                    NoticeKind::Error => " ERROR ",
                },
                theme.notice(notice.kind),
            ),
            Span::raw(" "),
            Span::raw(&notice.text),
        ])
    } else {
        Line::from(vec![
            key_hint("j/k", "move", theme),
            Span::raw("  "),
            key_hint("tab", "pane", theme),
            Span::raw("  "),
            key_hint("/", "query", theme),
            Span::raw("  "),
            key_hint("n", "namespace", theme),
            Span::raw("  "),
            key_hint("s", "signal", theme),
            Span::raw("  "),
            key_hint("c/x", "cancel/terminate", theme),
            Span::raw("  "),
            key_hint("?", "help", theme),
            Span::raw("  "),
            key_hint("q", "quit", theme),
        ])
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let popup = centered(area, 76, 24);
    frame.render_widget(Clear, popup);
    let help = Text::from(vec![
        help_section("NAVIGATION", theme),
        help_line("j / ↓", "next workflow or history event", theme),
        help_line("k / ↑", "previous workflow or history event", theme),
        help_line("g / G", "first / last item", theme),
        help_line("tab / enter", "switch workflow and history panes", theme),
        Line::default(),
        help_section("DATA", theme),
        help_line("/", "edit Temporal visibility query", theme),
        help_line("n", "switch namespace", theme),
        help_line("r", "refresh now", theme),
        help_line("a", "toggle automatic refresh", theme),
        Line::default(),
        help_section("CONTROL", theme),
        help_line("s", "send a named signal with JSON input", theme),
        help_line("c", "request graceful workflow cancellation", theme),
        help_line("x", "terminate workflow immediately", theme),
        Line::default(),
        help_line("q / ctrl-c", "quit", theme),
        help_line("? / esc", "close this help", theme),
    ]);
    frame.render_widget(
        Paragraph::new(help).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent())
                .title(Span::styled(" Keyboard help ", theme.title()))
                .padding(Padding::horizontal(2)),
        ),
        popup,
    );
}

fn render_query(frame: &mut Frame<'_>, area: Rect, input: &TextInput, theme: Theme) {
    let popup = centered(area, 82, 5);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.accent())
        .title(Span::styled(" Temporal visibility query ", theme.title()))
        .title_bottom(
            Line::from(" enter apply · esc cancel ")
                .alignment(Alignment::Right)
                .style(theme.muted()),
        );
    let horizontal_offset = input_horizontal_offset(input, popup.width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .scroll((0, horizontal_offset))
            .block(block),
        popup,
    );
    set_input_cursor(frame, popup, input, horizontal_offset);
}

fn render_namespaces(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize, theme: Theme) {
    let visible_namespaces = app.namespaces.len().clamp(3, 20);
    let height = u16::try_from(visible_namespaces).unwrap_or(20) + 4;
    let popup = centered(area, 72, height);
    frame.render_widget(Clear, popup);
    let items = app.namespaces.iter().map(|namespace| {
        let marker = if namespace.is_global {
            "global"
        } else {
            "local"
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!("{:<24}", namespace.name), theme.strong()),
            Span::styled(format!("{:<12}", namespace.retention), theme.muted()),
            Span::raw(format!("{marker:<10}")),
            Span::styled(&namespace.active_cluster, theme.muted()),
        ]))
    });
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent())
                .title(Span::styled(" Select namespace ", theme.title()))
                .title_bottom(
                    Line::from(" enter select · esc cancel ")
                        .alignment(Alignment::Right)
                        .style(theme.muted()),
                ),
        )
        .highlight_style(theme.selection())
        .highlight_symbol("› ");
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    action: ConfirmAction,
    workflow_id: &str,
    theme: Theme,
) {
    let popup = centered(area, 76, 8);
    frame.render_widget(Clear, popup);
    let severity = match action {
        ConfirmAction::Cancel => theme.warning(),
        ConfirmAction::Terminate => theme.error(),
    };
    let warning = match action {
        ConfirmAction::Cancel => {
            "The workflow may handle cancellation and perform cleanup before closing."
        }
        ConfirmAction::Terminate => {
            "Termination is immediate. Workflow code cannot intercept or clean up."
        }
    };
    let text = Text::from(vec![
        Line::from(vec![
            Span::raw("Really "),
            Span::styled(action.verb(), severity.add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(workflow_id, theme.strong()),
            Span::raw("?"),
        ]),
        Line::default(),
        Line::from(warning).style(theme.muted()),
        Line::from(vec![
            Span::styled(" y ", severity.add_modifier(Modifier::BOLD)),
            Span::raw(" confirm   "),
            Span::styled(" n / esc ", theme.key()),
            Span::raw(" cancel"),
        ]),
    ]);
    frame.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: true }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(severity)
                .title(Span::styled(" Confirmation ", severity)),
        ),
        popup,
    );
}

fn render_signal(frame: &mut Frame<'_>, area: Rect, form: &SignalForm, theme: Theme) {
    let popup = centered(area, 80, 10);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(" Send workflow signal ", theme.title()))
            .title_bottom(
                Line::from(" enter next/send · tab field · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let fields = Layout::vertical([Constraint::Length(3), Constraint::Length(3)])
        .spacing(1)
        .split(inner);
    let name_block = input_block(
        " Signal name ",
        form.active_field == SignalField::Name,
        theme,
    );
    let name_offset = input_horizontal_offset(&form.name, fields[0].width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(form.name.value.as_str())
            .scroll((0, name_offset))
            .block(name_block),
        fields[0],
    );
    let input_block = input_block(
        " JSON input ",
        form.active_field == SignalField::Input,
        theme,
    );
    let input_offset = input_horizontal_offset(&form.input, fields[1].width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(form.input.value.as_str())
            .scroll((0, input_offset))
            .block(input_block),
        fields[1],
    );
    match form.active_field {
        SignalField::Name => set_input_cursor(frame, fields[0], &form.name, name_offset),
        SignalField::Input => set_input_cursor(frame, fields[1], &form.input, input_offset),
    }
}

fn input_block(title: &'static str, active: bool, theme: Theme) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(active))
        .title(Span::styled(title, theme.strong()))
}

fn set_input_cursor(frame: &mut Frame<'_>, area: Rect, input: &TextInput, horizontal_offset: u16) {
    let before_cursor: String = input.value.chars().take(input.cursor).collect();
    let width = u16::try_from(UnicodeWidthStr::width(before_cursor.as_str())).unwrap_or(u16::MAX);
    let x = area
        .x
        .saturating_add(1)
        .saturating_add(width.saturating_sub(horizontal_offset))
        .min(area.right().saturating_sub(2));
    frame.set_cursor_position(Position::new(x, area.y.saturating_add(1)));
}

fn input_horizontal_offset(input: &TextInput, content_width: u16) -> u16 {
    let before_cursor: String = input.value.chars().take(input.cursor).collect();
    let cursor_width =
        u16::try_from(UnicodeWidthStr::width(before_cursor.as_str())).unwrap_or(u16::MAX);
    cursor_width.saturating_sub(content_width.saturating_sub(1))
}

fn render_too_small(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(
        Paragraph::new(format!(
            "temporal-tui needs at least {MIN_WIDTH}×{MIN_HEIGHT}\ncurrent: {}×{}",
            area.width, area.height
        ))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        area,
    );
}

fn centered(area: Rect, desired_width: u16, desired_height: u16) -> Rect {
    let width = desired_width.min(area.width.saturating_sub(2)).max(1);
    let height = desired_height.min(area.height.saturating_sub(2)).max(1);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn field_line<'a>(label: &'static str, value: &'a str, theme: Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), theme.muted()),
        Span::raw(value),
    ])
}

fn key_hint<'a>(key: &'a str, action: &'a str, theme: Theme) -> Span<'a> {
    Span::styled(format!(" {key} {action} "), theme.key())
}

fn help_section(label: &'static str, theme: Theme) -> Line<'static> {
    Line::from(Span::styled(label, theme.title()))
}

fn help_line<'a>(key: &'a str, description: &'a str, theme: Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{key:<16}"), theme.key()),
        Span::raw(description),
    ])
}

fn format_time(value: Option<&DateTime<Utc>>) -> String {
    value.map_or_else(
        || "—".to_string(),
        |time| {
            time.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

fn format_clock(value: Option<&DateTime<Utc>>) -> String {
    value.map_or_else(
        || "—".to_string(),
        |time| time.with_timezone(&Local).format("%H:%M:%S").to_string(),
    )
}

fn format_count(value: i64) -> String {
    if value >= 1_000_000 {
        format_decimal(value, 1_000_000, "m")
    } else if value >= 1_000 {
        format_decimal(value, 1_000, "k")
    } else {
        value.to_string()
    }
}

fn format_bytes(value: i64) -> String {
    let value = value.max(0);
    if value >= 1024 * 1024 {
        format_decimal(value, 1024 * 1024, " MiB")
    } else if value >= 1024 {
        format_decimal(value, 1024, " KiB")
    } else {
        format!("{value} B")
    }
}

fn format_decimal(value: i64, scale: i64, suffix: &str) -> String {
    let whole = value / scale;
    let tenths = value % scale * 10 / scale;
    format!("{whole}.{tenths}{suffix}")
}

#[derive(Debug, Clone, Copy)]
struct Theme {
    color: bool,
}

impl Theme {
    const fn new(color: bool) -> Self {
        Self { color }
    }

    fn style(self, color: Color) -> Style {
        if self.color {
            Style::default().fg(color)
        } else {
            Style::default()
        }
    }

    fn title(self) -> Style {
        self.style(Color::White).add_modifier(Modifier::BOLD)
    }

    fn strong(self) -> Style {
        self.style(Color::White)
    }

    fn muted(self) -> Style {
        self.style(Color::DarkGray)
    }

    fn accent(self) -> Style {
        self.style(Color::Cyan)
    }

    fn success(self) -> Style {
        self.style(Color::Green)
    }

    fn warning(self) -> Style {
        self.style(Color::Yellow)
    }

    fn error(self) -> Style {
        self.style(Color::Red)
    }

    fn key(self) -> Style {
        self.style(Color::Black)
            .bg(if self.color {
                Color::Gray
            } else {
                Color::Reset
            })
            .add_modifier(Modifier::BOLD)
    }

    fn border(self, active: bool) -> Style {
        if active { self.accent() } else { self.muted() }
    }

    fn selection(self) -> Style {
        if self.color {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
        }
    }

    fn table_header(self) -> Style {
        self.muted().add_modifier(Modifier::BOLD)
    }

    fn workflow_status(self, status: WorkflowStatus) -> Style {
        match status {
            WorkflowStatus::Running => self.accent(),
            WorkflowStatus::Completed | WorkflowStatus::ContinuedAsNew => self.success(),
            WorkflowStatus::Failed | WorkflowStatus::Terminated | WorkflowStatus::TimedOut => {
                self.error()
            }
            WorkflowStatus::Canceled | WorkflowStatus::Paused => self.warning(),
            WorkflowStatus::Unspecified | WorkflowStatus::Unknown(_) => self.muted(),
        }
        .add_modifier(Modifier::BOLD)
    }

    fn notice(self, kind: NoticeKind) -> Style {
        match kind {
            NoticeKind::Info => self.accent(),
            NoticeKind::Success => self.success(),
            NoticeKind::Error => self.error(),
        }
        .add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{TimeZone, Utc};
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        app::{AppConfig, Overlay},
        model::{ClusterInfo, HistoryEventSummary, WorkflowDetails, WorkflowKey, WorkflowSummary},
    };

    fn sample_app() -> App {
        let mut app = App::new(AppConfig {
            address: "localhost:7233".to_string(),
            namespace: "default".to_string(),
            query: String::new(),
            page_size: 200,
            refresh_interval: Duration::from_secs(5),
            auto_refresh: true,
            color: true,
        });
        app.cluster = Some(ClusterInfo {
            cluster_name: "dev".to_string(),
            server_version: "1.31.2".to_string(),
            ..Default::default()
        });
        let summary = WorkflowSummary {
            key: WorkflowKey {
                workflow_id: "order-42".to_string(),
                run_id: "run-abc".to_string(),
            },
            workflow_type: "OrderWorkflow".to_string(),
            task_queue: "orders".to_string(),
            status: WorkflowStatus::Running,
            start_time: Some(Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap()),
            close_time: None,
            history_length: 3,
            history_size_bytes: 2048,
        };
        app.workflows = vec![summary.clone()];
        app.details = Some(WorkflowDetails {
            summary,
            first_run_id: "run-abc".to_string(),
            parent_workflow_id: None,
            pending_activities: 1,
            pending_children: 0,
            pending_nexus_operations: 0,
            state_transition_count: 5,
            static_summary: Some("Process order".to_string()),
            static_details: None,
            events: vec![HistoryEventSummary {
                event_id: 1,
                event_type: "WORKFLOW EXECUTION STARTED".to_string(),
                event_time: Some(Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap()),
                detail: "OrderWorkflow · orders".to_string(),
            }],
        });
        app
    }

    fn rendered(app: &App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer[(x, y)].symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn renders_dashboard_data() {
        let output = rendered(&sample_app(), 140, 38);
        assert!(output.contains("temporal-tui"));
        assert!(output.contains("order-42"));
        assert!(output.contains("OrderWorkflow"));
        assert!(output.contains("WORKFLOW EXECUTION"));
        assert!(output.contains("ns/default"));
    }

    #[test]
    fn renders_destructive_confirmation() {
        let mut app = sample_app();
        app.overlay = Some(Overlay::Confirm {
            action: ConfirmAction::Terminate,
            key: app.workflows[0].key.clone(),
            workflow_id: "order-42".to_string(),
        });
        let output = rendered(&app, 120, 32);
        assert!(output.contains("Termination is immediate"));
        assert!(output.contains("order-42"));
        assert!(output.contains("confirm"));
    }

    #[test]
    fn renders_small_terminal_fallback() {
        let output = rendered(&sample_app(), 40, 10);
        assert!(output.contains("needs at least"));
        assert!(output.contains("40×10"));
    }

    #[test]
    fn byte_and_count_formatting_is_compact() {
        assert_eq!(format_bytes(2048), "2.0 KiB");
        assert_eq!(format_count(12_500), "12.5k");
    }

    #[test]
    fn long_unicode_input_scrolls_to_keep_cursor_visible() {
        let input = TextInput::new("WorkflowId = 'заказ-заказ-заказ'");
        let offset = input_horizontal_offset(&input, 12);
        assert!(offset > 0);

        let cursor_width = u16::try_from(UnicodeWidthStr::width(input.value.as_str())).unwrap();
        assert!(cursor_width.saturating_sub(offset) < 12);
    }
}
