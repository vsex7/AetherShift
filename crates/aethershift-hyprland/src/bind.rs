use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::action::HyprAction;

/// Raw binding info as returned by Hyprland `j/binds` socket command
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyprBind {
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub mouse: bool,
    #[serde(default)]
    pub release: bool,
    #[serde(default)]
    pub repeat: bool,
    #[serde(default, rename = "longPress")]
    pub long_press: bool,
    #[serde(default)]
    pub non_consuming: bool,
    #[serde(default)]
    pub auto_consuming: bool,
    #[serde(default)]
    pub has_description: bool,
    #[serde(default)]
    pub modmask: u32,
    #[serde(default)]
    pub submap: String,
    #[serde(default)]
    pub submap_universal: String,
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub keycode: i32,
    #[serde(default)]
    pub catch_all: bool,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub allow_input_capture: bool,
    #[serde(default)]
    pub dispatcher: String,
    #[serde(default)]
    pub arg: String,
}

impl HyprBind {
    /// Convert modmask bitmask into human-readable modifier list (e.g. ["SUPER", "SHIFT"])
    /// Bit 0: SHIFT (1)
    /// Bit 1: CAPS (2)
    /// Bit 2: CTRL (4)
    /// Bit 3: ALT (8)
    /// Bit 4: MOD2/NUM (16)
    /// Bit 5: MOD3 (32)
    /// Bit 6: SUPER / MOD4 (64)
    /// Bit 7: MOD5 (128)
    pub fn modifiers(&self) -> BTreeSet<&'static str> {
        let mut mods = BTreeSet::new();
        if self.modmask & 64 != 0 {
            mods.insert("SUPER");
        }
        if self.modmask & 4 != 0 {
            mods.insert("CTRL");
        }
        if self.modmask & 8 != 0 {
            mods.insert("ALT");
        }
        if self.modmask & 1 != 0 {
            mods.insert("SHIFT");
        }
        mods
    }

    /// Return a formatted key combo string if a key name is available
    pub fn formatted_combo(&self) -> Option<String> {
        if self.key.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        if self.modmask & 64 != 0 {
            parts.push("SUPER");
        }
        if self.modmask & 4 != 0 {
            parts.push("CTRL");
        }
        if self.modmask & 8 != 0 {
            parts.push("ALT");
        }
        if self.modmask & 1 != 0 {
            parts.push("SHIFT");
        }
        parts.push(&self.key);
        Some(parts.join(" + "))
    }
}

/// A target key binding specification to be bound in Hyprland
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyprKeyBinding {
    /// Key combination string, e.g. "SUPER + Q" or "CTRL + ALT + T"
    pub keys: String,
    /// Action to execute when triggered
    pub action: HyprAction,
    /// Optional description
    pub description: Option<String>,
}

impl HyprKeyBinding {
    pub fn new(keys: impl Into<String>, action: HyprAction) -> Self {
        Self {
            keys: keys.into(),
            action,
            description: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_hypr_bind() {
        let json_str = r#"{
            "locked": true,
            "mouse": false,
            "release": false,
            "repeat": true,
            "longPress": false,
            "non_consuming": false,
            "auto_consuming": false,
            "has_description": true,
            "modmask": 68,
            "submap": "",
            "submap_universal": "false",
            "key": "Z",
            "keycode": 0,
            "catch_all": false,
            "description": "Test binding",
            "allow_input_capture": false,
            "dispatcher": "__lua",
            "arg": "123"
        }"#;

        let bind: HyprBind = serde_json::from_str(json_str).expect("Failed to deserialize");
        assert!(bind.locked);
        assert!(!bind.mouse);
        assert_eq!(bind.modmask, 68); // 64 (SUPER) + 4 (CTRL)
        assert_eq!(bind.key, "Z");
        assert_eq!(bind.description, "Test binding");

        let mods = bind.modifiers();
        assert!(mods.contains("SUPER"));
        assert!(mods.contains("CTRL"));
        assert_eq!(mods.len(), 2);

        assert_eq!(bind.formatted_combo(), Some("SUPER + CTRL + Z".to_string()));
    }

    #[test]
    fn test_hypr_key_binding_builder() {
        let b = HyprKeyBinding::new("SUPER + Q", HyprAction::CloseWindow)
            .with_description("Close current window");
        assert_eq!(b.keys, "SUPER + Q");
        assert_eq!(b.action, HyprAction::CloseWindow);
        assert_eq!(b.description.as_deref(), Some("Close current window"));
    }
}
