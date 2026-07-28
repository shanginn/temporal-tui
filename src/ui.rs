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
    app::{
        App, BatchCreateField, BatchCreateForm, ConfirmAction, DeploymentCurrentField,
        DeploymentCurrentForm, DeploymentRampField, DeploymentRampForm, Focus, HandlerField,
        NoticeKind, Overlay, ResetField, ResetForm, ScheduleBackfillField, ScheduleBackfillForm,
        ScheduleConfirmAction, ScheduleCreateField, ScheduleCreateForm, ScheduleEditField,
        ScheduleEditForm, SearchAttributeAddField, SearchAttributeAddForm, SignalField, SignalForm,
        TextInput, View, WorkflowCallForm, WorkflowCallKind,
    },
    model::{FailureSummary, StructuredField, WorkflowCallResult, WorkflowStatus},
};

const MIN_WIDTH: u16 = 58;
const MIN_HEIGHT: u16 = 16;

/// Draw the complete dashboard.
pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        render_too_small(frame, area);
        return;
    }

    let theme = Theme::new(app.color);
    let vertical = Layout::vertical([
        Constraint::Length(4),
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
            Overlay::ScheduleQuery(input) => {
                render_schedule_query(frame, area, input, theme);
            }
            Overlay::TaskQueue(input) => render_task_queue_input(frame, area, input, theme),
            Overlay::SavedQueryPicker { selected } => {
                render_saved_queries(frame, area, app, *selected, theme);
            }
            Overlay::Aggregations { selected } => {
                render_aggregations(frame, area, app, *selected, theme);
            }
            Overlay::NamespacePicker { selected } => {
                render_namespaces(frame, area, app, *selected, theme);
            }
            Overlay::ProfilePicker { selected } => {
                render_profiles(frame, area, app, *selected, theme);
            }
            Overlay::SearchAttributes { selected } => {
                render_search_attributes(frame, area, app, *selected, theme);
            }
            Overlay::SearchAttributeAdd(form) => {
                render_search_attribute_add(frame, area, form, theme);
            }
            Overlay::SearchAttributeRemove { name, input } => {
                render_search_attribute_remove(frame, area, name, input, theme);
            }
            Overlay::DeploymentCurrent(form) => {
                render_deployment_current(frame, area, form, theme);
            }
            Overlay::DeploymentRamp(form) => {
                render_deployment_ramp(frame, area, form, theme);
            }
            Overlay::BatchCreate(form) => render_batch_create(frame, area, form, theme),
            Overlay::BatchConfirm {
                form,
                matched_workflows,
                input,
            } => render_batch_confirmation(frame, area, form, *matched_workflows, input, theme),
            Overlay::BatchStop { job_id, input } => {
                render_batch_stop(frame, area, job_id, input, theme);
            }
            Overlay::Confirm {
                action,
                workflow_id,
                input,
                ..
            } => render_confirmation(frame, area, *action, workflow_id, input, theme),
            Overlay::Signal(form) => render_signal(frame, area, form, theme),
            Overlay::WorkflowCall { kind, form } => {
                render_workflow_call(frame, area, *kind, form, theme);
            }
            Overlay::WorkflowCallResult {
                kind,
                result,
                scroll,
            } => render_workflow_call_result(frame, area, *kind, result, *scroll, theme),
            Overlay::Reset(form) => render_reset(frame, area, form, theme),
            Overlay::ScheduleCreate(form) => render_schedule_create(frame, area, form, theme),
            Overlay::ScheduleEdit(form) => render_schedule_edit(frame, area, form, theme),
            Overlay::ScheduleBackfill(form) => render_schedule_backfill(frame, area, form, theme),
            Overlay::ScheduleConfirm {
                action,
                schedule_id,
                input,
            } => render_schedule_confirmation(frame, area, *action, schedule_id, input, theme),
            Overlay::WorkflowChain { selected } => {
                render_workflow_chain(frame, area, app, *selected, theme);
            }
            Overlay::Inspector { scroll } => {
                render_inspector(frame, area, app, *scroll, theme);
            }
        }
    }
}

fn render_header(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let cluster = if app.switching_profile {
        format!(
            "switching to profile/{}…",
            app.pending_profile_name.as_deref().unwrap_or("unknown")
        )
    } else {
        app.cluster.as_ref().map_or_else(
            || "connecting…".to_string(),
            |cluster| {
                let name = if cluster.cluster_name.is_empty() {
                    "Temporal"
                } else {
                    cluster.cluster_name.as_str()
                };
                format!("{name} {}", cluster.server_version)
            },
        )
    };
    let refresh = if app.auto_refresh {
        format!("auto {}s", app.refresh_interval.as_secs())
    } else {
        "manual".to_string()
    };
    let active_query = if app.view == View::Schedules {
        &app.schedule_query
    } else {
        &app.query
    };
    let query = if app.view == View::Batches {
        "server-side batch jobs".to_string()
    } else if active_query.is_empty() {
        if app.view == View::Schedules {
            "all schedules".to_string()
        } else {
            "all workflows".to_string()
        }
    } else {
        format!("query: {active_query}")
    };
    let tabs = [
        View::Workflows,
        View::TaskQueues,
        View::Workers,
        View::Deployments,
        View::Schedules,
        View::Batches,
    ]
    .into_iter()
    .flat_map(|view| {
        let active = app.view == view;
        [
            Span::styled(
                format!(
                    " {} {} ",
                    view.number(),
                    if area.width < 104 {
                        view.short_label()
                    } else {
                        view.label()
                    }
                ),
                if active {
                    theme.selection()
                } else {
                    theme.key()
                },
            ),
            Span::raw(" "),
        ]
    })
    .collect::<Vec<_>>();
    let status = Line::from(vec![
        Span::styled(
            " ● ",
            if app.switching_profile {
                theme.warning()
            } else {
                theme.success()
            },
        ),
        Span::styled(cluster, theme.strong()),
        Span::raw("  "),
        Span::styled(format!("ns/{}", app.namespace), theme.accent()),
        Span::raw("  "),
        Span::raw(refresh),
        Span::raw("  "),
        Span::styled(
            if app.read_only {
                "READ ONLY"
            } else {
                "CONTROL"
            },
            if app.read_only {
                theme.warning()
            } else {
                theme.success()
            },
        ),
        Span::raw("  "),
        Span::styled(
            if app.codec_enabled {
                "CODEC ON"
            } else {
                "CODEC OFF"
            },
            if app.codec_enabled {
                theme.accent()
            } else {
                theme.muted()
            },
        ),
        Span::raw("  "),
        Span::styled(query, theme.muted()),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.border(false))
        .title(Span::styled(" temporal-tui ", theme.title()))
        .title_bottom(
            Line::from(format!(
                " {}{} ",
                app.profile_name
                    .as_ref()
                    .map_or_else(String::new, |name| format!("profile/{name} · ")),
                app.address
            ))
            .alignment(Alignment::Right)
            .style(theme.muted()),
        );
    frame.render_widget(
        Paragraph::new(Text::from(vec![Line::from(tabs), status])).block(block),
        area,
    );
}

fn render_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    match app.view {
        View::Workflows => render_workflow_dashboard(frame, app, area, theme),
        View::TaskQueues => render_task_queue_dashboard(frame, app, area, theme),
        View::Workers => render_worker_dashboard(frame, app, area, theme),
        View::Deployments => render_deployment_dashboard(frame, app, area, theme),
        View::Schedules => render_schedule_dashboard(frame, app, area, theme),
        View::Batches => render_batch_dashboard(frame, app, area, theme),
    }
}

fn render_workflow_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
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

fn master_detail_areas(area: Rect) -> [Rect; 2] {
    let areas = if area.width >= 106 {
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).split(area)
    } else {
        Layout::vertical([Constraint::Percentage(54), Constraint::Percentage(46)]).split(area)
    };
    [areas[0], areas[1]]
}

fn render_task_queue_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let [list_area, detail_area] = master_detail_areas(area);
    let title = if app.loading_task_queues {
        format!(" Task Queues ({}) ⟳ ", app.task_queues.len())
    } else {
        format!(" Task Queues ({}) ", app.task_queues.len())
    };
    let rows = app.task_queues.iter().map(|queue| {
        let health = task_queue_health(queue, theme);
        Row::new(vec![
            Cell::from(health.0).style(health.1),
            Cell::from(queue.queue_type.label()),
            Cell::from(queue.name.clone()),
            Cell::from(queue.stats.approximate_backlog_count.to_string()),
            Cell::from(format_duration_seconds(
                queue.stats.approximate_backlog_age_seconds,
            )),
            Cell::from(queue.pollers.len().to_string()),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Fill(2),
            Constraint::Length(9),
            Constraint::Length(10),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new([
            "HEALTH",
            "TYPE",
            "TASK QUEUE",
            "BACKLOG",
            "OLDEST",
            "POLLERS",
        ])
        .style(theme.table_header()),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.task_queues.is_empty()).then_some(app.selected_task_queue));
    frame.render_stateful_widget(table, list_area, &mut state);

    let Some(queue) = app.task_queues.get(app.selected_task_queue) else {
        render_empty_panel(
            frame,
            detail_area,
            " Task Queue diagnostics ",
            app.task_queues_error
                .as_deref()
                .unwrap_or("No Task Queues discovered from Workflows or Workers"),
            theme,
        );
        return;
    };
    let health = task_queue_health(queue, theme);
    let current = queue.current_deployment.as_ref().map_or_else(
        || "unversioned".to_string(),
        crate::model::DeploymentVersion::label,
    );
    let ramping = queue.ramping_deployment.as_ref().map_or_else(
        || "none".to_string(),
        |version| format!("{} @ {:.1}%", version.label(), queue.ramping_percentage),
    );
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Health      ", theme.muted()),
            Span::styled(health.0, health.1),
        ]),
        inspector_value("Queue", &queue.name, theme),
        inspector_value("Type", queue.queue_type.label(), theme),
        inspector_value(
            "Backlog",
            &format!(
                "{} · oldest {}",
                queue.stats.approximate_backlog_count,
                format_duration_seconds(queue.stats.approximate_backlog_age_seconds)
            ),
            theme,
        ),
        inspector_value(
            "Rates",
            &format!(
                "{:.2}/s added · {:.2}/s dispatched",
                queue.stats.tasks_add_rate, queue.stats.tasks_dispatch_rate
            ),
            theme,
        ),
        inspector_value("Current", &current, theme),
        inspector_value("Ramping", &ramping, theme),
    ];
    if let Some(limit) = queue.effective_rate_limit {
        lines.push(inspector_value(
            "Rate limit",
            &format!("{limit:.2}/s"),
            theme,
        ));
    }
    lines.push(Line::default());
    lines.push(inspector_section("POLLERS", theme));
    if queue.pollers.is_empty() {
        lines.push(Line::from("No pollers reported").style(theme.warning()));
    } else {
        for poller in &queue.pollers {
            let deployment = if poller.deployment_name.is_empty() {
                "unversioned".to_string()
            } else {
                format!("{}:{}", poller.deployment_name, poller.build_id)
            };
            lines.push(Line::from(vec![
                Span::styled(poller.identity.clone(), theme.strong()),
                Span::raw(format!(
                    " · {} · {:.2}/s · {}",
                    format_time(poller.last_access_time.as_ref()),
                    poller.rate_per_second,
                    deployment
                )),
            ]));
        }
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(" Task Queue diagnostics ", theme.title()))
                    .padding(Padding::horizontal(1)),
            ),
        detail_area,
    );
}

