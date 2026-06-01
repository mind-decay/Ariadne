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

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use ariadne_daemon::DaemonStatus;
use ariadne_watcher::{ChannelSink, Ignore, NotifyWatcher};

use crate::config::Config;

/// Period between watcher reconciliation passes — the R7 safeguard that unions
/// the notify stream with a gitignore-aware content-hash scan so a missed
/// event cannot leave the warm graph stale (tier-08 step 5). Matches
/// `ariadne watch` [src: crates/ariadne-cli/src/commands/watch.rs:13].
const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

/// Period between background Git-history re-walks in the daemon child. Commits
/// are far less frequent than file saves, and a re-walk whose watermark is
/// already at HEAD visits zero new commits, so a coarse interval keeps history
/// fresh without contending with the daemon's transient redb opens (tier-11a
/// step 6).
const HISTORY_REWALK_INTERVAL: Duration = Duration::from_secs(60);

/// Granularity at which the re-walk loop re-checks its stop flag while idling,
/// so it joins promptly on shutdown (mirrors the live engine's `PUMP_TICK`
/// [src: crates/ariadne-daemon/src/domain/live.rs:37]).
const REWALK_TICK: Duration = Duration::from_millis(200);

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

        // Schedule the periodic Git-history re-walk at the composition root, so
        // the daemon never depends on `ariadne-git` (RD7). `serve_live` hands
        // back an `IndexLock` once the warm engine exists; the re-walk opens
        // redb under it so its access is serialized with the daemon's
        // pump/accept-loop opens (single-open per process, tier-11a I1).
        // Best-effort: a config-load failure degrades to no history refresh,
        // never a crash.
        let history_stop = Arc::new(AtomicBool::new(false));
        let mut history: Option<JoinHandle<()>> = None;
        let result = ariadne_daemon::serve_live(root, rx, |index_lock| match Config::load(root) {
            Ok(config) => {
                history = Some(spawn_history_rewalk(
                    root.to_path_buf(),
                    config,
                    Arc::clone(&history_stop),
                    index_lock,
                ));
            }
            Err(e) => eprintln!("[daemon] history re-walk disabled: {e:#}"),
        })
        .context("serve live daemon");

        history_stop.store(true, Ordering::Relaxed);
        if let Some(handle) = history {
            let _ = handle.join();
        }
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

/// Spawn the background Git-history re-walk loop. Each tick refreshes churn /
/// co-change through [`crate::commands::index::refresh_history`] (the
/// composition root owns `ariadne-git`; the daemon stays adapter-isolated,
/// RD7). `lock` serializes the re-walk's transient redb open against the
/// daemon's pump and accept-loop opens, so neither side races to
/// `DatabaseAlreadyOpen` (single-open per process, tier-11a I1). A refresh
/// error is logged and retried next tick; the HEAD-oid watermark guarantees no
/// commit is double-counted [src: tier-11a step 6].
fn spawn_history_rewalk(
    root: PathBuf,
    config: Config,
    stop: Arc<AtomicBool>,
    lock: ariadne_daemon::IndexLock,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("ariadne-history-rewalk".into())
        .spawn(move || {
            while wait_or_stop(&stop) {
                if let Err(e) = crate::commands::index::refresh_history(&root, &config, Some(&lock))
                {
                    eprintln!("[daemon] history re-walk skipped: {e:#}");
                }
            }
        })
        .expect("spawn history re-walk thread")
}

/// Idle one [`HISTORY_REWALK_INTERVAL`], re-checking `stop` every
/// [`REWALK_TICK`]. Returns `true` to run another re-walk, `false` once asked to
/// stop — so the loop drains promptly on daemon shutdown.
fn wait_or_stop(stop: &AtomicBool) -> bool {
    let mut waited = Duration::ZERO;
    while waited < HISTORY_REWALK_INTERVAL {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        thread::sleep(REWALK_TICK);
        waited += REWALK_TICK;
    }
    !stop.load(Ordering::Relaxed)
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
