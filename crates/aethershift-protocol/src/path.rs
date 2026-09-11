use std::env;
use std::path::PathBuf;

pub const DEFAULT_SOCKET_NAME: &str = "aethershift.sock";

#[cfg(unix)]
fn get_current_uid() -> Option<u32> {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    Some(unsafe { getuid() })
}

#[cfg(not(unix))]
fn get_current_uid() -> Option<u32> {
    None
}

/// Returns the default Unix socket path for AetherShift IPC.
///
/// Priority:
/// 1. `$XDG_RUNTIME_DIR/aethershift.sock` (if `XDG_RUNTIME_DIR` is set)
/// 2. `/run/user/<uid>/aethershift.sock` (if `/run/user/<uid>` exists)
/// 3. `/tmp/aethershift.sock` (fallback)
pub fn default_socket_path() -> PathBuf {
    if let Ok(runtime_dir) = env::var("XDG_RUNTIME_DIR") {
        let trimmed = runtime_dir.trim();
        if !trimmed.is_empty() {
            let mut path = PathBuf::from(trimmed);
            path.push(DEFAULT_SOCKET_NAME);
            return path;
        }
    }

    if let Some(uid) = get_current_uid() {
        let user_run_dir = PathBuf::from(format!("/run/user/{uid}"));
        if user_run_dir.exists() {
            return user_run_dir.join(DEFAULT_SOCKET_NAME);
        }
    }

    PathBuf::from("/tmp").join(DEFAULT_SOCKET_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_socket_path() {
        // Test with custom XDG_RUNTIME_DIR
        let old_xdg = env::var("XDG_RUNTIME_DIR").ok();
        unsafe {
            env::set_var("XDG_RUNTIME_DIR", "/custom/run/dir");
        }
        let p = default_socket_path();
        assert_eq!(p, PathBuf::from("/custom/run/dir/aethershift.sock"));

        // Restore or remove
        unsafe {
            if let Some(old) = old_xdg {
                env::set_var("XDG_RUNTIME_DIR", old);
            } else {
                env::remove_var("XDG_RUNTIME_DIR");
            }
        }
    }
}
