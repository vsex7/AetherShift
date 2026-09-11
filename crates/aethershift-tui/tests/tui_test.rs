use std::time::Duration;
use tempfile::TempDir;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use aethershift_daemon::{AetherDaemon, DaemonConfig};
use aethershift_tui::app::{App, FocusPanel, ProfileItem, ALL_SNAP_LAYOUTS};
use aethershift_tui::ui;

#[tokio::test]
async fn test_app_navigation_and_focus() {
    let mut app = App::new(None);
    assert_eq!(app.focus, FocusPanel::Profiles);

    // Populate fake profiles
    app.profiles = vec![
        ProfileItem {
            name: "native".to_string(),
            description: "Default layout".to_string(),
            bindings_count: 5,
            is_active: true,
        },
        ProfileItem {
            name: "windows".to_string(),
            description: "Windows-like layout".to_string(),
            bindings_count: 12,
            is_active: false,
        },
        ProfileItem {
            name: "macos".to_string(),
            description: "Mac-like layout".to_string(),
            bindings_count: 10,
            is_active: false,
        },
    ];
    app.profiles_state.select(Some(0));

    // Test navigate down/up on Profiles
    app.next();
    assert_eq!(app.profiles_state.selected(), Some(1));
    app.next();
    assert_eq!(app.profiles_state.selected(), Some(2));
    app.next();
    assert_eq!(app.profiles_state.selected(), Some(0)); // wrap around
    app.previous();
    assert_eq!(app.profiles_state.selected(), Some(2));
    app.previous();
    assert_eq!(app.profiles_state.selected(), Some(1));

    // Test switch focus to Snaps
    app.toggle_focus();
    assert_eq!(app.focus, FocusPanel::Snaps);

    assert_eq!(app.selected_snap_index, 0);
    assert_eq!(app.selected_snap(), ALL_SNAP_LAYOUTS[0]);

    app.next();
    assert_eq!(app.selected_snap_index, 1);
    assert_eq!(app.selected_snap(), ALL_SNAP_LAYOUTS[1]);

    app.previous();
    assert_eq!(app.selected_snap_index, 0);
    app.previous();
    assert_eq!(app.selected_snap_index, ALL_SNAP_LAYOUTS.len() - 1);

    // Toggle back to Profiles
    app.toggle_focus();
    assert_eq!(app.focus, FocusPanel::Profiles);
}

#[tokio::test]
async fn test_ui_render_with_test_backend() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut app = App::new(None);
    app.profiles = vec![
        ProfileItem {
            name: "native".to_string(),
            description: "Default layout".to_string(),
            bindings_count: 5,
            is_active: true,
        },
        ProfileItem {
            name: "windows".to_string(),
            description: "Windows-like layout".to_string(),
            bindings_count: 12,
            is_active: false,
        },
    ];
    app.profiles_state.select(Some(0));

    // Draw UI and check that it doesn't panic
    let res = terminal.draw(|f| ui::draw(f, &mut app));
    assert!(res.is_ok());

    let buffer = terminal.backend().buffer();
    let content = format!("{buffer:?}");
    assert!(content.contains("AETHERSHIFT CONSOLE"));
    assert!(content.contains("Profiles"));
    assert!(content.contains("Snap Matrix"));
    assert!(content.contains("Stats & Recommendations"));
    assert!(content.contains("[Tab]"));
}

#[tokio::test]
async fn test_app_with_daemon_communication() {
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("aethershift_tui_test.sock");

    let daemon_config = DaemonConfig {
        socket_path: Some(socket_path.clone()),
        preset_dir: temp_dir.path().join("presets"),
        verbose: 0,
        no_notify: true,
    };

    let daemon = AetherDaemon::new(daemon_config);
    let daemon_task = tokio::spawn(async move {
        let _ = daemon.run().await;
    });

    // Wait for daemon socket to become ready
    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let mut app = App::new(Some(socket_path.clone()));
    assert!(!app.connected);

    // Initial refresh
    app.refresh().await;
    assert!(app.connected);
    assert!(app.status.is_some());
    assert!(!app.profiles.is_empty());

    // Switch profile
    if let Some(pos) = app.profiles.iter().position(|p| p.name == "windows") {
        app.profiles_state.select(Some(pos));
        app.switch_selected_profile().await;
        assert_eq!(app.status.as_ref().unwrap().active_profile, "windows");
    }

    // Cycle profile
    app.cycle_profile().await;
    assert_ne!(app.status.as_ref().unwrap().active_profile, "windows");

    // Restore baseline
    app.restore_baseline().await;
    assert_eq!(app.status.as_ref().unwrap().active_profile, "native");

    daemon_task.abort();
}
