use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::binding::{Action, Binding, KeyCombo};
use crate::conflict::{
    BindingClassification, BindingDecision, ConflictEntry, ConflictReport, ResolvedSwitchPlan,
    resolve_for_policy,
};
use crate::error::CoreError;
use crate::profile::Profile;
use crate::profile_metadata::ConflictPolicy;
use aethershift_protocol::{HistoryEntry, WindowPolicy};
use std::time::{SystemTime, UNIX_EPOCH};

/// A concrete execution plan detailing what to unbind and what to bind
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SwitchPlan {
    /// Target profile name
    pub target_profile: String,
    /// Key combinations that need to be unbound from Hyprland
    pub unbind: Vec<KeyCombo>,
    /// New bindings that need to be bound in Hyprland
    pub bind: Vec<Binding>,
}

impl SwitchPlan {
    pub fn is_empty(&self) -> bool {
        self.unbind.is_empty() && self.bind.is_empty()
    }
}

/// Baseline binding recorded from the environment/Hyprland
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineBinding {
    pub key_combo: KeyCombo,
    pub dispatcher: String,
    pub args: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl BaselineBinding {
    pub fn is_aethershift_owned(&self) -> bool {
        self.description
            .as_deref()
            .map(|desc| desc.starts_with("AetherShift:"))
            .unwrap_or(false)
    }

    pub fn to_binding(&self) -> Binding {
        Binding::new(
            self.key_combo.clone(),
            Action::Custom {
                dispatcher: self.dispatcher.clone(),
                args: self.args.clone(),
            },
        )
    }
}

/// Information about a known profile
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    pub description: String,
    pub bindings_count: usize,
    pub is_active: bool,
}

/// StateManager manages profiles, active overlays, baselines, and switch diff calculation
#[derive(Debug, Clone)]
pub struct StateManager {
    /// Known profiles mapped by profile name
    profiles: BTreeMap<String, Profile>,
    /// Currently active profile name
    active_profile: String,
    /// Active overlays: key_combo -> currently bound Binding
    active_overlays: HashMap<KeyCombo, Binding>,
    /// Baseline bindings: original Hyprland key bindings before any AetherShift overlays
    baseline_bindings: HashMap<KeyCombo, BaselineBinding>,
    replaced_baseline: HashMap<KeyCombo, BaselineBinding>,
    last_conflict_report: ConflictReport,
    total_skipped_conflicts: usize,
    total_forced_overrides: usize,
    window_policy: WindowPolicy,
    switch_history: Vec<HistoryEntry>,
    next_history_sequence: u64,
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new()
    }
}

impl StateManager {
    /// Create a new StateManager with built-in embedded presets preloaded
    pub fn new() -> Self {
        let mut mgr = Self {
            profiles: BTreeMap::new(),
            active_profile: "native".to_string(),
            active_overlays: HashMap::new(),
            baseline_bindings: HashMap::new(),
            replaced_baseline: HashMap::new(),
            last_conflict_report: ConflictReport::default(),
            total_skipped_conflicts: 0,
            total_forced_overrides: 0,
            window_policy: WindowPolicy::Omarchy,
            switch_history: Vec::new(),
            next_history_sequence: 1,
        };
        mgr.load_embedded_presets();
        mgr.load_embedded_hybrid();
        mgr
    }

    /// Load default embedded presets (native, windows, macos)
    fn load_embedded_presets(&mut self) {
        let native_toml = include_str!("../../../presets/native.toml");
        let windows_toml = include_str!("../../../presets/windows.toml");
        let macos_toml = include_str!("../../../presets/macos.toml");

        if let Ok(p) = Profile::from_toml_str(native_toml) {
            self.register_profile(p);
        }
        if let Ok(p) = Profile::from_toml_str(windows_toml) {
            self.register_profile(p);
        }
        if let Ok(p) = Profile::from_toml_str(macos_toml) {
            self.register_profile(p);
        }
    }

