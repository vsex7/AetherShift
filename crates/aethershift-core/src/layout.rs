use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Window snap layout presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapLayout {
    HalfLeft,
    HalfRight,
    HalfTop,
    HalfBottom,
    TwoThirdsLeft,
    OneThirdRight,
    OneThirdLeft,
    TwoThirdsRight,
    ThreeColumnsLeft,
    ThreeColumnsCenter,
    ThreeColumnsRight,
    Maximize,
    CenterFloating,
    RestoreOriginal,
}

impl SnapLayout {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::HalfLeft => "half-left",
            Self::HalfRight => "half-right",
            Self::HalfTop => "half-top",
            Self::HalfBottom => "half-bottom",
            Self::TwoThirdsLeft => "two-thirds-left",
            Self::OneThirdRight => "one-third-right",
            Self::OneThirdLeft => "one-third-left",
            Self::TwoThirdsRight => "two-thirds-right",
            Self::ThreeColumnsLeft => "three-columns-left",
            Self::ThreeColumnsCenter => "three-columns-center",
            Self::ThreeColumnsRight => "three-columns-right",
            Self::Maximize => "maximize",
            Self::CenterFloating => "center-floating",
            Self::RestoreOriginal => "restore-original",
        }
    }
}

impl FromStr for SnapLayout {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "half-left" | "left" => Ok(Self::HalfLeft),
            "half-right" | "right" => Ok(Self::HalfRight),
            "half-top" | "top" => Ok(Self::HalfTop),
            "half-bottom" | "bottom" => Ok(Self::HalfBottom),
            "two-thirds-left" | "2/3-left" => Ok(Self::TwoThirdsLeft),
            "one-third-right" | "1/3-right" => Ok(Self::OneThirdRight),
            "one-third-left" | "1/3-left" => Ok(Self::OneThirdLeft),
            "two-thirds-right" | "2/3-right" => Ok(Self::TwoThirdsRight),
            "three-columns-left" | "3col-left" | "three-col-left" => Ok(Self::ThreeColumnsLeft),
            "three-columns-center" | "3col-center" | "three-col-center" => {
                Ok(Self::ThreeColumnsCenter)
            }
            "three-columns-right" | "3col-right" | "three-col-right" => Ok(Self::ThreeColumnsRight),
            "maximize" | "max" => Ok(Self::Maximize),
            "center-floating" | "center" | "floating-center" => Ok(Self::CenterFloating),
            "restore-original" | "restore" => Ok(Self::RestoreOriginal),
            other => Err(format!("unknown snap layout: '{other}'")),
        }
    }
}

impl std::fmt::Display for SnapLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 2D Rectangle structure representing window or screen geometry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// LayoutEngine handles pure layout calculations and window geometry management
#[derive(Debug, Clone, Default)]
pub struct LayoutEngine;

impl LayoutEngine {
    /// Pure function to calculate window geometry given a SnapLayout and the monitor's work_area Rect.
    ///
    /// Layout calculations:
    /// - HalfLeft: (x, y, w/2, h)
    /// - HalfRight: (x + w/2, y, w - w/2, h)
    /// - HalfTop: (x, y, w, h/2)
    /// - HalfBottom: (x, y + h/2, w, h - h/2)
    /// - TwoThirdsLeft: (x, y, (w*2)/3, h)
    /// - OneThirdRight: (x + (w*2)/3, y, w - (w*2)/3, h)
    /// - OneThirdLeft: (x, y, w/3, h)
    /// - TwoThirdsRight: (x + w/3, y, w - w/3, h)
    /// - ThreeColumnsLeft: (x, y, w/3, h)
    /// - ThreeColumnsCenter: (x + w/3, y, w/3, h)
    /// - ThreeColumnsRight: (x + (w*2)/3, y, w - (w*2)/3, h)
    /// - Maximize: (x, y, w, h)
    /// - CenterFloating: (x + w*0.15, y + h*0.15, w*0.7, h*0.7)
    /// - RestoreOriginal: returns work_area as fallback if applied purely
    pub fn compute_geometry(layout: SnapLayout, work_area: Rect) -> Rect {
        let x = work_area.x;
        let y = work_area.y;
        let w = work_area.width;
        let h = work_area.height;

        match layout {
            SnapLayout::HalfLeft => Rect::new(x, y, w / 2, h),
            SnapLayout::HalfRight => Rect::new(x + w / 2, y, w - w / 2, h),
            SnapLayout::HalfTop => Rect::new(x, y, w, h / 2),
            SnapLayout::HalfBottom => Rect::new(x, y + h / 2, w, h - h / 2),
            SnapLayout::TwoThirdsLeft => Rect::new(x, y, (w * 2) / 3, h),
            SnapLayout::OneThirdRight => Rect::new(x + (w * 2) / 3, y, w - (w * 2) / 3, h),
            SnapLayout::OneThirdLeft => Rect::new(x, y, w / 3, h),
            SnapLayout::TwoThirdsRight => Rect::new(x + w / 3, y, w - w / 3, h),
            SnapLayout::ThreeColumnsLeft => Rect::new(x, y, w / 3, h),
            SnapLayout::ThreeColumnsCenter => Rect::new(x + w / 3, y, w / 3, h),
            SnapLayout::ThreeColumnsRight => Rect::new(x + (w * 2) / 3, y, w - (w * 2) / 3, h),
            SnapLayout::Maximize => Rect::new(x, y, w, h),
            SnapLayout::CenterFloating => {
                let fw = (w as f64 * 0.7).round() as i32;
                let fh = (h as f64 * 0.7).round() as i32;
                let fx = x + ((w - fw) / 2);
                let fy = y + ((h - fh) / 2);
                Rect::new(fx, fy, fw, fh)
            }
            SnapLayout::RestoreOriginal => Rect::new(x, y, w, h),
        }
    }
}

