use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result};
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures_util::StreamExt;
use tokio::{
    sync::mpsc,
    time::{MissedTickBehavior, interval},
};

use crate::{
    app::{App, Command, Message, OperationKind},
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
            limit,
        } => Message::WorkflowsLoaded {
            request_id,
            result: service
                .list_workflows(&namespace, &query, limit)
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
    }
}
