use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::error::CoreError;
use crate::profile::Profile;

/// Daemon and Profile usage statistics snapshot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UsageStats {
    pub profile_usage: HashMap<String, u64>,
    pub action_counts: HashMap<String, u64>,
    pub conflict_hits: HashMap<String, u64>,
    pub total_switches: u64,
    pub total_actions: u64,
    pub uptime_secs: u64,
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

impl Recommendation {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        message: impl Into<String>,
        suggestion_type: impl Into<String>,
        suggested_action: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            message: message.into(),
            suggestion_type: suggestion_type.into(),
            suggested_action,
        }
    }
}

/// StatsEngine tracks profile usage duration/activations, action frequencies, and conflict hits,
/// and generates actionable recommendations.
#[derive(Debug, Clone)]
pub struct StatsEngine {
    profile_usage: HashMap<String, u64>,
    action_counts: HashMap<String, u64>,
    conflict_hits: HashMap<String, u64>,
    total_switches: u64,
    total_actions: u64,
    active_profile: Option<String>,
    last_switch_time: std::time::Instant,
    start_time: std::time::Instant,
}

impl Default for StatsEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl StatsEngine {
    pub fn new() -> Self {
        let now = std::time::Instant::now();
        Self {
            profile_usage: HashMap::new(),
            action_counts: HashMap::new(),
            conflict_hits: HashMap::new(),
            total_switches: 0,
            total_actions: 0,
            active_profile: None,
            last_switch_time: now,
            start_time: now,
        }
    }

    /// Record a profile switch. Flushes duration of the previous active profile.
    pub fn record_switch(&mut self, new_profile: &str) {
        let now = std::time::Instant::now();
        if let Some(prev) = &self.active_profile {
            let elapsed_secs = now.duration_since(self.last_switch_time).as_secs();
            *self.profile_usage.entry(prev.clone()).or_insert(0) += elapsed_secs;
        }

        self.active_profile = Some(new_profile.to_string());
        self.last_switch_time = now;
        self.total_switches += 1;
    }

    /// Record action execution
    pub fn record_action(&mut self, action_name: &str) {
        *self
            .action_counts
            .entry(action_name.to_string())
            .or_insert(0) += 1;
        self.total_actions += 1;
    }

    /// Record conflict hit (e.g. key combo conflict or override)
    pub fn record_conflict(&mut self, combo: &str) {
        *self.conflict_hits.entry(combo.to_string()).or_insert(0) += 1;
    }

    /// Return current UsageStats snapshot
    pub fn get_stats(&self) -> UsageStats {
        let now = std::time::Instant::now();
        let mut usage = self.profile_usage.clone();
        if let Some(active) = &self.active_profile {
            let current_duration = now.duration_since(self.last_switch_time).as_secs();
            *usage.entry(active.clone()).or_insert(0) += current_duration;
        }

        UsageStats {
            profile_usage: usage,
            action_counts: self.action_counts.clone(),
            conflict_hits: self.conflict_hits.clone(),
            total_switches: self.total_switches,
            total_actions: self.total_actions,
            uptime_secs: now.duration_since(self.start_time).as_secs(),
        }
    }

    /// Generate intelligent optimization recommendations based on usage statistics
    pub fn generate_recommendations(&self, current_profile: &Profile) -> Vec<Recommendation> {
        let mut recs = Vec::new();

        // 1. High frequency action without shortcut in current profile
        for (action_name, &count) in &self.action_counts {
            if count >= 3 {
                let is_bound = current_profile.bindings.iter().any(|b| {
                    let (disp, arg) = b.action.to_hyprland_dispatcher();
                    action_name == &disp
                        || (action_name.starts_with("snap") && disp == "movefocus")
                        || (action_name == "close_window" && disp == "killactive")
                        || (action_name == "maximize" && disp == "fullscreen" && arg == "1")
                });

                if !is_bound {
                    recs.push(Recommendation::new(
                        format!("rec_action_{action_name}"),
                        format!("Frequent Action Unbound: {action_name}"),
                        format!(
                            "Action '{action_name}' has been invoked {count} times, but has no key binding in current profile '{}'. Consider assigning a shortcut.",
                            current_profile.name
                        ),
                        "missing_binding",
                        Some(action_name.clone()),
                    ));
                }
            }
        }

        // 2. High conflict hits on key combos
        for (combo, &hits) in &self.conflict_hits {
            if hits >= 2 {
                recs.push(Recommendation::new(
                    format!("rec_conflict_{combo}"),
                    format!("High Conflict Rate: {combo}"),
                    format!(
                        "Key combo '{combo}' has encountered conflicts {hits} times. Consider remapping this shortcut to prevent collision.",
                    ),
                    "conflict_resolution",
                    Some(combo.clone()),
                ));
            }
        }

        // 3. Unused profile warning if total_switches >= 5 and some profiles have 0 usage
        for (profile_name, &duration) in &self.profile_usage {
            if profile_name != &current_profile.name && duration == 0 && self.total_switches >= 5 {
                recs.push(Recommendation::new(
                    format!("rec_unused_profile_{profile_name}"),
                    format!("Unused Profile: {profile_name}"),
                    format!(
                        "Profile '{profile_name}' has not been actively used. You might consider removing or consolidating it.",
                    ),
                    "profile_cleanup",
                    Some(profile_name.clone()),
                ));
            }
        }

        recs.sort_by(|a, b| a.id.cmp(&b.id));
        recs
    }

