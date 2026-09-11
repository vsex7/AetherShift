use std::path::Path;
use std::str::FromStr;
use anyhow::{bail, Context, Result};
use aethershift_protocol::{
    call, Recommendation, Request, Response, SnapLayout, StatusInfo, UsageStats, WindowPolicy,
};

/// Send a request to the daemon at `socket_path` with friendly error handling
pub async fn send_daemon_request(socket_path: &Path, req: &Request) -> Result<Response> {
    match call(socket_path, req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            let path_str = socket_path.display();
            eprintln!("Error: Cannot connect to AetherShift daemon at '{path_str}'.");
            eprintln!();
            eprintln!("Is the daemon running?");
            eprintln!("You can start the daemon using:");
            eprintln!("    aethershift daemon");
            eprintln!("or directly with:");
            eprintln!("    aethershift-daemon");
            eprintln!();
            bail!("Daemon connection failed: {e}");
        }
    }
}

/// Execute command and print friendly output
pub async fn execute_command(
    socket_path: &Path,
    cmd: crate::cli::Command,
) -> Result<()> {
    use crate::cli::{Command, ProfileCommand, WindowModeAction, parse_window_policy};


    match cmd {
        Command::Status { json } => {
            let resp = send_daemon_request(socket_path, &Request::Status).await?;
            handle_status_response(resp, json)?;
        }
        Command::List { json } => {
            let resp = send_daemon_request(socket_path, &Request::ListProfiles).await?;
            handle_list_response(resp, json)?;
        }
        Command::Switch { profile, force } => {
            let resp = send_daemon_request(socket_path, &Request::Switch { profile, force }).await?;
            handle_simple_response(resp)?;
        }
        Command::Cycle => {
            let resp = send_daemon_request(socket_path, &Request::Cycle).await?;
            handle_simple_response(resp)?;
        }
        Command::Restore => {
            let resp = send_daemon_request(socket_path, &Request::Restore).await?;
            handle_simple_response(resp)?;
        }
        Command::WindowMode(args) => {
            let is_json = args.is_json();
            match args.action {
                Some(WindowModeAction::Set { policy, .. }) => {
                    let parsed_policy = parse_window_policy(&policy)
                        .map_err(|e| anyhow::anyhow!("{e}"))?;
                    let req = Request::WindowMode {
                        policy: Some(parsed_policy),
                    };
                    let resp = send_daemon_request(socket_path, &req).await?;
                    handle_window_mode_set_response(resp, parsed_policy, is_json)?;
                }
                Some(WindowModeAction::Status { .. }) | None => {
                    let req = Request::WindowMode { policy: None };
                    let resp = send_daemon_request(socket_path, &req).await?;
                    handle_window_mode_status_response(socket_path, resp, is_json).await?;
                }
            }
        }
        Command::Snap { layout } => {
            let snap_layout = SnapLayout::from_str(&layout)
                .map_err(|e| anyhow::anyhow!("Invalid snap layout: {}. Available: half-left, half-right, half-top, half-bottom, two-thirds-left, one-third-right, one-third-left, two-thirds-right, center, maximize, restore", e))?;
            let resp = send_daemon_request(socket_path, &Request::ApplyLayout { layout: snap_layout }).await?;
            handle_simple_response(resp)?;
        }
        Command::Monitor { direction } => {
            let resp = send_daemon_request(socket_path, &Request::MoveWindowToMonitor { direction }).await?;
            handle_simple_response(resp)?;
        }
        Command::Profile(profile_cmd) => match profile_cmd {
            ProfileCommand::Create { name, desc, copy_from } => {
                let req = Request::CreateProfile {
                    name,
                    description: desc,
                    copy_from,
                };
                let resp = send_daemon_request(socket_path, &req).await?;
                handle_simple_response(resp)?;
            }
            ProfileCommand::Bind { profile, key, action, desc } => {
                let req = Request::UpdateBinding {
                    profile,
                    key_combo: key,
                    action,
                    description: desc,
                };
                let resp = send_daemon_request(socket_path, &req).await?;
                handle_simple_response(resp)?;
            }
            ProfileCommand::Unbind { profile, key } => {
                let req = Request::RemoveBinding {
                    profile,
                    key_combo: key,
                };
                let resp = send_daemon_request(socket_path, &req).await?;
                handle_simple_response(resp)?;
            }
            ProfileCommand::Save { profile } => {
                let req = Request::SaveProfile { profile };
                let resp = send_daemon_request(socket_path, &req).await?;
                handle_simple_response(resp)?;
            }
            ProfileCommand::Delete { profile } => {
                let req = Request::DeleteProfile { profile };
                let resp = send_daemon_request(socket_path, &req).await?;
                handle_simple_response(resp)?;
            }
        },
        Command::Stats { json, export } => {
            if let Some(path) = export {
                let path_str = path.to_string_lossy().to_string();
                let resp = send_daemon_request(socket_path, &Request::ExportStats { path: Some(path_str) }).await?;
                handle_simple_response(resp)?;
            } else {
                let resp = send_daemon_request(socket_path, &Request::GetStats).await?;
                handle_stats_response(resp, json)?;
            }
        }
        Command::Recommend { json } => {
            let resp = send_daemon_request(socket_path, &Request::GetRecommendations).await?;
            handle_recommendations_response(resp, json)?;
        }
        Command::Tui => {
            aethershift_tui::run_tui(Some(socket_path.to_path_buf()))
                .await
                .context("TUI execution failed")?;
        }
        Command::Shutdown => {
            let resp = send_daemon_request(socket_path, &Request::Shutdown).await?;
            handle_simple_response(resp)?;
        }
        Command::Daemon(daemon_args) => {
            let config = aethershift_daemon::DaemonConfig {
                socket_path: Some(socket_path.to_path_buf()),
                preset_dir: daemon_args.preset_dir,
                verbose: daemon_args.verbose,
                no_notify: daemon_args.no_notify,
            };
            println!("Starting AetherShift daemon on {}...", socket_path.display());
            let daemon = aethershift_daemon::AetherDaemon::new(config);
            daemon.run().await.context("Daemon execution failed")?;
        }
    }

    Ok(())
}

