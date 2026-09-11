use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use aethershift_protocol::{DiagnosticCheck, DiagnosticStatus, DoctorReport, MetricsReport};

#[derive(Debug, Clone, Default)]
pub struct HealthState {
    pub active_profile: String,
    pub backend: String,
    pub backend_connected: bool,
    pub baseline_count: usize,
    pub active_connections: usize,
    pub total_switches: u64,
    pub failed_switches: u64,
    pub switch_duration_sum_us: u128,
    pub max_switch_duration_us: u128,
    pub last_switch_duration_us: u128,
    pub last_switch_succeeded: bool,
    pub skipped_conflicts: usize,
    pub forced_overrides: usize,
    pub plugins_loaded: usize,
    pub plugins_failed: usize,
    pub last_error: Option<String>,
}

impl HealthState {
    pub fn record_switch(
        &mut self,
        profile: &str,
        duration_us: u128,
        skipped: usize,
        forced: usize,
    ) {
        self.active_profile = profile.to_string();
        self.total_switches = self.total_switches.saturating_add(1);
        self.switch_duration_sum_us = self.switch_duration_sum_us.saturating_add(duration_us);
        self.max_switch_duration_us = self.max_switch_duration_us.max(duration_us);
        self.last_switch_duration_us = duration_us;
        self.last_switch_succeeded = true;
        self.skipped_conflicts = self.skipped_conflicts.saturating_add(skipped);
        self.forced_overrides = self.forced_overrides.saturating_add(forced);
    }

    pub fn record_restore(&mut self, duration_us: u128) {
        self.record_switch("native", duration_us, 0, 0);
    }

    pub fn record_failure(&mut self, message: impl Into<String>) {
        self.failed_switches = self.failed_switches.saturating_add(1);
        self.last_error = Some(message.into());
        self.last_switch_succeeded = false;
    }

    pub fn record_connection_open(&mut self) {
        self.active_connections = self.active_connections.saturating_add(1);
    }

    pub fn record_connection_close(&mut self) {
        self.active_connections = self.active_connections.saturating_sub(1);
    }

    pub fn average_switch_duration_us(&self) -> u128 {
        if self.total_switches == 0 {
            0
        } else {
            self.switch_duration_sum_us / self.total_switches as u128
        }
    }

    pub fn metrics(&self, uptime_secs: u64) -> MetricsReport {
        MetricsReport {
            uptime_secs,
            total_switches: self.total_switches,
            failed_switches: self.failed_switches,
            average_switch_duration_us: self.average_switch_duration_us(),
            max_switch_duration_us: self.max_switch_duration_us,
            skipped_conflicts: self.skipped_conflicts,
            forced_overrides: self.forced_overrides,
            active_connections: self.active_connections,
            plugins_loaded: self.plugins_loaded,
            plugins_failed: self.plugins_failed,
        }
    }
}

pub fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

pub struct DoctorInput<'a> {
    pub uptime_secs: u64,
    pub backend: &'a str,
    pub backend_connected: bool,
    pub backend_message: &'a str,
    pub baseline_count: usize,
    pub active_profile: &'a str,
    pub plugins_loaded: usize,
    pub plugins_failed: usize,
    pub last_error: Option<&'a str>,
}

fn check(
    id: &str,
    label: &str,
    status: DiagnosticStatus,
    message: impl Into<String>,
    remediation: Option<&str>,
) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.to_string(),
        label: label.to_string(),
        status,
        message: message.into(),
        remediation: remediation.map(|value| value.to_string()),
    }
}