    fn load_embedded_hybrid(&mut self) {
        let hybrid_toml = include_str!("../../../presets/hybrid.toml");
        if let Ok(profile) = Profile::from_toml_str(hybrid_toml) {
            self.register_profile(profile);
        }
    }

    /// Register or update a profile in the manager
    pub fn register_profile(&mut self, profile: Profile) {
        info!("Registering profile: {}", profile.name);
        self.profiles.insert(profile.name.clone(), profile);
    }

    /// Load all TOML profile files from a specified directory
    pub fn load_profiles_from_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize, CoreError> {
        let dir_ref = dir.as_ref();
        if !dir_ref.exists() || !dir_ref.is_dir() {
            return Ok(0);
        }

        let mut loaded = 0;
        let entries = std::fs::read_dir(dir_ref).map_err(|e| CoreError::Io {
            path: dir_ref.to_path_buf(),
            source: e,
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
                match Profile::from_file(&path) {
                    Ok(profile) => {
                        self.register_profile(profile);
                        loaded += 1;
                    }
                    Err(e) => {
                        warn!("Failed to load profile from {:?}: {}", path, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// Set or update the Hyprland baseline bindings
    pub fn set_baseline(&mut self, bindings: impl IntoIterator<Item = BaselineBinding>) {
        self.baseline_bindings = bindings
            .into_iter()
            .map(|b| (b.key_combo.clone(), b))
            .collect();
    }

    /// Get current baseline bindings
    pub fn baseline(&self) -> &HashMap<KeyCombo, BaselineBinding> {
        &self.baseline_bindings
    }

    /// Get current active profile name
    pub fn active_profile_name(&self) -> &str {
        &self.active_profile
    }

    /// Return the count of currently active overlays
    pub fn overlays_count(&self) -> usize {
        self.active_overlays.len()
    }

    /// Return a map of currently active overlays
    pub fn active_overlays(&self) -> &HashMap<KeyCombo, Binding> {
        &self.active_overlays
    }

    pub fn last_conflict_report(&self) -> &ConflictReport {
        &self.last_conflict_report
    }

    pub fn total_skipped_conflicts(&self) -> usize {
        self.total_skipped_conflicts
    }

    pub fn total_forced_overrides(&self) -> usize {
        self.total_forced_overrides
    }

    pub fn window_policy(&self) -> WindowPolicy {
        self.window_policy
    }

    pub fn set_window_policy(&mut self, policy: WindowPolicy) {
        self.window_policy = policy;
    }

    pub fn record_switch_history(
        &mut self,
        profile: &str,
        succeeded: bool,
        duration_us: u128,
        applied: usize,
        skipped: usize,
        forced: usize,
        error: Option<String>,
    ) {
        let sequence = self.next_history_sequence;
        self.next_history_sequence += 1;
        self.switch_history.push(HistoryEntry {
            sequence,
            unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|value| value.as_millis())
                .unwrap_or_default(),
            profile: profile.to_string(),
            succeeded,
            duration_us,
            applied,
            skipped,
            forced,
            error,
        });
        if self.switch_history.len() > 256 {
            self.switch_history.remove(0);
        }
    }

    pub fn switch_history(&self, limit: Option<usize>) -> Vec<HistoryEntry> {
        let count = limit.unwrap_or(20).min(self.switch_history.len());
        self.switch_history.iter().rev().take(count).cloned().collect()
    }

    /// List all available profiles and their metadata
    pub fn list_profiles(&self) -> Vec<ProfileInfo> {
        self.profiles
            .values()
            .map(|p| ProfileInfo {
                name: p.name.clone(),
                description: p.description.clone(),
                bindings_count: p.bindings.len(),
                is_active: p.name == self.active_profile,
            })
            .collect()
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Generate a diff execution plan to switch from current state to the target profile
    pub fn switch_profile(&self, target_name: &str) -> Result<SwitchPlan, CoreError> {
        Ok(self
            .switch_profile_with_policy(target_name, ConflictPolicy::Strict)?
            .plan)
    }

    pub fn switch_profile_with_policy(
        &self,
        target_name: &str,
        policy: ConflictPolicy,
    ) -> Result<ResolvedSwitchPlan, CoreError> {
        let target_profile = self
            .profiles
            .get(target_name)
            .ok_or_else(|| CoreError::ProfileNotFound(target_name.to_string()))?;

        // Target bindings indexed by KeyCombo
        let mut target_bindings_map: HashMap<KeyCombo, Binding> = HashMap::new();
        for b in &target_profile.bindings {
            target_bindings_map.insert(b.key_combo.clone(), b.clone());
        }

        let mut unbind = Vec::new();
        let mut bind = Vec::new();
        let mut report = ConflictReport::default();

        let record = |combo: &KeyCombo,
                      baseline: Option<&BaselineBinding>,
                      target: Option<&Binding>,
                      classification: BindingClassification,
                      decision: BindingDecision,
                      reason: String,
                      report: &mut ConflictReport| {
            report.entries.push(ConflictEntry {
                key_combo: combo.canonical_str(),
                baseline_dispatcher: baseline.map(|base| base.dispatcher.clone()),
                baseline_args: baseline.map(|base| base.args.clone()),
                target_action: target.map(|binding| binding.action.clone()),
                classification,
                decision,
                reason,
            });
            match decision {
                BindingDecision::Applied => report.applied += 1,
                BindingDecision::Skipped => report.skipped += 1,
                BindingDecision::Forced => report.forced += 1,
            }
        };

        // AetherShift-owned overlays are always safe to replace or remove.
        for (combo, current_binding) in &self.active_overlays {
            match target_bindings_map.get(combo) {
                Some(new_binding) => {
                    if new_binding.action != current_binding.action {
                        record(
                            combo,
                            None,
                            Some(new_binding),
                            BindingClassification::New,
                            BindingDecision::Applied,
                            "replacing previous AetherShift overlay".to_string(),
                            &mut report,
                        );
                        unbind.push(combo.clone());
                        bind.push(new_binding.clone());
                    } else {
                        record(
                            combo,
                            None,
                            Some(new_binding),
                            BindingClassification::Identical,
                            BindingDecision::Skipped,
                            "identical AetherShift overlay".to_string(),
                            &mut report,
                        );
                    }
                }
                None => {
                    record(
                        combo,
                        None,
                        None,
                        BindingClassification::New,
                        BindingDecision::Applied,
                        "removing previous AetherShift overlay".to_string(),
                        &mut report,
                    );
                    unbind.push(combo.clone());
                }
            }
        }

        // Target bindings are evaluated against protected baseline only when we do not own the combo.
        for (combo, new_binding) in &target_bindings_map {
            if self.active_overlays.contains_key(combo) {
                continue;
            }

            let baseline = self.baseline_bindings.get(combo);
            let (classification, decision, reason) =
                resolve_for_policy(combo, baseline, new_binding, policy);
            record(
                combo,
                baseline,
                Some(new_binding),
                classification,
                decision,
                reason,
                &mut report,
            );

            if classification != BindingClassification::Identical
                && decision != BindingDecision::Skipped
            {
                if classification == BindingClassification::Conflict {
                    unbind.push(combo.clone());
                }
                bind.push(new_binding.clone());
            }
        }

        // Stable ordering for deterministic test results & logs
        unbind.sort_by_key(|a| a.canonical_str());
        bind.sort_by_key(|a| a.key_combo.canonical_str());

        report.entries.sort_by(|a, b| a.key_combo.cmp(&b.key_combo));
        Ok(ResolvedSwitchPlan {
            plan: SwitchPlan {
                target_profile: target_name.to_string(),
                unbind,
                bind,
            },
            report,
        })
    }

    /// Commit the switch plan to update internal state
    pub fn commit_switch(&mut self, plan: &SwitchPlan) -> Result<(), CoreError> {
        let report = self.last_conflict_report.clone();
        self.commit_switch_with_report(plan, &report)
    }

    pub fn commit_switch_with_report(
        &mut self,
        plan: &SwitchPlan,
        report: &ConflictReport,
    ) -> Result<(), CoreError> {
        let target_profile = self
            .profiles
            .get(&plan.target_profile)
            .ok_or_else(|| CoreError::ProfileNotFound(plan.target_profile.clone()))?;

        // Rebuild active overlays according to target profile bindings
        let mut new_overlays = HashMap::new();
        for b in &target_profile.bindings {
            let dispatcher = b.action.to_hyprland_dispatcher();
            if let Some(base) = self.baseline_bindings.get(&b.key_combo)
                && base.dispatcher == dispatcher.0
                && base.args == dispatcher.1
            {
                continue;
            }
            new_overlays.insert(b.key_combo.clone(), b.clone());
        }

        // A combo whose own overlay was removed must restore its original baseline later.
        for combo in &plan.unbind {
            if let Some(base) = self.baseline_bindings.get(combo).cloned() {
                self.replaced_baseline.insert(combo.clone(), base);
            }
        }

        self.active_overlays = new_overlays;
        self.active_profile = plan.target_profile.clone();
        self.last_conflict_report = report.clone();
        self.total_skipped_conflicts = self.total_skipped_conflicts.saturating_add(report.skipped);
        self.total_forced_overrides = self.total_forced_overrides.saturating_add(report.forced);
        debug!(
            "Switched to profile '{}', active overlays: {}",
            self.active_profile,
            self.active_overlays.len()
        );

        Ok(())
    }

    /// Generate a plan to restore the environment to baseline (empty overlays / native)
    pub fn restore_plan(&self) -> SwitchPlan {
        let mut unbind: Vec<KeyCombo> = self.active_overlays.keys().cloned().collect();
        unbind.sort_by_key(|a| a.canonical_str());
        let mut bind: Vec<Binding> = self
            .replaced_baseline
            .values()
            .map(BaselineBinding::to_binding)
            .collect();
        bind.sort_by_key(|binding| binding.key_combo.canonical_str());

        SwitchPlan {
            target_profile: "native".to_string(),
            unbind,
            bind,
        }
    }

    /// Commit the restore plan to clear all active overlays and set active_profile to "native"
    pub fn commit_restore(&mut self) {
        self.active_overlays.clear();
        self.replaced_baseline.clear();
        self.active_profile = "native".to_string();
    }

    // ==========================================
    // Phase 3 Runtime Profile Lifecycle Management
    // ==========================================

    /// Create a new profile at runtime, optionally copying bindings from an existing profile.
    pub fn create_profile(
        &mut self,
        name: &str,
        description: Option<&str>,
        copy_from: Option<&str>,
    ) -> Result<(), CoreError> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return Err(CoreError::Validation {
                profile: name.to_string(),
                message: "Profile name cannot be empty".to_string(),
            });
        }

        if self.profiles.contains_key(trimmed_name) {
            return Err(CoreError::Validation {
                profile: trimmed_name.to_string(),
                message: format!("Profile '{trimmed_name}' already exists"),
            });
        }

        let (desc, bindings) = if let Some(source_name) = copy_from {
            let source_profile = self
                .profiles
                .get(source_name)
                .ok_or_else(|| CoreError::ProfileNotFound(source_name.to_string()))?;
            (
                description
                    .unwrap_or(&source_profile.description)
                    .to_string(),
                source_profile.bindings.clone(),
            )
        } else {
            (description.unwrap_or("").to_string(), Vec::new())
        };

        let profile = Profile::new(trimmed_name, desc, bindings);
        profile.validate()?;
        self.register_profile(profile);
        Ok(())
    }

    /// Add or update a key binding in a profile.
    /// If modifying the currently active profile, returns an incremental SwitchPlan to apply immediately.
    pub fn update_binding(
        &mut self,
        profile_name: &str,
        key_combo: &str,
        action: &str,
        description: Option<&str>,
    ) -> Result<Option<SwitchPlan>, CoreError> {
        let combo: KeyCombo = key_combo.parse()?;
        let act: Action = action.parse()?;

        let profile = self
            .profiles
            .get_mut(profile_name)
            .ok_or_else(|| CoreError::ProfileNotFound(profile_name.to_string()))?;

        let mut binding = Binding::new(combo.clone(), act);
        if let Some(desc) = description {
            binding = binding.with_description(desc);
        }

        let is_active = profile_name == self.active_profile;
        let mut old_binding = None;

        // Check if key_combo already exists in profile
        if let Some(idx) = profile.bindings.iter().position(|b| b.key_combo == combo) {
            old_binding = Some(profile.bindings[idx].clone());
            profile.bindings[idx] = binding.clone();
        } else {
            profile.bindings.push(binding.clone());
        }

        profile.validate()?;

        if is_active {
            // Update active_overlays
            self.active_overlays.insert(combo.clone(), binding.clone());

            let mut unbind = Vec::new();
            if let Some(prev) = old_binding
                && prev.action != binding.action
            {
                unbind.push(combo.clone());
            }

            Ok(Some(SwitchPlan {
                target_profile: profile_name.to_string(),
                unbind,
                bind: vec![binding],
            }))
        } else {
            Ok(None)
        }
    }

    /// Remove a key binding from a profile.
    /// If modifying the currently active profile, returns an incremental SwitchPlan to apply immediately.
    pub fn remove_binding(
        &mut self,
        profile_name: &str,
        key_combo: &str,
    ) -> Result<Option<SwitchPlan>, CoreError> {
        let combo: KeyCombo = key_combo.parse()?;

        let profile = self
            .profiles
            .get_mut(profile_name)
            .ok_or_else(|| CoreError::ProfileNotFound(profile_name.to_string()))?;

        let initial_len = profile.bindings.len();
        profile.bindings.retain(|b| b.key_combo != combo);

        if profile.bindings.len() == initial_len {
            return Err(CoreError::Validation {
                profile: profile_name.to_string(),
                message: format!("Key combo '{combo}' not found in profile '{profile_name}'"),
            });
        }

        let is_active = profile_name == self.active_profile;

        if is_active {
            self.active_overlays.remove(&combo);
            Ok(Some(SwitchPlan {
                target_profile: profile_name.to_string(),
                unbind: vec![combo],
                bind: Vec::new(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Default directory for user presets: ~/.config/aethershift/presets
    pub fn default_preset_dir() -> PathBuf {
        if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home)
                .join(".config")
                .join("aethershift")
                .join("presets")
        } else {
            PathBuf::from(".config/aethershift/presets")
        }
    }

    /// Save a profile as canonical TOML into target_dir (or default ~/.config/aethershift/presets)
    pub fn save_profile(
        &self,
        profile_name: &str,
        target_dir: Option<&Path>,
    ) -> Result<PathBuf, CoreError> {
        let profile = self
            .profiles
            .get(profile_name)
            .ok_or_else(|| CoreError::ProfileNotFound(profile_name.to_string()))?;

        let toml_str = profile.to_toml_string()?;

        let dir = target_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_preset_dir);

        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| CoreError::Io {
                path: dir.clone(),
                source: e,
            })?;
        }

        let file_path = dir.join(format!("{}.toml", profile.name));
        std::fs::write(&file_path, toml_str).map_err(|e| CoreError::Io {
            path: file_path.clone(),
            source: e,
        })?;

        info!(
            "Saved profile '{}' to {}",
            profile_name,
            file_path.display()
        );
        Ok(file_path)
    }

    /// Delete a profile from memory and clean up any persisted file in user_dir if present.
    /// Built-in native profile cannot be deleted.
    pub fn delete_profile(
        &mut self,
        profile_name: &str,
        user_dir: Option<&Path>,
    ) -> Result<(), CoreError> {
        if profile_name == "native" {
            return Err(CoreError::ProtectedProfile("native".to_string()));
        }

        if !self.profiles.contains_key(profile_name) {
            return Err(CoreError::ProfileNotFound(profile_name.to_string()));
        }

        // If currently active, cannot delete or should restore first
        if self.active_profile == profile_name {
            return Err(CoreError::Validation {
                profile: profile_name.to_string(),
                message: format!(
                    "Cannot delete currently active profile '{profile_name}'. Switch or restore first."
                ),
            });
        }

        self.profiles.remove(profile_name);

        // Delete persisted file if exists
        let dir = user_dir
            .map(|p| p.to_path_buf())
            .unwrap_or_else(Self::default_preset_dir);

        let file_path = dir.join(format!("{profile_name}.toml"));
        if file_path.exists() && file_path.is_file() {
            let _ = std::fs::remove_file(&file_path);
            info!("Deleted profile file at {}", file_path.display());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_delete_profile() {
        let mut sm = StateManager::new();

        // Create new profile copying from windows
        sm.create_profile("gaming", Some("Gaming profile"), Some("windows"))
            .unwrap();

        let gaming = sm.get_profile("gaming").unwrap();
        assert_eq!(gaming.name, "gaming");
        assert_eq!(gaming.description, "Gaming profile");
        assert_eq!(gaming.bindings.len(), 17);

        // Duplicate name fails
        assert!(sm.create_profile("gaming", None, None).is_err());

        // Cannot delete native
        assert!(sm.delete_profile("native", None).is_err());

        // Delete gaming profile
        sm.delete_profile("gaming", None).unwrap();
        assert!(sm.get_profile("gaming").is_none());
    }

    #[test]
    fn test_update_and_remove_binding() {
        let mut sm = StateManager::new();
        sm.create_profile("dev", Some("Dev profile"), None).unwrap();

        // Update binding in inactive profile returns None switch plan
        let plan = sm
            .update_binding(
                "dev",
                "SUPER + RETURN",
                "exec: alacritty",
                Some("Open terminal"),
            )
            .unwrap();
        assert!(plan.is_none());

        let dev = sm.get_profile("dev").unwrap();
        assert_eq!(dev.bindings.len(), 1);
        assert_eq!(dev.bindings[0].key_combo.canonical_str(), "SUPER + RETURN");

        // Switch to dev
        let switch_plan = sm.switch_profile("dev").unwrap();
        sm.commit_switch(&switch_plan).unwrap();
        assert_eq!(sm.active_profile_name(), "dev");
        assert_eq!(sm.overlays_count(), 1);

        // Update binding in active profile returns incremental SwitchPlan
        let plan_active = sm
            .update_binding("dev", "SUPER + Q", "close_window", None)
            .unwrap()
            .unwrap();
        assert_eq!(plan_active.bind.len(), 1);
        assert_eq!(plan_active.bind[0].key_combo.canonical_str(), "SUPER + Q");
        assert_eq!(sm.overlays_count(), 2);

        // Modify existing binding in active profile
        let plan_modify = sm
            .update_binding("dev", "SUPER + Q", "maximize", None)
            .unwrap()
            .unwrap();
        assert_eq!(plan_modify.unbind.len(), 1);
        assert_eq!(plan_modify.bind.len(), 1);
        assert_eq!(plan_modify.bind[0].action, Action::Maximize);

        // Remove binding in active profile
        let plan_remove = sm.remove_binding("dev", "SUPER + RETURN").unwrap().unwrap();
        assert_eq!(plan_remove.unbind.len(), 1);
        assert_eq!(plan_remove.unbind[0].canonical_str(), "SUPER + RETURN");
        assert_eq!(sm.overlays_count(), 1);
    }

    #[test]
    fn test_save_and_delete_profile_file() {
        let mut sm = StateManager::new();
        sm.create_profile("temp_prof", Some("Temp"), Some("windows"))
            .unwrap();

        let temp_dir = std::env::temp_dir().join("aethershift_state_test");
        let saved_path = sm.save_profile("temp_prof", Some(&temp_dir)).unwrap();
        assert!(saved_path.exists());

        // Verify TOML content
        let content = std::fs::read_to_string(&saved_path).unwrap();
        assert!(content.contains("temp_prof"));

        // Delete profile and cleanup file
        sm.delete_profile("temp_prof", Some(&temp_dir)).unwrap();
        assert!(!saved_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