fn task_queue_health(
    queue: &crate::model::TaskQueueSummary,
    theme: Theme,
) -> (&'static str, Style) {
    if queue.pollers.is_empty() && queue.stats.approximate_backlog_count > 0 {
        ("STALLED", theme.error())
    } else if queue.stats.approximate_backlog_count > 0
        && queue.stats.tasks_add_rate > queue.stats.tasks_dispatch_rate
    {
        ("GROWING", theme.warning())
    } else if queue.pollers.is_empty() {
        ("IDLE", theme.muted())
    } else {
        ("HEALTHY", theme.success())
    }
}

fn render_worker_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let [list_area, detail_area] = master_detail_areas(area);
    let title = if app.loading_workers {
        format!(
            " Workers ({}) · page {} ⟳ ",
            app.workers.len(),
            app.worker_page_number
        )
    } else {
        format!(
            " Workers ({}) · page {} ",
            app.workers.len(),
            app.worker_page_number
        )
    };
    let rows = app.workers.iter().map(|worker| {
        Row::new(vec![
            Cell::from(worker.status.clone()).style(worker_status_style(&worker.status, theme)),
            Cell::from(worker.identity.clone()),
            Cell::from(worker.task_queue.clone()),
            Cell::from(format!("{} {}", worker.sdk_name, worker.sdk_version)),
            Cell::from(worker.deployment.as_ref().map_or_else(
                || "unversioned".to_string(),
                crate::model::DeploymentVersion::label,
            )),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Fill(2),
            Constraint::Fill(2),
            Constraint::Length(18),
            Constraint::Fill(2),
        ],
    )
    .header(
        Row::new(["STATUS", "IDENTITY", "TASK QUEUE", "SDK", "DEPLOYMENT"])
            .style(theme.table_header()),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.workers.is_empty()).then_some(app.selected_worker));
    frame.render_stateful_widget(table, list_area, &mut state);

    let Some(details) = &app.worker_details else {
        render_empty_panel(
            frame,
            detail_area,
            " Worker diagnostics ",
            app.workers_error
                .as_deref()
                .unwrap_or(if app.loading_worker_details {
                    "Loading Worker heartbeat diagnostics…"
                } else {
                    "No heartbeat-enabled Workers reported"
                }),
            theme,
        );
        return;
    };
    let worker = &details.summary;
    let mut lines = vec![
        inspector_value("Instance", &worker.instance_key, theme),
        inspector_value("Identity", &worker.identity, theme),
        inspector_value(
            "Host",
            &format!("{} pid {}", worker.host_name, worker.process_id),
            theme,
        ),
        inspector_value("Task queue", &worker.task_queue, theme),
        inspector_value(
            "SDK",
            &format!("{} {}", worker.sdk_name, worker.sdk_version),
            theme,
        ),
        inspector_value(
            "Heartbeat",
            &format!(
                "{} · {:.1}s ago",
                format_time(details.heartbeat_time.as_ref()),
                details.elapsed_since_heartbeat_seconds
            ),
            theme,
        ),
        inspector_value(
            "Host usage",
            &format!(
                "CPU {:.1}% · memory {:.1}%",
                details.host_cpu_usage * 100.0,
                details.host_memory_usage * 100.0
            ),
            theme,
        ),
        inspector_value(
            "Pollers",
            &format!(
                "{} workflow · {} activity · {} nexus",
                details.workflow_pollers, details.activity_pollers, details.nexus_pollers
            ),
            theme,
        ),
        inspector_value(
            "Sticky cache",
            &format!(
                "{} entries · {} hits · {} misses",
                details.sticky_cache_size, details.sticky_cache_hits, details.sticky_cache_misses
            ),
            theme,
        ),
        Line::default(),
        inspector_section("SLOTS", theme),
    ];
    for (label, slots) in [
        ("Workflow", &details.workflow_slots),
        ("Activity", &details.activity_slots),
        ("Local activity", &details.local_activity_slots),
        ("Nexus", &details.nexus_slots),
    ] {
        lines.push(Line::from(format!(
            "{label:<16} used {} · available {} · processed {} · failed {} · {}",
            slots.used, slots.available, slots.processed, slots.failed, slots.supplier
        )));
    }
    if !worker.plugins.is_empty() {
        lines.push(Line::default());
        lines.push(inspector_value(
            "Plugins",
            &worker.plugins.join(", "),
            theme,
        ));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(" Worker diagnostics ", theme.title()))
                    .padding(Padding::horizontal(1)),
            ),
        detail_area,
    );
}

fn render_deployment_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let [list_area, detail_area] = master_detail_areas(area);
    let title = if app.loading_worker_deployments {
        format!(
            " Worker Deployments ({}) · page {} ⟳ ",
            app.worker_deployments.len(),
            app.deployment_page_number
        )
    } else {
        format!(
            " Worker Deployments ({}) · page {} ",
            app.worker_deployments.len(),
            app.deployment_page_number
        )
    };
    let rows = app.worker_deployments.iter().map(|deployment| {
        Row::new(vec![
            Cell::from(deployment.name.clone()),
            Cell::from(deployment.current_version.as_ref().map_or_else(
                || "unversioned".to_string(),
                |version| version.build_id.clone(),
            )),
            Cell::from(
                deployment
                    .ramping_version
                    .as_ref()
                    .map_or_else(|| "—".to_string(), |version| version.build_id.clone()),
            ),
            Cell::from(format!("{:.1}%", deployment.ramping_percentage)),
            Cell::from(format_time(deployment.create_time.as_ref())),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Fill(2),
            Constraint::Fill(2),
            Constraint::Fill(2),
            Constraint::Length(9),
            Constraint::Length(17),
        ],
    )
    .header(
        Row::new(["DEPLOYMENT", "CURRENT", "RAMPING", "TRAFFIC", "CREATED"])
            .style(theme.table_header()),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default().with_selected(
        (!app.worker_deployments.is_empty()).then_some(app.selected_worker_deployment),
    );
    frame.render_stateful_widget(table, list_area, &mut state);

    let Some(details) = &app.worker_deployment_details else {
        render_empty_panel(
            frame,
            detail_area,
            " Deployment routing ",
            app.worker_deployments_error.as_deref().unwrap_or(
                if app.loading_worker_deployment_details {
                    "Loading Worker Deployment routing…"
                } else {
                    "No Worker Deployments reported"
                },
            ),
            theme,
        );
        return;
    };
    let mut lines = vec![
        inspector_value("Name", &details.summary.name, theme),
        inspector_value("Manager", &details.manager_identity, theme),
        inspector_value("Modified by", &details.last_modifier_identity, theme),
        inspector_value("Propagation", &details.routing_update_state, theme),
        inspector_value(
            "Current",
            &details.summary.current_version.as_ref().map_or_else(
                || "unversioned".to_string(),
                crate::model::DeploymentVersion::label,
            ),
            theme,
        ),
        inspector_value(
            "Ramping",
            &details.summary.ramping_version.as_ref().map_or_else(
                || "none".to_string(),
                |version| {
                    format!(
                        "{} @ {:.1}%",
                        version.label(),
                        details.summary.ramping_percentage
                    )
                },
            ),
            theme,
        ),
        Line::default(),
        inspector_section("VERSIONS AND DRAINAGE", theme),
    ];
    if details.versions.is_empty() {
        lines.push(Line::from("No versions tracked").style(theme.muted()));
    }
    for version in &details.versions {
        let routing = if version.is_current {
            "CURRENT".to_string()
        } else if version.is_ramping {
            format!("RAMPING {:.1}%", version.ramp_percentage)
        } else {
            "INACTIVE".to_string()
        };
        lines.push(Line::from(vec![
            Span::styled(version.version.build_id.clone(), theme.strong()),
            Span::raw(format!(
                " · {} · {} · drainage {} · checked {}",
                version.status,
                routing,
                version.drainage_status,
                format_time(version.drainage_last_checked.as_ref())
            )),
        ]));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(" Deployment routing ", theme.title()))
                    .padding(Padding::horizontal(1)),
            ),
        detail_area,
    );
}

