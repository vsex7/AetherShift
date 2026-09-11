use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::CoreError;

const MAX_PLUGIN_OUTPUT_BYTES: usize = 64 * 1024;
const DEFAULT_PLUGIN_TIMEOUT_MS: u64 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginPermissionScope {
    None,
    HyprlandDispatch,
    Shell,
    Clipboard,
    Notification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    #[serde(default = "default_plugin_version")]
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub executable: String,
    #[serde(default = "default_plugin_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub permissions: Vec<PluginPermissionScope>,
    #[serde(default = "default_plugin_enabled")]
    pub enabled: bool,
}

fn default_plugin_version() -> String {
    "0.1.0".to_string()
}

fn default_plugin_timeout_ms() -> u64 {
    DEFAULT_PLUGIN_TIMEOUT_MS
}

fn default_plugin_enabled() -> bool {
    true
}

impl PluginManifest {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CoreError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref).map_err(|source| CoreError::Io {
            path: path_ref.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&content)
    }

    pub fn from_toml_str(content: &str) -> Result<Self, CoreError> {
        let manifest: Self = toml::from_str(content).map_err(|source| CoreError::TomlParse {
            path: "<plugin manifest>".into(),
            source,
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), CoreError> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .chars()
                .next()
                .map(|c| c.is_ascii_lowercase())
                .unwrap_or(false)
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(self
                .invalid("plugin id must be 1-64 lowercase ascii characters, digits, '-' or '_'"));
        }
        if self.version.trim().is_empty() {
            return Err(self.invalid("version cannot be empty"));
        }
        if self.executable.trim().is_empty() {
            return Err(self.invalid("executable cannot be empty"));
        }
        if self.timeout_ms == 0 || self.timeout_ms > 60_000 {
            return Err(self.invalid("timeout_ms must be between 1 and 60000"));
        }
        if self.permissions.contains(&PluginPermissionScope::None) && self.permissions.len() > 1 {
            return Err(self.invalid("'none' cannot be combined with other permission scopes"));
        }
        Ok(())
    }

    fn invalid(&self, message: &str) -> CoreError {
        CoreError::Validation {
            profile: self.id.clone(),
            message: format!("invalid plugin manifest: {message}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, LoadedPlugin>,
    failed: BTreeMap<String, String>,
}

impl PluginRegistry {
    pub fn load_dir(&mut self, dir: impl AsRef<Path>) -> Result<usize, CoreError> {
        let dir_ref = dir.as_ref();
        self.plugins.clear();
        self.failed.clear();
        if !dir_ref.exists() || !dir_ref.is_dir() {
            return Ok(0);
        }

        let mut loaded = 0;
        for entry in std::fs::read_dir(dir_ref).map_err(|source| CoreError::Io {
            path: dir_ref.to_path_buf(),
            source,
        })? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            match PluginManifest::from_file(&path) {
                Ok(manifest) => {
                    if self.plugins.contains_key(&manifest.id) {
                        self.failed
                            .insert(manifest.id.clone(), "duplicate plugin id".to_string());
                    } else {
                        self.plugins.insert(
                            manifest.id.clone(),
                            LoadedPlugin {
                                manifest,
                                source_path: path,
                            },
                        );
                        loaded += 1;
                    }
                }
                Err(error) => {
                    let id = path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    self.failed.insert(id, error.to_string());
                }
            }
        }
        Ok(loaded)
    }

    pub fn get(&self, id: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(id)
    }

    pub fn plugins(&self) -> impl Iterator<Item = &LoadedPlugin> {
        self.plugins.values()
    }

    pub fn failures(&self) -> impl Iterator<Item = (&String, &String)> {
        self.failed.iter()
    }

    pub fn loaded_count(&self) -> usize {
        self.plugins.len()
    }

    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginActionInput<'a> {
    pub action_id: &'a str,
    #[serde(default)]
    pub args: serde_json::Value,
    #[serde(default)]
    pub context: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginActionOutput {
    pub ok: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: serde_json::Value,
    #[serde(default)]
    pub retryable: bool,
}

pub async fn execute_plugin(
    plugin: &LoadedPlugin,
    input: &PluginActionInput<'_>,
) -> Result<PluginActionOutput, CoreError> {
    if !plugin.manifest.enabled {
        return Err(CoreError::Validation {
            profile: plugin.manifest.id.clone(),
            message: "plugin is disabled".to_string(),
        });
    }

    let stdin_json = serde_json::to_vec(input).map_err(|error| CoreError::Validation {
        profile: plugin.manifest.id.clone(),
        message: format!("failed to encode plugin input: {error}"),
    })?;

    let child = Command::new(&plugin.manifest.executable)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| CoreError::Io {
            path: PathBuf::from(&plugin.manifest.executable),
            source,
        })?;

    let output = tokio::time::timeout(
        Duration::from_millis(plugin.manifest.timeout_ms),
        async move {
            let mut child = child;
            let mut stdin = match child.stdin.take() {
                Some(stdin) => stdin,
                None => {
                    return Err(CoreError::Validation {
                        profile: "plugin".to_string(),
                        message: "plugin stdin unavailable".to_string(),
                    });
                }
            };
            stdin
                .write_all(&stdin_json)
                .await
                .map_err(|source| CoreError::Io {
                    path: PathBuf::from("plugin_stdin"),
                    source,
                })?;
            stdin.shutdown().await.map_err(|source| CoreError::Io {
                path: PathBuf::from("plugin_stdin"),
                source,
            })?;
            child
                .wait_with_output()
                .await
                .map_err(|source| CoreError::Io {
                    path: PathBuf::from("plugin"),
                    source,
                })
        },
    )
    .await
    .map_err(|_| CoreError::Validation {
        profile: plugin.manifest.id.clone(),
        message: format!("plugin timed out after {}ms", plugin.manifest.timeout_ms),
    })??;

    if output.stdout.len() > MAX_PLUGIN_OUTPUT_BYTES {
        return Err(CoreError::Validation {
            profile: plugin.manifest.id.clone(),
            message: "plugin output exceeds 64 KiB".to_string(),
        });
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Validation {
            profile: plugin.manifest.id.clone(),
            message: format!("plugin exited with {}: {}", output.status, stderr.trim()),
        });
    }

    let result: PluginActionOutput =
        serde_json::from_slice(&output.stdout).map_err(|error| CoreError::Validation {
            profile: plugin.manifest.id.clone(),
            message: format!("invalid plugin JSON output: {error}"),
        })?;
    Ok(result)
}
