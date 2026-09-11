use aethershift_core::binding::{Action, Binding, KeyCombo, Modifier};
use aethershift_core::profile::Profile;
use aethershift_core::state::{BaselineBinding, StateManager};
use std::str::FromStr;

#[test]
fn test_modifier_parsing_and_aliases() {
    assert_eq!(Modifier::from_str("SUPER").unwrap(), Modifier::Super);
    assert_eq!(Modifier::from_str("win").unwrap(), Modifier::Super);
    assert_eq!(Modifier::from_str("mod4").unwrap(), Modifier::Super);
    assert_eq!(Modifier::from_str("cmd").unwrap(), Modifier::Super);
    assert_eq!(Modifier::from_str("CTRL").unwrap(), Modifier::Ctrl);
    assert_eq!(Modifier::from_str("control").unwrap(), Modifier::Ctrl);
    assert_eq!(Modifier::from_str("alt").unwrap(), Modifier::Alt);
    assert_eq!(Modifier::from_str("option").unwrap(), Modifier::Alt);
    assert_eq!(Modifier::from_str("shift").unwrap(), Modifier::Shift);
    assert!(Modifier::from_str("invalid").is_err());
}

#[test]
fn test_key_combo_parsing_and_canonical() {
    let combo1 = KeyCombo::from_str("ALT + F4").unwrap();
    assert_eq!(combo1.canonical_str(), "ALT + F4");
    assert_eq!(combo1.hyprland_mods(), "ALT");
    assert_eq!(combo1.hyprland_key(), "F4");

    let combo2 = KeyCombo::from_str("CTRL + SHIFT + ESCAPE").unwrap();
    assert_eq!(combo2.canonical_str(), "CTRL + SHIFT + ESCAPE");
    assert_eq!(combo2.hyprland_mods(), "CTRL SHIFT");
    assert_eq!(combo2.hyprland_key(), "ESCAPE");

    let combo3 = KeyCombo::from_str("SHIFT + SUPER + CTRL + F").unwrap();
    assert_eq!(combo3.canonical_str(), "SUPER + CTRL + SHIFT + F");

    assert!(KeyCombo::from_str("").is_err());
    assert!(KeyCombo::from_str("CTRL +").is_err());
}

#[test]
fn test_action_dispatcher_conversion() {
    let close = Action::CloseWindow;
    assert_eq!(
        close.to_hyprland_dispatcher(),
        ("killactive".to_string(), "".to_string())
    );

    let snap_left = Action::SnapLeft;
    assert_eq!(
        snap_left.to_hyprland_dispatcher(),
        ("movefocus".to_string(), "l".to_string())
    );

    let snap_right = Action::SnapRight;
    assert_eq!(
        snap_right.to_hyprland_dispatcher(),
        ("movefocus".to_string(), "r".to_string())
    );

    let max = Action::Maximize;
    assert_eq!(
        max.to_hyprland_dispatcher(),
        ("fullscreen".to_string(), "1".to_string())
    );

    let restore = Action::Restore;
    assert_eq!(
        restore.to_hyprland_dispatcher(),
        ("fullscreen".to_string(), "0".to_string())
    );

    let exec = Action::Exec("foot".to_string());
    assert_eq!(
        exec.to_hyprland_dispatcher(),
        ("exec".to_string(), "foot".to_string())
    );

    let custom = Action::Custom {
        dispatcher: "movetoworkspace".to_string(),
        args: "2".to_string(),
    };
    assert_eq!(
        custom.to_hyprland_dispatcher(),
        ("movetoworkspace".to_string(), "2".to_string())
    );
}

#[test]
fn test_profile_toml_roundtrip() {
    let toml_str = r#"
name = "custom"
description = "Custom profile test"

[[bindings]]
key_combo = "SUPER + RETURN"
description = "Open terminal"
[bindings.action]
type = "exec"
arg = "alacritty"

[[bindings]]
key_combo = "SUPER + Q"
description = "Close window"
[bindings.action]
type = "close_window"
"#;

    let profile = Profile::from_toml_str(toml_str).unwrap();
    assert_eq!(profile.name, "custom");
    assert_eq!(profile.bindings.len(), 2);

    let serialized = profile.to_toml_string().unwrap();
    let reloaded = Profile::from_toml_str(&serialized).unwrap();
    assert_eq!(profile, reloaded);
}

