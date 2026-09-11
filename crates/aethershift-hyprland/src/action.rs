use std::fmt;

/// Direction for focus or window movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    /// Return the Hyprland direction code ('l', 'r', 'u', 'd')
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Left => "l",
            Direction::Right => "r",
            Direction::Up => "u",
            Direction::Down => "d",
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Target workspace: numeric ID, named workspace, or special workspace
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkspaceTarget {
    Id(i32),
    Name(String),
    Relative(i32),
    Previous,
}

impl WorkspaceTarget {
    pub fn to_lua_arg(&self) -> String {
        match self {
            WorkspaceTarget::Id(id) => id.to_string(),
            WorkspaceTarget::Name(name) => escape_lua_string(name),
            WorkspaceTarget::Relative(offset) => {
                if *offset >= 0 {
                    format!("\"m+{}\"", offset)
                } else {
                    format!("\"m{}\"", offset)
                }
            }
            WorkspaceTarget::Previous => "\"previous\"".to_string(),
        }
    }
}

impl From<i32> for WorkspaceTarget {
    fn from(id: i32) -> Self {
        WorkspaceTarget::Id(id)
    }
}

impl From<&str> for WorkspaceTarget {
    fn from(s: &str) -> Self {
        if let Ok(id) = s.parse::<i32>() {
            WorkspaceTarget::Id(id)
        } else {
            WorkspaceTarget::Name(s.to_string())
        }
    }
}

impl From<String> for WorkspaceTarget {
    fn from(s: String) -> Self {
        if let Ok(id) = s.parse::<i32>() {
            WorkspaceTarget::Id(id)
        } else {
            WorkspaceTarget::Name(s)
        }
    }
}

/// Helper to escape a string for inclusion in Lua code
pub fn escape_lua_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Common Hyprland actions that can be bound to key combinations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HyprAction {
    /// Close the active window: `hl.dsp.window.close()`
    CloseWindow,
    /// Toggle fullscreen mode: `hl.dsp.window.fullscreen({ mode = ... })` or `hl.dsp.window.fullscreen()`
    ToggleFullscreen(Option<i32>),
    /// Toggle floating state: `hl.dsp.window.float({ action = "toggle" })`
    ToggleFloating,
    /// Focus in direction: `hl.dsp.focus({ direction = ... })`
    Focus(Direction),
    /// Move active window in direction: `hl.dsp.window.move({ direction = ... })`
    MoveWindow(Direction),
    /// Focus workspace: `hl.dsp.focus({ workspace = ... })`
    Workspace(WorkspaceTarget),
    /// Move active window to workspace: `hl.dsp.window.move({ workspace = ... })`
    MoveToWorkspace(WorkspaceTarget),
    /// Execute arbitrary shell command: `hl.dsp.exec_cmd(...)`
    Exec(String),
    /// Custom Lua expression / code
    CustomLua(String),
}

