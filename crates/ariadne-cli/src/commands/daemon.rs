//! `ariadne daemon {start,stop,status}` — manage the background daemon.
//!
//! The CLI is the composition root, so it is the one crate that may depend on
//! the `ariadne-daemon` driving adapter [src: docs/adr/0007-cli-composition-root.md].
//! Each handler is a thin shim over `ariadne_daemon`'s lifecycle API.
//!
//! tier-08 makes the detached daemon live: when this process *is* the
//! re-executed daemon child, the CLI (not the daemon — driving adapters never
//! depend on each other) wires the `ariadne-watcher` to the daemon's
//! `serve_live` entry point over the `ariadne_core::Invalidation` channel
//! [src: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md step 5;
//!  ADR-0007].

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use ariadne_daemon::DaemonStatus;
use ariadne_watcher::{ChannelSink, Ignore, NotifyWatcher};

/// Period between watcher reconciliation passes — the R7 safeguard that unions
/// the notify stream with a gitignore-aware content-hash scan so a missed
/// event cannot leave the warm graph stale (tier-08 step 5). Matches
/// `ariadne watch` [src: crates/ariadne-cli/src/commands/watch.rs:13].
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// Start the background daemon and wait until it answers.
///
/// When invoked as the detached daemon child, this call *becomes* the daemon:
/// it owns the file watcher and blocks in `serve_live`, draining filesystem
/// invalidations into incremental warm-graph updates until `stop`. Otherwise
/// it spawns the detached child and waits for it to come up.
///
/// # Errors
/// Propagates watcher-start, daemon already-running, spawn, or start-timeout
/// failures.
pub fn start(root: &Path) -> Result<()> {
    if ariadne_daemon::running_as_daemon_child() {
        // We are the detached child. Own the watcher here (the composition
        // root) and feed its invalidations to the daemon's live warm graph.
        let (sink, rx) = ChannelSink::pair();
        let ignore = Ignore::build(root).context("build ignore matcher")?;
        let watcher = NotifyWatcher::start(root, ignore, Box::new(sink), RECONCILE_INTERVAL)
            .context("start watcher")?;
        let result = ariadne_daemon::serve_live(root, rx).context("serve live daemon");
        watcher.stop();
        return result;
    }

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
