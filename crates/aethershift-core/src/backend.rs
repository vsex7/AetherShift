use aethershift_hyprland::{HyprKeyBinding, HyprlandClient};
use async_trait::async_trait;

use crate::error::CoreError;
use crate::layout::Rect;
use crate::state::{BaselineBinding, SwitchPlan};

/// Supported compositor backend kinds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    Hyprland,
    Omarchy,
    Niri,
    Sway,
}

impl BackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Hyprland => "hyprland",
            Self::Omarchy => "omarchy",
            Self::Niri => "niri",
            Self::Sway => "sway",
        }
    }
}

impl std::str::FromStr for BackendKind {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "hyprland" => Ok(Self::Hyprland),
            "omarchy" => Ok(Self::Omarchy),
            "niri" => Ok(Self::Niri),
            "sway" => Ok(Self::Sway),
            _ => Err(CoreError::Validation {
                profile: "backend".to_string(),
                message: format!("Unknown backend: '{}'", s),
            }),
        }
    }
}

/// Runtime capability matrix for feature negotiation across compositors
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendCapabilities {
    pub dynamic_keybinds: bool,
    pub exact_window_geometry: bool,
    pub multi_monitor_aware: bool,
    pub socket_events: bool,
    pub floating_toggle: bool,
}

impl Default for BackendCapabilities {
    fn default() -> Self {
        Self {
            dynamic_keybinds: true,
            exact_window_geometry: true,
            multi_monitor_aware: true,
            socket_events: true,
            floating_toggle: true,
        }
    }
}

/// Runtime health reported by a compositor backend
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BackendHealth {
    pub kind: BackendKind,
    pub connected: bool,
    pub message: String,
    pub capabilities: BackendCapabilities,
}

/// The transport-neutral contract used by the daemon.
#[async_trait]
pub trait CompositorBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    fn capabilities(&self) -> BackendCapabilities;

    async fn detect(&self) -> Result<bool, CoreError>;

    async fn health(&self) -> Result<BackendHealth, CoreError>;

    async fn fetch_baseline(&self) -> Result<Vec<BaselineBinding>, CoreError>;

    async fn apply_plan(&self, plan: &SwitchPlan) -> Result<(), CoreError>;

    async fn apply_window_rect(
        &self,
        address: &str,
        rect: Rect,
        floating: bool,
    ) -> Result<(), CoreError>;

    async fn move_window_to_monitor(
        &self,
        address: Option<&str>,
        direction: &str,
    ) -> Result<(), CoreError>;
}

/// Omarchy / Hyprland default backend implementation
pub struct HyprlandBackend {
    client: HyprlandClient,
    is_omarchy: bool,
}

impl HyprlandBackend {
    pub fn new(client: HyprlandClient, is_omarchy: bool) -> Self {
        Self { client, is_omarchy }
    }

    pub fn client(&self) -> &HyprlandClient {
        &self.client
    }
}

#[async_trait]
impl CompositorBackend for HyprlandBackend {
    fn kind(&self) -> BackendKind {
        if self.is_omarchy {
            BackendKind::Omarchy
        } else {
            BackendKind::Hyprland
        }
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::default()
    }

    async fn detect(&self) -> Result<bool, CoreError> {
        Ok(self.client.socket_path().exists())
    }

    async fn health(&self) -> Result<BackendHealth, CoreError> {
        let connected = self.client.socket_path().exists();
        let message = if connected {
            format!(
                "Connected to {} at {}",
                self.kind().as_str(),
                self.client.socket_path().display()
            )
        } else {
            "Socket disconnected".to_string()
        };

        Ok(BackendHealth {
            kind: self.kind(),
            connected,
            message,
            capabilities: self.capabilities(),
        })
    }

    async fn fetch_baseline(&self) -> Result<Vec<BaselineBinding>, CoreError> {
        let raw = self.client.fetch_binds().await?;
        let baseline: Vec<BaselineBinding> = raw
            .into_iter()
            .filter_map(|b| {
                let combo = b.formatted_combo()?.parse().ok()?;
                Some(BaselineBinding {
                    key_combo: combo,
                    dispatcher: b.dispatcher,
                    args: b.arg,
                    description: Some(b.description),
                })
            })
            .collect();
        Ok(baseline)
    }

