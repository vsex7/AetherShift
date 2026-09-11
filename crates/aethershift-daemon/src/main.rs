use clap::Parser;
use tracing::{Level, error, info};
use tracing_subscriber::FmtSubscriber;

use aethershift_daemon::config::DaemonConfig;
use aethershift_daemon::daemon::AetherDaemon;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = DaemonConfig::parse();

    let log_level = match config.verbose {
        0 => Level::INFO,
        1 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder().with_max_level(log_level).finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    info!(
        "Starting AetherShift daemon (v{})...",
        env!("CARGO_PKG_VERSION")
    );
    let daemon = AetherDaemon::new(config);

    if let Err(e) = daemon.run().await {
        error!("AetherShift daemon error: {e}");
        std::process::exit(1);
    }

    info!("AetherShift daemon terminated gracefully.");
    Ok(())
}