fn render_schedule_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let [list_area, detail_area] = master_detail_areas(area);
    let title = if app.loading_schedules {
        format!(
            " Schedules ({}) · page {} ⟳ ",
            app.schedules.len(),
            app.schedule_page_number
        )
    } else {
        format!(
            " Schedules ({}) · page {} ",
            app.schedules.len(),
            app.schedule_page_number
        )
    };
    let rows = app.schedules.iter().map(|schedule| {
        Row::new(vec![
            Cell::from(if schedule.paused { "PAUSED" } else { "ACTIVE" }).style(
                if schedule.paused {
                    theme.warning()
                } else {
                    theme.success()
                },
            ),
            Cell::from(schedule.schedule_id.clone()),
            Cell::from(schedule.workflow_type.clone()),
            Cell::from(format_time(schedule.next_action_time.as_ref())),
            Cell::from(format_time(schedule.recent_action_time.as_ref())),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Fill(2),
            Constraint::Fill(2),
            Constraint::Length(17),
            Constraint::Length(17),
        ],
    )
    .header(
        Row::new(["STATE", "SCHEDULE ID", "WORKFLOW", "NEXT", "LAST"]).style(theme.table_header()),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.schedules.is_empty()).then_some(app.selected_schedule));
    frame.render_stateful_widget(table, list_area, &mut state);

    let Some(details) = &app.schedule_details else {
        render_empty_panel(
            frame,
            detail_area,
            " Schedule definition ",
            app.schedules_error
                .as_deref()
                .unwrap_or(if app.loading_schedule_details {
                    "Loading Schedule definition…"
                } else {
                    "No Schedules reported"
                }),
            theme,
        );
        return;
    };
    let state_label = if details.summary.paused {
        "PAUSED"
    } else {
        "ACTIVE"
    };
    let mut lines = vec![
        Line::from(vec![
            Span::styled("State       ", theme.muted()),
            Span::styled(
                state_label,
                if details.summary.paused {
                    theme.warning()
                } else {
                    theme.success()
                },
            ),
        ]),
        inspector_value("ID", &details.summary.schedule_id, theme),
        inspector_value("Workflow", &details.summary.workflow_type, theme),
        inspector_value("Workflow ID", &details.workflow_id, theme),
        inspector_value("Task queue", &details.task_queue, theme),
        inspector_value("Timezone", &details.timezone, theme),
        inspector_value("Overlap", &details.overlap_policy, theme),
        inspector_value("Catchup", &details.catchup_window, theme),
        inspector_value(
            "Policies",
            &format!(
                "pause-on-failure={} · keep-workflow-id={}",
                details.pause_on_failure, details.keep_original_workflow_id
            ),
            theme,
        ),
        inspector_value(
            "Actions",
            &format!(
                "{} taken · {} running · {} buffered · {} skipped",
                details.action_count,
                details.running_workflows.len(),
                details.buffer_size,
                details.overlap_skipped
            ),
            theme,
        ),
        inspector_value(
            "Next",
            &format_time(details.future_action_times.first()),
            theme,
        ),
    ];
    if !details.summary.notes.is_empty() {
        lines.push(inspector_value("Notes", &details.summary.notes, theme));
    }
    if details.limited_actions {
        lines.push(inspector_value(
            "Remaining",
            &details.remaining_actions.to_string(),
            theme,
        ));
    }
    lines.push(Line::default());
    lines.push(inspector_section("TIMING", theme));
    lines.extend(
        details
            .timing
            .iter()
            .map(|timing| Line::from(timing.clone())),
    );
    append_structured_fields(&mut lines, "INPUT", &details.input, theme);
    append_structured_fields(&mut lines, "MEMO", &details.memo, theme);
    append_structured_fields(
        &mut lines,
        "SEARCH ATTRIBUTES",
        &details.search_attributes,
        theme,
    );
    if !details.recent_actions.is_empty() {
        lines.push(Line::default());
        lines.push(inspector_section("RECENT ACTIONS", theme));
        lines.extend(details.recent_actions.iter().take(5).map(|action| {
            Line::from(format!(
                "{} · {} · {} · {}",
                format_time(action.actual_time.as_ref()),
                action.workflow_id,
                action.run_id,
                action.workflow_status
            ))
        }));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(" Schedule definition ", theme.title()))
                    .padding(Padding::horizontal(1)),
            ),
        detail_area,
    );
}

fn render_batch_dashboard(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let [list_area, detail_area] = master_detail_areas(area);
    let title = format!(
        " Batch operations ({}) · page {}{} ",
        app.batch_operations.len(),
        app.batch_page_number,
        if app.loading_batch_operations {
            " ⟳"
        } else {
            ""
        }
    );
    let rows = app.batch_operations.iter().map(|operation| {
        let state_style = match operation.state.as_str() {
            "RUNNING" => theme.warning(),
            "COMPLETED" => theme.success(),
            "FAILED" => theme.error(),
            _ => theme.muted(),
        };
        Row::new([
            Cell::from(operation.state.clone()).style(state_style),
            Cell::from(operation.job_id.clone()),
            Cell::from(format_time(operation.start_time.as_ref())),
            Cell::from(format_time(operation.close_time.as_ref())),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Fill(2),
            Constraint::Length(17),
            Constraint::Length(17),
        ],
    )
    .header(Row::new(["STATE", "JOB ID", "STARTED", "CLOSED"]).style(theme.table_header()))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(title, theme.title())),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.batch_operations.is_empty()).then_some(app.selected_batch_operation));
    frame.render_stateful_widget(table, list_area, &mut state);

    let Some(details) = &app.batch_operation_details else {
        render_empty_panel(
            frame,
            detail_area,
            " Batch operation ",
            app.batch_operations_error.as_deref().unwrap_or(
                if app.loading_batch_operation_details {
                    "Loading batch operation…"
                } else {
                    "No batch operations reported"
                },
            ),
            theme,
        );
        return;
    };
    let progress = if details.total_operation_count > 0 {
        let complete = i128::from(details.complete_operation_count.max(0));
        let total = i128::from(details.total_operation_count);
        let percentage_tenths = complete.saturating_mul(1_000) / total;
        format!("{}.{}%", percentage_tenths / 10, percentage_tenths % 10)
    } else {
        "—".to_string()
    };
    let lines = vec![
        inspector_value("Job ID", &details.summary.job_id, theme),
        inspector_value("State", &details.summary.state, theme),
        inspector_value("Operation", &details.operation_type, theme),
        inspector_value("Started", &format_time(details.summary.start_time.as_ref()), theme),
        inspector_value("Closed", &format_time(details.summary.close_time.as_ref()), theme),
        inspector_value("Progress", &progress, theme),
        inspector_value(
            "Counts",
            &format!(
                "{} complete · {} failed · {} total",
                details.complete_operation_count,
                details.failure_operation_count,
                details.total_operation_count
            ),
            theme,
        ),
        inspector_value("Identity", &details.identity, theme),
        inspector_value("Reason", &details.reason, theme),
        Line::default(),
        Line::from("Server-side batchers evaluate the frozen Visibility query; the TUI never expands targets client-side.")
            .style(theme.muted()),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(" Batch operation ", theme.title()))
                    .padding(Padding::horizontal(1)),
            ),
        detail_area,
    );
}

fn render_empty_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    message: &str,
    theme: Theme,
) {
    frame.render_widget(
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(theme.muted())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme.border(false))
                    .title(Span::styled(title, theme.title()))
                    .padding(Padding::vertical(1)),
            ),
        area,
    );
}

fn worker_status_style(status: &str, theme: Theme) -> Style {
    if status.contains("RUNNING") {
        theme.success()
    } else if status.contains("STOPPED") || status.contains("FAILED") {
        theme.error()
    } else {
        theme.warning()
    }
}

