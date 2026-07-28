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
    app::{App, Command, Message, OperationKind, UtilityKind},
    model::{ClusterInfo, WorkflowDetails},
    service::TemporalService,
    terminal::TerminalSession,
    ui,
};

/// Run the terminal event loop until the user quits.
///
/// # Errors
///
/// Returns an error if terminal drawing, input handling, or terminal-state
/// restoration fails.
pub async fn run(
    terminal: &mut TerminalSession,
    mut app: App,
    service: Arc<dyn TemporalService>,
) -> Result<()> {
    let (message_tx, mut message_rx) = mpsc::channel(64);
    dispatch_all(app.bootstrap(), &service, &message_tx);

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
                            dispatch_all(commands, &service, &message_tx);
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
                    let commands = app.handle_message(message);
                    dispatch_all(commands, &service, &message_tx);
                    true
                }
                _ = ticks.tick() => {
                    let had_notice = app.notice.is_some();
                    let commands = app.on_tick(std::time::Instant::now());
                    let changed = !commands.is_empty() || had_notice != app.notice.is_some();
                    dispatch_all(commands, &service, &message_tx);
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
    sender: &mpsc::Sender<Message>,
) {
    for command in commands {
        dispatch(command, Arc::clone(service), sender.clone());
    }
}

fn dispatch(command: Command, service: Arc<dyn TemporalService>, sender: mpsc::Sender<Message>) {
    tokio::spawn(async move {
        let message = execute(command, service.as_ref()).await;
        let _ = sender.send(message).await;
    });
}

async fn execute(command: Command, service: &dyn TemporalService) -> Message {
    match command {
        Command::LoadCluster => Message::ClusterLoaded(
            service
                .cluster_info()
                .await
                .map_err(|error| error.to_string()),
        ),
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