#[test]
fn test_profile_validation() {
    // Empty name
    let empty_name = Profile::new("", "desc", vec![]);
    assert!(empty_name.validate().is_err());

    // Duplicate key combos
    let combo = KeyCombo::from_str("ALT + F4").unwrap();
    let b1 = Binding::new(combo.clone(), Action::CloseWindow);
    let b2 = Binding::new(combo.clone(), Action::Maximize);
    let dup_profile = Profile::new("test", "desc", vec![b1, b2]);
    assert!(dup_profile.validate().is_err());
}

#[test]
fn test_presets_loading_in_state_manager() {
    let sm = StateManager::new();
    let profiles = sm.list_profiles();

    let names: Vec<String> = profiles.iter().map(|p| p.name.clone()).collect();
    assert!(names.contains(&"native".to_string()));
    assert!(names.contains(&"windows".to_string()));
    assert!(names.contains(&"macos".to_string()));
    assert!(names.contains(&"hybrid".to_string()));

    let windows = sm.get_profile("windows").unwrap();
    assert_eq!(windows.bindings.len(), 17);

    let macos = sm.get_profile("macos").unwrap();
    assert_eq!(macos.bindings.len(), 15);

    let native = sm.get_profile("native").unwrap();
    assert_eq!(native.bindings.len(), 0);
}

#[test]
fn test_state_manager_switch_and_diff() {
    let mut sm = StateManager::new();
    assert_eq!(sm.active_profile_name(), "native");
    assert_eq!(sm.overlays_count(), 0);

    // Switch from native to windows
    let plan = sm.switch_profile("windows").unwrap();
    assert_eq!(plan.target_profile, "windows");
    assert_eq!(plan.unbind.len(), 0);
    assert_eq!(plan.bind.len(), 17);

    sm.commit_switch(&plan).unwrap();
    assert_eq!(sm.active_profile_name(), "windows");
    assert_eq!(sm.overlays_count(), 17);

    // Switching to windows again should produce an empty plan (no-op diff)
    let plan_noop = sm.switch_profile("windows").unwrap();
    assert!(plan_noop.is_empty());

    // Switch from windows to macos
    let plan_macos = sm.switch_profile("macos").unwrap();
    assert_eq!(plan_macos.target_profile, "macos");
    // Eleven differing Windows overlays are removed; identical actions carry over.
    assert_eq!(plan_macos.unbind.len(), 15);
    // All macOS overlays should be bound.
    // Five identical actions carry over, leaving ten new macOS bindings.
    assert_eq!(plan_macos.bind.len(), 13);

    sm.commit_switch(&plan_macos).unwrap();
    assert_eq!(sm.active_profile_name(), "macos");
    assert_eq!(sm.overlays_count(), 15);

    // Restore plan
    let restore = sm.restore_plan();
    assert_eq!(restore.target_profile, "native");
    assert_eq!(restore.unbind.len(), 15);
    assert_eq!(restore.bind.len(), 0);

    sm.commit_restore();
    assert_eq!(sm.active_profile_name(), "native");
    assert_eq!(sm.overlays_count(), 0);
}

#[test]
fn test_partial_diff_overlapping_bindings() {
    let mut sm = StateManager::new();

    // Create profile A: SUPER+Q -> CloseWindow, SUPER+M -> FloatToggle
    let p_a = Profile::new(
        "profile_a",
        "A",
        vec![
            Binding::new(
                KeyCombo::from_str("SUPER + Q").unwrap(),
                Action::CloseWindow,
            ),
            Binding::new(
                KeyCombo::from_str("SUPER + M").unwrap(),
                Action::FloatToggle,
            ),
        ],
    );

    // Create profile B: SUPER+Q -> CloseWindow (identical!), SUPER+M -> Maximize (action changed!), SUPER+E -> SnapLeft (new!)
    let p_b = Profile::new(
        "profile_b",
        "B",
        vec![
            Binding::new(
                KeyCombo::from_str("SUPER + Q").unwrap(),
                Action::CloseWindow,
            ),
            Binding::new(KeyCombo::from_str("SUPER + M").unwrap(), Action::Maximize),
            Binding::new(KeyCombo::from_str("SUPER + E").unwrap(), Action::SnapLeft),
        ],
    );

    sm.register_profile(p_a);
    sm.register_profile(p_b);

    let plan_a = sm.switch_profile("profile_a").unwrap();
    sm.commit_switch(&plan_a).unwrap();
    assert_eq!(sm.overlays_count(), 2);

    // Switch from A to B
    let plan_b = sm.switch_profile("profile_b").unwrap();
    // SUPER + Q is identical: should NOT be in unbind or bind
    // SUPER + M action changed: must be in unbind and bind
    // SUPER + E is new: must be in bind
    assert_eq!(plan_b.unbind.len(), 1);
    assert_eq!(plan_b.unbind[0].canonical_str(), "SUPER + M");

    let bound_combos: Vec<String> = plan_b
        .bind
        .iter()
        .map(|b| b.key_combo.canonical_str())
        .collect();
    assert_eq!(bound_combos, vec!["SUPER + E", "SUPER + M"]);

    sm.commit_switch(&plan_b).unwrap();
    assert_eq!(sm.overlays_count(), 3);
}

