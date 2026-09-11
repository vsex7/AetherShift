use std::process::Command;
use tracing::debug;

const APP_NAME: &str = "AetherShift";
const DEFAULT_ICON: &str = "preferences-desktop-keyboard-shortcuts";
const ERROR_ICON: &str = "dialog-error";

/// Send a lightweight desktop notification via `notify-send` if not disabled.
pub fn send_notification(summary: &str, body: &str, is_error: bool, disabled: bool) {
    if disabled {
        return;
    }

    let icon = if is_error { ERROR_ICON } else { DEFAULT_ICON };
    let urgency = if is_error { "critical" } else { "normal" };

    let summary_owned = summary.to_string();
    let body_owned = body.to_string();

    std::thread::spawn(move || {
        let mut cmd = Command::new("notify-send");
        cmd.arg("-a")
            .arg(APP_NAME)
            .arg("-i")
            .arg(icon)
            .arg("-u")
            .arg(urgency)
            .arg(&summary_owned)
            .arg(&body_owned);

        match cmd.status() {
            Ok(status) => {
                debug!("notify-send executed with status: {:?}", status);
            }
            Err(e) => {
                debug!("notify-send failed to spawn: {}", e);
            }
        }
    });
}
