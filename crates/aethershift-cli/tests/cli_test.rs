use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UnixStream;

use aethershift_cli::cli::{Cli, Command};
use aethershift_cli::client::execute_command;
use aethershift_daemon::config::DaemonConfig;
use aethershift_daemon::daemon::AetherDaemon;
use clap::Parser;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(5000);

fn get_temp_sock(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let rand_val = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    p.push(format!("aethershift_cli_test_{name}_{}_{}.sock", std::process::id(), rand_val));
    p
}

#[tokio::test]
async fn test_cli_parsing() {
    let cli = Cli::try_parse_from(["aethershift", "status"]).unwrap();
    assert!(matches!(cli.command, Command::Status { json: false }));

    let cli_json = Cli::try_parse_from(["aethershift", "status", "--json"]).unwrap();
    assert!(matches!(cli_json.command, Command::Status { json: true }));

    let cli_switch = Cli::try_parse_from(["aethershift", "switch", "macos", "--force"]).unwrap();
    match cli_switch.command {
        Command::Switch { profile, force } => {
            assert_eq!(profile, "macos");
            assert!(force);
        }
        _ => panic!("Expected Switch command"),
    }

    let cli_cycle = Cli::try_parse_from(["aethershift", "cycle"]).unwrap();
    assert!(matches!(cli_cycle.command, Command::Cycle));

    let cli_restore = Cli::try_parse_from(["aethershift", "restore"]).unwrap();
    assert!(matches!(cli_restore.command, Command::Restore));

    let cli_shutdown = Cli::try_parse_from(["aethershift", "shutdown"]).unwrap();
    assert!(matches!(cli_shutdown.command, Command::Shutdown));
}

#[tokio::test]
async fn test_cli_commands_with_daemon() {
    let sock = get_temp_sock("cmd_e2e");
    let sock_clone = sock.clone();

    let daemon_config = DaemonConfig {
        socket_path: Some(sock.clone()),
        preset_dir: std::path::PathBuf::from("presets"),
        verbose: 0,
        no_notify: true,
    };

    let daemon = AetherDaemon::new(daemon_config);
    let daemon_handle = tokio::spawn(async move {
        let _ = daemon.run().await;
    });

    // Wait for daemon to be ready
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if sock.exists() {
            if let Ok(_) = UnixStream::connect(&sock).await {
                break;
            }
        }
    }

    // 1. Status
    let res = execute_command(&sock, Command::Status { json: false }).await;
    assert!(res.is_ok());

    let res_json = execute_command(&sock, Command::Status { json: true }).await;
    assert!(res_json.is_ok());

    // 2. List
    let res = execute_command(&sock, Command::List { json: false }).await;
    assert!(res.is_ok());

    let res_json = execute_command(&sock, Command::List { json: true }).await;
    assert!(res_json.is_ok());

    // 3. Switch
    let res = execute_command(&sock, Command::Switch { profile: "windows".to_string(), force: false }).await;
    assert!(res.is_ok());

    // 4. Cycle
    let res = execute_command(&sock, Command::Cycle).await;
    assert!(res.is_ok());

    // 5. Restore
    let res = execute_command(&sock, Command::Restore).await;
    assert!(res.is_ok());

    // 6. Shutdown
    let res = execute_command(&sock, Command::Shutdown).await;
    assert!(res.is_ok());

    let _ = daemon_handle.await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!sock_clone.exists());
}

#[tokio::test]
async fn test_cli_daemon_not_running() {
    let sock = get_temp_sock("not_running");
    let res = execute_command(&sock, Command::Status { json: false }).await;
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Daemon connection failed"));
}