impl HyprAction {
    /// Generate the corresponding Lua dispatcher expression
    pub fn to_lua_expr(&self) -> String {
        match self {
            HyprAction::CloseWindow => "hl.dsp.window.close()".to_string(),
            HyprAction::ToggleFullscreen(None) => "hl.dsp.window.fullscreen()".to_string(),
            HyprAction::ToggleFullscreen(Some(mode)) => {
                format!("hl.dsp.window.fullscreen({{ mode = {} }})", mode)
            }
            HyprAction::ToggleFloating => {
                "hl.dsp.window.float({ action = \"toggle\" })".to_string()
            }
            HyprAction::Focus(dir) => {
                format!("hl.dsp.focus({{ direction = \"{}\" }})", dir.as_str())
            }
            HyprAction::MoveWindow(dir) => {
                format!("hl.dsp.window.move({{ direction = \"{}\" }})", dir.as_str())
            }
            HyprAction::Workspace(target) => {
                format!("hl.dsp.focus({{ workspace = {} }})", target.to_lua_arg())
            }
            HyprAction::MoveToWorkspace(target) => {
                format!(
                    "hl.dsp.window.move({{ workspace = {} }})",
                    target.to_lua_arg()
                )
            }
            HyprAction::Exec(cmd) => {
                format!("hl.dsp.exec_cmd({})", escape_lua_string(cmd))
            }
            HyprAction::CustomLua(code) => code.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_lua_string() {
        assert_eq!(escape_lua_string("hello"), "\"hello\"");
        assert_eq!(
            escape_lua_string("hello \"world\""),
            "\"hello \\\"world\\\"\""
        );
        assert_eq!(
            escape_lua_string("path\\with\\slash"),
            "\"path\\\\with\\\\slash\""
        );
        assert_eq!(escape_lua_string("line1\nline2"), "\"line1\\nline2\"");
    }

    #[test]
    fn test_direction_strings() {
        assert_eq!(Direction::Left.as_str(), "l");
        assert_eq!(Direction::Right.as_str(), "r");
        assert_eq!(Direction::Up.as_str(), "u");
        assert_eq!(Direction::Down.as_str(), "d");
        assert_eq!(Direction::Left.to_string(), "l");
    }

    #[test]
    fn test_workspace_target() {
        assert_eq!(WorkspaceTarget::Id(3).to_lua_arg(), "3");
        assert_eq!(
            WorkspaceTarget::Name("special:scratch".to_string()).to_lua_arg(),
            "\"special:scratch\""
        );
        assert_eq!(WorkspaceTarget::Relative(1).to_lua_arg(), "\"m+1\"");
        assert_eq!(WorkspaceTarget::Relative(-1).to_lua_arg(), "\"m-1\"");
        assert_eq!(WorkspaceTarget::Previous.to_lua_arg(), "\"previous\"");

        let from_i32: WorkspaceTarget = 5.into();
        assert_eq!(from_i32, WorkspaceTarget::Id(5));

        let from_num_str: WorkspaceTarget = "42".into();
        assert_eq!(from_num_str, WorkspaceTarget::Id(42));

        let from_name_str: WorkspaceTarget = "web".into();
        assert_eq!(from_name_str, WorkspaceTarget::Name("web".to_string()));
    }

    #[test]
    fn test_hypr_action_lua_codegen() {
        assert_eq!(
            HyprAction::CloseWindow.to_lua_expr(),
            "hl.dsp.window.close()"
        );
        assert_eq!(
            HyprAction::ToggleFullscreen(None).to_lua_expr(),
            "hl.dsp.window.fullscreen()"
        );
        assert_eq!(
            HyprAction::ToggleFullscreen(Some(1)).to_lua_expr(),
            "hl.dsp.window.fullscreen({ mode = 1 })"
        );
        assert_eq!(
            HyprAction::ToggleFullscreen(Some(0)).to_lua_expr(),
            "hl.dsp.window.fullscreen({ mode = 0 })"
        );
        assert_eq!(
            HyprAction::ToggleFloating.to_lua_expr(),
            "hl.dsp.window.float({ action = \"toggle\" })"
        );
        assert_eq!(
            HyprAction::Focus(Direction::Left).to_lua_expr(),
            "hl.dsp.focus({ direction = \"l\" })"
        );
        assert_eq!(
            HyprAction::Focus(Direction::Right).to_lua_expr(),
            "hl.dsp.focus({ direction = \"r\" })"
        );
        assert_eq!(
            HyprAction::MoveWindow(Direction::Up).to_lua_expr(),
            "hl.dsp.window.move({ direction = \"u\" })"
        );
        assert_eq!(
            HyprAction::MoveWindow(Direction::Down).to_lua_expr(),
            "hl.dsp.window.move({ direction = \"d\" })"
        );
        assert_eq!(
            HyprAction::Workspace(1.into()).to_lua_expr(),
            "hl.dsp.focus({ workspace = 1 })"
        );
        assert_eq!(
            HyprAction::Workspace("special:magic".into()).to_lua_expr(),
            "hl.dsp.focus({ workspace = \"special:magic\" })"
        );
        assert_eq!(
            HyprAction::MoveToWorkspace(2.into()).to_lua_expr(),
            "hl.dsp.window.move({ workspace = 2 })"
        );
        assert_eq!(
            HyprAction::Exec("kitty -e btop".to_string()).to_lua_expr(),
            "hl.dsp.exec_cmd(\"kitty -e btop\")"
        );
        assert_eq!(
            HyprAction::CustomLua("print(\"custom\")".to_string()).to_lua_expr(),
            "print(\"custom\")"
        );
    }
}
