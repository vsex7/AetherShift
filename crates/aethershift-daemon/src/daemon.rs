use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{Mutex, broadcast};
use tracing::{debug, info, warn};

use aethershift_core::binding::{Action, KeyCombo};
use aethershift_core::layout::{GeometryMemory, LayoutEngine, Rect};
use aethershift_core::plugin::{PluginActionInput, PluginRegistry, execute_plugin};
use aethershift_core::state::{BaselineBinding, StateManager, SwitchPlan};
use aethershift_core::stats::StatsEngine;
use aethershift_hyprland::action::{Direction, HyprAction, WorkspaceTarget};
use aethershift_hyprland::bind::HyprKeyBinding;
use aethershift_hyprland::client::HyprlandClient;
use aethershift_protocol::{
    BindingInfo, ClientStream, Event, HistoryEntry, LayoutFeedback, MetricsFormat,
    PluginInfo as WirePluginInfo, PluginPermissionScope, Request, Response, ServerStream,
    SnapLayout, StatusInfo,
};

use crate::config::DaemonConfig;
use crate::error::DaemonError;
use crate::health::{DoctorInput, HealthState, doctor_report, metrics_text, plugin_dir};
use crate::notify::send_notification;

fn get_user_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config")
    } else {
        PathBuf::from("/tmp/.config")
    }
}

fn event_kind(event: &Event) -> &'static str {
    match event {
        Event::SwitchSucceeded { .. } => "switch-succeeded",
        Event::SwitchFailed { .. } => "switch-failed",
        Event::Restored { .. } => "restored",
        Event::WindowPolicyChanged { .. } => "window-policy-changed",
        Event::PresetsReloaded { .. } => "presets-reloaded",
    }
}

fn event_matches_filters(response: &Response, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }
    let Response::Event { event } = response else {
        return false;
    };
    filters.iter().any(|filter| filter == event_kind(event))
}

fn broadcast_event(events_tx: &broadcast::Sender<Response>, event: Event) {
    let _ = events_tx.send(Response::Event { event });
}

pub struct AetherDaemon {
    config: DaemonConfig,
    state: Arc<Mutex<StateManager>>,
    geometry_memory: Arc<Mutex<GeometryMemory>>,
    stats: Arc<Mutex<StatsEngine>>,
    plugins: Arc<Mutex<PluginRegistry>>,
    health: Arc<Mutex<HealthState>>,
    window_policy: Arc<Mutex<aethershift_protocol::WindowPolicy>>,
    hyprland: Option<HyprlandClient>,
    start_time: Instant,
    shutdown_tx: broadcast::Sender<()>,
    events_tx: broadcast::Sender<Response>,
}

fn plugin_to_wire(plugin: &aethershift_core::plugin::LoadedPlugin) -> WirePluginInfo {
    let permissions = plugin
        .manifest
        .permissions
        .iter()
        .map(|permission| match permission {
            aethershift_core::plugin::PluginPermissionScope::None => PluginPermissionScope::None,
            aethershift_core::plugin::PluginPermissionScope::HyprlandDispatch => {
                PluginPermissionScope::HyprlandDispatch
            }
            aethershift_core::plugin::PluginPermissionScope::Shell => PluginPermissionScope::Shell,
            aethershift_core::plugin::PluginPermissionScope::Clipboard => {
                PluginPermissionScope::Clipboard
            }
            aethershift_core::plugin::PluginPermissionScope::Notification => {
                PluginPermissionScope::Notification
            }
        })
        .collect();
    WirePluginInfo {
        id: plugin.manifest.id.clone(),
        version: plugin.manifest.version.clone(),
        description: plugin.manifest.description.clone(),
        executable: plugin.manifest.executable.clone(),
        timeout_ms: plugin.manifest.timeout_ms,
        permissions,
        enabled: plugin.manifest.enabled,
        error: None,
    }
}