#[test]
fn test_baseline_bindings() {
    let mut sm = StateManager::new();
    let baseline_combo = KeyCombo::from_str("SUPER + Q").unwrap();
    let baseline = BaselineBinding {
        key_combo: baseline_combo.clone(),
        dispatcher: "killactive".to_string(),
        args: "".to_string(),
        description: None,
    };

    sm.set_baseline(vec![baseline.clone()]);
    assert_eq!(sm.baseline().len(), 1);
    assert_eq!(sm.baseline().get(&baseline_combo), Some(&baseline));
}

#[test]
fn test_phase3_layout_engine_and_memory() {
    use aethershift_core::layout::{GeometryMemory, LayoutEngine, Rect, SnapLayout};

    let work_area = Rect::new(0, 0, 1920, 1080);
    let left = LayoutEngine::compute_geometry(SnapLayout::HalfLeft, work_area);
    assert_eq!(left, Rect::new(0, 0, 960, 1080));

    let mut memory = GeometryMemory::new();
    let win = "window_1";
    let initial_rect = Rect::new(100, 100, 600, 400);

    assert!(memory.record_if_absent(win, initial_rect));
    // Second call should not overwrite
    assert!(!memory.record_if_absent(win, left));

    let restored = memory.restore_original(win);
    assert_eq!(restored, Some(initial_rect));
    assert_eq!(memory.restore_original(win), None);
}

#[test]
fn test_phase3_state_manager_profile_lifecycle() {
    let mut sm = StateManager::new();

    // Create a new profile
    sm.create_profile("workflow", Some("Daily workflow"), Some("windows"))
        .unwrap();
    assert_eq!(sm.get_profile("workflow").unwrap().bindings.len(), 17);

    // Save profile to temporary location
    let tmp = std::env::temp_dir().join("aethershift_lifecycle_test");
    let path = sm.save_profile("workflow", Some(&tmp)).unwrap();
    assert!(path.exists());

    // Switch to workflow and update binding
    let plan = sm.switch_profile("workflow").unwrap();
    sm.commit_switch(&plan).unwrap();

    let diff = sm
        .update_binding(
            "workflow",
            "SUPER + SHIFT + W",
            "close_window",
            Some("Close"),
        )
        .unwrap();
    assert!(diff.is_some());
    assert_eq!(
        diff.unwrap().bind[0].key_combo.canonical_str(),
        "SUPER + SHIFT + W"
    );

    // Remove binding
    let diff_rem = sm.remove_binding("workflow", "SUPER + SHIFT + W").unwrap();
    assert!(diff_rem.is_some());
    assert_eq!(
        diff_rem.unwrap().unbind[0].canonical_str(),
        "SUPER + SHIFT + W"
    );

    // Switch away before deleting
    let restore = sm.restore_plan();
    sm.commit_switch(&restore).unwrap();

    // Delete profile
    sm.delete_profile("workflow", Some(&tmp)).unwrap();
    assert!(!path.exists());
    assert!(sm.get_profile("workflow").is_none());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_phase3_stats_engine_and_recommendations() {
    use aethershift_core::stats::StatsEngine;

    let mut stats = StatsEngine::new();
    stats.record_switch("windows");
    for _ in 0..10 {
        stats.record_action("snap_left");
    }
    for _ in 0..3 {
        stats.record_conflict("SUPER + TAB");
    }

    let sm = StateManager::new();
    let native_profile = sm.get_profile("native").unwrap();
    let recs = stats.generate_recommendations(native_profile);

    assert!(recs.iter().any(|r| r.id == "rec_action_snap_left"));
    assert!(recs.iter().any(|r| r.id == "rec_conflict_SUPER + TAB"));

    let json = stats.export_json().unwrap();
    assert!(json.contains("snap_left"));
    assert!(json.contains("SUPER + TAB"));
}