fn render_workflows(frame: &mut Frame<'_>, app: &App, area: Rect, theme: Theme) {
    let count = app.workflow_count.as_ref().map_or_else(
        || app.workflows.len().to_string(),
        |count| format!("≈{}", count.total),
    );
    let title = if app.loading_workflows {
        format!(" Workflows ({count}) · page {} ⟳ ", app.page_number)
    } else {
        format!(" Workflows ({count}) · page {} ", app.page_number)
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
                if details.history_next_page_token.is_empty() {
                    format!(" History ({}) ", details.events.len())
                } else {
                    format!(
                        " History (loaded {} of {} · H older) ",
                        details.events.len(),
                        details.summary.history_length
                    )
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
        let mut spans = vec![
            key_hint("1-6", "views", theme),
            Span::raw("  "),
            key_hint("j/k", "move", theme),
            Span::raw("  "),
        ];
        match app.view {
            View::Workflows => spans.extend([
                key_hint("tab", "pane", theme),
                Span::raw("  "),
                key_hint("/", "query", theme),
                Span::raw("  "),
                key_hint("[/]", "pages", theme),
                Span::raw("  "),
                key_hint("v", "inspect", theme),
                Span::raw("  "),
                key_hint("e/o", "export/web", theme),
                Span::raw("  "),
            ]),
            View::TaskQueues => spans.extend([key_hint("/", "queue name", theme), Span::raw("  ")]),
            View::Workers => {
                spans.extend([key_hint("[/]", "pages", theme), Span::raw("  ")]);
            }
            View::Deployments => spans.extend([
                key_hint("C/R", "current/ramp", theme),
                Span::raw("  "),
                key_hint("[/]", "pages", theme),
                Span::raw("  "),
            ]),
            View::Schedules => spans.extend([
                key_hint("/", "query", theme),
                Span::raw("  "),
                key_hint("N/E", "new/edit", theme),
                Span::raw("  "),
                key_hint("p/t/b/d", "control", theme),
                Span::raw("  "),
                key_hint("[/]", "pages", theme),
                Span::raw("  "),
            ]),
            View::Batches => spans.extend([
                key_hint("N", "new", theme),
                Span::raw("  "),
                key_hint("s", "stop", theme),
                Span::raw("  "),
                key_hint("[/]", "pages", theme),
                Span::raw("  "),
            ]),
        }
        spans.extend([
            key_hint("r", "refresh", theme),
            Span::raw("  "),
            key_hint("n", "namespace", theme),
            Span::raw("  "),
            key_hint("P", "profile", theme),
            Span::raw("  "),
            key_hint("?", "help", theme),
            Span::raw("  "),
            key_hint("q", "quit", theme),
        ]);
        Line::from(spans)
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn render_help(frame: &mut Frame<'_>, area: Rect, theme: Theme) {
    let popup = centered(area, 88, 42);
    frame.render_widget(Clear, popup);
    let help = Text::from(vec![
        help_section("VIEWS", theme),
        help_line("1", "Workflows and complete history", theme),
        help_line(
            "2",
            "Task Queue backlog, throughput, routing, pollers",
            theme,
        ),
        help_line("3", "heartbeat-enabled Worker runtime diagnostics", theme),
        help_line("4", "GA Worker Deployment routing and drainage", theme),
        help_line("5", "Schedule visibility, definition, and control", theme),
        help_line("6", "server-side Batch Operation jobs and progress", theme),
        help_line(
            "/",
            "query Workflows or inspect a Task Queue by name",
            theme,
        ),
        Line::default(),
        help_section("NAVIGATION", theme),
        help_line("j / ↓", "next workflow or history event", theme),
        help_line("k / ↑", "previous workflow or history event", theme),
        help_line("g / G", "first / last item", theme),
        help_line("tab / enter", "switch workflow and history panes", theme),
        help_line("[ / ]", "previous / next workflow page", theme),
        Line::default(),
        help_section("DATA", theme),
        help_line("/", "edit Temporal visibility query", theme),
        help_line("f", "select a saved visibility query", theme),
        help_line("#", "show GROUP BY visibility counts", theme),
        help_line("n", "switch namespace", theme),
        help_line(
            "A",
            "inspect and manage the Search Attribute registry",
            theme,
        ),
        help_line("P", "switch configured Temporal connection profile", theme),
        help_line("r", "refresh now", theme),
        help_line("a", "toggle automatic refresh", theme),
        help_line("H", "load the next older history page", theme),
        help_line("C", "show all runs in this workflow chain", theme),
        help_line(
            "v",
            "inspect payloads, failures, memo, and attributes",
            theme,
        ),
        help_line("y", "copy workflow and run IDs", theme),
        help_line("e / o", "safe JSON export / open Temporal Web UI", theme),
        Line::default(),
        help_section("CONTROL", theme),
        help_line("s", "send a named signal with JSON input", theme),
        help_line("Q / U", "invoke a Workflow Query / Update handler", theme),
        help_line("p", "pause or unpause the selected Workflow", theme),
        help_line("R", "reset at a valid Workflow Task event boundary", theme),
        help_line("c", "request graceful workflow cancellation", theme),
        help_line("x", "terminate workflow immediately", theme),
        Line::default(),
        help_section("SCHEDULE CONTROL", theme),
        help_line("/", "edit the Schedule visibility query", theme),
        help_line("N / E", "create / safely edit a Schedule", theme),
        help_line("p", "pause or unpause a Schedule", theme),
        help_line("t / b", "trigger now / backfill a time range", theme),
        help_line("d", "delete with exact Schedule ID confirmation", theme),
        Line::default(),
        help_section("WORKER DEPLOYMENT CONTROL", theme),
        help_line("C", "set or promote the Current build ID", theme),
        help_line(
            "R",
            "set or clear the Ramping build ID and percentage",
            theme,
        ),
        Line::default(),
        help_section("BATCH CONTROL", theme),
        help_line(
            "N",
            "preview and start a server-side batch operation",
            theme,
        ),
        help_line(
            "s",
            "stop a running batch job with exact-ID confirmation",
            theme,
        ),
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

fn render_schedule_query(frame: &mut Frame<'_>, area: Rect, input: &TextInput, theme: Theme) {
    let popup = centered(area, 82, 5);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.accent())
        .title(Span::styled(" Schedule visibility query ", theme.title()))
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

fn render_task_queue_input(frame: &mut Frame<'_>, area: Rect, input: &TextInput, theme: Theme) {
    let popup = centered(area, 72, 5);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme.accent())
        .title(Span::styled(" Inspect Task Queue by name ", theme.title()))
        .title_bottom(
            Line::from(" enter load · esc cancel ")
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

fn render_saved_queries(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    selected: usize,
    theme: Theme,
) {
    let visible = app.saved_queries.len().clamp(3, 18);
    let popup = centered(
        area,
        92,
        u16::try_from(visible).unwrap_or(18).saturating_add(4),
    );
    frame.render_widget(Clear, popup);
    let items = app.saved_queries.iter().map(|filter| {
        ListItem::new(Line::from(vec![
            Span::styled(format!("{:<24}", filter.name), theme.strong()),
            Span::styled(&filter.query, theme.muted()),
        ]))
    });
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent())
                .title(Span::styled(" Saved visibility queries ", theme.title()))
                .title_bottom(
                    Line::from(" enter apply · esc cancel ")
                        .alignment(Alignment::Right)
                        .style(theme.muted()),
                ),
        )
        .highlight_style(theme.selection())
        .highlight_symbol("› ");
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, popup, &mut state);
}

fn render_aggregations(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    selected: usize,
    theme: Theme,
) {
    let groups = app
        .workflow_count
        .as_ref()
        .map_or(&[][..], |count| count.groups.as_slice());
    let visible = groups.len().clamp(3, 20);
    let popup = centered(
        area,
        84,
        u16::try_from(visible).unwrap_or(20).saturating_add(5),
    );
    frame.render_widget(Clear, popup);
    let rows = groups.iter().map(|group| {
        Row::new(vec![
            Cell::from(group.values.join(" · ")),
            Cell::from(group.count.to_string()),
        ])
    });
    let table = Table::new(rows, [Constraint::Fill(1), Constraint::Length(14)])
        .header(Row::new(["GROUP VALUES", "COUNT"]).style(theme.table_header()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent())
                .title(Span::styled(" Visibility aggregation ", theme.title()))
                .title_bottom(
                    Line::from(" j/k navigate · esc close ")
                        .alignment(Alignment::Right)
                        .style(theme.muted()),
                ),
        )
        .row_highlight_style(theme.selection())
        .highlight_symbol("› ");
    let mut state = TableState::default().with_selected((!groups.is_empty()).then_some(selected));
    frame.render_stateful_widget(table, popup, &mut state);
}

fn render_profiles(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize, theme: Theme) {
    let visible = app.profiles.len().clamp(3, 20);
    let popup = centered(
        area,
        104,
        u16::try_from(visible).unwrap_or(20).saturating_add(5),
    );
    frame.render_widget(Clear, popup);
    let rows = app.profiles.iter().map(|profile| {
        let active = app.profile_name.as_deref() == Some(profile.name.as_str());
        let marker = if active {
            "ACTIVE"
        } else if profile.is_default {
            "DEFAULT"
        } else {
            ""
        };
        Row::new([
            marker,
            profile.name.as_str(),
            profile.address.as_str(),
            profile.namespace.as_str(),
            if profile.read_only {
                "READ ONLY"
            } else {
                "CONTROL"
            },
            if profile.codec_enabled {
                "CODEC"
            } else {
                "PLAIN"
            },
        ])
        .style(if active {
            theme.strong()
        } else {
            theme.muted()
        })
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Fill(2),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(10),
            Constraint::Length(7),
        ],
    )
    .header(
        Row::new([
            "STATE",
            "PROFILE",
            "ADDRESS",
            "NAMESPACE",
            "MODE",
            "PAYLOAD",
        ])
        .style(theme.table_header()),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(" Switch Temporal profile ", theme.title()))
            .title_bottom(
                Line::from(" enter connect · secrets resolve only after selection · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state =
        TableState::default().with_selected((!app.profiles.is_empty()).then_some(selected));
    frame.render_stateful_widget(table, popup, &mut state);
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

fn render_search_attributes(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    selected: usize,
    theme: Theme,
) {
    let visible = app.search_attributes.len().clamp(8, 24);
    let popup = centered(
        area,
        104,
        u16::try_from(visible).unwrap_or(24).saturating_add(5),
    );
    frame.render_widget(Clear, popup);
    let rows = app.search_attributes.iter().map(|attribute| {
        Row::new([
            if attribute.custom { "CUSTOM" } else { "SYSTEM" },
            attribute.name.as_str(),
            attribute.value_type.as_str(),
            attribute.storage_type.as_str(),
        ])
        .style(if attribute.custom {
            theme.strong()
        } else {
            theme.muted()
        })
    });
    let status = if app.loading_search_attributes {
        "loading…".to_string()
    } else if let Some(error) = &app.search_attributes_error {
        format!("unavailable: {error}")
    } else {
        format!("{} entries", app.search_attributes.len())
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Fill(3),
            Constraint::Length(18),
            Constraint::Fill(2),
        ],
    )
    .header(Row::new(["SCOPE", "NAME", "VALUE TYPE", "STORAGE"]).style(theme.table_header()))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(
                format!(" Search Attributes · {} · {status} ", app.namespace),
                theme.title(),
            ))
            .title_bottom(
                Line::from(" a add · d remove custom · r refresh · esc close ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state = TableState::default()
        .with_selected((!app.search_attributes.is_empty()).then_some(selected));
    frame.render_stateful_widget(table, popup, &mut state);
}

fn render_search_attribute_add(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &SearchAttributeAddForm,
    theme: Theme,
) {
    let popup = centered(area, 88, 16);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(" Register Search Attribute ", theme.title()))
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
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    let name_offset = render_form_input(
        frame,
        areas[0],
        " Name ",
        &form.name,
        form.active_field == SearchAttributeAddField::Name,
        theme,
    );
    let type_offset = render_form_input(
        frame,
        areas[1],
        " Type: Text | Keyword | Int | Double | Bool | Datetime | KeywordList ",
        &form.value_type,
        form.active_field == SearchAttributeAddField::ValueType,
        theme,
    );
    let confirmation_offset = render_form_input(
        frame,
        areas[2],
        " Type the exact name to confirm ",
        &form.confirmation,
        form.active_field == SearchAttributeAddField::Confirmation,
        theme,
    );
    frame.render_widget(
        Paragraph::new("Registration mutates the namespace Search Attribute schema.")
            .style(theme.warning()),
        areas[3],
    );
    match form.active_field {
        SearchAttributeAddField::Name => {
            set_input_cursor(frame, areas[0], &form.name, name_offset);
        }
        SearchAttributeAddField::ValueType => {
            set_input_cursor(frame, areas[1], &form.value_type, type_offset);
        }
        SearchAttributeAddField::Confirmation => {
            set_input_cursor(frame, areas[2], &form.confirmation, confirmation_offset);
        }
    }
}

fn render_search_attribute_remove(
    frame: &mut Frame<'_>,
    area: Rect,
    name: &str,
    input: &TextInput,
    theme: Theme,
) {
    let popup = centered(area, 84, 11);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.error())
            .title(Span::styled(" Remove Search Attribute ", theme.error()))
            .title_bottom(
                Line::from(" enter remove · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("Remove custom attribute "),
            Span::styled(name, theme.strong()),
            Span::raw("?"),
        ])),
        areas[0],
    );
    let offset = render_form_input(
        frame,
        areas[1],
        " Exact attribute name ",
        input,
        true,
        theme,
    );
    frame.render_widget(
        Paragraph::new("Existing indexed values may become unavailable to Visibility queries.")
            .style(theme.warning()),
        areas[2],
    );
    set_input_cursor(frame, areas[1], input, offset);
}

fn render_deployment_current(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &DeploymentCurrentForm,
    theme: Theme,
) {
    let popup = centered(area, 90, 15);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(
                format!(" Set Current · {} ", form.deployment_name),
                theme.title(),
            ))
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
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new("Blank build ID routes Current traffic to unversioned Workers.")
            .style(theme.muted()),
        areas[0],
    );
    let build_offset = render_form_input(
        frame,
        areas[1],
        " Current build ID ",
        &form.build_id,
        form.active_field == DeploymentCurrentField::BuildId,
        theme,
    );
    let confirmation_offset = render_form_input(
        frame,
        areas[2],
        " Type the exact Deployment name ",
        &form.confirmation,
        form.active_field == DeploymentCurrentField::Confirmation,
        theme,
    );
    frame.render_widget(
        Paragraph::new(
            "Uses a fresh conflict token; missing Task Queue and no-poller safety checks remain \
             enabled.",
        )
        .style(theme.warning())
        .wrap(Wrap { trim: true }),
        areas[3],
    );
    match form.active_field {
        DeploymentCurrentField::BuildId => {
            set_input_cursor(frame, areas[1], &form.build_id, build_offset);
        }
        DeploymentCurrentField::Confirmation => {
            set_input_cursor(frame, areas[2], &form.confirmation, confirmation_offset);
        }
    }
}

fn render_deployment_ramp(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &DeploymentRampForm,
    theme: Theme,
) {
    let popup = centered(area, 90, 18);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(
                format!(" Configure Ramp · {} ", form.deployment_name),
                theme.title(),
            ))
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
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new("Use blank build ID with 0% to clear the Ramping Version.")
            .style(theme.muted()),
        areas[0],
    );
    let build_offset = render_form_input(
        frame,
        areas[1],
        " Ramping build ID ",
        &form.build_id,
        form.active_field == DeploymentRampField::BuildId,
        theme,
    );
    let percentage_offset = render_form_input(
        frame,
        areas[2],
        " Percentage 0-100 ",
        &form.percentage,
        form.active_field == DeploymentRampField::Percentage,
        theme,
    );
    let confirmation_offset = render_form_input(
        frame,
        areas[3],
        " Type the exact Deployment name ",
        &form.confirmation,
        form.active_field == DeploymentRampField::Confirmation,
        theme,
    );
    frame.render_widget(
        Paragraph::new("The server's missing-queue and no-poller protections are never bypassed.")
            .style(theme.warning()),
        areas[4],
    );
    match form.active_field {
        DeploymentRampField::BuildId => {
            set_input_cursor(frame, areas[1], &form.build_id, build_offset);
        }
        DeploymentRampField::Percentage => {
            set_input_cursor(frame, areas[2], &form.percentage, percentage_offset);
        }
        DeploymentRampField::Confirmation => {
            set_input_cursor(frame, areas[3], &form.confirmation, confirmation_offset);
        }
    }
}

