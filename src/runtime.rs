use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::Utc;
use crossterm::event::{Event, EventStream, KeyEventKind};
use directories::BaseDirs;
use futures_util::StreamExt;
use serde::Serialize;
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval},
};

use crate::{
    app::{App, Command, Message, OperationKind, ProfileConnectionInfo, UtilityKind},
    config::ConfigStore,
    model::{ClusterInfo, WorkflowDetails},
    service::{GrpcTemporalService, TemporalService},
    terminal::TerminalSession,
    ui,
};

enum RuntimeMessage {
    App(Message),
    ProfileConnected {
        request_id: u64,
        result: Result<ConnectedProfile, String>,
    },
}

struct ConnectedProfile {
    service: Arc<dyn TemporalService>,
    info: ProfileConnectionInfo,
}

/// Run the terminal event loop until the user quits.
///
/// # Errors
///
/// Returns an error if terminal drawing, input handling, or terminal-state
/// restoration fails.
pub async fn run(
    terminal: &mut TerminalSession,
    mut app: App,
    initial_service: Arc<dyn TemporalService>,
    config_store: ConfigStore,
) -> Result<()> {
    let mut service = initial_service;
    let (message_tx, mut message_rx) = mpsc::channel(64);
    dispatch_all(app.bootstrap(), &service, &config_store, &message_tx);

    let mut events = EventStream::new();
    let mut ticks = interval(Duration::from_millis(200));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let result = async {
        terminal
            .terminal_mut()
            .draw(|frame| ui::render(frame, &app))
            .context("could not draw terminal UI")?;

        loop {
            let needs_redraw = tokio::select! {
                maybe_event = events.next() => {
                    match maybe_event {
                        Some(Ok(Event::Key(key)))
                            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                        {
                            let commands = app.handle_key(key);
                            dispatch_all(commands, &service, &config_store, &message_tx);
                            true
                        }
                        Some(Ok(Event::Resize(_, _))) => true,
                        Some(Ok(_)) => false,
                        Some(Err(error)) => {
                            return Err(error).context("could not read terminal event");
                        }
                        None => return Ok(()),
                    }
                }
                Some(message) = message_rx.recv() => {
                    let commands = match message {
                        RuntimeMessage::App(message) => app.handle_message(message),
                        RuntimeMessage::ProfileConnected { request_id, result } => {
                            if app.expects_profile_switch(request_id) {
                                let result = result.map(|connected| {
                                    service = connected.service;
                                    connected.info
                                });
                                app.handle_message(Message::ProfileSwitchFinished {
                                    request_id,
                                    result,
                                })
                            } else {
                                Vec::new()
                            }
                        }
                    };
                    dispatch_all(commands, &service, &config_store, &message_tx);
                    true
                }
                _ = ticks.tick() => {
                    let had_notice = app.notice.is_some();
                    let commands = app.on_tick(std::time::Instant::now());
                    let changed = !commands.is_empty() || had_notice != app.notice.is_some();
                    dispatch_all(commands, &service, &config_store, &message_tx);
                    changed
                }
            };

            if app.should_quit {
                return Ok(());
            }
            if needs_redraw {
                terminal
                    .terminal_mut()
                    .draw(|frame| ui::render(frame, &app))
                    .context("could not draw terminal UI")?;
            }
        }
    }
    .await;

    let restore_result = terminal.restore();
    result.and(restore_result)
}

fn dispatch_all(
    commands: Vec<Command>,
    service: &Arc<dyn TemporalService>,
    config_store: &ConfigStore,
    sender: &mpsc::Sender<RuntimeMessage>,
) {
    for command in commands {
        dispatch(
            command,
            Arc::clone(service),
            config_store.clone(),
            sender.clone(),
        );
    }
}

fn dispatch(
    command: Command,
    service: Arc<dyn TemporalService>,
    config_store: ConfigStore,
    sender: mpsc::Sender<RuntimeMessage>,
) {
    if let Command::SwitchProfile {
        request_id,
        profile_name,
    } = command
    {
        tokio::spawn(async move {
            let result = connect_profile(config_store, profile_name).await;
            let _ = sender
                .send(RuntimeMessage::ProfileConnected { request_id, result })
                .await;
        });
    } else {
        tokio::spawn(async move {
            let message = execute(command, service.as_ref()).await;
            let _ = sender.send(RuntimeMessage::App(message)).await;
        });
    }
}

