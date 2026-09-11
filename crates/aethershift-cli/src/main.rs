use clap::Parser;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use aethershift_cli::cli::Cli;
use aethershift_cli::client::execute_command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Default to WARN log level for CLI to keep output clean, unless RUST_LOG is set
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::WARN)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let socket_path = cli.resolved_socket_path();

    if let Err(e) = execute_command(&socket_path, cli.command).await {
        // If the error was not already printed with friendly guidance, print it
        if !e.to_string().contains("Daemon connection failed") {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }

    Ok(())
}