fn render_batch_create(frame: &mut Frame<'_>, area: Rect, form: &BatchCreateForm, theme: Theme) {
    let popup = centered(area, 108, 22);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(" Preview Batch Operation ", theme.title()))
            .title_bottom(
                Line::from(" enter next/preview · tab field · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(inner);
    let left = Layout::vertical([Constraint::Length(3); 4])
        .spacing(1)
        .split(columns[0]);
    let right = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .spacing(1)
    .split(columns[1]);
    let fields = [
        (
            BatchCreateField::JobId,
            left[0],
            " Unique Job ID ",
            &form.job_id,
        ),
        (
            BatchCreateField::Operation,
            right[0],
            " cancel | terminate | signal | delete ",
            &form.operation,
        ),
        (
            BatchCreateField::VisibilityQuery,
            left[1],
            " Non-empty Visibility query ",
            &form.visibility_query,
        ),
        (
            BatchCreateField::Reason,
            right[1],
            " Audit reason ",
            &form.reason,
        ),
        (
            BatchCreateField::MaxOperationsPerSecond,
            left[2],
            " Max operations/sec (0 = server default) ",
            &form.max_operations_per_second,
        ),
        (
            BatchCreateField::SignalName,
            right[2],
            " Signal name (signal only) ",
            &form.signal_name,
        ),
        (
            BatchCreateField::SignalInput,
            left[3],
            " Signal JSON input ",
            &form.signal_input,
        ),
    ];
    for (field, field_area, label, input) in fields {
        let offset = render_form_input(
            frame,
            field_area,
            label,
            input,
            form.active_field == field,
            theme,
        );
        if form.active_field == field {
            set_input_cursor(frame, field_area, input, offset);
        }
    }
    frame.render_widget(
        Paragraph::new(
            "Preview counts matching Workflows through Temporal before any job can start. Targets \
             stay server-side and are never enumerated by the TUI.",
        )
        .style(theme.warning())
        .wrap(Wrap { trim: true }),
        right[3],
    );
}

fn render_batch_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &BatchCreateForm,
    matched_workflows: i64,
    input: &TextInput,
    theme: Theme,
) {
    let popup = centered(area, 104, 17);
    frame.render_widget(Clear, popup);
    let operation = form.operation.value.trim().to_ascii_uppercase();
    let dangerous = matches!(operation.as_str(), "TERMINATE" | "DELETE");
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(if dangerous {
                theme.error()
            } else {
                theme.warning()
            })
            .title(Span::styled(
                " Confirm server-side Batch Operation ",
                if dangerous {
                    theme.error()
                } else {
                    theme.title()
                },
            ))
            .title_bottom(
                Line::from(" enter start · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(operation, theme.strong()),
            Span::raw(" will target "),
            Span::styled(matched_workflows.to_string(), theme.warning()),
            Span::raw(" matching Workflow Executions."),
        ])),
        areas[0],
    );
    frame.render_widget(
        Paragraph::new(form.visibility_query.value.as_str())
            .style(theme.muted())
            .wrap(Wrap { trim: false }),
        areas[1],
    );
    frame.render_widget(
        Paragraph::new(format!(
            "Job ID: {} · max {}/sec",
            form.job_id.value.trim(),
            form.max_operations_per_second.value.trim()
        )),
        areas[2],
    );
    let offset = render_form_input(
        frame,
        areas[3],
        " Type the exact Job ID ",
        input,
        true,
        theme,
    );
    frame.render_widget(
        Paragraph::new(
            "This starts one Temporal server-side batch job; it does not issue per-Workflow \
             client calls.",
        )
        .style(if dangerous {
            theme.error()
        } else {
            theme.warning()
        })
        .wrap(Wrap { trim: true }),
        areas[4],
    );
    set_input_cursor(frame, areas[3], input, offset);
}

fn render_batch_stop(
    frame: &mut Frame<'_>,
    area: Rect,
    job_id: &str,
    input: &TextInput,
    theme: Theme,
) {
    let popup = centered(area, 84, 11);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(" Stop Batch Operation ", theme.title()))
            .title_bottom(
                Line::from(" enter stop · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let areas = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(1),
    ])
    .spacing(1)
    .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("Stop running job "),
            Span::styled(job_id, theme.strong()),
            Span::raw("?"),
        ])),
        areas[0],
    );
    let offset = render_form_input(frame, areas[1], " Exact Job ID ", input, true, theme);
    frame.render_widget(
        Paragraph::new("Already completed per-Workflow operations are not rolled back.")
            .style(theme.warning()),
        areas[2],
    );
    set_input_cursor(frame, areas[1], input, offset);
}

fn render_workflow_chain(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    selected: usize,
    theme: Theme,
) {
    let visible = app.workflow_chain.len().clamp(3, 20);
    let popup = centered(
        area,
        104,
        u16::try_from(visible).unwrap_or(20).saturating_add(5),
    );
    frame.render_widget(Clear, popup);
    let rows = app.workflow_chain.iter().map(|workflow| {
        Row::new(vec![
            Cell::from(workflow.status.label()).style(theme.workflow_status(workflow.status)),
            Cell::from(workflow.key.run_id.clone()),
            Cell::from(format_time(workflow.start_time.as_ref())),
            Cell::from(format_count(workflow.history_length)),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Fill(3),
            Constraint::Length(17),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(["STATUS", "RUN ID", "STARTED", "EVENTS"]).style(theme.table_header()))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(
                format!(" Workflow chain ({}) ", app.workflow_chain.len()),
                theme.title(),
            ))
            .title_bottom(
                Line::from(" j/k navigate · esc close ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
    )
    .row_highlight_style(theme.selection())
    .highlight_symbol("› ");
    let mut state =
        TableState::default().with_selected((!app.workflow_chain.is_empty()).then_some(selected));
    frame.render_stateful_widget(table, popup, &mut state);
}

fn render_inspector(frame: &mut Frame<'_>, area: Rect, app: &App, scroll: u16, theme: Theme) {
    let popup = centered(area, 112, area.height.saturating_sub(4).clamp(12, 44));
    frame.render_widget(Clear, popup);
    let mut lines = Vec::new();
    if let Some(details) = &app.details {
        lines.push(inspector_section("WORKFLOW", theme));
        lines.push(inspector_value("First run", &details.first_run_id, theme));
        if let Some(parent) = &details.parent_workflow_id {
            lines.push(inspector_value("Parent", parent, theme));
        }
        if let Some(root) = &details.root_workflow_id {
            lines.push(inspector_value("Root", root, theme));
        }
        if let Some(reset) = &details.reset_run_id {
            lines.push(inspector_value("Reset run", reset, theme));
        }
        if let Some(static_details) = &details.static_details {
            lines.push(inspector_value("Details", static_details, theme));
        }
        if details.cancel_requested {
            lines.push(Line::from("Cancellation has been requested").style(theme.warning()));
        }
        append_structured_fields(&mut lines, "MEMO", &details.memo, theme);
        append_structured_fields(
            &mut lines,
            "SEARCH ATTRIBUTES",
            &details.search_attributes,
            theme,
        );

        if !details.pending_activity_details.is_empty() {
            lines.push(Line::default());
            lines.push(inspector_section("PENDING ACTIVITIES", theme));
            for activity in &details.pending_activity_details {
                lines.push(Line::from(vec![
                    Span::styled(activity.activity_id.clone(), theme.strong()),
                    Span::raw(" · "),
                    Span::raw(activity.activity_type.clone()),
                    Span::raw(" · "),
                    Span::styled(activity.state.clone(), theme.warning()),
                    Span::raw(format!(" · attempt {}", activity.attempt)),
                ]));
                if let Some(failure) = &activity.last_failure {
                    append_failure(&mut lines, failure, 1, theme);
                }
            }
        }

        if let Some(event) = details.events.get(app.selected_event) {
            lines.push(Line::default());
            lines.push(inspector_section(
                &format!("EVENT {} · {}", event.event_id, event.event_type),
                theme,
            ));
            if !event.detail.is_empty() {
                lines.push(inspector_value("Detail", &event.detail, theme));
            }
            for field in &event.fields {
                append_structured_field(&mut lines, field, theme);
            }
            if let Some(failure) = &event.failure {
                lines.push(Line::default());
                lines.push(inspector_section("FAILURE", theme));
                append_failure(&mut lines, failure, 0, theme);
            }
        }
    }
    if lines.is_empty() {
        lines.push(Line::from("No workflow detail is available").style(theme.muted()));
    }
    let paragraph = Paragraph::new(Text::from(lines))
        .scroll((scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme.accent())
                .title(Span::styled(" Workflow inspector ", theme.title()))
                .title_bottom(
                    Line::from(" j/k or page up/down scroll · esc close ")
                        .alignment(Alignment::Right)
                        .style(theme.muted()),
                )
                .padding(Padding::horizontal(1)),
        );
    frame.render_widget(paragraph, popup);
}

fn append_structured_fields(
    lines: &mut Vec<Line<'static>>,
    title: &str,
    fields: &[StructuredField],
    theme: Theme,
) {
    if fields.is_empty() {
        return;
    }
    lines.push(Line::default());
    lines.push(inspector_section(title, theme));
    for field in fields {
        append_structured_field(lines, field, theme);
    }
}

fn append_structured_field(lines: &mut Vec<Line<'static>>, field: &StructuredField, theme: Theme) {
    lines.push(Line::from(vec![
        Span::styled(field.name.clone(), theme.strong()),
        Span::styled(
            format!("  {} · {} bytes", field.encoding, field.size_bytes),
            theme.muted(),
        ),
    ]));
    lines.extend(field.value.lines().map(|line| {
        Line::from(format!("  {line}")).style(if field.redacted {
            theme.warning()
        } else {
            theme.muted()
        })
    }));
}

fn append_failure(
    lines: &mut Vec<Line<'static>>,
    failure: &FailureSummary,
    depth: usize,
    theme: Theme,
) {
    let indent = "  ".repeat(depth);
    lines.push(Line::from(vec![
        Span::styled(format!("{indent}{}", failure.kind), theme.error()),
        Span::raw(if failure.source.is_empty() {
            String::new()
        } else {
            format!(" · {}", failure.source)
        }),
    ]));
    lines.push(Line::from(format!("{indent}{}", failure.message)));
    if let Some(attributes) = &failure.encoded_attributes {
        append_structured_field(lines, attributes, theme);
    }
    if !failure.stack_trace.is_empty() {
        lines.extend(
            failure
                .stack_trace
                .lines()
                .map(|line| Line::from(format!("{indent}  {line}")).style(theme.muted())),
        );
    }
    if let Some(cause) = &failure.cause {
        lines.push(Line::from(format!("{indent}Caused by:")).style(theme.muted()));
        append_failure(lines, cause, depth.saturating_add(1), theme);
    }
}

fn inspector_section(title: &str, theme: Theme) -> Line<'static> {
    Line::from(title.to_string()).style(theme.title())
}

fn inspector_value(label: &str, value: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), theme.muted()),
        Span::raw(value.to_string()),
    ])
}

