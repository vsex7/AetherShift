use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use crate::error::CoreError;

/// Normalized modifier representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Modifier {
    Super,
    Ctrl,
    Alt,
    Shift,
}

impl Modifier {
    /// Canonical name used in Hyprland dispatcher commands (e.g., SUPER, CTRL, ALT, SHIFT)
    pub fn as_str(&self) -> &'static str {
        match self {
            Modifier::Super => "SUPER",
            Modifier::Ctrl => "CTRL",
            Modifier::Alt => "ALT",
            Modifier::Shift => "SHIFT",
        }
    }
}

impl FromStr for Modifier {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean = s.trim().to_uppercase();
        match clean.as_str() {
            "SUPER" | "MOD4" | "WIN" | "WINDOWS" | "LOGO" | "CMD" | "COMMAND" => {
                Ok(Modifier::Super)
            }
            "CTRL" | "CONTROL" => Ok(Modifier::Ctrl),
            "ALT" | "MOD1" | "OPTION" | "OPT" => Ok(Modifier::Alt),
            "SHIFT" => Ok(Modifier::Shift),
            _ => Err(CoreError::InvalidKeyCombo {
                raw: s.to_string(),
                reason: format!("Unknown modifier '{s}'. Expected SUPER, CTRL, ALT, or SHIFT"),
            }),
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Normalized Key Combination, e.g., "SUPER + LEFT", "ALT + F4", "CTRL + SHIFT + ESCAPE"
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeyCombo {
    pub modifiers: BTreeSet<Modifier>,
    pub key: String,
}

impl KeyCombo {
    pub fn new(
        modifiers: impl IntoIterator<Item = Modifier>,
        key: impl Into<String>,
    ) -> Result<Self, CoreError> {
        let key_str = key.into().trim().to_uppercase();
        if key_str.is_empty() {
            return Err(CoreError::InvalidKeyCombo {
                raw: String::new(),
                reason: "Key part cannot be empty".to_string(),
            });
        }
        let mods: BTreeSet<Modifier> = modifiers.into_iter().collect();
        Ok(Self {
            modifiers: mods,
            key: key_str,
        })
    }

    /// Return canonical string representation (e.g. "CTRL + SHIFT + ESCAPE")
    pub fn canonical_str(&self) -> String {
        let mut parts = Vec::new();
        // Prescribed order: SUPER, CTRL, ALT, SHIFT
        for m in [
            Modifier::Super,
            Modifier::Ctrl,
            Modifier::Alt,
            Modifier::Shift,
        ] {
            if self.modifiers.contains(&m) {
                parts.push(m.as_str());
            }
        }
        parts.push(&self.key);
        parts.join(" + ")
    }

    /// Modifiers formatted for Hyprland (e.g. "SUPER" or "SUPER_CTRL" or "" for none)
    pub fn hyprland_mods(&self) -> String {
        let mut parts = Vec::new();
        for m in [
            Modifier::Super,
            Modifier::Ctrl,
            Modifier::Alt,
            Modifier::Shift,
        ] {
            if self.modifiers.contains(&m) {
                parts.push(m.as_str());
            }
        }
        if parts.is_empty() {
            "".to_string()
        } else {
            parts.join(" ")
        }
    }

    /// Key string formatted for Hyprland dispatcher
    pub fn hyprland_key(&self) -> &str {
        &self.key
    }
}

impl FromStr for KeyCombo {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_trimmed = s.trim();
        if s_trimmed.is_empty() {
            return Err(CoreError::InvalidKeyCombo {
                raw: s.to_string(),
                reason: "Key combo cannot be empty".to_string(),
            });
        }

        let tokens: Vec<&str> = s_trimmed.split('+').map(|t| t.trim()).collect();
        if tokens.is_empty() {
            return Err(CoreError::InvalidKeyCombo {
                raw: s.to_string(),
                reason: "Key combo cannot be empty".to_string(),
            });
        }

        let mut modifiers = BTreeSet::new();
        let key_token = tokens.last().copied().unwrap_or("");

        if key_token.is_empty() {
            return Err(CoreError::InvalidKeyCombo {
                raw: s.to_string(),
                reason: "Key part missing after '+'".to_string(),
            });
        }

        for &mod_str in &tokens[..tokens.len() - 1] {
            let m = Modifier::from_str(mod_str)?;
            modifiers.insert(m);
        }

        let key_norm = key_token.to_uppercase();

        Ok(KeyCombo {
            modifiers,
            key: key_norm,
        })
    }
}

impl fmt::Display for KeyCombo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical_str())
    }
}

impl Serialize for KeyCombo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.canonical_str())
    }
}

impl<'de> Deserialize<'de> for KeyCombo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        KeyCombo::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Action to execute upon key binding trigger
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "arg", rename_all = "snake_case")]
pub enum Action {
    CloseWindow,
    SnapLeft,
    SnapRight,
    Maximize,
    Restore,
    FloatToggle,
    Fullscreen,
    WorkspaceNext,
    WorkspacePrev,
    CycleWindowNext,
    CycleWindowPrev,
    MoveToWorkspaceNext,
    MoveToWorkspacePrev,
    ToggleSpecialWorkspace(Option<String>),
    ToggleLauncher(Option<String>),
    Exec(String),
    Plugin(String),
    Custom { dispatcher: String, args: String },
}

