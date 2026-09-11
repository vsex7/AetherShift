use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Window snap layout presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapLayout {
    HalfLeft,
    HalfRight,
    HalfTop,
    HalfBottom,
    TwoThirdsLeft,
    OneThirdRight,
    OneThirdLeft,
    TwoThirdsRight,
    ThreeColumnsLeft,
    ThreeColumnsCenter,
    ThreeColumnsRight,
    Maximize,
    CenterFloating,
    RestoreOriginal,
}

impl SnapLayout {
    /// Returns the canonical kebab-case string representation of the layout.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HalfLeft => "half-left",
            Self::HalfRight => "half-right",
            Self::HalfTop => "half-top",
            Self::HalfBottom => "half-bottom",
            Self::TwoThirdsLeft => "two-thirds-left",
            Self::OneThirdRight => "one-third-right",
            Self::OneThirdLeft => "one-third-left",
            Self::TwoThirdsRight => "two-thirds-right",
            Self::ThreeColumnsLeft => "three-columns-left",
            Self::ThreeColumnsCenter => "three-columns-center",
            Self::ThreeColumnsRight => "three-columns-right",
            Self::Maximize => "maximize",
            Self::CenterFloating => "center-floating",
            Self::RestoreOriginal => "restore-original",
        }
    }
}

impl FromStr for SnapLayout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "half-left" | "left" => Ok(Self::HalfLeft),
            "half-right" | "right" => Ok(Self::HalfRight),
            "half-top" | "top" => Ok(Self::HalfTop),
            "half-bottom" | "bottom" => Ok(Self::HalfBottom),
            "two-thirds-left" | "2/3-left" => Ok(Self::TwoThirdsLeft),
            "one-third-right" | "1/3-right" => Ok(Self::OneThirdRight),
            "one-third-left" | "1/3-left" => Ok(Self::OneThirdLeft),
            "two-thirds-right" | "2/3-right" => Ok(Self::TwoThirdsRight),
            "three-columns-left" | "3col-left" | "three-col-left" => Ok(Self::ThreeColumnsLeft),
            "three-columns-center" | "3col-center" | "three-col-center" => {
                Ok(Self::ThreeColumnsCenter)
            }
            "three-columns-right" | "3col-right" | "three-col-right" => Ok(Self::ThreeColumnsRight),
            "maximize" | "max" => Ok(Self::Maximize),
            "center-floating" | "center" | "floating-center" => Ok(Self::CenterFloating),
            "restore-original" | "restore" => Ok(Self::RestoreOriginal),
            other => Err(format!("unknown snap layout: '{other}'")),
        }
    }
}

impl std::fmt::Display for SnapLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Daemon usage statistics
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub profile_usage: HashMap<String, u64>,
    pub action_counts: HashMap<String, u64>,
    pub conflict_hits: HashMap<String, u64>,
    pub total_switches: u64,
    pub total_actions: u64,
    pub uptime_secs: u64,
}

/// Conflict resolution policy used when a target binding collides with a protected baseline binding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    #[default]
    Strict,
    Force,
    Smart,
}

impl ConflictPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Force => "force",
            Self::Smart => "smart",
        }
    }
}

/// Runtime policy applied to newly created ordinary windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowPolicy {
    #[default]
    Omarchy,
    Tiled,
    Floating,
    FollowProfile,
}

impl WindowPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Omarchy => "omarchy",
            Self::Tiled => "tiled",
            Self::Floating => "floating",
            Self::FollowProfile => "follow-profile",
        }
    }
}

/// Structured daemon event pushed on a subscribed UDS connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "kebab-case")]
pub enum Event {
    SwitchSucceeded {
        profile: String,
        duration_us: u128,
        applied: usize,
        skipped: usize,
        forced: usize,
    },
    SwitchFailed {
        profile: String,
        error: String,
    },
    Restored {
        duration_us: u128,
        unbound: usize,
        rebound: usize,
    },
    WindowPolicyChanged {
        policy: WindowPolicy,
    },
    PresetsReloaded {
        count: usize,
    },
}

/// Profile and shortcut optimization recommendation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recommendation {
    pub id: String,
    pub title: String,
    pub message: String,
    pub suggestion_type: String,
    pub suggested_action: Option<String>,
}

/// Local, opt-in permission scopes supported by script plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPermissionScope {
    None,
    HyprlandDispatch,
    Shell,
    Clipboard,
    Notification,
}

/// A registered Phase 5 script-backed action plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub version: String,
    pub description: String,
    pub executable: String,
    pub timeout_ms: u64,
    pub permissions: Vec<PluginPermissionScope>,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One diagnostic check performed by doctor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub id: String,
    pub label: String,
    pub status: DiagnosticStatus,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
}

