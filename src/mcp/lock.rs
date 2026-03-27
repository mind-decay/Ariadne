use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::diagnostic::FatalError;

/// RAII lock guard that releases the lock file on drop.
pub struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    /// Acquire the lock and return a guard that releases it on drop.
    pub fn acquire(path: &Path) -> Result<Self, FatalError> {
        acquire_lock(path)?;
        Ok(Self {
            lock_path: path.to_path_buf(),
        })
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LockContent {
    pid: u32,
    started_at: String,
}

#[derive(Debug)]
pub enum LockStatus {
    Free,
    HeldByUs,
    HeldByOther { pid: u32 },
    Stale { pid: u32 },
}

impl LockStatus {
    pub fn is_free(&self) -> bool {
        matches!(self, LockStatus::Free)
    }
    pub fn is_held_by_us(&self) -> bool {
        matches!(self, LockStatus::HeldByUs)
    }
    pub fn is_stale(&self) -> bool {
        matches!(self, LockStatus::Stale { .. })
    }
}

/// Acquire the lock file. Removes stale locks automatically.
/// Returns E011 if held by another live process.
pub fn acquire_lock(lock_path: &Path) -> Result<(), FatalError> {
    let status = check_lock(lock_path)?;
    match status {
        LockStatus::Free => write_lock(lock_path),
        LockStatus::Stale { .. } => {
            // Remove stale lock, then acquire (caller should emit W016)
            let _ = std::fs::remove_file(lock_path);
            write_lock(lock_path)
        }
        LockStatus::HeldByUs => Ok(()),
        LockStatus::HeldByOther { pid } => Err(FatalError::LockFileHeld {
            pid,
            lock_path: lock_path.to_path_buf(),
        }),
    }
}

/// Release the lock file.
pub fn release_lock(lock_path: &Path) -> Result<(), FatalError> {
    if lock_path.exists() {
        std::fs::remove_file(lock_path).map_err(|e| FatalError::OutputNotWritable {
            path: lock_path.to_path_buf(),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}

/// Check the status of the lock file without modifying it.
/// If the lock file is corrupted (unreadable or invalid JSON), treat it as stale
/// rather than returning a confusing GraphCorrupted error.
pub fn check_lock(lock_path: &Path) -> Result<LockStatus, FatalError> {
    if !lock_path.exists() {
        return Ok(LockStatus::Free);
    }
    let content = match std::fs::read_to_string(lock_path) {
        Ok(c) => c,
        Err(_) => return Ok(LockStatus::Stale { pid: 0 }),
    };
    let lock: LockContent = match serde_json::from_str(&content) {
        Ok(l) => l,
        Err(_) => return Ok(LockStatus::Stale { pid: 0 }),
    };

    let current_pid = std::process::id();
    if lock.pid == current_pid {
        return Ok(LockStatus::HeldByUs);
    }

    if is_process_alive(lock.pid) {
        Ok(LockStatus::HeldByOther { pid: lock.pid })
    } else {
        Ok(LockStatus::Stale { pid: lock.pid })
    }
}

fn write_lock(lock_path: &Path) -> Result<(), FatalError> {
    let content = LockContent {
        pid: std::process::id(),
        started_at: current_timestamp(),
    };
    let json = serde_json::to_string_pretty(&content).unwrap();
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(lock_path, json).map_err(|e| FatalError::OutputNotWritable {
        path: lock_path.to_path_buf(),
        reason: e.to_string(),
    })
}

#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    // kill(pid, 0) checks if process exists without sending a signal.
    // Returns 0 if process exists and we have permission.
    // Returns -1 with errno EPERM if process exists but we lack permission.
    // Returns -1 with errno ESRCH if process does not exist.
    let ret = unsafe { libc::kill(pid as i32, 0) };
    if ret == 0 {
        return true;
    }
    // Check errno: EPERM means process exists but owned by another user
    let err = std::io::Error::last_os_error();
    err.raw_os_error() == Some(1) // EPERM = 1 on all Unix platforms
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    // Conservative: assume alive on non-Unix
    true
}

fn current_timestamp() -> String {
    let now = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        now.year(),
        now.month() as u8,
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
    )
}

/// Send SIGTERM to a process. Returns Ok(true) if signal sent, Ok(false) if process not found
/// or if the PID is invalid/dangerous. Returns Err on permission denied or other OS errors.
#[cfg(unix)]
pub fn terminate_process(pid: u32) -> Result<bool, std::io::Error> {
    // Guard against dangerous PID values:
    // - PID 0 sends signal to entire process group
    // - PID 1 is init/launchd — never kill it
    // - Values > i32::MAX wrap to negative when cast, signaling process groups
    if pid <= 1 || pid > i32::MAX as u32 {
        return Ok(false);
    }

    let ret = unsafe { libc::kill(pid as i32, 15) }; // 15 = SIGTERM
    if ret == 0 {
        Ok(true)
    } else {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(3) => Ok(false), // ESRCH - no such process
            Some(1) => {
                // EPERM - process exists but owned by another user
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("permission denied: cannot send SIGTERM to pid {} (owned by another user)", pid),
                ))
            }
            _ => Err(err),
        }
    }
}

// Minimal libc bindings for kill(2)
#[cfg(unix)]
mod libc {
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}
