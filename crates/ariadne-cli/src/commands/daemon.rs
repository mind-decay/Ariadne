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
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use ariadne_daemon::{DaemonStatus, ScipFactsBatch};
use ariadne_watcher::{ChannelSink, Ignore, NotifyWatcher};

use crate::config::Config;

/// Environment opt-out for the daemon's background SCIP pass — the daemon-side
/// parallel to `ariadne index --no-scip`. SCIP is default-on (scip-driven-edges
/// D6); set this to any value to keep the warm graph on the tree-sitter resolver
/// only [src: docs/adr/0026-default-on-out-of-band-scip.md].
const NO_SCIP_ENV: &str = "ARIADNE_NO_SCIP";

/// Settle delay before the first background SCIP pass, so the heavy external
/// indexer builds start only after the warm graph is up and any initial burst of
/// edits has quiesced. SCIP is off the synchronous path, so this delay never
/// affects query or incremental-commit latency (R9, ADR-0026).
const SCIP_SETTLE: Duration = Duration::from_secs(2);

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
        // SCIP is default-on; `ARIADNE_NO_SCIP` is the daemon-side opt-out
        // parallel to `index --no-scip` (scip-driven-edges D6, ADR-0026).
        let scip_enabled = std::env::var_os(NO_SCIP_ENV).is_none();
        let history_stop = Arc::new(AtomicBool::new(false));
        let scip_stop = Arc::new(AtomicBool::new(false));
        let mut history: Option<JoinHandle<()>> = None;
        let result = ariadne_daemon::serve_live(root, rx, |index_lock, scip_tx| {
            match Config::load(root) {
                Ok(config) => {
                    history = Some(spawn_history_rewalk(
                        root.to_path_buf(),
                        config,
                        Arc::clone(&history_stop),
                        index_lock,
                    ));
                }
                Err(e) => eprintln!("[daemon] history re-walk disabled: {e:#}"),
            }
            // Schedule the out-of-band SCIP pass at the composition root, so the
            // daemon never links `ariadne-scip` (RD7-style isolation): the CLI
            // runs the external indexers and ships pre-computed pure-core facts
            // over `scip_tx` to the live pump. The pass is detached, not joined:
            // its only interruption point is the pre-build settle, so shutdown
            // signals `scip_stop` (abandoning a pass still settling) and returns
            // without waiting — a `daemon stop` issued mid-build never blocks for
            // the seconds-to-minutes the indexer build takes (audit F1).
            if scip_enabled {
                spawn_scip_pass(root.to_path_buf(), scip_tx, Arc::clone(&scip_stop));
            }
        })
        .context("serve live daemon");

        history_stop.store(true, Ordering::Relaxed);
        scip_stop.store(true, Ordering::Relaxed);
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

/// Spawn the daemon's single background SCIP pass (ADR-0026). After a short
/// settle — abandoned early if the daemon is already stopping — run the external
/// indexers via [`crate::domain::scip_facts`] and ship the extracted pure-core
/// facts to the live pump over `tx`. The daemon never links `ariadne-scip`; only
/// the pre-computed facts cross the channel, exactly as only pre-computed Git
/// hunks cross it for `diff_blast_radius` (RD7 / ADR-0023). A degraded run (no
/// indexer binary on PATH) yields an empty batch and sends nothing, so covered
/// files keep the precise tree-sitter resolver (plan D4). One pass establishes
/// SCIP coverage for the warm graph; a file edited afterward correctly falls
/// back to the resolver until a future pass (hash-gated, plan D4).
///
/// The thread is DETACHED (its handle is dropped), never joined on shutdown:
/// the [`settle_or_stop`] gate is the only interruption point, so once a pass
/// has entered the uninterruptible external indexer build, joining it would make
/// `daemon stop` block for the seconds-to-minutes the indexers take (audit F1).
/// `tx.send` is best-effort and fails harmlessly once `serve_live` drops the
/// receiver, so an abandoned in-flight pass simply discards its facts when the
/// daemon exits.
fn spawn_scip_pass(root: PathBuf, tx: Sender<ScipFactsBatch>, stop: Arc<AtomicBool>) {
    thread::Builder::new()
        .name("ariadne-scip-pass".into())
        .spawn(move || {
            if !settle_or_stop(&stop, SCIP_SETTLE) {
                return;
            }
            let facts = crate::domain::scip_facts(&root);
            if !facts.is_empty() {
                let _ = tx.send(facts);
            }
        })
        .expect("spawn scip pass thread");
}

/// Idle up to `delay`, re-checking `stop` every [`REWALK_TICK`]. Returns `false`
/// once asked to stop so a pass abandons cleanly on a fast shutdown.
fn settle_or_stop(stop: &AtomicBool, delay: Duration) -> bool {
    let mut waited = Duration::ZERO;
    while waited < delay {
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