async fn connect_profile(
    config_store: ConfigStore,
    profile_name: String,
) -> Result<ConnectedProfile, String> {
    let resolution_name = profile_name.clone();
    let resolved = tokio::task::spawn_blocking(move || {
        let config = config_store.load()?;
        config_store.resolve_profile(&config, &resolution_name)
    })
    .await
    .map_err(|error| format!("profile resolution task failed: {error}"))?
    .map_err(|error| format!("could not resolve profile/{profile_name}: {error}"))?;

    let address = resolved.connection.address.clone();
    let codec_enabled = resolved.connection.payload_codec.is_some();
    let namespace = resolved.namespace;
    let web_ui_url = resolved.web_ui_url;
    let read_only = resolved.read_only;
    let service = GrpcTemporalService::connect(resolved.connection)
        .await
        .map_err(|error| format!("could not connect profile/{profile_name}: {error}"))?;
    service
        .cluster_info()
        .await
        .map_err(|error| format!("could not verify profile/{profile_name}: {error}"))?;

    Ok(ConnectedProfile {
        service: Arc::new(service),
        info: ProfileConnectionInfo {
            name: profile_name,
            address,
            namespace,
            read_only,
            codec_enabled,
            web_ui_url,
        },
    })
}

async fn execute(command: Command, service: &dyn TemporalService) -> Message {
    match command {
        Command::LoadCluster { request_id } => Message::ClusterLoaded {
            request_id,
            result: service
                .cluster_info()
                .await
                .map_err(|error| error.to_string()),
        },
        Command::SwitchProfile { .. } => {
            unreachable!("profile switching is handled by the runtime dispatcher")
        }
        Command::LoadNamespaces { request_id } => Message::NamespacesLoaded {
            request_id,
            result: service
                .list_namespaces()
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkflows {
            request_id,
            namespace,
            query,
            page_size,
            next_page_token,
        } => Message::WorkflowsLoaded {
            request_id,
            result: service
                .list_workflows(&namespace, &query, page_size, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::CountWorkflows {
            request_id,
            namespace,
            query,
        } => Message::WorkflowCountLoaded {
            request_id,
            result: service
                .count_workflows(&namespace, &query)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadDetails {
            request_id,
            namespace,
            key,
        } => Message::DetailsLoaded {
            request_id,
            result: service
                .describe_workflow(&namespace, &key)
                .await
                .map(Box::new)
                .map_err(|error| error.to_string()),
        },
        Command::LoadHistoryPage {
            request_id,
            namespace,
            key,
            next_page_token,
        } => Message::HistoryPageLoaded {
            request_id,
            result: service
                .load_history_page(&namespace, &key, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkflowChain {
            request_id,
            namespace,
            workflow_id,
        } => Message::WorkflowChainLoaded {
            request_id,
            result: service
                .list_workflow_chain(&namespace, &workflow_id)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadTaskQueues {
            request_id,
            namespace,
            names,
        } => Message::TaskQueuesLoaded {
            request_id,
            result: service
                .list_task_queues(&namespace, names)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkers {
            request_id,
            namespace,
            query,
            page_size,
            next_page_token,
        } => Message::WorkersLoaded {
            request_id,
            result: service
                .list_workers(&namespace, &query, page_size, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkerDetails {
            request_id,
            namespace,
            instance_key,
        } => Message::WorkerDetailsLoaded {
            request_id,
            result: service
                .describe_worker(&namespace, &instance_key)
                .await
                .map(Box::new)
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkerDeployments {
            request_id,
            namespace,
            page_size,
            next_page_token,
        } => Message::WorkerDeploymentsLoaded {
            request_id,
            result: service
                .list_worker_deployments(&namespace, page_size, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadWorkerDeploymentDetails {
            request_id,
            namespace,
            name,
        } => Message::WorkerDeploymentDetailsLoaded {
            request_id,
            result: service
                .describe_worker_deployment(&namespace, &name)
                .await
                .map(Box::new)
                .map_err(|error| error.to_string()),
        },
        Command::LoadSchedules {
            request_id,
            namespace,
            query,
            page_size,
            next_page_token,
        } => Message::SchedulesLoaded {
            request_id,
            result: service
                .list_schedules(&namespace, &query, page_size, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadScheduleDetails {
            request_id,
            namespace,
            schedule_id,
        } => Message::ScheduleDetailsLoaded {
            request_id,
            result: service
                .describe_schedule(&namespace, &schedule_id)
                .await
                .map(Box::new)
                .map_err(|error| error.to_string()),
        },
        Command::LoadSearchAttributes {
            request_id,
            namespace,
        } => Message::SearchAttributesLoaded {
            request_id,
            result: service
                .list_search_attributes(&namespace)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::AddSearchAttribute {
            request_id,
            namespace,
            name,
            value_type,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::AddSearchAttribute,
            result: service
                .add_search_attribute(&namespace, &name, &value_type)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::RemoveSearchAttribute {
            request_id,
            namespace,
            name,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::RemoveSearchAttribute,
            result: service
                .remove_search_attribute(&namespace, &name)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::SetDeploymentCurrent {
            request_id,
            namespace,
            deployment_name,
            build_id,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::SetDeploymentCurrent,
            result: service
                .set_worker_deployment_current_version(&namespace, &deployment_name, &build_id)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::SetDeploymentRamp {
            request_id,
            namespace,
            deployment_name,
            build_id,
            percentage,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::SetDeploymentRamp,
            result: service
                .set_worker_deployment_ramping_version(
                    &namespace,
                    &deployment_name,
                    &build_id,
                    percentage,
                )
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadBatchOperations {
            request_id,
            namespace,
            page_size,
            next_page_token,
        } => Message::BatchOperationsLoaded {
            request_id,
            result: service
                .list_batch_operations(&namespace, page_size, next_page_token)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::LoadBatchOperationDetails {
            request_id,
            namespace,
            job_id,
        } => Message::BatchOperationDetailsLoaded {
            request_id,
            result: service
                .describe_batch_operation(&namespace, &job_id)
                .await
                .map(Box::new)
                .map_err(|error| error.to_string()),
        },
        Command::PreviewBatchOperation {
            request_id,
            namespace,
            form,
            request,
        } => Message::BatchOperationPreviewLoaded {
            request_id,
            form,
            result: service
                .count_workflows(&namespace, &request.visibility_query)
                .await
                .map(|count| count.total)
                .map_err(|error| error.to_string()),
        },
        Command::StartBatchOperation {
            request_id,
            namespace,
            request,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::StartBatchOperation,
            result: service
                .start_batch_operation(&namespace, request)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::StopBatchOperation {
            request_id,
            namespace,
            job_id,
            reason,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::StopBatchOperation,
            result: service
                .stop_batch_operation(&namespace, &job_id, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::QueryWorkflow {
            request_id,
            namespace,
            key,
            query_name,
            arguments,
        } => Message::WorkflowCallFinished {
            request_id,
            kind: crate::app::WorkflowCallKind::Query,
            result: service
                .query_workflow(&namespace, &key, &query_name, arguments)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::UpdateWorkflow {
            request_id,
            namespace,
            key,
            update_name,
            arguments,
        } => Message::WorkflowCallFinished {
            request_id,
            kind: crate::app::WorkflowCallKind::Update,
            result: service
                .update_workflow(&namespace, &key, &update_name, arguments)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::PauseWorkflow {
            request_id,
            namespace,
            key,
            reason,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::PauseWorkflow,
            result: service
                .pause_workflow(&namespace, &key, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::UnpauseWorkflow {
            request_id,
            namespace,
            key,
            reason,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::UnpauseWorkflow,
            result: service
                .unpause_workflow(&namespace, &key, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::ResetWorkflow {
            request_id,
            namespace,
            key,
            event_id,
            reason,
        } => Message::ResetFinished {
            request_id,
            result: service
                .reset_workflow(&namespace, &key, event_id, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::Cancel {
            request_id,
            namespace,
            key,
            reason,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::Cancel,
            result: service
                .cancel_workflow(&namespace, &key, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::CreateSchedule {
            request_id,
            namespace,
            request,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::CreateSchedule,
            result: service
                .create_schedule(&namespace, request)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::UpdateSchedule {
            request_id,
            namespace,
            schedule_id,
            request,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::UpdateSchedule,
            result: service
                .update_schedule(&namespace, &schedule_id, request)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::PauseSchedule {
            request_id,
            namespace,
            schedule_id,
            note,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::PauseSchedule,
            result: service
                .pause_schedule(&namespace, &schedule_id, &note)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::UnpauseSchedule {
            request_id,
            namespace,
            schedule_id,
            note,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::UnpauseSchedule,
            result: service
                .unpause_schedule(&namespace, &schedule_id, &note)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::TriggerSchedule {
            request_id,
            namespace,
            schedule_id,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::TriggerSchedule,
            result: service
                .trigger_schedule(&namespace, &schedule_id)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::BackfillSchedule {
            request_id,
            namespace,
            schedule_id,
            request,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::BackfillSchedule,
            result: service
                .backfill_schedule(&namespace, &schedule_id, request)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::DeleteSchedule {
            request_id,
            namespace,
            schedule_id,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::DeleteSchedule,
            result: service
                .delete_schedule(&namespace, &schedule_id)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::Terminate {
            request_id,
            namespace,
            key,
            reason,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::Terminate,
            result: service
                .terminate_workflow(&namespace, &key, &reason)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::Signal {
            request_id,
            namespace,
            key,
            signal_name,
            input,
        } => Message::OperationFinished {
            request_id,
            operation: OperationKind::Signal,
            result: service
                .signal_workflow(&namespace, &key, &signal_name, input)
                .await
                .map_err(|error| error.to_string()),
        },
        Command::Copy { request_id, text } => Message::UtilityFinished {
            request_id,
            operation: UtilityKind::Copy,
            result: tokio::task::spawn_blocking(move || copy_to_clipboard(&text))
                .await
                .map_err(|error| format!("clipboard task failed: {error}"))
                .and_then(std::convert::identity),
        },
        Command::Export {
            request_id,
            namespace,
            cluster,
            details,
        } => Message::UtilityFinished {
            request_id,
            operation: UtilityKind::Export,
            result: tokio::task::spawn_blocking(move || {
                export_workflow(&namespace, cluster.as_ref(), &details)
            })
            .await
            .map_err(|error| format!("export task failed: {error}"))
            .and_then(std::convert::identity),
        },
        Command::OpenWeb { request_id, url } => Message::UtilityFinished {
            request_id,
            operation: UtilityKind::OpenWeb,
            result: tokio::task::spawn_blocking(move || {
                open::that_detached(&url)
                    .map(|()| url)
                    .map_err(|error| format!("could not open Temporal Web UI: {error}"))
            })
            .await
            .map_err(|error| format!("open task failed: {error}"))
            .and_then(std::convert::identity),
        },
    }
}

fn copy_to_clipboard(text: &str) -> Result<String, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("could not open clipboard: {error}"))?;
    clipboard
        .set_text(text)
        .map_err(|error| format!("could not copy to clipboard: {error}"))?;
    Ok(text.to_string())
}

#[derive(Serialize)]
struct WorkflowExport<'a> {
    schema_version: u32,
    exported_at: chrono::DateTime<Utc>,
    namespace: &'a str,
    cluster: Option<&'a ClusterInfo>,
    redaction: &'static str,
    workflow: &'a WorkflowDetails,
}

fn export_workflow(
    namespace: &str,
    cluster: Option<&ClusterInfo>,
    details: &WorkflowDetails,
) -> Result<String, String> {
    let base =
        BaseDirs::new().ok_or_else(|| "could not determine the user data directory".to_string())?;
    let directory = base.data_local_dir().join("temporal-tui").join("exports");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;

    let workflow_id = safe_filename_component(&details.summary.key.workflow_id);
    let run_id = safe_filename_component(&details.summary.key.run_id);
    let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
    let filename = format!("{workflow_id}-{run_id}-{timestamp}.json");
    let path = directory.join(filename);
    let export = WorkflowExport {
        schema_version: 1,
        exported_at: Utc::now(),
        namespace,
        cluster,
        redaction: "fields with sensitive names are replaced by <redacted>",
        workflow: details,
    };
    let bytes = serde_json::to_vec_pretty(&export)
        .map_err(|error| format!("could not serialize workflow export: {error}"))?;
    write_new_private_file(&path, &bytes)?;
    Ok(path.display().to_string())
}

fn safe_filename_component(value: &str) -> String {
    let mut result = value
        .chars()
        .take(80)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if result.is_empty() || result == "." || result == ".." {
        result = "workflow".to_string();
    }
    result
}

fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("could not sync {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_filename_components_cannot_escape_directory() {
        assert_eq!(
            safe_filename_component("../../orders/42"),
            ".._.._orders_42"
        );
        assert_eq!(safe_filename_component(""), "workflow");
        assert_eq!(safe_filename_component("valid-id"), "valid-id");
    }
}