impl AetherDaemon {
    pub fn new(config: DaemonConfig) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        let (events_tx, _) = broadcast::channel(64);
        Self {
            config,
            state: Arc::new(Mutex::new(StateManager::new())),
            geometry_memory: Arc::new(Mutex::new(GeometryMemory::new())),
            stats: Arc::new(Mutex::new(StatsEngine::new())),
            plugins: Arc::new(Mutex::new(PluginRegistry::default())),
            health: Arc::new(Mutex::new(HealthState {
                backend: "hyprland".to_string(),
                ..HealthState::default()
            })),
            window_policy: Arc::new(Mutex::new(aethershift_protocol::WindowPolicy::Omarchy)),
            hyprland: None,
            start_time: Instant::now(),
            shutdown_tx,
            events_tx,
        }
    }

    /// Construct with an explicit mockable or pre-configured HyprlandClient
    pub fn with_hyprland(mut self, client: Option<HyprlandClient>) -> Self {
        self.hyprland = client;
        self
    }

    /// Single-instance check and socket acquisition.
    pub async fn bind_socket(socket_path: &Path) -> Result<UnixListener, DaemonError> {
        if socket_path.exists() {
            debug!(
                "Checking if existing socket at {} is active...",
                socket_path.display()
            );
            match UnixStream::connect(socket_path).await {
                Ok(stream) => {
                    let mut client = ClientStream::new(stream);
                    if client.send(&Request::Status).await.is_ok()
                        && tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            client.recv(),
                        )
                        .await
                        .is_ok()
                    {
                        return Err(DaemonError::AlreadyRunning(socket_path.to_path_buf()));
                    }
                    warn!(
                        "Removing inactive/stale socket file at {}",
                        socket_path.display()
                    );
                    let _ = std::fs::remove_file(socket_path);
                }
                Err(_) => {
                    debug!("Socket exists but connection refused. Cleaning stale socket...");
                    let _ = std::fs::remove_file(socket_path);
                }
            }
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| DaemonError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let listener = UnixListener::bind(socket_path).map_err(|source| DaemonError::Io {
            path: socket_path.to_path_buf(),
            source,
        })?;
        Ok(listener)
    }

    /// Main daemon lifecycle runner
    pub async fn run(mut self) -> Result<(), DaemonError> {
        let socket_path = self.config.resolved_socket_path();
        let listener = Self::bind_socket(&socket_path).await?;
        info!("AetherShift daemon listening on {}", socket_path.display());

        // Connect to Hyprland
        if self.hyprland.is_none() {
            match HyprlandClient::discover() {
                Ok(client) => {
                    info!(
                        "Connected to Hyprland socket at: {}",
                        client.socket_path().display()
                    );
                    self.hyprland = Some(client);
                }
                Err(e) => {
                    warn!(
                        "Hyprland discovery warning: {}. Running in standalone/mock mode.",
                        e
                    );
                }
            }
        }

        // Initialize baseline
        if let Some(ref client) = self.hyprland {
            match client.fetch_binds().await {
                Ok(raw_binds) => {
                    let count = raw_binds.len();
                    let baseline_items: Vec<BaselineBinding> = raw_binds
                        .into_iter()
                        .filter_map(|b| {
                            let raw_key = b.formatted_combo();
                            let combo: KeyCombo = raw_key?.parse().ok()?;
                            Some(BaselineBinding {
                                key_combo: combo,
                                dispatcher: b.dispatcher,
                                args: b.arg,
                                description: Some(b.description),
                            })
                        })
                        .collect();
                    let mut st = self.state.lock().await;
                    st.set_baseline(baseline_items);
                    self.health.lock().await.backend_connected = true;
                    self.health.lock().await.baseline_count = count;
                    info!("Fetched {} baseline binds from Hyprland", count);
                }
                Err(e) => {
                    warn!("Could not fetch baseline binds from Hyprland: {}", e);
                    self.health.lock().await.backend_connected = false;
                }
            }
        }

        // Load custom presets
        if self.config.preset_dir.exists() {
            let mut st = self.state.lock().await;
            match st.load_profiles_from_dir(&self.config.preset_dir) {
                Ok(n) => info!(
                    "Loaded {} custom presets from {}",
                    n,
                    self.config.preset_dir.display()
                ),
                Err(e) => warn!(
                    "Failed loading presets from {}: {}",
                    self.config.preset_dir.display(),
                    e
                ),
            }
        }

        // Also check XDG user presets dir: ~/.config/aethershift/presets
        let user_presets = get_user_config_dir().join("aethershift").join("presets");
        if user_presets.exists() {
            let mut st = self.state.lock().await;
            if let Ok(n) = st.load_profiles_from_dir(&user_presets) {
                info!("Loaded {} user presets from {}", n, user_presets.display());
            }
        }

        let plugins_dir = plugin_dir();
        {
            let mut plugins = self.plugins.lock().await;
            match plugins.load_dir(&plugins_dir) {
                Ok(count) => {
                    let failures = plugins.failed_count();
                    let mut health = self.health.lock().await;
                    health.plugins_loaded = count;
                    health.plugins_failed = failures;
                    info!(
                        "Loaded {} action plugins from {}",
                        count,
                        plugins_dir.display()
                    );
                }
                Err(error) => warn!(
                    "Failed loading plugin directory {}: {}",
                    plugins_dir.display(),
                    error
                ),
            }
        }

        // Background event listener for Phase D (WindowMode)
        if let Some(ref client) = self.hyprland {
            let event_client = client.clone();
            let event_window_policy = self.window_policy.clone();
            let event_state = self.state.clone();
            let mut event_shutdown_rx = self.shutdown_tx.subscribe();

            tokio::spawn(async move {
                debug!("Starting Hyprland event listener task...");
                match event_client.connect_events().await {
                    Ok(stream) => {
                        use tokio::io::{AsyncBufReadExt, BufReader};
                        let (reader, _) = stream.into_split();
                        let mut lines = BufReader::new(reader).lines();

                        loop {
                            tokio::select! {
                                line_res = lines.next_line() => {
                                    match line_res {
                                        Ok(Some(line)) => {
                                            if let Some(event) = aethershift_hyprland::HyprEvent::parse(&line) {
                                                if let aethershift_hyprland::HyprEvent::WindowOpened { address, .. } = event {
                                                    let policy = *event_window_policy.lock().await;
                                                    match policy {
                                                        aethershift_protocol::WindowPolicy::Tiled => {
                                                            let cmd = format!(
                                                                "hl.dispatch(hl.dsp.window.float({{ action = \"disable\", address = \"{}\" }}))",
                                                                address
                                                            );
                                                            if let Err(e) = event_client.eval_lua(&cmd).await {
                                                                debug!("Failed to unset float for new window {}: {}", address, e);
                                                            }
                                                        }
                                                        aethershift_protocol::WindowPolicy::Floating => {
                                                            let cmd = format!(
                                                                "hl.dispatch(hl.dsp.window.float({{ action = \"enable\", address = \"{}\" }}))",
                                                                address
                                                            );
                                                            if let Err(e) = event_client.eval_lua(&cmd).await {
                                                                debug!("Failed to set float for new window {}: {}", address, e);
                                                            }
                                                        }
                                                        aethershift_protocol::WindowPolicy::FollowProfile => {
                                                            let is_macos = {
                                                                let st = event_state.lock().await;
                                                                st.active_profile_name() == "macos"
                                                            };
                                                            let action = if is_macos { "enable" } else { "disable" };
                                                            let cmd = format!(
                                                                "hl.dispatch(hl.dsp.window.float({{ action = \"{}\", address = \"{}\" }}))",
                                                                action, address
                                                            );
                                                            if let Err(e) = event_client.eval_lua(&cmd).await {
                                                                debug!("Failed to set/unset float (FollowProfile) for new window {}: {}", address, e);
                                                            }
                                                        }
                                                        aethershift_protocol::WindowPolicy::Omarchy => {
                                                            // No operation; respect default compositor behavior
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            debug!("Hyprland event stream closed by server");
                                            break;
                                        }
                                        Err(e) => {
                                            warn!("Error reading Hyprland event socket2: {}", e);
                                            break;
                                        }
                                    }
                                }
                                _ = event_shutdown_rx.recv() => {
                                    debug!("Hyprland event listener received shutdown signal");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Could not connect to Hyprland event socket: {}. Event listening disabled.",
                            e
                        );
                    }
                }
            });
        }

        let mut shutdown_rx = self.shutdown_tx.subscribe();
        let state = self.state.clone();
        let geometry_memory = self.geometry_memory.clone();
        let stats = self.stats.clone();
        let plugins = self.plugins.clone();
        let health = self.health.clone();
        let window_policy = self.window_policy.clone();
        let hyprland = self.hyprland.clone();
        let start_time = self.start_time;
        let shutdown_tx = self.shutdown_tx.clone();
        let events_tx = self.events_tx.clone();
        let config = self.config.clone();

        loop {
            tokio::select! {
                res = listener.accept() => {
                    match res {
                        Ok((stream, _)) => {
                            let state = state.clone();
                            let geometry_memory = geometry_memory.clone();
                            let stats = stats.clone();
                            let plugins = plugins.clone();
                            let health = health.clone();
                            let window_policy = window_policy.clone();
                            let hyprland = hyprland.clone();
                            let shutdown_tx = shutdown_tx.clone();
                            let events_tx = events_tx.clone();
                            let config = config.clone();

                            tokio::spawn(async move {
                                health.lock().await.record_connection_open();
                                let mut server = ServerStream::new(stream);

                                while let Ok(req) = server.recv().await {
                                    if let Request::Subscribe { filters } = req {
                                        let response = Response::ok("Event subscription established");
                                        if server.send(&response).await.is_err() {
                                            break;
                                        }
                                        let mut events = events_tx.subscribe();
                                        while let Ok(event) = events.recv().await {
                                            if event_matches_filters(&event, &filters)
                                                && server.send(&event).await.is_err()
                                            {
                                                break;
                                            }
                                        }
                                        break;
                                    }

                                    let resp = Self::handle_request(
                                        req,
                                        &state,
                                        &geometry_memory,
                                        &stats,
                                        &plugins,
                                        &health,
                                        &window_policy,
                                        &hyprland,
                                        start_time,
                                        &shutdown_tx,
                                        &events_tx,
                                        &config,
                                    ).await;
                                    health.lock().await.record_connection_close();
                                    if server.send(&resp).await.is_err() {
                                        break;
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            warn!("Accept error: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Received internal shutdown trigger");
                    break;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Received SIGINT/Ctrl+C signal");
                    break;
                }
            }
        }

        Self::cleanup(&socket_path, &self.state, &self.hyprland).await;
        Ok(())
    }

    /// Convert Core Action to HyprAction
    pub fn core_action_to_hypr(action: &Action) -> HyprAction {
        match action {
            Action::CloseWindow => HyprAction::CloseWindow,
            Action::SnapLeft => HyprAction::Focus(Direction::Left),
            Action::SnapRight => HyprAction::Focus(Direction::Right),
            Action::Maximize => HyprAction::ToggleFullscreen(Some(1)),
            Action::Restore => HyprAction::ToggleFullscreen(Some(0)),
            Action::FloatToggle => HyprAction::ToggleFloating,
            Action::Fullscreen => HyprAction::ToggleFullscreen(None),
            Action::WorkspaceNext => HyprAction::Workspace(WorkspaceTarget::Relative(1)),
            Action::WorkspacePrev => HyprAction::Workspace(WorkspaceTarget::Relative(-1)),
            Action::ToggleSpecialWorkspace(name) => {
                let s = name.as_deref().unwrap_or("scratchpad");
                HyprAction::CustomLua(format!("hl.dsp.workspace.toggle_special(\"{}\")", s))
            }
            Action::ToggleLauncher(cmd) => {
                let cmd_str = cmd
                    .as_deref()
                    .unwrap_or("omarchy-launch-walker || rofi -show drun || wofi --show drun");
                HyprAction::Exec(cmd_str.to_string())
            }
            Action::Exec(cmd) => HyprAction::Exec(cmd.clone()),
            Action::Plugin(id) => HyprAction::Exec(format!("aethershift plugin run {}", id)),
            Action::Custom { dispatcher, args } => {
                HyprAction::CustomLua(format!("hl.dsp.{}(\"{}\")", dispatcher, args))
            }
            Action::CycleWindowNext => {
                HyprAction::CustomLua("hl.dsp.window.cycle_next()".to_string())
            }
            Action::CycleWindowPrev => {
                HyprAction::CustomLua("hl.dsp.window.cycle_next({ prev = true })".to_string())
            }
            Action::MoveToWorkspaceNext => {
                HyprAction::MoveToWorkspace(WorkspaceTarget::Relative(1))
            }
            Action::MoveToWorkspacePrev => {
                HyprAction::MoveToWorkspace(WorkspaceTarget::Relative(-1))
            }
        }
    }

    /// Convert KeyCombo and Action to HyprKeyBinding
    pub fn plan_binding_to_hypr(
        combo: &KeyCombo,
        action: &Action,
        desc: Option<&str>,
    ) -> HyprKeyBinding {
        let keys_str = combo.canonical_str();
        let hypr_action = Self::core_action_to_hypr(action);
        let mut binding = HyprKeyBinding::new(keys_str, hypr_action);
        if let Some(d) = desc {
            binding = binding.with_description(d);
        }
        binding
    }

    /// Execute a SwitchPlan against Hyprland and update state
    pub async fn execute_plan(
        plan: &SwitchPlan,
        state: &Mutex<StateManager>,
        hyprland: &Option<HyprlandClient>,
    ) -> Result<(), DaemonError> {
        if let Some(client) = hyprland {
            let unbinds: Vec<String> = plan
                .unbind
                .iter()
                .map(|combo| combo.canonical_str())
                .collect();

            let binds: Vec<HyprKeyBinding> = plan
                .bind
                .iter()
                .map(|b| {
                    Self::plan_binding_to_hypr(&b.key_combo, &b.action, b.description.as_deref())
                })
                .collect();

            client
                .apply_batch(&unbinds, &binds)
                .await
                .map_err(|e| DaemonError::Hyprland(e))?;
        }

        let mut st = state.lock().await;
        st.commit_switch(plan).map_err(|e| DaemonError::Core(e))?;

        Ok(())
    }

    /// Handle Cycle request
    pub async fn handle_cycle(
        state: &Mutex<StateManager>,
        stats: &Mutex<StatsEngine>,
        _plugins: &Mutex<PluginRegistry>,
        health: &Mutex<HealthState>,
        hyprland: &Option<HyprlandClient>,
    ) -> Result<String, DaemonError> {
        let target_name = {
            let st = state.lock().await;
            let profiles = st.list_profiles();
            if profiles.is_empty() {
                return Ok("No profiles available to cycle".to_string());
            }
            let active = st.active_profile_name();
            let current_idx = profiles.iter().position(|p| p.name == active).unwrap_or(0);
            let next_idx = (current_idx + 1) % profiles.len();
            profiles[next_idx].name.clone()
        };

        let plan = {
            let st = state.lock().await;
            st.switch_profile(&target_name)?
        };

        let started = Instant::now();
        Self::execute_plan(&plan, state, hyprland).await?;
        stats.lock().await.record_switch(&target_name);
        let report = {
            let st = state.lock().await;
            st.last_conflict_report().clone()
        };
        health.lock().await.record_switch(
            &target_name,
            started.elapsed().as_micros(),
            report.skipped,
            report.forced,
        );
        Ok(format!("Cycled to profile '{}'", target_name))
    }

    /// Handle a single incoming client request
    pub async fn handle_request(
        req: Request,
        state: &Mutex<StateManager>,
        geometry_memory: &Mutex<GeometryMemory>,
        stats: &Mutex<StatsEngine>,
        plugins: &Mutex<PluginRegistry>,
        health: &Mutex<HealthState>,
        window_policy: &Mutex<aethershift_protocol::WindowPolicy>,
        hyprland: &Option<HyprlandClient>,
        start_time: Instant,
        shutdown_tx: &broadcast::Sender<()>,
        events_tx: &broadcast::Sender<Response>,
        config: &DaemonConfig,
    ) -> Response {
        let disabled = config.notifications_disabled();

        match req {
            Request::Status => {
                let st = state.lock().await;
                let health_snapshot = health.lock().await.clone();
                let status_info = StatusInfo::new(
                    st.active_profile_name(),
                    st.overlays_count(),
                    start_time.elapsed().as_secs(),
                    env!("CARGO_PKG_VERSION"),
                );
                let current_policy = *window_policy.lock().await;
                let status_info = StatusInfo {
                    window_policy: current_policy,
                    skipped_conflicts: health_snapshot.skipped_conflicts,
                    forced_overrides: health_snapshot.forced_overrides,
                    last_switch_duration_us: health_snapshot.last_switch_duration_us,
                    last_switch_succeeded: health_snapshot.last_switch_succeeded,
                    ..status_info
                };
                match Response::ok_with_data("Status retrieved", &status_info) {
                    Ok(resp) => resp,
                    Err(e) => Response::err("INTERNAL_ERROR", e.to_string()),
                }
            }
            Request::ListProfiles => {
                let st = state.lock().await;
                let profiles = st.list_profiles();
                match Response::ok_with_data("Profiles retrieved", &profiles) {
                    Ok(resp) => resp,
                    Err(e) => Response::err("INTERNAL_ERROR", e.to_string()),
                }
            }
            Request::Switch { profile, force } => {
                let plan = {
                    let st = state.lock().await;
                    if !force && st.active_profile_name() == profile {
                        let msg = format!("Already on profile '{}'", profile);
                        send_notification("AetherShift", &msg, false, disabled);
                        return Response::ok(msg);
                    }
                    match st.switch_profile(&profile) {
                        Ok(p) => p,
                        Err(e) => {
                            let err_msg =
                                format!("Failed to switch to profile '{}': {}", profile, e);
                            send_notification("AetherShift Error", &err_msg, true, disabled);
                            return Response::err("SWITCH_FAILED", e.to_string());
                        }
                    }
                };

                let started = Instant::now();
                match Self::execute_plan(&plan, state, hyprland).await {
                    Ok(()) => {
                        stats.lock().await.record_switch(&profile);
                        let report = {
                            let st = state.lock().await;
                            st.last_conflict_report().clone()
                        };
                        state.lock().await.record_switch_history(
                            &profile,
                            true,
                            started.elapsed().as_micros(),
                            report.applied,
                            report.skipped,
                            report.forced,
                            None,
                        );
                        health.lock().await.record_switch(
                            &profile,
                            started.elapsed().as_micros(),
                            report.skipped,
                            report.forced,
                        );
                        broadcast_event(
                            events_tx,
                            Event::SwitchSucceeded {
                                profile: profile.clone(),
                                duration_us: started.elapsed().as_micros(),
                                applied: report.applied,
                                skipped: report.skipped,
                                forced: report.forced,
                            },
                        );
                        let msg = format!(
                            "Switched to profile '{}': {} applied, {} skipped, {} forced",
                            profile, report.applied, report.skipped, report.forced
                        );
                        send_notification("AetherShift", &msg, false, disabled);
                        Response::ok(msg)
                    }
                    Err(e) => {
                        let err_msg = format!("Error applying profile '{}': {}", profile, e);
                        health.lock().await.record_failure(err_msg.clone());
                        state.lock().await.record_switch_history(
                            &profile,
                            false,
                            started.elapsed().as_micros(),
                            0,
                            0,
                            0,
                            Some(e.to_string()),
                        );
                        broadcast_event(
                            events_tx,
                            Event::SwitchFailed {
                                profile: profile.clone(),
                                error: e.to_string(),
                            },
                        );
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        Response::err("HYPRLAND_ERROR", e.to_string())
                    }
                }
            }
            Request::Cycle => {
                match Self::handle_cycle(state, stats, plugins, health, hyprland).await {
                    Ok(msg) => {
                        send_notification("AetherShift", &msg, false, disabled);
                        Response::ok(msg)
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to cycle profile: {}", e);
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        Response::err("CYCLE_FAILED", e.to_string())
                    }
                }
            }
            Request::Restore => {
                *window_policy.lock().await = aethershift_protocol::WindowPolicy::Omarchy;
                let plan = {
                    let st = state.lock().await;
                    st.restore_plan()
                };

                match Self::execute_plan(&plan, state, hyprland).await {
                    Ok(()) => {
                        let started = Instant::now();
                        stats.lock().await.record_switch("native");
                        health
                            .lock()
                            .await
                            .record_restore(started.elapsed().as_micros());
                        state.lock().await.record_switch_history(
                            "native",
                            true,
                            started.elapsed().as_micros(),
                            0,
                            0,
                            0,
                            None,
                        );
                        broadcast_event(
                            events_tx,
                            Event::Restored {
                                duration_us: started.elapsed().as_micros(),
                                unbound: plan.unbind.len(),
                                rebound: plan.bind.len(),
                            },
                        );
                        let msg = "Restored baseline keybindings";
                        send_notification("AetherShift", msg, false, disabled);
                        Response::ok("Successfully restored baseline keybindings")
                    }
                    Err(e) => {
                        let err_msg = format!("Failed to restore baseline: {}", e);
                        health.lock().await.record_failure(err_msg.clone());
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        Response::err("RESTORE_FAILED", e.to_string())
                    }
                }
            }
            Request::Shutdown => {
                info!("Shutdown requested by client");
                let plan = {
                    let st = state.lock().await;
                    st.restore_plan()
                };
                let _ = Self::execute_plan(&plan, state, hyprland).await;
                let _ = shutdown_tx.send(());
                Response::ok("Daemon is shutting down")
            }

            Request::WindowMode { policy } => {
                let mut wp = window_policy.lock().await;
                if let Some(p) = policy {
                    *wp = p;
                }
                let current_policy = *wp;
                Response::ok_with_data(
                    format!("Window policy is '{}'", current_policy.as_str()),
                    &current_policy,
                )
                .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
            }

            Request::SwitchWithPolicy { profile, policy } => {
                let core_policy = match policy {
                    aethershift_protocol::ConflictPolicy::Strict => {
                        aethershift_core::profile_metadata::ConflictPolicy::Strict
                    }
                    aethershift_protocol::ConflictPolicy::Force => {
                        aethershift_core::profile_metadata::ConflictPolicy::Force
                    }
                    aethershift_protocol::ConflictPolicy::Smart => {
                        aethershift_core::profile_metadata::ConflictPolicy::Smart
                    }
                };
                let plan = {
                    let st = state.lock().await;
                    match st.switch_profile_with_policy(&profile, core_policy) {
                        Ok(resolved) => resolved.plan,
                        Err(e) => {
                            broadcast_event(
                                events_tx,
                                Event::SwitchFailed {
                                    profile: profile.clone(),
                                    error: e.to_string(),
                                },
                            );
                            return Response::err("SWITCH_FAILED", e.to_string());
                        }
                    }
                };
                let started = Instant::now();
                match Self::execute_plan(&plan, state, hyprland).await {
                    Ok(()) => {
                        stats.lock().await.record_switch(&profile);
                        let report = state.lock().await.last_conflict_report().clone();
                        state.lock().await.record_switch_history(
                            &profile,
                            true,
                            started.elapsed().as_micros(),
                            report.applied,
                            report.skipped,
                            report.forced,
                            None,
                        );
                        health.lock().await.record_switch(
                            &profile,
                            started.elapsed().as_micros(),
                            report.skipped,
                            report.forced,
                        );
                        broadcast_event(
                            events_tx,
                            Event::SwitchSucceeded {
                                profile: profile.clone(),
                                duration_us: started.elapsed().as_micros(),
                                applied: report.applied,
                                skipped: report.skipped,
                                forced: report.forced,
                            },
                        );
                        let msg = format!(
                            "Switched to profile '{}': {} applied, {} skipped, {} forced",
                            profile, report.applied, report.skipped, report.forced
                        );
                        send_notification("AetherShift", &msg, false, disabled);
                        Response::ok(msg)
                    }
                    Err(e) => {
                        health.lock().await.record_failure(e.to_string());
                        state.lock().await.record_switch_history(
                            &profile,
                            false,
                            started.elapsed().as_micros(),
                            0,
                            0,
                            0,
                            Some(e.to_string()),
                        );
                        broadcast_event(
                            events_tx,
                            Event::SwitchFailed {
                                profile: profile.clone(),
                                error: e.to_string(),
                            },
                        );
                        Response::err("HYPRLAND_ERROR", e.to_string())
                    }
                }
            }

            Request::History { limit } => {
                let entries: Vec<HistoryEntry> = state.lock().await.switch_history(limit);
                Response::ok_with_data("Switch history retrieved", &entries)
                    .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
            }

            Request::ReloadPresets => {
                let mut st = state.lock().await;
                match st.load_profiles_from_dir(&config.preset_dir) {
                    Ok(count) => {
                        broadcast_event(events_tx, Event::PresetsReloaded { count });
                        Response::ok(format!("Reloaded {count} presets"))
                    }
                    Err(e) => Response::err("RELOAD_FAILED", e.to_string()),
                }
            }

            Request::UpsertProfile { toml } => {
                let mut st = state.lock().await;
                match aethershift_core::Profile::from_toml_str(&toml) {
                    Ok(profile) => {
                        let name = profile.name.clone();
                        let count = profile.bindings.len();
                        st.register_profile(profile);
                        Response::ok(format!("Profile '{name}' loaded with {count} bindings"))
                    }
                    Err(e) => Response::err("PROFILE_INVALID", e.to_string()),
                }
            }

            Request::GetProfile { name } => {
                let st = state.lock().await;
                match st.get_profile(&name) {
                    Some(profile) => {
                        let bindings = profile
                            .bindings
                            .iter()
                            .map(|binding| BindingInfo {
                                key_combo: binding.key_combo.canonical_str(),
                                action: binding.action.display_name(),
                                description: binding.description.clone(),
                            })
                            .collect::<Vec<_>>();
                        let payload = serde_json::json!({
                            "name": profile.name,
                            "description": profile.description,
                            "window_policy": profile.window_policy,
                            "bindings": bindings,
                        });
                        Response::ok_with_data("Profile retrieved", &payload)
                            .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
                    }
                    None => Response::err(
                        "PROFILE_NOT_FOUND",
                        format!("Profile '{name}' was not found"),
                    ),
                }
            }

            // Phase 3 Window Layout Actions
            Request::ApplyLayout { layout, preview } => {
                let client = match hyprland {
                    Some(c) => c,
                    None => {
                        let err_msg = "Hyprland client is not connected";
                        send_notification("AetherShift Error", err_msg, true, disabled);
                        return Response::err("NO_HYPRLAND", err_msg);
                    }
                };

                let active_win = match client.fetch_active_window().await {
                    Ok(Some(w)) => w,
                    Ok(None) => {
                        let err_msg = "No active window found to apply layout";
                        send_notification("AetherShift Error", err_msg, true, disabled);
                        return Response::err("NO_WINDOW", err_msg);
                    }
                    Err(e) => {
                        let err_msg = format!("Failed fetching active window: {}", e);
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        return Response::err("HYPRLAND_ERROR", err_msg);
                    }
                };

                let monitors = match client.fetch_monitors().await {
                    Ok(m) => m,
                    Err(e) => {
                        let err_msg = format!("Failed fetching monitors: {}", e);
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        return Response::err("HYPRLAND_ERROR", err_msg);
                    }
                };

                let mon = monitors
                    .iter()
                    .find(|m| m.id == active_win.monitor)
                    .or_else(|| monitors.iter().find(|m| m.focused))
                    .or_else(|| monitors.first());

                let mon = match mon {
                    Some(m) => m,
                    None => {
                        let err_msg = "No monitor detected";
                        send_notification("AetherShift Error", err_msg, true, disabled);
                        return Response::err("NO_MONITOR", err_msg);
                    }
                };

                let (ax, ay, aw, ah) = mon.work_area();
                let work_rect = Rect::new(ax, ay, aw, ah);

                let core_layout = match layout {
                    SnapLayout::HalfLeft => aethershift_core::SnapLayout::HalfLeft,
                    SnapLayout::HalfRight => aethershift_core::SnapLayout::HalfRight,
                    SnapLayout::HalfTop => aethershift_core::SnapLayout::HalfTop,
                    SnapLayout::HalfBottom => aethershift_core::SnapLayout::HalfBottom,
                    SnapLayout::TwoThirdsLeft => aethershift_core::SnapLayout::TwoThirdsLeft,
                    SnapLayout::OneThirdRight => aethershift_core::SnapLayout::OneThirdRight,
                    SnapLayout::OneThirdLeft => aethershift_core::SnapLayout::OneThirdLeft,
                    SnapLayout::TwoThirdsRight => aethershift_core::SnapLayout::TwoThirdsRight,
                    SnapLayout::ThreeColumnsLeft => aethershift_core::SnapLayout::ThreeColumnsLeft,
                    SnapLayout::ThreeColumnsCenter => {
                        aethershift_core::SnapLayout::ThreeColumnsCenter
                    }
                    SnapLayout::ThreeColumnsRight => {
                        aethershift_core::SnapLayout::ThreeColumnsRight
                    }
                    SnapLayout::Maximize => aethershift_core::SnapLayout::Maximize,
                    SnapLayout::CenterFloating => aethershift_core::SnapLayout::CenterFloating,
                    SnapLayout::RestoreOriginal => aethershift_core::SnapLayout::RestoreOriginal,
                };

                let target_rect = if core_layout == aethershift_core::SnapLayout::RestoreOriginal {
                    let mut mem = geometry_memory.lock().await;
                    match mem.restore_original(&active_win.address) {
                        Some(orig) => orig,
                        None => Rect::new(
                            active_win.at.0,
                            active_win.at.1,
                            active_win.size.0,
                            active_win.size.1,
                        ),
                    }
                } else {
                    let orig_rect = Rect::new(
                        active_win.at.0,
                        active_win.at.1,
                        active_win.size.0,
                        active_win.size.1,
                    );
                    geometry_memory
                        .lock()
                        .await
                        .record_if_absent(&active_win.address, orig_rect);
                    LayoutEngine::compute_geometry(core_layout, work_rect)
                };

                if preview {
                    let message = format!(
                        "Previewed layout '{}' on monitor '{}': [{}, {}, {}, {}]",
                        layout.as_str(),
                        mon.name,
                        target_rect.x,
                        target_rect.y,
                        target_rect.width,
                        target_rect.height
                    );
                    let feedback = LayoutFeedback {
                        layout: layout.as_str().to_string(),
                        preview: true,
                        monitor: mon.name.clone(),
                        from: Some([
                            active_win.at.0,
                            active_win.at.1,
                            active_win.size.0,
                            active_win.size.1,
                        ]),
                        to: [
                            target_rect.x,
                            target_rect.y,
                            target_rect.width,
                            target_rect.height,
                        ],
                        message: Some(message.clone()),
                    };
                    return Response::ok_with_data(message, &feedback)
                        .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()));
                }

                match client
                    .apply_window_rect(
                        &active_win.address,
                        target_rect.x,
                        target_rect.y,
                        target_rect.width,
                        target_rect.height,
                        true,
                    )
                    .await
                {
                    Ok(()) => {
                        let message = format!(
                            "Applied layout '{}' on monitor '{}': [{}, {}, {}, {}]",
                            layout.as_str(),
                            mon.name,
                            target_rect.x,
                            target_rect.y,
                            target_rect.width,
                            target_rect.height
                        );
                        let feedback = LayoutFeedback {
                            layout: layout.as_str().to_string(),
                            preview: false,
                            monitor: mon.name.clone(),
                            from: Some([
                                active_win.at.0,
                                active_win.at.1,
                                active_win.size.0,
                                active_win.size.1,
                            ]),
                            to: [
                                target_rect.x,
                                target_rect.y,
                                target_rect.width,
                                target_rect.height,
                            ],
                            message: Some(message.clone()),
                        };
                        let action_name = format!("snap_{}", layout.as_str());
                        stats.lock().await.record_action(&action_name);
                        send_notification("AetherShift", &message, false, disabled);
                        Response::ok_with_data(message, &feedback)
                            .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
                    }
                    Err(e) => {
                        let err_msg = format!("Failed applying layout: {}", e);
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        Response::err("LAYOUT_FAILED", err_msg)
                    }
                }
            }

            Request::MoveWindowToMonitor { direction } => {
                let client = match hyprland {
                    Some(c) => c,
                    None => {
                        let err_msg = "Hyprland client is not connected";
                        send_notification("AetherShift Error", err_msg, true, disabled);
                        return Response::err("NO_HYPRLAND", err_msg);
                    }
                };

                match client.move_window_to_monitor(None, &direction).await {
                    Ok(()) => {
                        stats.lock().await.record_action("move_window_to_monitor");
                        let notif_body = format!("Moved window to monitor '{}'", direction);
                        send_notification("AetherShift", &notif_body, false, disabled);
                        Response::ok(format!("Moved window to monitor '{}'", direction))
                    }
                    Err(e) => {
                        let err_msg = format!("Failed moving window: {}", e);
                        send_notification("AetherShift Error", &err_msg, true, disabled);
                        Response::err("MONITOR_MOVE_FAILED", err_msg)
                    }
                }
            }

            // Phase 3 Profile Runtime Editing & Persistence
            Request::SaveCurrentAsProfile { name, description } => {
                let mut st = state.lock().await;
                match st.save_current_as_profile(&name, description.as_deref()) {
                    Ok(()) => Response::ok(format!("Saved current overlays as profile '{}'", name)),
                    Err(e) => Response::err("SAVE_CURRENT_FAILED", e.to_string()),
                }
            }

            Request::CreateProfile {
                name,
                description,
                copy_from,
            } => {
                let mut st = state.lock().await;
                match st.create_profile(&name, description.as_deref(), copy_from.as_deref()) {
                    Ok(()) => Response::ok(format!("Profile '{}' created successfully", name)),
                    Err(e) => Response::err("CREATE_PROFILE_FAILED", e.to_string()),
                }
            }

            Request::UpdateBinding {
                profile,
                key_combo,
                action,
                description,
            } => {
                let plan_opt = {
                    let mut st = state.lock().await;
                    match st.update_binding(&profile, &key_combo, &action, description.as_deref()) {
                        Ok(p) => p,
                        Err(e) => return Response::err("UPDATE_BINDING_FAILED", e.to_string()),
                    }
                };

                if let Some(plan) = plan_opt {
                    if let Err(e) = Self::execute_plan(&plan, state, hyprland).await {
                        return Response::err(
                            "HYPRLAND_ERROR",
                            format!("Updated in memory but failed to apply: {}", e),
                        );
                    }
                }
                Response::ok(format!(
                    "Binding '{}' -> '{}' updated in profile '{}'",
                    key_combo, action, profile
                ))
            }

            Request::RemoveBinding { profile, key_combo } => {
                let plan_opt = {
                    let mut st = state.lock().await;
                    match st.remove_binding(&profile, &key_combo) {
                        Ok(p) => p,
                        Err(e) => return Response::err("REMOVE_BINDING_FAILED", e.to_string()),
                    }
                };

                if let Some(plan) = plan_opt {
                    if let Err(e) = Self::execute_plan(&plan, state, hyprland).await {
                        return Response::err(
                            "HYPRLAND_ERROR",
                            format!("Removed in memory but failed to unbind: {}", e),
                        );
                    }
                }
                Response::ok(format!(
                    "Binding '{}' removed from profile '{}'",
                    key_combo, profile
                ))
            }

            Request::SaveProfile { profile } => {
                let res = {
                    let st = state.lock().await;
                    st.save_profile(&profile, None)
                };
                match res {
                    Ok(path) => {
                        Response::ok(format!("Profile '{}' saved to {}", profile, path.display()))
                    }
                    Err(e) => Response::err("SAVE_PROFILE_FAILED", e.to_string()),
                }
            }

            Request::DeleteProfile { profile } => {
                let is_active = {
                    let st = state.lock().await;
                    st.active_profile_name() == profile
                };
                if is_active {
                    let plan = {
                        let st = state.lock().await;
                        st.restore_plan()
                    };
                    let _ = Self::execute_plan(&plan, state, hyprland).await;
                }
                let res = {
                    let mut st = state.lock().await;
                    st.delete_profile(&profile, None)
                };
                match res {
                    Ok(()) => {
                        let msg = if is_active {
                            format!(
                                "Active profile '{}' restored to native and deleted successfully",
                                profile
                            )
                        } else {
                            format!("Profile '{}' deleted successfully", profile)
                        };
                        Response::ok(msg)
                    }
                    Err(e) => Response::err("DELETE_PROFILE_FAILED", e.to_string()),
                }
            }

            // Phase 3 Stats and Recommendations
            Request::GetStats => {
                let st = stats.lock().await;
                let usage_stats = st.get_stats();
                match Response::ok_with_data("Usage statistics retrieved", &usage_stats) {
                    Ok(r) => r,
                    Err(e) => Response::err("SERIALIZATION_ERROR", e.to_string()),
                }
            }

            Request::GetRecommendations => {
                let current_profile = {
                    let st = state.lock().await;
                    st.get_profile(&st.active_profile_name()).cloned()
                };

                match current_profile {
                    Some(prof) => {
                        let recs = stats.lock().await.generate_recommendations(&prof);
                        match Response::ok_with_data("Recommendations generated", &recs) {
                            Ok(r) => r,
                            Err(e) => Response::err("SERIALIZATION_ERROR", e.to_string()),
                        }
                    }
                    None => Response::ok_with_data(
                        "No active profile",
                        &Vec::<aethershift_protocol::Recommendation>::new(),
                    )
                    .unwrap(),
                }
            }

            Request::ExportStats { path } => {
                let res = {
                    let st = stats.lock().await;
                    if let Some(ref p) = path {
                        st.export_to_file(Path::new(p))
                    } else {
                        let default_dir = get_user_config_dir().join("aethershift");
                        let _ = std::fs::create_dir_all(&default_dir);
                        let file_path = default_dir.join("stats.json");
                        st.export_to_file(&file_path).map(|_| ())
                    }
                };

                match res {
                    Ok(()) => Response::ok("Statistics successfully exported"),
                    Err(e) => Response::err("EXPORT_FAILED", e.to_string()),
                }
            }

            Request::Doctor { export } => {
                let (active_profile, baseline_count) = {
                    let st = state.lock().await;
                    (st.active_profile_name().to_string(), st.baseline().len())
                };
                let health_snapshot = health.lock().await.clone();
                let backend_message = if health_snapshot.backend_connected {
                    "Hyprland IPC connected"
                } else {
                    "Hyprland IPC unavailable; running in state-only mode"
                };
                let report = doctor_report(&DoctorInput {
                    uptime_secs: start_time.elapsed().as_secs(),
                    backend: &health_snapshot.backend,
                    backend_connected: health_snapshot.backend_connected,
                    backend_message,
                    baseline_count,
                    active_profile: &active_profile,
                    plugins_loaded: health_snapshot.plugins_loaded,
                    plugins_failed: health_snapshot.plugins_failed,
                    last_error: health_snapshot.last_error.as_deref(),
                });

                match export.as_deref() {
                    Some("-") => match serde_json::to_string_pretty(&report) {
                        Ok(json) => {
                            println!("{json}");
                            Response::ok("Diagnostic report exported to stdout")
                        }
                        Err(e) => Response::err("INTERNAL_ERROR", e.to_string()),
                    },
                    Some(path) => match serde_json::to_string_pretty(&report)
                        .map_err(|e| e.to_string())
                        .and_then(|json| {
                            std::fs::write(path, json + "\n").map_err(|e| e.to_string())
                        }) {
                        Ok(()) => Response::ok(format!("Diagnostic report exported to {path}")),
                        Err(error) => Response::err("EXPORT_FAILED", error),
                    },
                    None => Response::ok_with_data("Diagnostic report generated", &report)
                        .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string())),
                }
            }

            Request::Metrics { format } => {
                let snapshot = health.lock().await.clone();
                let metrics = snapshot.metrics(start_time.elapsed().as_secs());
                match format {
                    MetricsFormat::Json => Response::ok_with_data("Metrics retrieved", &metrics)
                        .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string())),
                    MetricsFormat::Text => Response::ok(metrics_text(&metrics)),
                }
            }

            Request::PluginList => {
                let registry = plugins.lock().await;
                let infos: Vec<WirePluginInfo> = registry.plugins().map(plugin_to_wire).collect();
                Response::ok_with_data("Plugins retrieved", &infos)
                    .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
            }

            Request::PluginReload => {
                let mut registry = plugins.lock().await;
                match registry.load_dir(plugin_dir()) {
                    Ok(count) => {
                        let failures = registry.failed_count();
                        let mut health_state = health.lock().await;
                        health_state.plugins_loaded = count;
                        health_state.plugins_failed = failures;
                        Response::ok(format!("Reloaded {count} action plugins"))
                    }
                    Err(e) => Response::err("PLUGIN_RELOAD_FAILED", e.to_string()),
                }
            }

            Request::PluginValidate { id } => {
                let registry = plugins.lock().await;
                let selected: Vec<WirePluginInfo> = registry
                    .plugins()
                    .filter(|plugin| {
                        id.as_deref()
                            .map(|wanted| plugin.manifest.id == wanted)
                            .unwrap_or(true)
                    })
                    .map(plugin_to_wire)
                    .collect();
                let failures: Vec<String> = registry
                    .failures()
                    .filter(|(failed_id, _)| {
                        id.as_deref()
                            .map(|wanted| failed_id.as_str() == wanted)
                            .unwrap_or(true)
                    })
                    .map(|(failed_id, error)| format!("{failed_id}: {error}"))
                    .collect();
                let payload = serde_json::json!({ "valid": failures.is_empty(), "plugins": selected, "failures": failures });
                Response::ok_with_data("Plugin validation completed", &payload)
                    .unwrap_or_else(|e| Response::err("INTERNAL_ERROR", e.to_string()))
            }

            Request::PluginRun { id } => {
                let plugin = plugins.lock().await.get(&id).cloned();
                let Some(plugin) = plugin else {
                    return Response::err(
                        "PLUGIN_NOT_FOUND",
                        format!("Plugin '{id}' is not loaded"),
                    );
                };
                let input = PluginActionInput {
                    action_id: &id,
                    args: serde_json::Value::Null,
                    context: serde_json::json!({ "profile": state.lock().await.active_profile_name() }),
                };
                match execute_plugin(&plugin, &input).await {
                    Ok(output) if output.ok => Response::ok(output.message),
                    Ok(output) => Response::err("PLUGIN_ACTION_FAILED", output.message),
                    Err(e) => Response::err("PLUGIN_ACTION_FAILED", e.to_string()),
                }
            }

            // Fallbacks for other request variants
            _ => Response::ok("Request processed (no-op)"),
        }
    }

    pub async fn cleanup(
        socket_path: &Path,
        state: &Mutex<StateManager>,
        hyprland: &Option<HyprlandClient>,
    ) {
        info!("Executing cleanup: restoring Hyprland baseline...");
        let plan = {
            let st = state.lock().await;
            st.restore_plan()
        };
        let _ = Self::execute_plan(&plan, state, hyprland).await;

        if socket_path.exists() {
            debug!("Removing socket file at {}", socket_path.display());
            let _ = std::fs::remove_file(socket_path);
        }
        info!("Cleanup completed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aethershift_core::binding::Action;
    use aethershift_hyprland::action::{Direction, HyprAction, WorkspaceTarget};

    #[test]
    fn test_core_action_to_hypr() {
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::CloseWindow),
            HyprAction::CloseWindow
        );
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::SnapLeft),
            HyprAction::Focus(Direction::Left)
        );
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::Maximize),
            HyprAction::ToggleFullscreen(Some(1))
        );
        assert_eq!(
            AetherDaemon::core_action_to_hypr(&Action::WorkspaceNext),
            HyprAction::Workspace(WorkspaceTarget::Relative(1))
        );
    }
}