fn render_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    action: ConfirmAction,
    workflow_id: &str,
    input: &TextInput,
    theme: Theme,
) {
    let popup = centered(area, 80, 11);
    frame.render_widget(Clear, popup);
    let severity = match action {
        ConfirmAction::Cancel | ConfirmAction::Pause | ConfirmAction::Unpause => theme.warning(),
        ConfirmAction::Terminate => theme.error(),
    };
    let warning = match action {
        ConfirmAction::Cancel => {
            "The workflow may handle cancellation and perform cleanup before closing."
        }
        ConfirmAction::Terminate => {
            "Termination is immediate. Workflow code cannot intercept or clean up."
        }
        ConfirmAction::Pause => {
            "Pausing stops new Workflow Tasks until the execution is explicitly unpaused."
        }
        ConfirmAction::Unpause => {
            "Unpausing allows the Workflow Execution to resume processing Workflow Tasks."
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
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(severity)
        .title(Span::styled(" Confirmation ", severity))
        .title_bottom(
            Line::from(" type Workflow ID exactly · enter confirm · esc cancel ")
                .alignment(Alignment::Right)
                .style(theme.muted()),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let areas = Layout::vertical([Constraint::Length(4), Constraint::Length(3)]).split(inner);
    frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: true }), areas[0]);
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(severity)
        .title(Span::styled(" Workflow ID ", theme.strong()));
    let horizontal_offset = input_horizontal_offset(input, areas[1].width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .scroll((0, horizontal_offset))
            .block(input_block),
        areas[1],
    );
    set_input_cursor(frame, areas[1], input, horizontal_offset);
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

fn render_workflow_call(
    frame: &mut Frame<'_>,
    area: Rect,
    kind: WorkflowCallKind,
    form: &WorkflowCallForm,
    theme: Theme,
) {
    let popup = centered(area, 82, 10);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(
                format!(" Invoke Workflow {} ", kind.label()),
                theme.title(),
            ))
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
    let name_offset = render_form_input(
        frame,
        fields[0],
        " Handler name ",
        &form.name,
        form.active_field == HandlerField::Name,
        theme,
    );
    let input_offset = render_form_input(
        frame,
        fields[1],
        " JSON arguments [] ",
        &form.input,
        form.active_field == HandlerField::Input,
        theme,
    );
    match form.active_field {
        HandlerField::Name => set_input_cursor(frame, fields[0], &form.name, name_offset),
        HandlerField::Input => set_input_cursor(frame, fields[1], &form.input, input_offset),
    }
}

fn render_workflow_call_result(
    frame: &mut Frame<'_>,
    area: Rect,
    kind: WorkflowCallKind,
    result: &WorkflowCallResult,
    scroll: u16,
    theme: Theme,
) {
    let popup = centered(area, 104, area.height.saturating_sub(6).clamp(14, 38));
    frame.render_widget(Clear, popup);
    let mut lines = vec![inspector_value("Handler", &result.handler, theme)];
    if let Some(update_id) = &result.update_id {
        lines.push(inspector_value("Update ID", update_id, theme));
    }
    append_structured_fields(&mut lines, "RESULT", &result.fields, theme);
    if result.fields.is_empty() && result.failure.is_none() {
        lines.push(Line::from("Handler returned no payloads").style(theme.muted()));
    }
    if let Some(failure) = &result.failure {
        lines.push(Line::default());
        lines.push(inspector_section("FAILURE", theme));
        append_failure(&mut lines, failure, 0, theme);
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((scroll, 0))
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(if result.failure.is_some() {
                        theme.error()
                    } else {
                        theme.accent()
                    })
                    .title(Span::styled(
                        format!(" Workflow {} result ", kind.label()),
                        theme.title(),
                    ))
                    .title_bottom(
                        Line::from(" j/k or page up/down scroll · esc close ")
                            .alignment(Alignment::Right)
                            .style(theme.muted()),
                    )
                    .padding(Padding::horizontal(1)),
            ),
        popup,
    );
}

fn render_reset(frame: &mut Frame<'_>, area: Rect, form: &ResetForm, theme: Theme) {
    let popup = centered(area, 88, 15);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.error())
            .title(Span::styled(" Reset Workflow Execution ", theme.error()))
            .title_bottom(
                Line::from(" enter next/reset · tab field · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(2),
    ])
    .spacing(1)
    .split(inner);
    let event_offset = render_form_input(
        frame,
        areas[0],
        " Workflow Task event ID ",
        &form.event_id,
        form.active_field == ResetField::EventId,
        theme,
    );
    let confirmation_offset = render_form_input(
        frame,
        areas[1],
        " Exact Workflow ID ",
        &form.confirmation,
        form.active_field == ResetField::Confirmation,
        theme,
    );
    frame.render_widget(
        Paragraph::new(
            "Reset terminates the current run and starts a new run from a valid Workflow Task \
             completed/failed/timed-out/started boundary.",
        )
        .style(theme.warning())
        .wrap(Wrap { trim: true }),
        areas[2],
    );
    match form.active_field {
        ResetField::EventId => set_input_cursor(frame, areas[0], &form.event_id, event_offset),
        ResetField::Confirmation => {
            set_input_cursor(frame, areas[1], &form.confirmation, confirmation_offset);
        }
    }
}

fn render_schedule_create(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &ScheduleCreateForm,
    theme: Theme,
) {
    let popup = centered(area, 108, 20);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.accent())
            .title(Span::styled(" Create Schedule ", theme.title()))
            .title_bottom(
                Line::from(" enter next/create · tab/backtab field · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let columns = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(inner);
    let left = Layout::vertical([Constraint::Length(3); 4])
        .spacing(1)
        .split(columns[0]);
    let right = Layout::vertical([Constraint::Length(3); 4])
        .spacing(1)
        .split(columns[1]);
    let fields = [
        (
            ScheduleCreateField::ScheduleId,
            left[0],
            " Schedule ID ",
            &form.schedule_id,
        ),
        (
            ScheduleCreateField::WorkflowId,
            right[0],
            " Workflow ID ",
            &form.workflow_id,
        ),
        (
            ScheduleCreateField::WorkflowType,
            left[1],
            " Workflow type ",
            &form.workflow_type,
        ),
        (
            ScheduleCreateField::TaskQueue,
            right[1],
            " Task Queue ",
            &form.task_queue,
        ),
        (
            ScheduleCreateField::Expression,
            left[2],
            " Cron / @every expression ",
            &form.expression,
        ),
        (
            ScheduleCreateField::Timezone,
            right[2],
            " IANA timezone ",
            &form.timezone,
        ),
        (
            ScheduleCreateField::Input,
            left[3],
            " JSON arguments [] ",
            &form.input,
        ),
        (ScheduleCreateField::Notes, right[3], " Notes ", &form.notes),
    ];
    for (field, field_area, label, input) in fields {
        let offset = render_form_input(
            frame,
            field_area,
            label,
            input,
            form.active_field == field,
            theme,
        );
        if form.active_field == field {
            set_input_cursor(frame, field_area, input, offset);
        }
    }
}

fn render_schedule_edit(frame: &mut Frame<'_>, area: Rect, form: &ScheduleEditForm, theme: Theme) {
    let popup = centered(area, 90, 16);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.warning())
            .title(Span::styled(
                format!(" Edit Schedule {} ", form.schedule_id),
                theme.title(),
            ))
            .title_bottom(
                Line::from(" blank expression preserves timing · enter next/save · esc cancel ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(3),
    ])
    .spacing(1)
    .split(inner);
    let fields = [
        (
            ScheduleEditField::Expression,
            areas[0],
            " Replacement cron / @every (blank preserves) ",
            &form.expression,
        ),
        (
            ScheduleEditField::Timezone,
            areas[1],
            " IANA timezone ",
            &form.timezone,
        ),
        (ScheduleEditField::Notes, areas[2], " Notes ", &form.notes),
    ];
    for (field, field_area, label, input) in fields {
        let offset = render_form_input(
            frame,
            field_area,
            label,
            input,
            form.active_field == field,
            theme,
        );
        if form.active_field == field {
            set_input_cursor(frame, field_area, input, offset);
        }
    }
}

