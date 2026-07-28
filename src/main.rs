use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use temporal_tui::{
    app::App,
    auth::AuthSession,
    cli::Cli,
    config::ConfigStore,
    runtime,
    service::{GrpcTemporalService, TemporalService},
    terminal::TerminalSession,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let store = ConfigStore::discover(cli.config.clone())?;
    if cli.run_config_command(&store).await? {
        return Ok(());
    }
    let user_config = store.load()?;
    let launch = cli.launch_config(&store, &user_config)?;
    let auth = launch
        .auth
        .map(|auth| AuthSession::load(&auth.profile_name, auth.profile))
        .transpose()?;
    let service: Arc<dyn TemporalService> =
        Arc::new(GrpcTemporalService::connect_with_auth(launch.connection, auth).await?);
    let app = App::new(launch.app);
    let mut terminal = TerminalSession::new()?;

    runtime::run(&mut terminal, app, service, store).await
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("off"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .compact()
        .init();
}
