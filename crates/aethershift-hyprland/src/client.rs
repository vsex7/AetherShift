use std::env;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::{debug, error, warn};

use crate::action::{HyprAction, escape_lua_string};
use crate::bind::{HyprBind, HyprKeyBinding};
use crate::error::HyprlandError;
use crate::models::{HyprMonitor, HyprWindow};

/// Client for communicating with the Hyprland IPC Unix domain socket
#[derive(Debug, Clone)]
pub struct HyprlandClient {
    socket_path: PathBuf,
}

impl HyprlandClient {
    /// Create a new HyprlandClient with an explicit socket path
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
        }
    }

    /// Automatically discover the Hyprland socket path using
    /// `HYPRLAND_INSTANCE_SIGNATURE` and `XDG_RUNTIME_DIR`.
    ///
    /// Resolves to `$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`.
    pub fn discover() -> Result<Self, HyprlandError> {
        let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .map_err(|_| HyprlandError::MissingInstanceSignature)?;

        let runtime_dir =
            env::var("XDG_RUNTIME_DIR").map_err(|_| HyprlandError::MissingRuntimeDir)?;

        let socket_path = Path::new(&runtime_dir)
            .join("hypr")
            .join(signature)
            .join(".socket.sock");

        if !socket_path.exists() {
            return Err(HyprlandError::SocketNotFound { path: socket_path });
        }

        Ok(Self::new(socket_path))
    }

    /// Return the underlying socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Create a client for Hyprland's event socket at an explicit path.
    pub fn new_event_client(socket_path: impl Into<PathBuf>) -> Self {
        Self::new(socket_path)
    }

    /// Discover the event socket independently of an existing control client.
    pub fn discover_event_socket() -> Result<Self, HyprlandError> {
        let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .map_err(|_| HyprlandError::MissingInstanceSignature)?;
        let runtime_dir =
            env::var("XDG_RUNTIME_DIR").map_err(|_| HyprlandError::MissingRuntimeDir)?;
        let path = Path::new(&runtime_dir)
            .join("hypr")
            .join(signature)
            .join(".socket2.sock");
        if !path.exists() {
            return Err(HyprlandError::SocketNotFound { path });
        }
        Ok(Self::new(path))
    }

    /// Get socket2 path for Hyprland events
    pub fn socket2_path(&self) -> PathBuf {
        let parent = self.socket_path.parent().unwrap_or_else(|| Path::new("."));
        parent.join(".socket2.sock")
    }

    /// Connect to socket2 for listening to Hyprland real-time events
    pub async fn connect_events(&self) -> Result<UnixStream, HyprlandError> {
        let path = self.socket2_path();
        UnixStream::connect(&path)
            .await
            .map_err(|source| HyprlandError::Io { path, source })
    }

    /// Read the next parsed Hyprland event. `Ok(None)` means the event socket closed.
    pub async fn read_event(stream: &mut UnixStream) -> Result<Option<HyprEvent>, HyprlandError> {
        let mut line = String::new();
        let mut reader = BufReader::new(&mut *stream);
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|source| HyprlandError::Io {
                path: PathBuf::from(".socket2.sock"),
                source,
            })?;
        if bytes == 0 {
            return Ok(None);
        }
        Ok(HyprEvent::parse(&line))
    }

    /// Send a raw command to the Hyprland socket and read the complete response
    pub async fn send_command(&self, command: &str) -> Result<String, HyprlandError> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|source| HyprlandError::Io {
                path: self.socket_path.clone(),
                source,
            })?;

        stream
            .write_all(command.as_bytes())
            .await
            .map_err(|source| HyprlandError::Io {
                path: self.socket_path.clone(),
                source,
            })?;

        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|source| HyprlandError::Io {
                path: self.socket_path.clone(),
                source,
            })?;

        let res_str = String::from_utf8_lossy(&response).to_string();
        Ok(res_str)
    }

    /// Execute a Lua expression/statement via `eval <lua_code>`
    pub async fn eval_lua(&self, lua_code: &str) -> Result<String, HyprlandError> {
        let cmd = format!("eval {}", lua_code);
        let res = self.send_command(&cmd).await?;

        if let Some(err_msg) = res.strip_prefix("error: ") {
            return Err(HyprlandError::EvalError(err_msg.trim().to_string()));
        }

        Ok(res)
    }

    /// Fetch all current keybinds from Hyprland via `j/binds`
    pub async fn fetch_binds(&self) -> Result<Vec<HyprBind>, HyprlandError> {
        let json_data = self.send_command("j/binds").await?;
        let binds: Vec<HyprBind> = serde_json::from_str(&json_data)?;
        Ok(binds)
    }

    /// Fetch active and configured monitors via `j/monitors`
    pub async fn fetch_monitors(&self) -> Result<Vec<HyprMonitor>, HyprlandError> {
        let json_data = self.send_command("j/monitors").await?;
        let monitors: Vec<HyprMonitor> = serde_json::from_str(&json_data)?;
        Ok(monitors)
    }

    /// Fetch currently active window via `j/activewindow`
    pub async fn fetch_active_window(&self) -> Result<Option<HyprWindow>, HyprlandError> {
        let json_data = self.send_command("j/activewindow").await?;
        let trimmed = json_data.trim();
        if trimmed.is_empty() || trimmed == "{}" {
            return Ok(None);
        }

        let win: HyprWindow = serde_json::from_str(trimmed)?;
        if win.address.is_empty() {
            Ok(None)
        } else {
            Ok(Some(win))
        }
    }

    /// Fetch all client windows via `j/clients`
    pub async fn fetch_clients(&self) -> Result<Vec<HyprWindow>, HyprlandError> {
        let json_data = self.send_command("j/clients").await?;
        let windows: Vec<HyprWindow> = serde_json::from_str(&json_data)?;
        Ok(windows)
    }

    /// Apply an exact window rectangle (x, y, w, h) to target window.
    /// Sets floating mode and uses Lua dispatchers to move and resize with pixel precision.
    pub async fn apply_window_rect(
        &self,
        address: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        set_floating: bool,
    ) -> Result<(), HyprlandError> {
        let addr_quoted = escape_lua_string(address);

        if set_floating {
            let float_lua = format!(
                "hl.dispatch(hl.dsp.window.float({{ action = \"set\", address = {} }}))",
                addr_quoted
            );
            let _ = self.eval_lua(&float_lua).await?;
        }

        // Resize first then move to avoid screen edge clipping
        let resize_lua = format!(
            "hl.dispatch(hl.dsp.window.resize({{ x = {}, y = {}, mode = \"exact\", address = {} }}))",
            width, height, addr_quoted
        );
        let _ = self.eval_lua(&resize_lua).await?;

        let move_lua = format!(
            "hl.dispatch(hl.dsp.window.move({{ x = {}, y = {}, mode = \"exact\", address = {} }}))",
            x, y, addr_quoted
        );
        let _ = self.eval_lua(&move_lua).await?;

        Ok(())
    }

    /// Move window across monitors
    pub async fn move_window_to_monitor(
        &self,
        address: Option<&str>,
        direction: &str,
    ) -> Result<(), HyprlandError> {
        let lua = match address {
            Some(addr) => format!(
                "hl.dispatch(hl.dsp.window.move({{ monitor = {}, address = {} }}))",
                escape_lua_string(direction),
                escape_lua_string(addr)
            ),
            None => format!(
                "hl.dispatch(hl.dsp.window.move({{ monitor = {} }}))",
                escape_lua_string(direction)
            ),
        };
        self.eval_lua(&lua).await?;
        Ok(())
    }

    /// Bind a key combination to an action:
    /// `eval hl.bind("<keys>", <dispatcher>, { desc = "<desc>" })`
    pub async fn bind_key(&self, keys: &str, action: &HyprAction) -> Result<(), HyprlandError> {
        self.bind_key_with_description(keys, action, None).await
    }

    /// Bind a key combination to an action with an optional description
    pub async fn bind_key_with_description(
        &self,
        keys: &str,
        action: &HyprAction,
        description: Option<&str>,
    ) -> Result<(), HyprlandError> {
        let lua = Self::format_bind_lua(keys, action, description);
        debug!("Binding key: {}", lua);
        self.eval_lua(&lua).await?;
        Ok(())
    }

    /// Unbind a key combination:
    /// `eval hl.unbind("<keys>")`
    pub async fn unbind_key(&self, keys: &str) -> Result<(), HyprlandError> {
        let lua = Self::format_unbind_lua(keys);
        debug!("Unbinding key: {}", lua);
        self.eval_lua(&lua).await?;
        Ok(())
    }

    /// Helper to format Lua code for a single `hl.bind` call
    pub fn format_bind_lua(keys: &str, action: &HyprAction, description: Option<&str>) -> String {
        let escaped_keys = escape_lua_string(keys);
        let action_expr = action.to_lua_expr();
        let opts = match description {
            Some(desc) => format!("{{ desc = {} }}", escape_lua_string(desc)),
            None => "{}".to_string(),
        };
        format!("hl.bind({}, {}, {})", escaped_keys, action_expr, opts)
    }

    /// Helper to format Lua code for a single `hl.unbind` call
    pub fn format_unbind_lua(keys: &str) -> String {
        let escaped_keys = escape_lua_string(keys);
        format!("hl.unbind({})", escaped_keys)
    }

    /// Apply a batch of unbinds and binds safely.
    pub async fn apply_batch(
        &self,
        unbinds: &[String],
        binds: &[HyprKeyBinding],
    ) -> Result<(), HyprlandError> {
        if let Err(batch_error) = self.apply_batch_atomic(unbinds, binds).await {
            warn!("Atomic Hyprland batch failed, falling back to sequential mode: {batch_error}");
            self.apply_batch_sequential(unbinds, binds).await?;
        }
        Ok(())
    }

    /// Apply all operations in one Hyprland batch command.
    pub async fn apply_batch_atomic(
        &self,
        unbinds: &[String],
        binds: &[HyprKeyBinding],
    ) -> Result<(), HyprlandError> {
        if unbinds.is_empty() && binds.is_empty() {
            return Ok(());
        }

        let mut operations = Vec::with_capacity(unbinds.len() + binds.len());
        for key in unbinds {
            operations.push(format!("eval {}", Self::format_unbind_lua(key)));
        }
        for binding in binds {
            operations.push(format!(
                "eval {}",
                Self::format_bind_lua(
                    &binding.keys,
                    &binding.action,
                    binding.description.as_deref()
                )
            ));
        }

        let command = format!("[[BATCH]]{}", operations.join("; "));
        debug!("Applying atomic Hyprland batch: {command}");
        let response = self.send_command(&command).await?;
        if let Some(err) = response.strip_prefix("error:") {
            return Err(HyprlandError::BatchError(err.trim().to_string()));
        }
        Ok(())
    }

    /// Sequential fallback retained for older Hyprland builds or detailed error reporting.
    pub async fn apply_batch_sequential(
        &self,
        unbinds: &[String],
        binds: &[HyprKeyBinding],
    ) -> Result<(), HyprlandError> {
        let mut errors = Vec::new();

        for key in unbinds {
            if let Err(e) = self.unbind_key(key).await {
                warn!("Failed to unbind '{}': {}", key, e);
                errors.push(format!("unbind '{}': {}", key, e));
            }
        }

        for binding in binds {
            if let Err(e) = self
                .bind_key_with_description(
                    &binding.keys,
                    &binding.action,
                    binding.description.as_deref(),
                )
                .await
            {
                error!("Failed to bind '{}': {}", binding.keys, e);
                errors.push(format!("bind '{}': {}", binding.keys, e));
            }
        }

        if !errors.is_empty() {
            return Err(HyprlandError::BatchError(errors.join("; ")));
        }

        Ok(())
    }

    /// Apply a tiling/floating policy to one newly opened window.
    pub async fn set_window_state(
        &self,
        address: &str,
        state: HyprWindowState,
    ) -> Result<(), HyprlandError> {
        let dispatcher = match state {
            HyprWindowState::Tiled => "tile",
            HyprWindowState::Floating => "float",
        };
        let address = if address.starts_with("0x") {
            address.to_string()
        } else {
            format!("0x{address}")
        };
        self.send_command(&format!("dispatch {dispatcher} address:{address}"))
            .await?
            .strip_prefix("error:")
            .map(|err| HyprlandError::EvalError(err.trim().to_string()))
            .map_or(Ok(()), Err)
    }
}

