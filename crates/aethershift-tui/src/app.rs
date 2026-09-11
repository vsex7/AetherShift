use std::path::PathBuf;
use std::time::Instant;

use aethershift_protocol::{
    Recommendation, Request, Response, SnapLayout, StatusInfo, UsageStats, call,
    default_socket_path,
};
use ratatui::widgets::ListState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPanel {
    Profiles,
    Snaps,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProfileItem {
    pub name: String,
    pub description: String,
    pub bindings_count: usize,
    pub is_active: bool,
}

pub const ALL_SNAP_LAYOUTS: [SnapLayout; 14] = [
    SnapLayout::HalfLeft,
    SnapLayout::HalfRight,
    SnapLayout::HalfTop,
    SnapLayout::HalfBottom,
    SnapLayout::TwoThirdsLeft,
    SnapLayout::OneThirdRight,
    SnapLayout::OneThirdLeft,
    SnapLayout::TwoThirdsRight,
    SnapLayout::ThreeColumnsLeft,
    SnapLayout::ThreeColumnsCenter,
    SnapLayout::ThreeColumnsRight,
    SnapLayout::CenterFloating,
    SnapLayout::Maximize,
    SnapLayout::RestoreOriginal,
];

pub struct App {
    pub socket_path: PathBuf,
    pub focus: FocusPanel,
    pub profiles: Vec<ProfileItem>,
    pub profiles_state: ListState,
    pub selected_snap_index: usize,
    pub status: Option<StatusInfo>,
    pub stats: Option<UsageStats>,
    pub recommendations: Vec<Recommendation>,
    pub message: Option<(String, Instant, bool)>, // (msg, time, is_error)
    pub connected: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(socket_path: Option<PathBuf>) -> Self {
        let path = socket_path.unwrap_or_else(default_socket_path);
        let mut profiles_state = ListState::default();
        profiles_state.select(Some(0));

        Self {
            socket_path: path,
            focus: FocusPanel::Profiles,
            profiles: Vec::new(),
            profiles_state,
            selected_snap_index: 0,
            status: None,
            stats: None,
            recommendations: Vec::new(),
            message: None,
            connected: false,
            should_quit: false,
        }
    }

    pub fn set_info_message(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), Instant::now(), false));
    }

    pub fn set_error_message(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), Instant::now(), true));
    }

    pub fn clear_expired_message(&mut self) {
        if let Some((_, time, _)) = self.message {
            if time.elapsed().as_secs() >= 5 {
                self.message = None;
            }
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPanel::Profiles => FocusPanel::Snaps,
            FocusPanel::Snaps => FocusPanel::Profiles,
        };
    }

    pub fn next(&mut self) {
        match self.focus {
            FocusPanel::Profiles => {
                if self.profiles.is_empty() {
                    return;
                }
                let i = match self.profiles_state.selected() {
                    Some(i) => {
                        if i + 1 >= self.profiles.len() {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.profiles_state.select(Some(i));
            }
            FocusPanel::Snaps => {
                let total = ALL_SNAP_LAYOUTS.len();
                self.selected_snap_index = (self.selected_snap_index + 1) % total;
            }
        }
    }

    pub fn previous(&mut self) {
        match self.focus {
            FocusPanel::Profiles => {
                if self.profiles.is_empty() {
                    return;
                }
                let i = match self.profiles_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.profiles.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.profiles_state.select(Some(i));
            }
            FocusPanel::Snaps => {
                let total = ALL_SNAP_LAYOUTS.len();
                if self.selected_snap_index == 0 {
                    self.selected_snap_index = total.saturating_sub(1);
                } else {
                    self.selected_snap_index -= 1;
                }
            }
        }
    }

    pub fn selected_profile(&self) -> Option<&ProfileItem> {
        let idx = self.profiles_state.selected()?;
        self.profiles.get(idx)
    }

    pub fn selected_snap(&self) -> SnapLayout {
        ALL_SNAP_LAYOUTS[self.selected_snap_index]
    }

    pub async fn refresh(&mut self) {
        // Fetch status
        let status_res = call(&self.socket_path, &Request::Status).await;
        match status_res {
            Ok(Response::Success { data, .. }) => {
                self.connected = true;
                if let Some(val) = data {
                    if let Ok(info) = serde_json::from_value::<StatusInfo>(val) {
                        self.status = Some(info);
                    }
                }
            }
            _ => {
                self.connected = false;
                self.status = None;
            }
        }

        // Fetch profiles
        if self.connected {
            if let Ok(Response::Success { data, .. }) =
                call(&self.socket_path, &Request::ListProfiles).await
            {
                if let Some(val) = data {
                    if let Ok(items) = serde_json::from_value::<Vec<ProfileItem>>(val) {
                        self.profiles = items;
                        if let Some(sel) = self.profiles_state.selected() {
                            if sel >= self.profiles.len() && !self.profiles.is_empty() {
                                self.profiles_state.select(Some(self.profiles.len() - 1));
                            }
                        } else if !self.profiles.is_empty() {
                            self.profiles_state.select(Some(0));
                        }
                    }
                }
            }

            // Fetch stats
            if let Ok(Response::Success { data, .. }) =
                call(&self.socket_path, &Request::GetStats).await
            {
                if let Some(val) = data {
                    if let Ok(s) = serde_json::from_value::<UsageStats>(val) {
                        self.stats = Some(s);
                    }
                }
            }

            // Fetch recommendations
            if let Ok(Response::Success { data, .. }) =
                call(&self.socket_path, &Request::GetRecommendations).await
            {
                if let Some(val) = data {
                    if let Ok(r) = serde_json::from_value::<Vec<Recommendation>>(val) {
                        self.recommendations = r;
                    }
                }
            }
        }
    }

    pub async fn switch_selected_profile(&mut self) {
        let name = match self.selected_profile() {
            Some(p) => p.name.clone(),
            None => return,
        };

        match call(
            &self.socket_path,
            &Request::Switch {
                profile: name.clone(),
                force: false,
            },
        )
        .await
        {
            Ok(Response::Success { message, .. }) => {
                self.set_info_message(message);
                self.refresh().await;
            }
            Ok(Response::Error { message, .. }) => {
                self.set_error_message(message);
            }
            Err(e) => {
                self.set_error_message(format!("IPC error: {e}"));
            }
            _ => {}
        }
    }

    pub async fn cycle_profile(&mut self) {
        match call(&self.socket_path, &Request::Cycle).await {
            Ok(Response::Success { message, .. }) => {
                self.set_info_message(message);
                self.refresh().await;
            }
            Ok(Response::Error { message, .. }) => {
                self.set_error_message(message);
            }
            Err(e) => {
                self.set_error_message(format!("IPC error: {e}"));
            }
            _ => {}
        }
    }

    pub async fn restore_baseline(&mut self) {
        match call(&self.socket_path, &Request::Restore).await {
            Ok(Response::Success { message, .. }) => {
                self.set_info_message(message);
                self.refresh().await;
            }
            Ok(Response::Error { message, .. }) => {
                self.set_error_message(message);
            }
            Err(e) => {
                self.set_error_message(format!("IPC error: {e}"));
            }
            _ => {}
        }
    }

    pub async fn apply_snap(&mut self) {
        let layout = self.selected_snap();
        match call(
            &self.socket_path,
            &Request::ApplyLayout {
                layout,
                preview: false,
            },
        )
        .await
        {
            Ok(Response::Success { message, .. }) => {
                self.set_info_message(message);
                self.refresh().await;
            }
            Ok(Response::Error { message, .. }) => {
                self.set_error_message(message);
            }
            Err(e) => {
                self.set_error_message(format!("IPC error: {e}"));
            }
            _ => {}
        }
    }
}