impl Action {
    /// Return the Hyprland dispatcher name and arguments for this action
    pub fn to_hyprland_dispatcher(&self) -> (String, String) {
        match self {
            Action::CloseWindow => ("killactive".to_string(), "".to_string()),
            Action::SnapLeft => ("movefocus".to_string(), "l".to_string()),
            Action::SnapRight => ("movefocus".to_string(), "r".to_string()),
            Action::Maximize => ("fullscreen".to_string(), "1".to_string()), // 1 = maximize/keep gaps/status
            Action::Restore => ("fullscreen".to_string(), "0".to_string()),
            Action::FloatToggle => ("togglefloating".to_string(), "".to_string()),
            Action::Fullscreen => ("fullscreen".to_string(), "0".to_string()), // toggle standard fullscreen
            Action::WorkspaceNext => ("workspace".to_string(), "m+1".to_string()),
            Action::WorkspacePrev => ("workspace".to_string(), "m-1".to_string()),
            Action::CycleWindowNext => ("cyclenext".to_string(), "".to_string()),
            Action::CycleWindowPrev => ("cycleprev".to_string(), "".to_string()),
            Action::MoveToWorkspaceNext => ("movetoworkspacesilent".to_string(), "e+1".to_string()),
            Action::MoveToWorkspacePrev => ("movetoworkspacesilent".to_string(), "e-1".to_string()),
            Action::ToggleSpecialWorkspace(name) => {
                let arg = name.as_deref().unwrap_or("");
                ("togglespecialworkspace".to_string(), arg.to_string())
            }
            Action::ToggleLauncher(cmd) => {
                let cmd = cmd
                    .as_deref()
                    .unwrap_or("omarchy-launch-walker || rofi -show drun || wofi --show drun");
                ("exec".to_string(), cmd.to_string())
            }
            Action::Exec(cmd) => ("exec".to_string(), cmd.clone()),
            Action::Plugin(id) => (
                "exec".to_string(),
                format!("aethershift plugin run {}", shell_quote_plugin_id(id)),
            ),
            Action::Custom { dispatcher, args } => (dispatcher.clone(), args.clone()),
        }
    }
}

impl FromStr for Action {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(CoreError::InvalidAction {
                raw: s.to_string(),
                reason: "Action cannot be empty".to_string(),
            });
        }

        // Try JSON parsing first (in case it's serialized Action JSON: {"type": "close_window"} or {"type": "exec", "arg": "foot"})
        if trimmed.starts_with('{')
            && let Ok(action) = serde_json::from_str::<Action>(trimmed)
        {
            return Ok(action);
        }

        // String commands and dispatchers
        let lower = trimmed.to_lowercase();
        match lower.as_str() {
            "close" | "close_window" | "closewindow" | "killactive" => Ok(Action::CloseWindow),
            "snap_left" | "snapleft" | "left" => Ok(Action::SnapLeft),
            "snap_right" | "snapright" | "right" => Ok(Action::SnapRight),
            "maximize" | "max" => Ok(Action::Maximize),
            "restore" => Ok(Action::Restore),
            "float_toggle" | "togglefloating" | "toggle_float" | "float" => Ok(Action::FloatToggle),
            "fullscreen" => Ok(Action::Fullscreen),
            "workspace_next" | "workspacenext" | "next_workspace" => Ok(Action::WorkspaceNext),
            "workspace_prev" | "workspaceprev" | "prev_workspace" => Ok(Action::WorkspacePrev),
            "cycle_window_next" | "cycle_next" | "cyclenext" => Ok(Action::CycleWindowNext),
            "cycle_window_prev" | "cycle_prev" | "cycleprev" => Ok(Action::CycleWindowPrev),
            "move_to_workspace_next" | "move_workspace_next" => Ok(Action::MoveToWorkspaceNext),
            "move_to_workspace_prev" | "move_workspace_prev" => Ok(Action::MoveToWorkspacePrev),
            "launcher" | "toggle_launcher" => Ok(Action::ToggleLauncher(None)),
            "special_workspace" | "toggle_special_workspace" => {
                Ok(Action::ToggleSpecialWorkspace(None))
            }
            _ => {
                // If it starts with exec:, treat the rest as cmd
                if let Some(cmd) = trimmed
                    .strip_prefix("exec:")
                    .or_else(|| trimmed.strip_prefix("exec "))
                {
                    Ok(Action::Exec(cmd.trim().to_string()))
                } else if let Some(spec) = trimmed
                    .strip_prefix("plugin:")
                    .or_else(|| trimmed.strip_prefix("plugin "))
                {
                    let id = spec.split('?').next().unwrap_or("").trim();
                    if id.is_empty() {
                        Err(CoreError::InvalidAction {
                            raw: s.to_string(),
                            reason: "plugin action requires an id".to_string(),
                        })
                    } else {
                        Ok(Action::Plugin(id.to_string()))
                    }
                } else if let Some((disp, args)) = trimmed.split_once(':') {
                    Ok(Action::Custom {
                        dispatcher: disp.trim().to_string(),
                        args: args.trim().to_string(),
                    })
                } else {
                    // Fallback to exec command
                    Ok(Action::Exec(trimmed.to_string()))
                }
            }
        }
    }
}

fn shell_quote_plugin_id(id: &str) -> String {
    let mut quoted = String::with_capacity(id.len() + 2);
    quoted.push('\'');
    for ch in id.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// A single binding item mapping a key combo to an action
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Binding {
    pub key_combo: KeyCombo,
    pub action: Action,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
}

impl Binding {
    pub fn new(key_combo: KeyCombo, action: Action) -> Self {
        Self {
            key_combo,
            action,
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}