/// Full doctor report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub healthy: bool,
    pub backend: String,
    pub checks: Vec<DiagnosticCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_unix_ms: Option<u128>,
}

/// On-demand metrics snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricsReport {
    pub uptime_secs: u64,
    pub total_switches: u64,
    pub failed_switches: u64,
    pub average_switch_duration_us: u128,
    pub max_switch_duration_us: u128,
    pub skipped_conflicts: usize,
    pub forced_overrides: usize,
    pub active_connections: usize,
    pub plugins_loaded: usize,
    pub plugins_failed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetricsFormat {
    Json,
    Text,
}

/// Request sent from client to daemon
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum Request {
    Status,
    ListProfiles,
    Switch {
        profile: String,
        force: bool,
    },
    Cycle,
    Restore,
    Shutdown,

    // Phase 2 experience layer
    SwitchWithPolicy {
        profile: String,
        policy: ConflictPolicy,
    },
    WindowMode {
        policy: Option<WindowPolicy>,
    },
    History {
        limit: Option<usize>,
    },
    ReloadPresets,
    UpsertProfile {
        toml: String,
    },
    GetProfile {
        name: String,
    },
    Subscribe {
        filters: Vec<String>,
    },

    // Window layout
    ApplyLayout {
        layout: SnapLayout,
        #[serde(default)]
        preview: bool,
    },
    MoveWindowToMonitor {
        direction: String,
    },

    // Profile runtime editing and persistence
    SaveCurrentAsProfile {
        name: String,
        description: Option<String>,
    },
    SwapWindow {
        direction: String,
    },
    CreateProfile {
        name: String,
        description: Option<String>,
        copy_from: Option<String>,
    },
    UpdateBinding {
        profile: String,
        key_combo: String,
        action: String,
        description: Option<String>,
    },
    RemoveBinding {
        profile: String,
        key_combo: String,
    },
    SaveProfile {
        profile: String,
    },
    DeleteProfile {
        profile: String,
    },

    // Statistics and recommendations
    GetStats,
    GetRecommendations,
    ExportStats {
        path: Option<String>,
    },

    // Phase 5 production readiness
    Doctor {
        export: Option<String>,
    },
    Metrics {
        format: MetricsFormat,
    },
    PluginReload,
    PluginList,
    PluginValidate {
        id: Option<String>,
    },
    PluginRun {
        id: String,
    },
}

/// Response returned from daemon to client
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum Response {
    Success {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        data: Option<serde_json::Value>,
    },
    Error {
        code: String,
        message: String,
    },
    Event {
        event: Event,
    },
}

impl Response {
    pub fn ok(message: impl Into<String>) -> Self {
        Self::Success {
            message: message.into(),
            data: None,
        }
    }

    pub fn ok_with_data<T: Serialize>(
        message: impl Into<String>,
        data: &T,
    ) -> Result<Self, serde_json::Error> {
        let value = serde_json::to_value(data)?;
        Ok(Self::Success {
            message: message.into(),
            data: Some(value),
        })
    }