pub fn format_status_text(status: &StatusInfo) -> String {
    let uptime_str = format_duration(status.uptime_secs);
    format!(
        "AetherShift Daemon Status\n========================\n  Active Profile:   {}\n  Window Policy:    {}\n  Active Overlays:  {}\n  Daemon Uptime:    {}\n  Daemon Version:   v{}",
        status.active_profile,
        status.window_policy.as_str(),
        status.overlays_count,
        uptime_str,
        status.version
    )
}

fn handle_status_response(resp: Response, json_output: bool) -> Result<()> {
    match resp {
        Response::Success { message: _, data } => {
            let data_val = data.unwrap_or(serde_json::Value::Null);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&data_val)?);
            } else {
                let status: StatusInfo = serde_json::from_value(data_val)?;
                println!("{}", format_status_text(&status));
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

#[derive(serde::Deserialize)]
struct ProfileInfoItem {
    pub name: String,
    pub description: String,
    pub bindings_count: usize,
    pub is_active: bool,
}

fn handle_list_response(resp: Response, json_output: bool) -> Result<()> {
    match resp {
        Response::Success { message: _, data } => {
            let data_val = data.unwrap_or(serde_json::Value::Null);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&data_val)?);
            } else {
                let profiles: Vec<ProfileInfoItem> = serde_json::from_value(data_val)?;
                println!("Available Profiles:");
                println!("{:<15} {:<10} {:<10} DESCRIPTION", "PROFILE", "BINDINGS", "STATUS");
                println!("{:-<15} {:-<10} {:-<10} {:-<35}", "", "", "", "");
                for p in profiles {
                    let status = if p.is_active { "* active" } else { "" };
                    println!(
                        "{:<15} {:<10} {:<10} {}",
                        p.name, p.bindings_count, status, p.description
                    );
                }
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

fn handle_stats_response(resp: Response, json_output: bool) -> Result<()> {
    match resp {
        Response::Success { data, .. } => {
            let data_val = data.unwrap_or(serde_json::Value::Null);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&data_val)?);
            } else {
                let stats: UsageStats = serde_json::from_value(data_val)?;
                println!("AetherShift Usage Statistics");
                println!("===========================");
                println!("  Total Profile Switches:  {}", stats.total_switches);
                println!("  Total Actions Executed:  {}", stats.total_actions);
                println!("  Daemon Uptime:           {}", format_duration(stats.uptime_secs));

                if !stats.action_counts.is_empty() {
                    println!();
                    println!("Top Actions Executed:");
                    let mut actions: Vec<(&String, &u64)> = stats.action_counts.iter().collect();
                    actions.sort_by(|a, b| b.1.cmp(a.1));
                    for (action, count) in actions.iter().take(10) {
                        println!("  {:<25} {} times", action, count);
                    }
                }

                if !stats.profile_usage.is_empty() {
                    println!();
                    println!("Profile Usage Durations:");
                    for (prof, duration) in &stats.profile_usage {
                        println!("  {:<20} {}", prof, format_duration(*duration));
                    }
                }
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

fn handle_recommendations_response(resp: Response, json_output: bool) -> Result<()> {
    match resp {
        Response::Success { data, .. } => {
            let data_val = data.unwrap_or(serde_json::Value::Null);
            if json_output {
                println!("{}", serde_json::to_string_pretty(&data_val)?);
            } else {
                let recs: Vec<Recommendation> = serde_json::from_value(data_val)?;
                if recs.is_empty() {
                    println!("No recommendations at this time. Keep using AetherShift to generate personalized insights!");
                } else {
                    println!("Personalized Ergonomic Recommendations");
                    println!("=====================================");
                    for (i, rec) in recs.iter().enumerate() {
                        println!("{}. [{}] {}", i + 1, rec.suggestion_type, rec.title);
                        println!("   {}", rec.message);
                        if let Some(ref act) = rec.suggested_action {
                            println!("   Tip: Run `aethershift profile bind <profile> <key> {}`", act);
                        }
                        println!();
                    }
                }
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

fn handle_simple_response(resp: Response) -> Result<()> {
    match resp {
        Response::Success { message, .. } => {
            println!("{}", message);
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m {s}s")
    } else if hours > 0 {
        format!("{hours}h {mins}m {s}s")
    } else if mins > 0 {
        format!("{mins}m {s}s")
    } else {
        format!("{s}s")
    }
}

fn extract_policy_string(val: &serde_json::Value) -> Option<String> {
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = val.as_object() {
        if let Some(p) = obj.get("policy").and_then(|v| v.as_str()) {
            return Some(p.to_string());
        }
        if let Some(p) = obj.get("window_policy").and_then(|v| v.as_str()) {
            return Some(p.to_string());
        }
        if let Some(p) = obj.get("mode").and_then(|v| v.as_str()) {
            return Some(p.to_string());
        }
    }
    if let Ok(wp) = serde_json::from_value::<WindowPolicy>(val.clone()) {
        return Some(wp.as_str().to_string());
    }
    None
}

fn extract_policy_from_str(s: &str) -> Option<String> {
    let lower = s.to_lowercase();
    for candidate in &["floating", "tiled", "omarchy", "follow-profile"] {
        if lower.contains(candidate) {
            return Some(candidate.to_string());
        }
    }
    if lower.contains("follow_profile") {
        return Some("follow-profile".to_string());
    }
    None
}

async fn handle_window_mode_status_response(
    socket_path: &Path,
    resp: Response,
    json_output: bool,
) -> Result<()> {
    match resp {
        Response::Success { message, data } => {
            let policy_opt = data.as_ref().and_then(extract_policy_string);

            let policy = if let Some(p) = policy_opt {
                p
            } else if let Some(p) = extract_policy_from_str(&message) {
                p
            } else {
                // Query Request::Status as reliable fallback to retrieve the active window policy
                let status_resp = send_daemon_request(socket_path, &Request::Status).await?;
                match status_resp {
                    Response::Success { data: Some(val), .. } => {
                        let status: StatusInfo = serde_json::from_value(val)?;
                        status.window_policy.as_str().to_string()
                    }
                    _ => "omarchy".to_string(),
                }
            };

            if json_output {
                let json_val = if let Some(ref val) = data {
                    if val.is_object() {
                        val.clone()
                    } else {
                        serde_json::json!({
                            "policy": policy,
                            "window_policy": policy,
                        })
                    }
                } else {
                    serde_json::json!({
                        "policy": policy,
                        "window_policy": policy,
                    })
                };
                println!("{}", serde_json::to_string_pretty(&json_val)?);
            } else {
                println!("Window Policy: {}", policy);
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}

fn handle_window_mode_set_response(
    resp: Response,
    policy: WindowPolicy,
    json_output: bool,
) -> Result<()> {
    match resp {
        Response::Success { message, data } => {
            let display_msg = if message == "Request processed (no-op)" || message.is_empty() {
                format!("Window policy set to '{}'", policy.as_str())
            } else {
                message
            };

            if json_output {
                let json_val = if let Some(ref val) = data {
                    if val.is_object() {
                        val.clone()
                    } else {
                        serde_json::json!({
                            "status": "success",
                            "policy": policy.as_str(),
                            "window_policy": policy.as_str(),
                            "message": display_msg,
                        })
                    }
                } else {
                    serde_json::json!({
                        "status": "success",
                        "policy": policy.as_str(),
                        "window_policy": policy.as_str(),
                        "message": display_msg,
                    })
                };
                println!("{}", serde_json::to_string_pretty(&json_val)?);
            } else {
                println!("{display_msg}");
            }
            Ok(())
        }
        Response::Error { code, message } => {
            bail!("Daemon returned error [{code}]: {message}");
        }
        _ => Ok(()),
    }
}