pub fn doctor_report(input: &DoctorInput<'_>) -> DoctorReport {
    let mut checks = Vec::new();

    checks.push(check(
        "environment",
        "Desktop environment",
        if input.backend_connected {
            DiagnosticStatus::Ok
        } else {
            DiagnosticStatus::Error
        },
        if input.backend_connected {
            format!("Compositor backend '{}' is connected", input.backend)
        } else {
            "Compositor backend is not connected; daemon can serve state but cannot apply bindings".to_string()
        },
        if input.backend_connected {
            None
        } else {
            Some("Start inside a Hyprland/Omarchy session or set HYPRLAND_INSTANCE_SIGNATURE/XDG_RUNTIME_DIR")
        },
    ));

    checks.push(check(
        "baseline",
        "Baseline bindings",
        if input.baseline_count > 0 {
            DiagnosticStatus::Ok
        } else {
            DiagnosticStatus::Warning
        },
        format!("{} baseline bindings captured", input.baseline_count),
        if input.baseline_count > 0 {
            None
        } else {
            Some("Ensure Hyprland IPC is reachable before switching profiles")
        },
    ));

    checks.push(check(
        "profile",
        "Active profile",
        DiagnosticStatus::Ok,
        format!("Active profile is '{}'", input.active_profile),
        None,
    ));

    checks.push(check(
        "plugins",
        "Script action plugins",
        if input.plugins_failed == 0 {
            DiagnosticStatus::Ok
        } else {
            DiagnosticStatus::Warning
        },
        format!(
            "{} plugins loaded, {} invalid or duplicate plugins quarantined",
            input.plugins_loaded, input.plugins_failed
        ),
        if input.plugins_failed == 0 {
            None
        } else {
            Some("Run `aethershift plugin validate` and remove or fix invalid manifests")
        },
    ));

    if let Some(error) = input.last_error {
        checks.push(check(
            "last-error",
            "Last runtime error",
            DiagnosticStatus::Warning,
            error.to_string(),
            Some("Review daemon logs and run a switch/restore after fixing the cause"),
        ));
    } else {
        checks.push(check(
            "last-error",
            "Last runtime error",
            DiagnosticStatus::Ok,
            "No runtime errors recorded",
            None,
        ));
    }

    let healthy = checks
        .iter()
        .all(|item| item.status != DiagnosticStatus::Error);

    DoctorReport {
        healthy,
        backend: input.backend.to_string(),
        checks,
        generated_unix_ms: Some(unix_ms()),
    }
}

pub fn metrics_text(metrics: &MetricsReport) -> String {
    json!({
        "uptime_secs": metrics.uptime_secs,
        "total_switches": metrics.total_switches,
        "failed_switches": metrics.failed_switches,
        "average_switch_duration_us": metrics.average_switch_duration_us,
        "max_switch_duration_us": metrics.max_switch_duration_us,
        "skipped_conflicts": metrics.skipped_conflicts,
        "forced_overrides": metrics.forced_overrides,
        "active_connections": metrics.active_connections,
        "plugins_loaded": metrics.plugins_loaded,
        "plugins_failed": metrics.plugins_failed,
    })
    .to_string()
}

pub fn plugin_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(dir)
            .join("aethershift")
            .join("plugins")
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home)
            .join(".config")
            .join("aethershift")
            .join("plugins")
    } else {
        std::path::PathBuf::from("/tmp/.config/aethershift/plugins")
    }
}

pub fn current_unix_ms() -> u128 {
    unix_ms()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_metrics_and_doctor() {
        let mut health = HealthState::default();
        health.record_switch("windows", 1200, 2, 1);
        health.record_failure("ipc down");
        health.record_connection_open();

        let metrics = health.metrics(30);
        assert_eq!(metrics.total_switches, 1);
        assert_eq!(metrics.failed_switches, 1);
        assert_eq!(metrics.average_switch_duration_us, 1200);

        let report = doctor_report(&DoctorInput {
            uptime_secs: 30,
            backend: "hyprland",
            backend_connected: false,
            backend_message: "offline",
            baseline_count: 0,
            active_profile: "native",
            plugins_loaded: 1,
            plugins_failed: 1,
            last_error: Some("ipc down"),
        });
        assert!(!report.healthy);
        assert_eq!(report.backend, "hyprland");
        assert!(report.checks.len() >= 5);
    }
}
