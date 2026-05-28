//! `ariadne daemon {start,stop,status}` — manage the background daemon.
//!
//! The CLI is the composition root, so it is the one crate that may depend on
//! the `ariadne-daemon` driving adapter [src: docs/adr/0007-cli-composition-root.md].
//! Each handler is a thin shim over `ariadne_daemon`'s lifecycle API.

use std::path::Path;

use anyhow::{Context, Result};
use ariadne_daemon::DaemonStatus;

/// Start the background daemon and wait until it answers.
///
/// # Errors
/// Propagates a daemon already-running, spawn, or start-timeout failure.
pub fn start(root: &Path) -> Result<()> {
    let report = ariadne_daemon::start(root).context("start daemon")?;
    // `spawned == false` means this process *was* the detached daemon and has
    // just shut down; its stdout is nulled, so the message is for the parent.
    if report.spawned {
        println!("daemon started (pid {})", report.pid);
    }
    Ok(())
}

/// Stop the running daemon (idempotent).
///
/// # Errors
/// Propagates a shutdown-timeout failure.
pub fn stop(root: &Path) -> Result<()> {
    ariadne_daemon::stop(root).context("stop daemon")?;
    println!("daemon stopped");
    Ok(())
}

/// Report whether the daemon is running.
///
/// # Errors
/// Propagates a malformed-protocol failure from the status probe.
pub fn status(root: &Path) -> Result<()> {
    match ariadne_daemon::status(root).context("daemon status")? {
        DaemonStatus::Running { pid: Some(pid) } => println!("daemon running (pid {pid})"),
        DaemonStatus::Running { pid: None } => println!("daemon running (pid unknown)"),
        DaemonStatus::Stopped => println!("daemon stopped"),
    }
    Ok(())
}
