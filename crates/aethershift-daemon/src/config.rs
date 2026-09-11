use aethershift_protocol::default_socket_path;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "aethershift-daemon",
    author,
    version,
    about = "AetherShift background daemon managing dynamic Hyprland keybinding profiles"
)]
pub struct DaemonConfig {
    /// Unix domain socket path for client-daemon communication
    #[arg(short, long)]
    pub socket_path: Option<PathBuf>,

    /// Directory containing custom profile TOML definitions
    #[arg(short, long, default_value = "presets")]
    pub preset_dir: PathBuf,

    /// Verbose logging level (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Disable desktop notifications via notify-send
    #[arg(long, default_value_t = false)]
    pub no_notify: bool,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            preset_dir: PathBuf::from("presets"),
            verbose: 0,
            no_notify: false,
        }
    }
}

impl DaemonConfig {
    pub fn resolved_socket_path(&self) -> PathBuf {
        self.socket_path
            .clone()
            .or_else(|| std::env::var("AETHERSHIFT_SOCKET").ok().map(PathBuf::from))
            .unwrap_or_else(default_socket_path)
    }

    pub fn resolved_preset_dir(&self) -> PathBuf {
        if let Ok(env_dir) = std::env::var("AETHERSHIFT_PRESET_DIR") {
            PathBuf::from(env_dir)
        } else {
            self.preset_dir.clone()
        }
    }

    pub fn notifications_disabled(&self) -> bool {
        self.no_notify
            || std::env::var("AETHERSHIFT_NO_NOTIFY")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
    }
}
