use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

use crate::binding::Binding;
use crate::error::CoreError;
use crate::profile_metadata::{ConflictPolicy, WindowPolicyKind};

/// A profile represents a cohesive set of key bindings and metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub bindings: Vec<Binding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conflicts_policy: Option<ConflictPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_policy: Option<WindowPolicyKind>,
}

impl Profile {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        bindings: Vec<Binding>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            bindings,
            priority: None,
            conflicts_policy: None,
            window_policy: None,
        }
    }

    /// Load a profile from a TOML string and validate it
    pub fn from_toml_str(content: &str) -> Result<Self, CoreError> {
        let profile: Profile = toml::from_str(content).map_err(|e| CoreError::TomlParse {
            path: "<string>".into(),
            source: e,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Load a profile from a TOML file and validate it
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref).map_err(|e| CoreError::Io {
            path: path_ref.to_path_buf(),
            source: e,
        })?;
        let profile: Profile = toml::from_str(&content).map_err(|e| CoreError::TomlParse {
            path: path_ref.to_path_buf(),
            source: e,
        })?;
        profile.validate()?;
        Ok(profile)
    }

    /// Serialize this profile to a formatted TOML string
    pub fn to_toml_string(&self) -> Result<String, CoreError> {
        toml::to_string_pretty(self).map_err(|e| CoreError::Validation {
            profile: self.name.clone(),
            message: format!("Serialization failed: {e}"),
        })
    }

    /// Validate the profile: non-empty name, no duplicate key combos, valid bindings
    pub fn validate(&self) -> Result<(), CoreError> {
        let trimmed_name = self.name.trim();
        if trimmed_name.is_empty() {
            return Err(CoreError::Validation {
                profile: self.name.clone(),
                message: "Profile name cannot be empty".to_string(),
            });
        }

        let mut seen_combos = HashSet::new();
        for b in &self.bindings {
            let combo_str = b.key_combo.canonical_str();
            if !seen_combos.insert(combo_str.clone()) {
                return Err(CoreError::Validation {
                    profile: self.name.clone(),
                    message: format!("Duplicate key_combo '{combo_str}' found in profile bindings"),
                });
            }
        }

        Ok(())
    }
}