    async fn apply_plan(&self, plan: &SwitchPlan) -> Result<(), CoreError> {
        let unbinds: Vec<String> = plan
            .unbind
            .iter()
            .map(|combo| combo.canonical_str())
            .collect();

        let binds: Vec<HyprKeyBinding> = plan
            .bind
            .iter()
            .map(|b| {
                let keys = b.key_combo.canonical_str();
                let (disp, args) = b.action.to_hyprland_dispatcher();
                hypr_key_binding(&keys, &disp, &args, b.description.as_deref())
            })
            .collect();

        self.client.apply_batch(&unbinds, &binds).await?;
        Ok(())
    }

    async fn apply_window_rect(
        &self,
        address: &str,
        rect: Rect,
        floating: bool,
    ) -> Result<(), CoreError> {
        self.client
            .apply_window_rect(address, rect.x, rect.y, rect.width, rect.height, floating)
            .await?;
        Ok(())
    }

    async fn move_window_to_monitor(
        &self,
        address: Option<&str>,
        direction: &str,
    ) -> Result<(), CoreError> {
        self.client
            .move_window_to_monitor(address, direction)
            .await?;
        Ok(())
    }
}

/// Mock / Experimental Sway backend stub providing graceful fallback
pub struct SwayBackend {
    socket_path: Option<std::path::PathBuf>,
}

impl SwayBackend {
    pub fn new() -> Self {
        let path = std::env::var("SWAYSOCK").ok().map(std::path::PathBuf::from);
        Self { socket_path: path }
    }
}

#[async_trait]
impl CompositorBackend for SwayBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Sway
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            dynamic_keybinds: false, // Sway requires config reloading for binds
            exact_window_geometry: true,
            multi_monitor_aware: true,
            socket_events: true,
            floating_toggle: true,
        }
    }

    async fn detect(&self) -> Result<bool, CoreError> {
        Ok(self
            .socket_path
            .as_ref()
            .map(|p| p.exists())
            .unwrap_or(false))
    }

    async fn health(&self) -> Result<BackendHealth, CoreError> {
        let connected = self.detect().await?;
        Ok(BackendHealth {
            kind: BackendKind::Sway,
            connected,
            message: if connected {
                "Sway IPC active (experimental)".to_string()
            } else {
                "SWAYSOCK not found or socket inactive".to_string()
            },
            capabilities: self.capabilities(),
        })
    }

    async fn fetch_baseline(&self) -> Result<Vec<BaselineBinding>, CoreError> {
        Ok(Vec::new())
    }

    async fn apply_plan(&self, _plan: &SwitchPlan) -> Result<(), CoreError> {
        Ok(())
    }

    async fn apply_window_rect(
        &self,
        _address: &str,
        _rect: Rect,
        _floating: bool,
    ) -> Result<(), CoreError> {
        Ok(())
    }

    async fn move_window_to_monitor(
        &self,
        _address: Option<&str>,
        _direction: &str,
    ) -> Result<(), CoreError> {
        Ok(())
    }
}

impl From<aethershift_hyprland::HyprlandError> for CoreError {
    fn from(value: aethershift_hyprland::HyprlandError) -> Self {
        CoreError::Validation {
            profile: "backend".to_string(),
            message: value.to_string(),
        }
    }
}

pub fn hypr_key_binding(
    keys: &str,
    action: &str,
    args: &str,
    description: Option<&str>,
) -> HyprKeyBinding {
    let lua = format!("hl.dsp.{}(\"{}\")", action, args);
    let mut binding = HyprKeyBinding::new(
        keys.to_string(),
        aethershift_hyprland::HyprAction::CustomLua(lua),
    );
    if let Some(desc) = description {
        binding = binding.with_description(desc);
    }
    binding
}
