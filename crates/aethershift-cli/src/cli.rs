use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};
use aethershift_protocol::{default_socket_path, WindowPolicy};

#[derive(Debug, Parser)]
#[command(
    name = "aethershift",
    author,
    version,
    about = "Dynamic keybinding & window paradigm manager for Hyprland",
    long_about = "AetherShift CLI allows you to switch, cycle, restore, snap window layouts, customize profiles, and inspect usage statistics through the background daemon."
)]
pub struct Cli {
    /// Explicit Unix domain socket path to connect to daemon
    #[arg(short, long, global = true)]
    pub socket: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolved_socket_path(&self) -> PathBuf {
        self.socket
            .clone()
            .or_else(|| std::env::var("AETHERSHIFT_SOCKET").ok().map(PathBuf::from))
            .unwrap_or_else(default_socket_path)
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Query and display the current AetherShift daemon status
    Status {
        /// Output status information in JSON format
        #[arg(long)]
        json: bool,
    },

    /// List all available profiles and indicate the active one
    List {
        /// Output profile list in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Switch to a specified profile
    Switch {
        /// Name of the profile to switch to (e.g. windows, macos, native, hybrid)
        profile: String,

        /// Force re-applying bindings even if profile is already active
        #[arg(short, long)]
        force: bool,
    },

    /// Cycle to the next available profile
    Cycle,

    /// Restore baseline bindings (unload all overlays)
    Restore,

    /// Query or set the window paradigm mode policy
    #[command(name = "window-mode", visible_alias = "window")]
    WindowMode(WindowModeArgs),

    /// Snap active window into a high-precision geometry layout
    Snap {
        /// Layout preset (e.g. half-left, half-right, two-thirds-left, one-third-right, center, maximize, restore)
        layout: String,
    },

    /// Move the active window across monitors
    Monitor {
        /// Direction or monitor name (e.g. l, r, u, d, or exact monitor name)
        direction: String,
    },

    /// Manage and customize runtime profiles
    #[command(subcommand)]
    Profile(ProfileCommand),

    /// Display usage statistics and action metrics
    Stats {
        /// Output in JSON format
        #[arg(long)]
        json: bool,

        /// Export statistics directly to a JSON file
        #[arg(long)]
        export: Option<PathBuf>,
    },

    /// Display smart ergonomic recommendations based on usage patterns
    Recommend {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Launch the interactive terminal console (TUI)
    Tui,

    /// Request the AetherShift daemon to shut down
    Shutdown,

    /// Launch or manage the AetherShift daemon process
    Daemon(DaemonArgs),
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Create a new profile in runtime memory
    Create {
        /// Name of the new profile
        name: String,
        /// Optional description
        #[arg(short, long)]
        desc: Option<String>,
        /// Optional base profile to copy bindings from
        #[arg(short, long)]
        copy_from: Option<String>,
    },

    /// Add or update a key binding in a profile
    Bind {
        /// Target profile name
        profile: String,
        /// Key combination (e.g. "SUPER + W", "ALT + F4")
        key: String,
        /// Action identifier (e.g. close_window, snap_left, maximize, "exec:foot")
        action: String,
        /// Optional description
        #[arg(short, long)]
        desc: Option<String>,
    },

    /// Remove a key binding from a profile
    Unbind {
        /// Target profile name
        profile: String,
        /// Key combination to remove
        key: String,
    },

    /// Persist a profile from memory to the XDG config presets directory
    Save {
        /// Profile name to save
        profile: String,
    },

    /// Delete a profile from memory and disk
    Delete {
        /// Profile name to delete
        profile: String,
    },
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    /// Directory containing custom profile TOML definitions
    #[arg(short, long, default_value = "presets")]
    pub preset_dir: PathBuf,

    /// Verbose logging level (-v for debug, -vv for trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// Disable desktop notifications
    #[arg(long)]
    pub no_notify: bool,

}

#[derive(Debug, Args, Clone, PartialEq, Eq)]
pub struct WindowModeArgs {
    #[command(subcommand)]
    pub action: Option<WindowModeAction>,

    /// Output in JSON format
    #[arg(long)]
    pub json: bool,
}

impl WindowModeArgs {
    pub fn is_json(&self) -> bool {
        self.json
            || match &self.action {
                Some(WindowModeAction::Set { json, .. }) => *json,
                Some(WindowModeAction::Status { json }) => *json,
                None => false,
            }
    }
}

#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum WindowModeAction {
    /// Set the active window paradigm policy
    Set {
        /// Policy name: floating, tiled, omarchy, follow-profile
        policy: String,

        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },

    /// Query the current window paradigm policy
    #[command(alias = "get")]
    Status {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
}

pub fn parse_window_policy(s: &str) -> Result<WindowPolicy, String> {
    match s.trim().to_lowercase().replace('_', "-").as_str() {
        "omarchy" => Ok(WindowPolicy::Omarchy),
        "tiled" => Ok(WindowPolicy::Tiled),
        "floating" => Ok(WindowPolicy::Floating),
        "follow-profile" | "follow" => Ok(WindowPolicy::FollowProfile),
        _ => Err(format!(
            "Invalid window policy: '{}'. Available policies: omarchy, tiled, floating, follow-profile",
            s
        )),
    }
}