/// Geometry memory storing the original un-snapped window positions
#[derive(Debug, Clone, Default)]
pub struct GeometryMemory {
    memories: HashMap<String, Rect>,
}

impl GeometryMemory {
    pub fn new() -> Self {
        Self {
            memories: HashMap::new(),
        }
    }

    /// Record a window's original rect if not already present.
    /// Returns true if it was recorded (first time), or false if it was already memorized.
    pub fn record_if_absent(&mut self, address: impl Into<String>, rect: Rect) -> bool {
        let addr = address.into();
        if let std::collections::hash_map::Entry::Vacant(e) = self.memories.entry(addr) {
            e.insert(rect);
            true
        } else {
            false
        }
    }

    /// Retrieve and remove the original geometry for a window, restoring it from memory.
    pub fn restore_original(&mut self, address: &str) -> Option<Rect> {
        self.memories.remove(address)
    }

    /// Peek at the original geometry without removing it.
    pub fn peek_original(&self, address: &str) -> Option<&Rect> {
        self.memories.get(address)
    }

    /// Clear all stored geometries.
    pub fn clear(&mut self) {
        self.memories.clear();
    }

    /// Number of remembered window geometries.
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_geometry() {
        let work = Rect::new(0, 0, 1920, 1080);

        // HalfLeft
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfLeft, work),
            Rect::new(0, 0, 960, 1080)
        );

        // HalfRight
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfRight, work),
            Rect::new(960, 0, 960, 1080)
        );

        // HalfTop
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfTop, work),
            Rect::new(0, 0, 1920, 540)
        );

        // HalfBottom
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfBottom, work),
            Rect::new(0, 540, 1920, 540)
        );

        // TwoThirdsLeft: (w*2)/3 = 3840 / 3 = 1280
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::TwoThirdsLeft, work),
            Rect::new(0, 0, 1280, 1080)
        );

        // OneThirdRight: x = 1280, w = 1920 - 1280 = 640
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::OneThirdRight, work),
            Rect::new(1280, 0, 640, 1080)
        );

        // OneThirdLeft: w/3 = 640
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::OneThirdLeft, work),
            Rect::new(0, 0, 640, 1080)
        );

        // TwoThirdsRight: x = 640, w = 1920 - 640 = 1280
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::TwoThirdsRight, work),
            Rect::new(640, 0, 1280, 1080)
        );

        // ThreeColumnsLeft: (0, 0, 640, 1080)
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::ThreeColumnsLeft, work),
            Rect::new(0, 0, 640, 1080)
        );

        // ThreeColumnsCenter: (640, 0, 640, 1080)
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::ThreeColumnsCenter, work),
            Rect::new(640, 0, 640, 1080)
        );

        // ThreeColumnsRight: (1280, 0, 640, 1080)
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::ThreeColumnsRight, work),
            Rect::new(1280, 0, 640, 1080)
        );

        // Maximize
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::Maximize, work),
            Rect::new(0, 0, 1920, 1080)
        );

        // CenterFloating: 70% of 1920 = 1344, 70% of 1080 = 756
        // fx = (1920 - 1344) / 2 = 288
        // fy = (1080 - 756) / 2 = 162
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::CenterFloating, work),
            Rect::new(288, 162, 1344, 756)
        );

        // Off-origin work area
        let work_offset = Rect::new(1920, 50, 1920, 1030);
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfLeft, work_offset),
            Rect::new(1920, 50, 960, 1030)
        );
        assert_eq!(
            LayoutEngine::compute_geometry(SnapLayout::HalfRight, work_offset),
            Rect::new(2880, 50, 960, 1030)
        );
    }

    #[test]
    fn test_odd_dimension_compute_geometry() {
        // Test with non-divisible dimensions to check boundary handling
        let work = Rect::new(10, 20, 1001, 501);
        let left = LayoutEngine::compute_geometry(SnapLayout::HalfLeft, work);
        let right = LayoutEngine::compute_geometry(SnapLayout::HalfRight, work);

        assert_eq!(left.width + right.width, work.width);
        assert_eq!(left.x, work.x);
        assert_eq!(right.x, left.x + left.width);

        let top = LayoutEngine::compute_geometry(SnapLayout::HalfTop, work);
        let bottom = LayoutEngine::compute_geometry(SnapLayout::HalfBottom, work);
        assert_eq!(top.height + bottom.height, work.height);
        assert_eq!(top.y, work.y);
        assert_eq!(bottom.y, top.y + top.height);
    }

    #[test]
    fn test_geometry_memory() {
        let mut mem = GeometryMemory::new();
        assert!(mem.is_empty());

        let win1 = "0x55aa11";
        let rect1 = Rect::new(100, 100, 800, 600);

        // First record succeeds
        assert!(mem.record_if_absent(win1, rect1));
        assert_eq!(mem.len(), 1);

        // Second record for same window does not overwrite
        let rect_new = Rect::new(0, 0, 1920, 1080);
        assert!(!mem.record_if_absent(win1, rect_new));
        assert_eq!(mem.peek_original(win1), Some(&rect1));

        // Restore returns original rect and removes it
        let restored = mem.restore_original(win1);
        assert_eq!(restored, Some(rect1));
        assert!(mem.is_empty());
        assert_eq!(mem.restore_original(win1), None);
    }
}
