use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UnixStream;
use aethershift_core::state::ProfileInfo;
use aethershift_daemon::config::DaemonConfig;
use aethershift_daemon::daemon::AetherDaemon;
use aethershift_protocol::{
    call, Request, Response, StatusInfo,
};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1000);

fn get_temp_sock(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let rand_val = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    p.push(format!("aethershift_test_{name}_{}_{}.sock", std::process::id(), rand_val));
    p
}

#[tokio::test]
async fn test_daemon_lifecycle_and_requests() {
    let sock = get_temp_sock("lifecycle");
    let sock_clone = sock.clone();

    let config = DaemonConfig {
        socket_path: Some(sock.clone()),
        preset_dir: std::path::PathBuf::from("presets"),
        verbose: 2,
        no_notify: true,
    };

    let daemon = AetherDaemon::new(config);

    let daemon_handle = tokio::spawn(async move {
        daemon.run().await.expect("daemon run error");
    });

    // Wait for daemon to be ready
    let mut connected = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sock.exists() {
            if let Ok(_) = UnixStream::connect(&sock).await {
                connected = true;
                break;
            }
        }
    }
    assert!(connected, "Daemon failed to start listening");

    // 1. Status
    let resp = call(&sock, &Request::Status).await.expect("call Status");
    match resp {
        Response::Success { message, data } => {
            assert_eq!(message, "Status retrieved");
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.active_profile, "native");
            assert_eq!(info.overlays_count, 0);
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    // 2. ListProfiles
    let resp = call(&sock, &Request::ListProfiles).await.expect("call ListProfiles");
    match resp {
        Response::Success { message: _, data } => {
            let profiles: Vec<ProfileInfo> = serde_json::from_value(data.unwrap()).unwrap();
            let names: Vec<String> = profiles.into_iter().map(|p| p.name).collect();
            assert!(names.contains(&"native".to_string()));
            assert!(names.contains(&"windows".to_string()));
            assert!(names.contains(&"macos".to_string()));
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    // 3. Switch to windows
    let resp = call(&sock, &Request::Switch { profile: "windows".to_string(), force: false })
        .await
        .expect("call Switch windows");
    match resp {
        Response::Success { message, .. } => {
            assert!(message.contains("Successfully switched to profile 'windows'"));
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    // Verify status has changed
    let resp = call(&sock, &Request::Status).await.expect("call Status 2");
    match resp {
        Response::Success { data, .. } => {
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.active_profile, "windows");
            assert!(info.overlays_count >= 8);
        }
        _ => panic!("Status failed"),
    }

    // 4. Cycle to next profile
    let resp = call(&sock, &Request::Cycle).await.expect("call Cycle");
    match resp {
        Response::Success { message, .. } => {
            assert!(message.contains("Cycled to profile"));
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    // 5. Restore baseline
    let resp = call(&sock, &Request::Restore).await.expect("call Restore");
    match resp {
        Response::Success { message, .. } => {
            assert!(message.contains("Successfully restored"));
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    let resp = call(&sock, &Request::Status).await.expect("call Status 3");
    match resp {
        Response::Success { data, .. } => {
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.active_profile, "native");
            assert_eq!(info.overlays_count, 0);
        }
        _ => panic!("Status failed"),
    }

    // 6. Shutdown
    let resp = call(&sock, &Request::Shutdown).await.expect("call Shutdown");
    match resp {
        Response::Success { message, .. } => {
            assert!(message.contains("shutting down"));
        }
        Response::Error { code, message } => panic!("Unexpected error: {code}: {message}"),
        Response::Event { .. } => panic!("Unexpected event"),
    }

    // Wait for daemon to exit and verify socket cleanup
    let _ = tokio::time::timeout(Duration::from_secs(3), daemon_handle).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!sock_clone.exists(), "Socket file should be removed on shutdown");
}

#[tokio::test]
async fn test_single_instance_check() {
    let sock = get_temp_sock("single_inst");

    let config1 = DaemonConfig {
        socket_path: Some(sock.clone()),
        preset_dir: std::path::PathBuf::from("presets"),
        verbose: 0,
            no_notify: true,
    };

    let daemon1 = AetherDaemon::new(config1);
    let handle1 = tokio::spawn(async move {
        let _ = daemon1.run().await;
    });

    // Wait for daemon 1 to be ready
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sock.exists() {
            if let Ok(_) = UnixStream::connect(&sock).await {
                break;
            }
        }
    }

    // Daemon 2 attempting to bind same active socket should fail with AlreadyRunning
    let bind_res = AetherDaemon::bind_socket(&sock).await;
    match bind_res {
        Err(aethershift_daemon::DaemonError::AlreadyRunning(p)) => {
            assert_eq!(p, sock);
        }
        other => panic!("Expected AlreadyRunning error, got {:?}", other.err()),
    }

    // Stop daemon 1
    let _ = call(&sock, &Request::Shutdown).await;
    let _ = handle1.await;
}

#[test]
fn test_notification_disabled_logic() {
    let config = DaemonConfig {
        no_notify: true,
        ..Default::default()
    };
    assert!(config.notifications_disabled());

    let config2 = DaemonConfig {
        no_notify: false,
        ..Default::default()
    };
    unsafe {
        std::env::set_var("AETHERSHIFT_NO_NOTIFY", "1");
    }
    assert!(config2.notifications_disabled());

    unsafe {
        std::env::set_var("AETHERSHIFT_NO_NOTIFY", "true");
    }
    assert!(config2.notifications_disabled());

    unsafe {
        std::env::set_var("AETHERSHIFT_NO_NOTIFY", "0");
    }
    assert!(!config2.notifications_disabled());

    unsafe {
        std::env::remove_var("AETHERSHIFT_NO_NOTIFY");
    }
    assert!(!config2.notifications_disabled());
}

#[tokio::test]
async fn test_window_mode_request_handling() {
    use aethershift_protocol::WindowPolicy;

    let sock = get_temp_sock("window_mode");
    let sock_clone = sock.clone();

    let config = DaemonConfig {
        socket_path: Some(sock.clone()),
        preset_dir: std::path::PathBuf::from("presets"),
        verbose: 0,
        no_notify: true,
    };

    let daemon = AetherDaemon::new(config);
    let daemon_handle = tokio::spawn(async move {
        let _ = daemon.run().await;
    });

    // Wait for daemon to be ready
    let mut connected = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sock.exists() {
            if let Ok(_) = UnixStream::connect(&sock).await {
                connected = true;
                break;
            }
        }
    }
    assert!(connected, "Daemon failed to start listening");

    // 1. Check initial WindowPolicy is Omarchy via Status
    let resp = call(&sock, &Request::Status).await.expect("Status");
    match resp {
        Response::Success { data, .. } => {
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.window_policy, WindowPolicy::Omarchy);
        }
        _ => panic!("Expected status success"),
    }

    // 2. Query WindowMode with None -> returns Omarchy
    let resp = call(&sock, &Request::WindowMode { policy: None }).await.expect("WindowMode None");
    match resp {
        Response::Success { data, .. } => {
            let policy: WindowPolicy = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(policy, WindowPolicy::Omarchy);
        }
        _ => panic!("Expected WindowMode success"),
    }

    // 3. Set WindowMode to Tiled
    let resp = call(&sock, &Request::WindowMode { policy: Some(WindowPolicy::Tiled) }).await.expect("WindowMode Tiled");
    match resp {
        Response::Success { data, .. } => {
            let policy: WindowPolicy = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(policy, WindowPolicy::Tiled);
        }
        _ => panic!("Expected WindowMode success"),
    }

    // Verify Status reflects Tiled
    let resp = call(&sock, &Request::Status).await.expect("Status");
    match resp {
        Response::Success { data, .. } => {
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.window_policy, WindowPolicy::Tiled);
        }
        _ => panic!("Expected status success"),
    }

    // 4. Set WindowMode to Floating
    let resp = call(&sock, &Request::WindowMode { policy: Some(WindowPolicy::Floating) }).await.expect("WindowMode Floating");
    match resp {
        Response::Success { data, .. } => {
            let policy: WindowPolicy = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(policy, WindowPolicy::Floating);
        }
        _ => panic!("Expected WindowMode success"),
    }

    // 5. Restore baseline should reset policy back to Omarchy
    let resp = call(&sock, &Request::Restore).await.expect("Restore");
    match resp {
        Response::Success { .. } => {}
        _ => panic!("Expected restore success"),
    }

    let resp = call(&sock, &Request::Status).await.expect("Status after restore");
    match resp {
        Response::Success { data, .. } => {
            let info: StatusInfo = serde_json::from_value(data.unwrap()).unwrap();
            assert_eq!(info.window_policy, WindowPolicy::Omarchy);
        }
        _ => panic!("Expected status success"),
    }

    // Shutdown daemon
    let _ = call(&sock, &Request::Shutdown).await;
    let _ = tokio::time::timeout(Duration::from_secs(3), daemon_handle).await;
    let _ = std::fs::remove_file(sock_clone);
}
