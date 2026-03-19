use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::diagnostic::FatalError;

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
pub fn check_lock(lock_path: &Path) -> Result<LockStatus, FatalError> {
    if !lock_path.exists() {
        return Ok(LockStatus::Free);
    }
    let content = std::fs::read_to_string(lock_path).map_err(|e| FatalError::GraphCorrupted {
        path: lock_path.to_path_buf(),
        reason: e.to_string(),
    })?;
    let lock: LockContent =
        serde_json::from_str(&content).map_err(|e| FatalError::GraphCorrupted {
            path: lock_path.to_path_buf(),
            reason: e.to_string(),
        })?;

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
    // kill(pid, 0) checks if process exists without sending a signal
    unsafe { libc::kill(pid as i32, 0) == 0 }
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

// Re-export libc for unix kill check
#[cfg(unix)]
mod libc {
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
}