/// Window layout state used by the Phase 2 new-window policy controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyprWindowState {
    Tiled,
    Floating,
}

/// Parsed events currently consumed by AetherShift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyprEvent {
    WindowOpened {
        address: String,
        class: String,
        title: String,
    },
    Unknown(String),
}

impl HyprEvent {
    pub fn parse(line: &str) -> Option<Self> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            return None;
        }

        let Some((event, payload)) = line.split_once(">>") else {
            return Some(Self::Unknown(line.to_string()));
        };

        match event {
            "openwindow" => {
                let mut fields = payload.splitn(4, ',');
                let address = fields.next().unwrap_or_default().to_string();
                let _workspace = fields.next();
                let class = fields.next().unwrap_or_default().to_string();
                let title = fields.next().unwrap_or_default().to_string();
                Some(Self::WindowOpened {
                    address,
                    class,
                    title,
                })
            }
            _ => Some(Self::Unknown(line.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Direction;
    use tokio::net::UnixListener;

    #[test]
    fn test_format_bind_and_unbind_lua() {
        let bind_no_desc =
            HyprlandClient::format_bind_lua("SUPER + Q", &HyprAction::CloseWindow, None);
        assert_eq!(
            bind_no_desc,
            "hl.bind(\"SUPER + Q\", hl.dsp.window.close(), {})"
        );

        let bind_with_desc = HyprlandClient::format_bind_lua(
            "SUPER + LEFT",
            &HyprAction::Focus(Direction::Left),
            Some("Focus left"),
        );
        assert_eq!(
            bind_with_desc,
            "hl.bind(\"SUPER + LEFT\", hl.dsp.focus({ direction = \"l\" }), { desc = \"Focus left\" })"
        );

        let unbind = HyprlandClient::format_unbind_lua("SUPER + Q");
        assert_eq!(unbind, "hl.unbind(\"SUPER + Q\")");
    }

    #[tokio::test]
    async fn test_mock_socket_communication() {
        let sock_path = std::env::temp_dir().join(format!(
            "hypr_test_{}_{}.sock",
            std::process::id(),
            fastrand()
        ));
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).expect("bind listener");

        // Spawn mock server
        let server_handle = tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = vec![0u8; 2048];
                let n = stream.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);

                if req == "j/binds" {
                    let json = r#"[{"modmask": 64, "key": "Q", "description": "Quit"}]"#;
                    stream.write_all(json.as_bytes()).await.unwrap();
                } else if req == "j/monitors" {
                    let json = r#"[{
                        "id": 0, "name": "HDMI-A-1", "width": 2560, "height": 1440,
                        "x": 0, "y": 0, "scale": 1.0, "focused": true, "reserved": [0, 26, 0, 0]
                    }]"#;
                    stream.write_all(json.as_bytes()).await.unwrap();
                } else if req == "j/activewindow" {
                    let json = r#"{
                        "address": "0x1234", "at": [10, 36], "size": [1000, 800],
                        "workspace": {"id": 1, "name": "1"}, "floating": false,
                        "monitor": 0, "class": "test", "title": "Test Window", "pid": 999
                    }"#;
                    stream.write_all(json.as_bytes()).await.unwrap();
                } else if req.starts_with("eval hl.bind(\"FAIL\"") {
                    let err = "error: failed to parse key string";
                    stream.write_all(err.as_bytes()).await.unwrap();
                } else if req.starts_with("eval ") {
                    stream.write_all(b"ok").await.unwrap();
                } else {
                    stream.write_all(b"unknown").await.unwrap();
                }
            }
        });

        let client = HyprlandClient::new(&sock_path);

        // Test fetch_binds
        let binds = client.fetch_binds().await.expect("fetch_binds");
        assert_eq!(binds.len(), 1);
        assert_eq!(binds[0].key, "Q");

        // Test fetch_monitors
        let monitors = client.fetch_monitors().await.expect("fetch_monitors");
        assert_eq!(monitors.len(), 1);
        assert_eq!(monitors[0].name, "HDMI-A-1");
        assert_eq!(monitors[0].work_area(), (0, 26, 2560, 1414));

        // Test fetch_active_window
        let active = client
            .fetch_active_window()
            .await
            .expect("fetch_active_window");
        assert!(active.is_some());
        assert_eq!(active.unwrap().address, "0x1234");

        // Test apply_window_rect
        client
            .apply_window_rect("0x1234", 0, 26, 1280, 1414, true)
            .await
            .expect("apply_window_rect");

        // Test bind_key & unbind_key
        client
            .bind_key("SUPER + W", &HyprAction::CloseWindow)
            .await
            .expect("bind_key");
        client.unbind_key("SUPER + W").await.expect("unbind_key");

        server_handle.abort();
        let _ = std::fs::remove_file(&sock_path);
    }

    fn fastrand() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(100);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    #[test]
    fn test_parse_openwindow_event() {
        let event = HyprEvent::parse("openwindow>>0x55f0,1,foot,foot\n").unwrap();
        assert_eq!(
            event,
            HyprEvent::WindowOpened {
                address: "0x55f0".to_string(),
                class: "foot".to_string(),
                title: "foot".to_string(),
            }
        );
        assert_eq!(
            HyprEvent::parse("unknown>>x"),
            Some(HyprEvent::Unknown("unknown>>x".to_string()))
        );
        assert_eq!(HyprEvent::parse("\n"), None);
    }

    #[tokio::test]
    async fn test_atomic_batch_single_command() {
        let sock_path = std::env::temp_dir().join(format!("hypr_batch_{}.sock", fastrand()));
        let _ = std::fs::remove_file(&sock_path);
        let listener = UnixListener::bind(&sock_path).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            assert!(request.starts_with("[[BATCH]]"));
            assert_eq!(request.matches("eval ").count(), 2);
            stream.write_all(b"ok").await.unwrap();
            request
        });

        let client = HyprlandClient::new(&sock_path);
        client
            .apply_batch_atomic(
                &["SUPER + Q".to_string()],
                &[HyprKeyBinding::new("SUPER + W", HyprAction::CloseWindow)],
            )
            .await
            .unwrap();
        let request = server.await.unwrap();
        assert!(request.contains("hl.unbind(\"SUPER + Q\")"));
        assert!(request.contains("hl.bind(\"SUPER + W\""));
        let _ = std::fs::remove_file(&sock_path);
    }
}
