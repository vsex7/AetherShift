use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "aethershift-tui",
    about = "Interactive terminal console for AetherShift",
    version
)]
struct Args {
    /// Path to the AetherShift daemon Unix domain socket
    #[arg(short, long)]
    socket: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    aethershift_tui::run_tui(args.socket).await
}
