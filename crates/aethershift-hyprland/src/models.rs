use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyprMonitor {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub width: i32,
    pub height: i32,
    #[serde(rename = "refreshRate")]
    pub refresh_rate: Option<f64>,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub focused: bool,
    #[serde(default)]
    pub reserved: Vec<i32>,
}

impl HyprMonitor {
    /// Compute the usable work area accounting for reserved panels/bars.
    /// Reserved array layout in Hyprland is typically [left, top, right, bottom].
    pub fn work_area(&self) -> (i32, i32, i32, i32) {
        let left = self.reserved.first().copied().unwrap_or(0);
        let top = self.reserved.get(1).copied().unwrap_or(0);
        let right = self.reserved.get(2).copied().unwrap_or(0);
        let bottom = self.reserved.get(3).copied().unwrap_or(0);

        let area_x = self.x + left;
        let area_y = self.y + top;
        let area_w = (self.width - left - right).max(1);
        let area_h = (self.height - top - bottom).max(1);

        (area_x, area_y, area_w, area_h)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyprWindowWorkspace {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HyprWindow {
    pub address: String,
    pub mapped: Option<bool>,
    pub hidden: Option<bool>,
    pub at: (i32, i32),
    pub size: (i32, i32),
    pub workspace: HyprWindowWorkspace,
    pub floating: bool,
    pub monitor: i64,
    #[serde(rename = "class")]
    pub window_class: String,
    pub title: String,
    pub pid: u32,
}
