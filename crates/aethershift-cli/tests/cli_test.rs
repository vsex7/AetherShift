use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::net::UnixStream;

use aethershift_cli::cli::{Cli, Command, WindowModeAction, WindowModeArgs};
use aethershift_cli::client::execute_command;
use aethershift_daemon::config::DaemonConfig;
use aethershift_daemon::daemon::AetherDaemon;
use clap::Parser;

static TEST_COUNTER: AtomicU64 = AtomicU64::new(5000);

fn get_temp_sock(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let rand_val = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    p.push(format!(
        "aethershift_cli_test_{name}_{}_{}.sock",
        std::process::id(),
        rand_val
    ));
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
    let res = execute_command(
        &sock,
        Command::Switch {
            profile: "windows".to_string(),
            force: false,
        },
    )
    .await;
    assert!(res.is_ok());

    // 4. Cycle
    let res = execute_command(&sock, Command::Cycle).await;
    assert!(res.is_ok());

    // 5. Restore
    let res = execute_command(&sock, Command::Restore).await;
    assert!(res.is_ok());

    // 5b. WindowMode query (status)
    let res = execute_command(
        &sock,
        Command::WindowMode(WindowModeArgs {
            action: Some(WindowModeAction::Status { json: false }),
            json: false,
        }),
    )
    .await;
    assert!(res.is_ok());

    let res_json = execute_command(
        &sock,
        Command::WindowMode(WindowModeArgs {
            action: Some(WindowModeAction::Status { json: true }),
            json: false,
        }),
    )
    .await;
    assert!(res_json.is_ok());

    // 5c. WindowMode query (no args)
    let res = execute_command(
        &sock,
        Command::WindowMode(WindowModeArgs {
            action: None,
            json: false,
        }),
    )
    .await;
    assert!(res.is_ok());

    let res_json = execute_command(
        &sock,
        Command::WindowMode(WindowModeArgs {
            action: None,
            json: true,
        }),
    )
    .await;
    assert!(res_json.is_ok());

    // 5d. WindowMode set policies
    for policy in &["floating", "tiled", "omarchy", "follow-profile"] {
        let res = execute_command(
            &sock,
            Command::WindowMode(WindowModeArgs {
                action: Some(WindowModeAction::Set {
                    policy: policy.to_string(),
                    json: false,
                }),
                json: false,
            }),
        )
        .await;
        assert!(res.is_ok());

        let res_json = execute_command(
            &sock,
            Command::WindowMode(WindowModeArgs {
                action: Some(WindowModeAction::Set {
                    policy: policy.to_string(),
                    json: true,
                }),
                json: false,
            }),
        )
        .await;
        assert!(res_json.is_ok());
    }

    // 5e. WindowMode invalid policy returns error
    let res = execute_command(
        &sock,
        Command::WindowMode(WindowModeArgs {
            action: Some(WindowModeAction::Set {
                policy: "invalid-policy".to_string(),
                json: false,
            }),
            json: false,
        }),
    )
    .await;
    assert!(res.is_err());

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

#[tokio::test]
async fn test_window_mode_parsing() {
    // aethershift window-mode
    let cli = Cli::try_parse_from(["aethershift", "window-mode"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            assert!(args.action.is_none());
            assert!(!args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // aethershift window-mode --json
    let cli = Cli::try_parse_from(["aethershift", "window-mode", "--json"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            assert!(args.action.is_none());
            assert!(args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // aethershift window-mode status
    let cli = Cli::try_parse_from(["aethershift", "window-mode", "status"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            assert!(matches!(
                args.action,
                Some(WindowModeAction::Status { json: false })
            ));
            assert!(!args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // aethershift window-mode status --json
    let cli = Cli::try_parse_from(["aethershift", "window-mode", "status", "--json"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            assert!(matches!(
                args.action,
                Some(WindowModeAction::Status { json: true })
            ));
            assert!(args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // aethershift window-mode set floating
    let cli = Cli::try_parse_from(["aethershift", "window-mode", "set", "floating"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            match args.action {
                Some(WindowModeAction::Set { ref policy, json }) => {
                    assert_eq!(policy, "floating");
                    assert!(!json);
                }
                _ => panic!("Expected Set action"),
            }
            assert!(!args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // aethershift window-mode set floating --json
    let cli =
        Cli::try_parse_from(["aethershift", "window-mode", "set", "floating", "--json"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => {
            match args.action {
                Some(WindowModeAction::Set { ref policy, json }) => {
                    assert_eq!(policy, "floating");
                    assert!(json);
                }
                _ => panic!("Expected Set action"),
            }
            assert!(args.is_json());
        }
        _ => panic!("Expected WindowMode command"),
    }

    // Short/alias command: aethershift window set tiled
    let cli = Cli::try_parse_from(["aethershift", "window", "set", "tiled"]).unwrap();
    match cli.command {
        Command::WindowMode(args) => match args.action {
            Some(WindowModeAction::Set { policy, .. }) => {
                assert_eq!(policy, "tiled");
            }
            _ => panic!("Expected Set action"),
        },
        _ => panic!("Expected WindowMode command via window alias"),
    }

    // Short/alias command: aethershift window
    let cli = Cli::try_parse_from(["aethershift", "window"]).unwrap();
    assert!(matches!(cli.command, Command::WindowMode(_)));

    // Short/alias command: aethershift window status
    let cli = Cli::try_parse_from(["aethershift", "window", "status"]).unwrap();
    assert!(matches!(cli.command, Command::WindowMode(_)));
}

#[test]
fn test_parse_window_policy() {
    use aethershift_cli::cli::parse_window_policy;
    use aethershift_protocol::WindowPolicy;

    assert_eq!(
        parse_window_policy("floating").unwrap(),
        WindowPolicy::Floating
    );
    assert_eq!(
        parse_window_policy("Floating").unwrap(),
        WindowPolicy::Floating
    );
    assert_eq!(parse_window_policy("tiled").unwrap(), WindowPolicy::Tiled);
    assert_eq!(parse_window_policy("TILED").unwrap(), WindowPolicy::Tiled);
    assert_eq!(
        parse_window_policy("omarchy").unwrap(),
        WindowPolicy::Omarchy
    );
    assert_eq!(
        parse_window_policy("follow-profile").unwrap(),
        WindowPolicy::FollowProfile
    );
    assert_eq!(
        parse_window_policy("follow_profile").unwrap(),
        WindowPolicy::FollowProfile
    );
    assert_eq!(
        parse_window_policy("follow").unwrap(),
        WindowPolicy::FollowProfile
    );

    assert!(parse_window_policy("invalid-policy").is_err());
}

#[test]
fn test_status_formatting_includes_window_policy() {
    use aethershift_cli::client::format_status_text;
    use aethershift_protocol::{StatusInfo, WindowPolicy};

    let mut status = StatusInfo::new("macos", 3, 120, "0.1.0");
    status.window_policy = WindowPolicy::Floating;

    let text = format_status_text(&status);
    assert!(text.contains("Window Policy:    floating"));
    assert!(text.contains("Active Profile:   macos"));

    status.window_policy = WindowPolicy::Tiled;
    let text = format_status_text(&status);
    assert!(text.contains("Window Policy:    tiled"));
}