    pub fn err(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

/// Status information structure returned in Status response data
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusInfo {
    pub active_profile: String,
    pub overlays_count: usize,
    pub uptime_secs: u64,
    pub version: String,
    #[serde(default)]
    pub window_policy: WindowPolicy,
    #[serde(default)]
    pub skipped_conflicts: usize,
    #[serde(default)]
    pub forced_overrides: usize,
    #[serde(default)]
    pub last_switch_duration_us: u128,
    #[serde(default)]
    pub last_switch_succeeded: bool,
}

/// One in-memory switch history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub sequence: u64,
    pub unix_ms: u128,
    pub profile: String,
    pub succeeded: bool,
    pub duration_us: u128,
    pub applied: usize,
    pub skipped: usize,
    pub forced: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// A single user-facing keybinding row for cheat-sheet clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingInfo {
    pub key_combo: String,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Structured result of a snap or preview operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutFeedback {
    pub layout: String,
    pub preview: bool,
    pub monitor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<[i32; 4]>,
    pub to: [i32; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl StatusInfo {
    pub fn new(
        active_profile: impl Into<String>,
        overlays_count: usize,
        uptime_secs: u64,
        version: impl Into<String>,
    ) -> Self {
        Self {
            active_profile: active_profile.into(),
            overlays_count,
            uptime_secs,
            version: version.into(),
            window_policy: WindowPolicy::Omarchy,
            skipped_conflicts: 0,
            forced_overrides: 0,
            last_switch_duration_us: 0,
            last_switch_succeeded: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let requests = vec![
            (Request::Status, r#"{"type":"Status"}"#),
            (Request::ListProfiles, r#"{"type":"ListProfiles"}"#),
            (
                Request::Switch {
                    profile: "gaming".to_string(),
                    force: true,
                },
                r#"{"type":"Switch","payload":{"profile":"gaming","force":true}}"#,
            ),
            (Request::Cycle, r#"{"type":"Cycle"}"#),
            (Request::Restore, r#"{"type":"Restore"}"#),
            (Request::Shutdown, r#"{"type":"Shutdown"}"#),
            (
                Request::ApplyLayout {
                    layout: SnapLayout::HalfLeft,
                    preview: false,
                },
                r#"{"type":"ApplyLayout","payload":{"layout":"half-left","preview":false}}"#,
            ),
            (
                Request::MoveWindowToMonitor {
                    direction: "right".to_string(),
                },
                r#"{"type":"MoveWindowToMonitor","payload":{"direction":"right"}}"#,
            ),
            (
                Request::CreateProfile {
                    name: "custom".to_string(),
                    description: Some("My custom profile".to_string()),
                    copy_from: Some("windows".to_string()),
                },
                r#"{"type":"CreateProfile","payload":{"name":"custom","description":"My custom profile","copy_from":"windows"}}"#,
            ),
            (
                Request::UpdateBinding {
                    profile: "custom".to_string(),
                    key_combo: "Super+Return".to_string(),
                    action: "terminal".to_string(),
                    description: Some("Launch terminal".to_string()),
                },
                r#"{"type":"UpdateBinding","payload":{"profile":"custom","key_combo":"Super+Return","action":"terminal","description":"Launch terminal"}}"#,
            ),
            (
                Request::RemoveBinding {
                    profile: "custom".to_string(),
                    key_combo: "Super+Return".to_string(),
                },
                r#"{"type":"RemoveBinding","payload":{"profile":"custom","key_combo":"Super+Return"}}"#,
            ),
            (
                Request::SaveProfile {
                    profile: "custom".to_string(),
                },
                r#"{"type":"SaveProfile","payload":{"profile":"custom"}}"#,
            ),
            (
                Request::DeleteProfile {
                    profile: "custom".to_string(),
                },
                r#"{"type":"DeleteProfile","payload":{"profile":"custom"}}"#,
            ),
            (Request::GetStats, r#"{"type":"GetStats"}"#),
            (
                Request::GetRecommendations,
                r#"{"type":"GetRecommendations"}"#,
            ),
            (
                Request::ExportStats {
                    path: Some("/tmp/stats.json".to_string()),
                },
                r#"{"type":"ExportStats","payload":{"path":"/tmp/stats.json"}}"#,
            ),
            (
                Request::ExportStats { path: None },
                r#"{"type":"ExportStats","payload":{"path":null}}"#,
            ),
        ];

        for (req, expected_json) in requests {
            let serialized = serde_json::to_string(&req).expect("serialize request");
            assert_eq!(serialized, expected_json);
            let deserialized: Request =
                serde_json::from_str(&serialized).expect("deserialize request");
            assert_eq!(deserialized, req);
        }
    }

    #[test]
    fn test_snap_layout_parsing_and_str() {
        let cases = vec![
            (
                SnapLayout::HalfLeft,
                "half-left",
                &["half-left", "left"][..],
            ),
            (
                SnapLayout::HalfRight,
                "half-right",
                &["half-right", "right"],
            ),
            (SnapLayout::HalfTop, "half-top", &["half-top", "top"]),
            (
                SnapLayout::HalfBottom,
                "half-bottom",
                &["half-bottom", "bottom"],
            ),
            (
                SnapLayout::TwoThirdsLeft,
                "two-thirds-left",
                &["two-thirds-left", "2/3-left"],
            ),
            (
                SnapLayout::OneThirdRight,
                "one-third-right",
                &["one-third-right", "1/3-right"],
            ),
            (
                SnapLayout::OneThirdLeft,
                "one-third-left",
                &["one-third-left", "1/3-left"],
            ),
            (
                SnapLayout::TwoThirdsRight,
                "two-thirds-right",
                &["two-thirds-right", "2/3-right"],
            ),
            (
                SnapLayout::ThreeColumnsLeft,
                "three-columns-left",
                &["three-columns-left", "3col-left", "three-col-left"],
            ),
            (
                SnapLayout::ThreeColumnsCenter,
                "three-columns-center",
                &["three-columns-center", "3col-center", "three-col-center"],
            ),
            (
                SnapLayout::ThreeColumnsRight,
                "three-columns-right",
                &["three-columns-right", "3col-right", "three-col-right"],
            ),
            (SnapLayout::Maximize, "maximize", &["maximize", "max"]),
            (
                SnapLayout::CenterFloating,
                "center-floating",
                &["center-floating", "center", "floating-center"],
            ),
            (
                SnapLayout::RestoreOriginal,
                "restore-original",
                &["restore-original", "restore"],
            ),
        ];

        for (layout, canonical, aliases) in cases {
            assert_eq!(layout.as_str(), canonical);
            assert_eq!(layout.to_string(), canonical);

            // test serde json serialization uses kebab-case
            let json = serde_json::to_string(&layout).expect("serde serialize");
            assert_eq!(json, format!("\"{canonical}\""));
            let deserialized: SnapLayout = serde_json::from_str(&json).expect("serde deserialize");
            assert_eq!(deserialized, layout);

            // test from_str aliases and case insensitivity
            for alias in aliases {
                assert_eq!(SnapLayout::from_str(alias).unwrap(), layout);
                assert_eq!(SnapLayout::from_str(&alias.to_uppercase()).unwrap(), layout);
            }
        }

        assert!(SnapLayout::from_str("invalid-layout").is_err());
    }

    #[test]
    fn test_usage_stats_serialization() {
        let mut stats = UsageStats::default();
        stats.profile_usage.insert("windows".to_string(), 12);
        stats.action_counts.insert("snap_left".to_string(), 42);
        stats.conflict_hits.insert("Super+Tab".to_string(), 3);
        stats.total_switches = 15;
        stats.total_actions = 100;
        stats.uptime_secs = 7200;

        let json = serde_json::to_string(&stats).expect("serialize UsageStats");
        let deserialized: UsageStats = serde_json::from_str(&json).expect("deserialize UsageStats");
        assert_eq!(deserialized, stats);
    }

    #[test]
    fn test_recommendation_serialization() {
        let rec = Recommendation {
            id: "rec_1".to_string(),
            title: "Unused Binding".to_string(),
            message: "You rarely use Super+Shift+W".to_string(),
            suggestion_type: "shortcut_cleanup".to_string(),
            suggested_action: Some("remove_binding".to_string()),
        };

        let json = serde_json::to_string(&rec).expect("serialize Recommendation");
        let deserialized: Recommendation =
            serde_json::from_str(&json).expect("deserialize Recommendation");
        assert_eq!(deserialized, rec);
    }

    #[test]
    fn test_response_serialization() {
        let resp_ok = Response::ok("Operation successful");
        let json_ok = serde_json::to_string(&resp_ok).expect("serialize resp_ok");
        assert_eq!(
            json_ok,
            r#"{"status":"Success","message":"Operation successful"}"#
        );
        let deserialized_ok: Response =
            serde_json::from_str(&json_ok).expect("deserialize resp_ok");
        assert_eq!(deserialized_ok, resp_ok);

        let status_info = StatusInfo::new("default", 3, 120, "0.1.0");
        let resp_with_data =
            Response::ok_with_data("Status retrieved", &status_info).expect("ok_with_data");
        let json_data = serde_json::to_string(&resp_with_data).expect("serialize resp_with_data");
        let deserialized_data: Response =
            serde_json::from_str(&json_data).expect("deserialize resp_with_data");
        assert_eq!(deserialized_data, resp_with_data);

        let resp_err = Response::err("PROFILE_NOT_FOUND", "Profile foo does not exist");
        let json_err = serde_json::to_string(&resp_err).expect("serialize resp_err");
        assert_eq!(
            json_err,
            r#"{"status":"Error","code":"PROFILE_NOT_FOUND","message":"Profile foo does not exist"}"#
        );
        let deserialized_err: Response =
            serde_json::from_str(&json_err).expect("deserialize resp_err");
        assert_eq!(deserialized_err, resp_err);
    }

    #[test]
    fn test_status_info() {
        let status = StatusInfo {
            active_profile: "hypr-nord".to_string(),
            overlays_count: 2,
            uptime_secs: 3600,
            version: "0.1.0".to_string(),
            window_policy: WindowPolicy::Omarchy,
            skipped_conflicts: 0,
            forced_overrides: 0,
            last_switch_duration_us: 0,
            last_switch_succeeded: true,
        };
        let json = serde_json::to_string(&status).expect("serialize status");
        let deserialized: StatusInfo = serde_json::from_str(&json).expect("deserialize status");
        assert_eq!(deserialized, status);
    }
}