    /// Export statistics as a JSON string
    pub fn export_json(&self) -> Result<String, CoreError> {
        let stats = self.get_stats();
        serde_json::to_string_pretty(&stats).map_err(|e| CoreError::Validation {
            profile: "stats".to_string(),
            message: format!("Failed to serialize stats to JSON: {e}"),
        })
    }

    /// Export statistics to a JSON file
    pub fn export_to_file(&self, path: impl AsRef<Path>) -> Result<(), CoreError> {
        let path_ref = path.as_ref();
        let json = self.export_json()?;
        if let Some(parent) = path_ref.parent()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| CoreError::Io {
                path: parent.to_path_buf(),
                source: e,
            })?;
        }
        let mut file = File::create(path_ref).map_err(|e| CoreError::Io {
            path: path_ref.to_path_buf(),
            source: e,
        })?;
        file.write_all(json.as_bytes()).map_err(|e| CoreError::Io {
            path: path_ref.to_path_buf(),
            source: e,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::{Action, Binding, KeyCombo};
    use std::str::FromStr;

    #[test]
    fn test_stats_engine_recording() {
        let mut stats_engine = StatsEngine::new();

        stats_engine.record_switch("windows");
        assert_eq!(stats_engine.total_switches, 1);

        stats_engine.record_action("snap_left");
        stats_engine.record_action("snap_left");
        stats_engine.record_action("close_window");
        assert_eq!(stats_engine.total_actions, 3);

        stats_engine.record_conflict("SUPER + Q");
        stats_engine.record_conflict("SUPER + Q");

        let stats = stats_engine.get_stats();
        assert_eq!(stats.total_switches, 1);
        assert_eq!(stats.total_actions, 3);
        assert_eq!(stats.action_counts.get("snap_left"), Some(&2));
        assert_eq!(stats.action_counts.get("close_window"), Some(&1));
        assert_eq!(stats.conflict_hits.get("SUPER + Q"), Some(&2));
        assert!(stats.profile_usage.contains_key("windows"));
    }

    #[test]
    fn test_stats_recommendations() {
        let mut stats_engine = StatsEngine::new();
        stats_engine.record_switch("custom");

        // snap_left called 5 times
        for _ in 0..5 {
            stats_engine.record_action("snap_left");
        }
        // conflict on ALT + F4 3 times
        for _ in 0..3 {
            stats_engine.record_conflict("ALT + F4");
        }

        let profile = Profile::new(
            "custom",
            "test",
            vec![Binding::new(
                KeyCombo::from_str("SUPER + Q").unwrap(),
                Action::CloseWindow,
            )],
        );

        let recs = stats_engine.generate_recommendations(&profile);
        assert!(!recs.is_empty());

        let action_rec = recs.iter().find(|r| r.id == "rec_action_snap_left");
        assert!(action_rec.is_some());
        assert_eq!(action_rec.unwrap().suggestion_type, "missing_binding");

        let conflict_rec = recs.iter().find(|r| r.id == "rec_conflict_ALT + F4");
        assert!(conflict_rec.is_some());
        assert_eq!(conflict_rec.unwrap().suggestion_type, "conflict_resolution");
    }

    #[test]
    fn test_export_json_and_file() {
        let mut stats_engine = StatsEngine::new();
        stats_engine.record_switch("native");
        stats_engine.record_action("maximize");

        let json = stats_engine.export_json().unwrap();
        assert!(json.contains("total_actions"));
        assert!(json.contains("maximize"));

        let temp_dir = std::env::temp_dir().join("aethershift_test_stats");
        let temp_file = temp_dir.join("stats.json");
        stats_engine.export_to_file(&temp_file).unwrap();
        assert!(temp_file.exists());

        let content = std::fs::read_to_string(&temp_file).unwrap();
        assert!(content.contains("maximize"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
