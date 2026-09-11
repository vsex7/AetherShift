use serde::{Deserialize, Serialize};

use crate::binding::{Action, Binding};
use crate::profile_metadata::ConflictPolicy;
use crate::state::{BaselineBinding, SwitchPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingClassification {
    New,
    Identical,
    Conflict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingDecision {
    Applied,
    Skipped,
    Forced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictEntry {
    pub key_combo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_dispatcher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_args: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_action: Option<Action>,
    pub classification: BindingClassification,
    pub decision: BindingDecision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConflictReport {
    pub entries: Vec<ConflictEntry>,
    pub applied: usize,
    pub skipped: usize,
    pub forced: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSwitchPlan {
    pub plan: SwitchPlan,
    pub report: ConflictReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticFamily {
    Close,
    MaximizeRestore,
    Float,
    WindowMotion,
    WorkspaceNavigation,
    LauncherSpecial,
    ExecCustom,
}

pub fn action_semantic_family(action: &Action) -> SemanticFamily {
    match action {
        Action::CloseWindow => SemanticFamily::Close,
        Action::Maximize | Action::Restore | Action::Fullscreen => SemanticFamily::MaximizeRestore,
        Action::FloatToggle => SemanticFamily::Float,
        Action::SnapLeft | Action::SnapRight => SemanticFamily::WindowMotion,
        Action::CycleWindowNext | Action::CycleWindowPrev => SemanticFamily::WindowMotion,
        Action::WorkspaceNext
        | Action::WorkspacePrev
        | Action::MoveToWorkspaceNext
        | Action::MoveToWorkspacePrev => SemanticFamily::WorkspaceNavigation,
        Action::ToggleSpecialWorkspace(_) | Action::ToggleLauncher(_) => {
            SemanticFamily::LauncherSpecial
        }
        Action::Exec(_) | Action::Plugin(_) | Action::Custom { .. } => SemanticFamily::ExecCustom,
    }
}

pub fn baseline_semantic_family(dispatcher: &str, _args: &str) -> SemanticFamily {
    let dispatcher = dispatcher.to_ascii_lowercase();
    match dispatcher.as_str() {
        "killactive" | "closewindow" => SemanticFamily::Close,
        "fullscreen" => SemanticFamily::MaximizeRestore,
        "togglefloating" | "float" => SemanticFamily::Float,
        "movewindow" | "movefocus" => SemanticFamily::WindowMotion,
        "workspace" | "movetoworkspace" | "movetoworkspacesilent" => {
            SemanticFamily::WorkspaceNavigation
        }
        "exec" => SemanticFamily::ExecCustom,
        _ => {
            if dispatcher.starts_with("toggle") || dispatcher.starts_with("special") {
                SemanticFamily::LauncherSpecial
            } else {
                SemanticFamily::ExecCustom
            }
        }
    }
}

pub fn resolve_for_policy(
    _combo: &crate::binding::KeyCombo,
    baseline: Option<&BaselineBinding>,
    target: &Binding,
    policy: ConflictPolicy,
) -> (BindingClassification, BindingDecision, String) {
    let Some(base) = baseline else {
        return (
            BindingClassification::New,
            BindingDecision::Applied,
            "no protected baseline binding".to_string(),
        );
    };

    if base.dispatcher == target.action.to_hyprland_dispatcher().0
        && base.args == target.action.to_hyprland_dispatcher().1
    {
        return (
            BindingClassification::Identical,
            BindingDecision::Skipped,
            "target action is identical to protected baseline".to_string(),
        );
    }

    if policy == ConflictPolicy::Force {
        return (
            BindingClassification::Conflict,
            BindingDecision::Forced,
            "force policy overrides protected baseline".to_string(),
        );
    }

    let same_family = action_semantic_family(&target.action)
        == baseline_semantic_family(&base.dispatcher, &base.args);
    if policy == ConflictPolicy::Smart && same_family {
        (
            BindingClassification::Conflict,
            BindingDecision::Forced,
            "smart policy allows same-family override".to_string(),
        )
    } else {
        (
            BindingClassification::Conflict,
            BindingDecision::Skipped,
            "protected baseline binding preserved".to_string(),
        )
    }
}