fn render_schedule_backfill(
    frame: &mut Frame<'_>,
    area: Rect,
    form: &ScheduleBackfillForm,
    theme: Theme,
) {
    let popup = centered(area, 108, 12);
    frame.render_widget(Clear, popup);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(theme.error())
            .title(Span::styled(
                format!(" Backfill Schedule {} ", form.schedule_id),
                theme.error(),
            ))
            .title_bottom(
                Line::from(" RFC3339 bounds · exact Schedule ID · enter next/backfill ")
                    .alignment(Alignment::Right)
                    .style(theme.muted()),
            ),
        popup,
    );
    let inner = popup.inner(ratatui::layout::Margin {
        vertical: 1,
        horizontal: 2,
    });
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Length(3)])
        .spacing(1)
        .split(inner);
    let top = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(rows[0]);
    let bottom = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .spacing(1)
        .split(rows[1]);
    let fields = [
        (
            ScheduleBackfillField::Start,
            top[0],
            " Start (exclusive) ",
            &form.start_time,
        ),
        (
            ScheduleBackfillField::End,
            top[1],
            " End (inclusive) ",
            &form.end_time,
        ),
        (
            ScheduleBackfillField::Overlap,
            bottom[0],
            " Overlap policy ",
            &form.overlap_policy,
        ),
        (
            ScheduleBackfillField::Confirmation,
            bottom[1],
            " Exact Schedule ID ",
            &form.confirmation,
        ),
    ];
    for (field, field_area, label, input) in fields {
        let offset = render_form_input(
            frame,
            field_area,
            label,
            input,
            form.active_field == field,
            theme,
        );
        if form.active_field == field {
            set_input_cursor(frame, field_area, input, offset);
        }
    }
}

