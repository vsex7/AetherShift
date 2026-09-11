use serde::{Deserialize, Serialize};

/// How conflicts with protected baseline bindings are resolved.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictPolicy {
    #[default]
    Strict,
    Force,
    Smart,
}

impl ConflictPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Force => "force",
            Self::Smart => "smart",
        }
    }
}

/// Window policy associated with a profile for `follow-profile`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowPolicyKind {
    #[default]
    Omarchy,
    Tiled,
    Floating,
    FollowProfile,
}

impl WindowPolicyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Omarchy => "omarchy",
            Self::Tiled => "tiled",
            Self::Floating => "floating",
            Self::FollowProfile => "follow-profile",
        }
    }
}
