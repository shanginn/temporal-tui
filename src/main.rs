use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use temporal_tui::{
    app::App,
    cli::Cli,
    runtime,
    service::{GrpcTemporalService, TemporalService},
    terminal::TerminalSession,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let config = cli.connection_config()?;
    let service: Arc<dyn TemporalService> = Arc::new(GrpcTemporalService::connect(config).await?);
    let app = App::new(cli.app_config());
    let mut terminal = TerminalSession::new()?;

    runtime::run(&mut terminal, app, service).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .compact()
        .init();
}
