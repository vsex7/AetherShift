pub mod config;
pub mod daemon;
pub mod error;
pub mod health;
pub mod notify;

pub use config::DaemonConfig;
pub use daemon::AetherDaemon;
pub use error::DaemonError;
pub use health::HealthState;

#[cfg(test)]
mod tests {
    use super::*;
    use aethershift_core::binding::Action;
    use aethershift_hyprland::action::HyprAction;

    #[test]
    fn test_core_action_to_hypr() {
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::CloseWindow),
            HyprAction::CloseWindow
        );
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::FloatToggle),
            HyprAction::ToggleFloating
        );
    }
}