fn render_schedule_confirmation(
    frame: &mut Frame<'_>,
    area: Rect,
    action: ScheduleConfirmAction,
    schedule_id: &str,
    input: &TextInput,
    theme: Theme,
) {
    let popup = centered(area, 82, 11);
    frame.render_widget(Clear, popup);
    let severity = theme.error();
    let warning = match action {
        ScheduleConfirmAction::Trigger => {
            "This starts the Schedule action now and may create a production Workflow Execution."
        }
        ScheduleConfirmAction::Delete => {
            "Deletion is permanent. Existing Workflow Executions are not deleted."
        }
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(severity)
        .title(Span::styled(" Schedule confirmation ", severity))
        .title_bottom(
            Line::from(" type Schedule ID exactly · enter confirm · esc cancel ")
                .alignment(Alignment::Right)
                .style(theme.muted()),
        );
    let inner = block.inner(popup);
    frame.render_widget(block, popup);
    let areas = Layout::vertical([Constraint::Length(4), Constraint::Length(3)]).split(inner);
    frame.render_widget(
        Paragraph::new(Text::from(vec![
            Line::from(vec![
                Span::raw("Really "),
                Span::styled(action.verb(), severity.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(schedule_id, theme.strong()),
                Span::raw("?"),
            ]),
            Line::default(),
            Line::from(warning).style(theme.muted()),
        ]))
        .wrap(Wrap { trim: true }),
        areas[0],
    );
    let offset = render_form_input(frame, areas[1], " Schedule ID ", input, true, theme);
    set_input_cursor(frame, areas[1], input, offset);
}

fn render_form_input(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &'static str,
    input: &TextInput,
    active: bool,
    theme: Theme,
) -> u16 {
    let offset = input_horizontal_offset(input, area.width.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(input.value.as_str())
            .scroll((0, offset))
            .block(input_block(title, active, theme)),
        area,
    );
    offset
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

fn format_duration_seconds(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    if seconds >= 86_400.0 {
        format!("{:.1}d", seconds / 86_400.0)
    } else if seconds >= 3_600.0 {
        format!("{:.1}h", seconds / 3_600.0)
    } else if seconds >= 60.0 {
        format!("{:.1}m", seconds / 60.0)
    } else if seconds >= 1.0 {
        format!("{seconds:.1}s")
    } else {
        format!("{:.0}ms", seconds * 1_000.0)
    }
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
        app::{AppConfig, Overlay, ProfileSummary},
        model::{
            BatchOperationDetails, BatchOperationSummary, ClusterInfo, DeploymentVersion,
            DeploymentVersionSummary, HistoryEventSummary, PollerSummary, ScheduleActionResult,
            ScheduleDetails, ScheduleSummary, SearchAttributeSummary, StructuredField,
            TaskQueueStats, TaskQueueSummary, TaskQueueType, WorkerDeploymentDetails,
            WorkerDeploymentSummary, WorkerDetails, WorkerSlots, WorkerSummary, WorkflowCallResult,
            WorkflowCount, WorkflowCountGroup, WorkflowDetails, WorkflowKey, WorkflowSummary,
        },
    };

    fn sample_app() -> App {
        let mut app = App::new(AppConfig {
            address: "localhost:7233".to_string(),
            profile_name: Some("dev".to_string()),
            namespace: "default".to_string(),
            query: String::new(),
            page_size: 200,
            refresh_interval: Duration::from_secs(5),
            auto_refresh: true,
            color: true,
            read_only: false,
            force_read_only: false,
            codec_enabled: false,
            web_ui_url: Some("http://localhost:8233".to_string()),
            saved_queries: Vec::new(),
            profiles: Vec::new(),
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
            parent_run_id: None,
            root_workflow_id: None,
            root_run_id: None,
            reset_run_id: None,
            cancel_requested: false,
            pending_activities: 1,
            pending_activity_details: Vec::new(),
            pending_children: 0,
            pending_nexus_operations: 0,
            state_transition_count: 5,
            static_summary: Some("Process order".to_string()),
            static_details: None,
            memo: Vec::new(),
            search_attributes: Vec::new(),
            events: vec![HistoryEventSummary {
                event_id: 1,
                event_type: "WORKFLOW EXECUTION STARTED".to_string(),
                event_time: Some(Utc.with_ymd_and_hms(2026, 7, 27, 8, 0, 0).unwrap()),
                detail: "OrderWorkflow · orders".to_string(),
                fields: Vec::new(),
                failure: None,
            }],
            history_next_page_token: Vec::new(),
            history_archived: false,
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
            input: TextInput::default(),
        });
        let output = rendered(&app, 120, 32);
        assert!(output.contains("Termination is immediate"));
        assert!(output.contains("order-42"));
        assert!(output.contains("confirm"));
    }

    #[test]
    fn renders_payload_inspector_and_aggregations() {
        let mut app = sample_app();
        app.details.as_mut().unwrap().memo.push(StructuredField {
            name: "api_key".to_string(),
            encoding: "json/plain".to_string(),
            value: "<redacted>".to_string(),
            size_bytes: 12,
            redacted: true,
        });
        app.overlay = Some(Overlay::Inspector { scroll: 0 });
        let inspector = rendered(&app, 130, 40);
        assert!(inspector.contains("Workflow inspector"));
        assert!(inspector.contains("MEMO"));
        assert!(inspector.contains("<redacted>"));

        app.workflow_count = Some(WorkflowCount {
            total: 2,
            groups: vec![WorkflowCountGroup {
                values: vec!["FAILED".to_string()],
                count: 2,
            }],
        });
        app.overlay = Some(Overlay::Aggregations { selected: 0 });
        let aggregation = rendered(&app, 120, 32);
        assert!(aggregation.contains("Visibility aggregation"));
        assert!(aggregation.contains("FAILED"));
    }

    #[test]
    fn renders_task_queue_worker_and_deployment_diagnostics() {
        let deployment = DeploymentVersion {
            deployment_name: "payments".to_string(),
            build_id: "v1".to_string(),
        };

        let mut task_queue_app = sample_app();
        task_queue_app.view = View::TaskQueues;
        task_queue_app.task_queues = vec![TaskQueueSummary {
            name: "payments".to_string(),
            queue_type: TaskQueueType::Activity,
            pollers: vec![PollerSummary {
                identity: "worker-a".to_string(),
                last_access_time: None,
                rate_per_second: 8.0,
                deployment_name: "payments".to_string(),
                build_id: "v1".to_string(),
            }],
            stats: TaskQueueStats {
                approximate_backlog_count: 2,
                approximate_backlog_age_seconds: 9.0,
                tasks_add_rate: 1.0,
                tasks_dispatch_rate: 2.0,
            },
            current_deployment: Some(deployment.clone()),
            ramping_deployment: None,
            ramping_percentage: 0.0,
            effective_rate_limit: Some(25.0),
        }];
        let task_queues = rendered(&task_queue_app, 140, 38);
        assert!(task_queues.contains("TASK QUEUES"));
        assert!(task_queues.contains("HEALTHY"));
        assert!(task_queues.contains("payments:v1"));
        assert!(task_queues.contains("25.00/s"));

        let worker = WorkerSummary {
            instance_key: "instance-a".to_string(),
            identity: "worker-a".to_string(),
            task_queue: "payments".to_string(),
            deployment: Some(deployment.clone()),
            sdk_name: "temporal-sdk-rust".to_string(),
            sdk_version: "0.2.0".to_string(),
            status: "RUNNING".to_string(),
            start_time: None,
            host_name: "host-a".to_string(),
            process_id: "4242".to_string(),
            plugins: vec!["otel@1.0".to_string()],
        };
        let slots = WorkerSlots {
            available: 8,
            used: 2,
            supplier: "Fixed".to_string(),
            processed: 100,
            failed: 1,
        };
        let mut worker_app = sample_app();
        worker_app.view = View::Workers;
        worker_app.workers = vec![worker.clone()];
        worker_app.worker_details = Some(WorkerDetails {
            summary: worker,
            heartbeat_time: None,
            elapsed_since_heartbeat_seconds: 2.0,
            host_cpu_usage: 0.25,
            host_memory_usage: 0.5,
            workflow_slots: slots.clone(),
            activity_slots: slots,
            local_activity_slots: WorkerSlots::default(),
            nexus_slots: WorkerSlots::default(),
            workflow_pollers: 4,
            activity_pollers: 2,
            nexus_pollers: 1,
            sticky_cache_hits: 30,
            sticky_cache_misses: 2,
            sticky_cache_size: 12,
        });
        let workers = rendered(&worker_app, 140, 38);
        assert!(workers.contains("WORKERS"));
        assert!(workers.contains("CPU 25.0%"));
        assert!(workers.contains("Fixed"));
        assert!(workers.contains("otel@1.0"));

        let deployment_summary = WorkerDeploymentSummary {
            name: "payments".to_string(),
            create_time: None,
            current_version: Some(deployment.clone()),
            ramping_version: None,
            ramping_percentage: 0.0,
            latest_version: Some(deployment.clone()),
        };
        let mut deployment_app = sample_app();
        deployment_app.view = View::Deployments;
        deployment_app.worker_deployments = vec![deployment_summary.clone()];
        deployment_app.worker_deployment_details = Some(WorkerDeploymentDetails {
            summary: deployment_summary,
            versions: vec![DeploymentVersionSummary {
                version: deployment,
                status: "CURRENT".to_string(),
                create_time: None,
                is_current: true,
                is_ramping: false,
                ramp_percentage: 0.0,
                drainage_status: "DRAINED".to_string(),
                drainage_last_checked: None,
            }],
            manager_identity: "release-controller".to_string(),
            last_modifier_identity: "operator-a".to_string(),
            routing_update_state: "COMPLETED".to_string(),
        });
        let deployments = rendered(&deployment_app, 140, 38);
        assert!(deployments.contains("DEPLOYMENTS"));
        assert!(deployments.contains("release-controller"));
        assert!(deployments.contains("DRAINED"));
        assert!(deployments.contains("COMPLETED"));

        deployment_app.overlay = Some(Overlay::DeploymentCurrent(DeploymentCurrentForm {
            deployment_name: "payments".to_string(),
            build_id: TextInput::new("2026.07.28"),
            confirmation: TextInput::default(),
            active_field: DeploymentCurrentField::BuildId,
        }));
        let current = rendered(&deployment_app, 140, 38);
        assert!(current.contains("Set Current"));
        assert!(current.contains("missing Task Queue"));

        deployment_app.overlay = Some(Overlay::DeploymentRamp(DeploymentRampForm {
            deployment_name: "payments".to_string(),
            build_id: TextInput::new("2026.07.29"),
            percentage: TextInput::new("10"),
            confirmation: TextInput::default(),
            active_field: DeploymentRampField::BuildId,
        }));
        let ramp = rendered(&deployment_app, 140, 38);
        assert!(ramp.contains("Configure Ramp"));
        assert!(ramp.contains("never bypassed"));
    }

    #[test]
    fn renders_schedule_control_plane_and_workflow_call_results() {
        let summary = ScheduleSummary {
            schedule_id: "hourly-orders".to_string(),
            paused: false,
            notes: "production hourly run".to_string(),
            workflow_type: "OrderWorkflow".to_string(),
            next_action_time: Some(Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap()),
            recent_action_time: None,
            state_size_bytes: 2_048,
        };
        let mut app = sample_app();
        app.view = View::Schedules;
        app.schedules = vec![summary.clone()];
        app.schedule_details = Some(ScheduleDetails {
            summary,
            workflow_id: "scheduled-order".to_string(),
            task_queue: "orders".to_string(),
            timing: vec!["every 1h".to_string()],
            timezone: "UTC".to_string(),
            overlap_policy: "SKIP".to_string(),
            catchup_window: "1h".to_string(),
            pause_on_failure: true,
            keep_original_workflow_id: false,
            limited_actions: false,
            remaining_actions: 0,
            action_count: 4,
            missed_catchup_window: 0,
            overlap_skipped: 1,
            buffer_dropped: 0,
            buffer_size: 0,
            running_workflows: Vec::new(),
            recent_actions: vec![ScheduleActionResult {
                scheduled_time: None,
                actual_time: Some(Utc.with_ymd_and_hms(2026, 7, 28, 11, 0, 0).unwrap()),
                workflow_id: "scheduled-order-1".to_string(),
                run_id: "run-schedule".to_string(),
                workflow_status: "COMPLETED".to_string(),
            }],
            future_action_times: vec![Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap()],
            create_time: None,
            update_time: None,
            input: vec![StructuredField {
                name: "input".to_string(),
                encoding: "json/plain".to_string(),
                value: r#"{"region":"eu"}"#.to_string(),
                size_bytes: 15,
                redacted: false,
            }],
            memo: Vec::new(),
            search_attributes: Vec::new(),
        });
        let schedule = rendered(&app, 150, 42);
        assert!(schedule.contains("SCHEDULES"));
        assert!(schedule.contains("hourly-orders"));
        assert!(schedule.contains("every 1h"));
        assert!(schedule.contains("RECENT ACTIONS"));

        app.overlay = Some(Overlay::WorkflowCallResult {
            kind: WorkflowCallKind::Update,
            result: WorkflowCallResult {
                handler: "approve".to_string(),
                update_id: Some("update-1".to_string()),
                fields: vec![StructuredField {
                    name: "result".to_string(),
                    encoding: "json/plain".to_string(),
                    value: "true".to_string(),
                    size_bytes: 4,
                    redacted: false,
                }],
                failure: None,
            },
            scroll: 0,
        });
        let result = rendered(&app, 130, 36);
        assert!(result.contains("Workflow Update result"));
        assert!(result.contains("update-1"));
        assert!(result.contains("true"));

        app.overlay = Some(Overlay::ScheduleCreate(ScheduleCreateForm::default()));
        let create = rendered(&app, 130, 36);
        assert!(create.contains("Create Schedule"));
        assert!(create.contains("Cron / @every"));
        assert!(create.contains("JSON arguments []"));
    }

    #[test]
    fn renders_search_attribute_registry_and_exact_mutation_forms() {
        let mut app = sample_app();
        app.search_attributes = vec![
            SearchAttributeSummary {
                name: "CustomerTier".to_string(),
                value_type: "KEYWORD".to_string(),
                storage_type: "keyword".to_string(),
                custom: true,
            },
            SearchAttributeSummary {
                name: "WorkflowId".to_string(),
                value_type: "KEYWORD".to_string(),
                storage_type: "keyword".to_string(),
                custom: false,
            },
        ];
        app.overlay = Some(Overlay::SearchAttributes { selected: 0 });
        let registry = rendered(&app, 120, 36);
        assert!(registry.contains("Search Attributes"));
        assert!(registry.contains("CustomerTier"));
        assert!(registry.contains("CUSTOM"));

        app.overlay = Some(Overlay::SearchAttributeAdd(
            SearchAttributeAddForm::default(),
        ));
        let add = rendered(&app, 120, 36);
        assert!(add.contains("Register Search Attribute"));
        assert!(add.contains("KeywordList"));

        app.overlay = Some(Overlay::SearchAttributeRemove {
            name: "CustomerTier".to_string(),
            input: TextInput::default(),
        });
        let remove = rendered(&app, 120, 36);
        assert!(remove.contains("Remove Search Attribute"));
        assert!(remove.contains("CustomerTier"));
    }

    #[test]
    fn renders_batch_control_plane_and_exact_confirmation_forms() {
        let summary = BatchOperationSummary {
            job_id: "cancel-stale-orders".to_string(),
            state: "RUNNING".to_string(),
            start_time: Some(Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap()),
            close_time: None,
        };
        let mut app = sample_app();
        app.view = View::Batches;
        app.batch_operations = vec![summary.clone()];
        app.batch_operation_details = Some(BatchOperationDetails {
            summary,
            operation_type: "CANCEL".to_string(),
            total_operation_count: 12,
            complete_operation_count: 9,
            failure_operation_count: 1,
            identity: "temporal-tui".to_string(),
            reason: "stale order cleanup".to_string(),
        });
        let dashboard = rendered(&app, 150, 42);
        assert!(dashboard.contains("BATCHES"));
        assert!(dashboard.contains("cancel-stale-orders"));
        assert!(dashboard.contains("CANCEL"));
        assert!(dashboard.contains("9 complete · 1 failed · 12 total"));
        assert!(dashboard.contains("server-side"));

        let form = BatchCreateForm {
            job_id: TextInput::new("cancel-stale-orders"),
            operation: TextInput::new("terminate"),
            visibility_query: TextInput::new("WorkflowType = 'OrderWorkflow'"),
            reason: TextInput::new("stale order cleanup"),
            max_operations_per_second: TextInput::new("10"),
            signal_name: TextInput::default(),
            signal_input: TextInput::new("{}"),
            active_field: BatchCreateField::JobId,
        };
        app.overlay = Some(Overlay::BatchCreate(form.clone()));
        let create = rendered(&app, 150, 42);
        assert!(create.contains("Preview Batch Operation"));
        assert!(create.contains("Visibility query"));
        assert!(create.contains("Targets"));

        app.overlay = Some(Overlay::BatchConfirm {
            form,
            matched_workflows: 37,
            input: TextInput::default(),
        });
        let confirmation = rendered(&app, 150, 42);
        assert!(confirmation.contains("37 matching Workflow Executions"));
        assert!(confirmation.contains("WorkflowType = 'OrderWorkflow'"));
        assert!(confirmation.contains("Type the exact Job ID"));

        app.overlay = Some(Overlay::BatchStop {
            job_id: "cancel-stale-orders".to_string(),
            input: TextInput::default(),
        });
        let stop = rendered(&app, 120, 32);
        assert!(stop.contains("Stop Batch Operation"));
        assert!(stop.contains("cancel-stale-orders"));
        assert!(stop.contains("Exact Job ID"));
    }

    #[test]
    fn renders_non_secret_profile_picker_and_switching_state() {
        let mut app = sample_app();
        app.profiles = vec![
            ProfileSummary {
                name: "dev".to_string(),
                address: "127.0.0.1:7233".to_string(),
                namespace: "default".to_string(),
                read_only: false,
                codec_enabled: false,
                is_default: true,
            },
            ProfileSummary {
                name: "production".to_string(),
                address: "production.tmprl.cloud:7233".to_string(),
                namespace: "payments.a1b2c".to_string(),
                read_only: true,
                codec_enabled: true,
                is_default: false,
            },
        ];
        app.overlay = Some(Overlay::ProfilePicker { selected: 1 });
        let picker = rendered(&app, 150, 42);
        assert!(picker.contains("Switch Temporal profile"));
        assert!(picker.contains("production.tmprl.cloud:7233"));
        assert!(picker.contains("READ ONLY"));
        assert!(picker.contains("secrets resolve only after selection"));

        app.overlay = None;
        app.switching_profile = true;
        app.pending_profile_name = Some("production".to_string());
        let switching = rendered(&app, 120, 32);
        assert!(switching.contains("switching to profile/production"));
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
