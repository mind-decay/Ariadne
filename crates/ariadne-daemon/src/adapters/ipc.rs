//! `interprocess` local-socket transport plus the lifecycle orchestration
//! that binds it to the filesystem, and the warm-graph serve loop.
//!
//! The single external transport here is `interprocess`: the listener and
//! client connect over a Unix domain socket (named pipe on Windows)
//! addressed by the `<root>/.ariadne/daemon.sock` path
//! [src: <https://docs.rs/interprocess/2.4.2/interprocess/local_socket/index.html>].
//! The pidfile read/write, residue removal, and detached-process spawn are
//! plain `std` glue around that transport; the pure decisions they act on
//! live in [`crate::domain::lifecycle`] and the framing in
//! `crate::adapters::codec`.
//!
//! ## Warm graph
//! On startup the daemon opens `<root>/.ariadne/index.redb`, builds the
//! in-RAM `WarmCatalog` (petgraph + name/path/metadata indices), and drops
//! the storage handle — the warm state lives in RAM behind an [`RwLock`]
//! (concurrent reads, exclusive refresh). Queries dispatch against it; a
//! request carrying a newer redb revision than the catalog was built from
//! triggers a rebuild before the reply (risk R-B2)
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md steps 4, 6].
//!
//! ## Shutdown signalling
//! `stop` removes the pidfile, then opens one connection to wake the
//! blocking accept loop. After answering, the daemon sees its pidfile is
//! gone and exits, removing the socket [src: tier-06 step 5].

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, ListenerOptions, Name, Stream};

use ariadne_core::{DaemonRequest, DaemonResponse, Invalidation};
use ariadne_storage::RedbStorage;

use crate::adapters::codec;
use crate::domain::catalog::{WarmCatalog, index_path};
use crate::domain::dispatch;
use crate::domain::lifecycle::{DaemonPaths, DaemonStatus, Pid, ReclaimDecision, reclaim_decision};
use crate::domain::live::LiveEngine;
use crate::errors::DaemonError;

/// Poll cadence while waiting for the daemon to come up or go down.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long `start` waits for the detached daemon to answer.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
/// How long `stop` waits for the daemon to release its socket.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
/// Environment marker set on the detached child so the re-executed `start`
/// becomes the daemon instead of spawning yet another child.
const RUN_ENV: &str = "ARIADNE_DAEMON_RUN";

/// Outcome of a `start` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartReport {
    /// PID of the daemon process now serving the socket.
    pub pid: u32,
    /// `true` when this call spawned a detached daemon; `false` when this
    /// process *was* the detached daemon (the re-executed child).
    pub spawned: bool,
}

// ---- transport -----------------------------------------------------------

/// Address the `<root>/.ariadne/daemon.sock` path as an `interprocess` name.
fn socket_name(paths: &DaemonPaths) -> Result<Name<'static>, DaemonError> {
    paths
        .socket
        .clone()
        .to_fs_name::<GenericFilePath>()
        .map_err(DaemonError::from)
}

/// Send one request and read the response over a fresh connection.
fn round_trip(paths: &DaemonPaths, req: &DaemonRequest) -> Result<DaemonResponse, DaemonError> {
    let mut stream = Stream::connect(socket_name(paths)?)?;
    codec::write_frame(&mut stream, &codec::encode_request(req)?)?;
    let payload = codec::read_frame(&mut stream)?;
    codec::decode_response(&payload)
}

/// Whether a daemon answers the liveness handshake on the socket. Any IO or
/// protocol failure counts as not alive — the basis for stale-residue
/// reclamation.
fn is_alive(paths: &DaemonPaths) -> bool {
    matches!(
        round_trip(paths, &DaemonRequest::ping()),
        Ok(DaemonResponse::Pong)
    )
}

// ---- warm graph -----------------------------------------------------------

/// Open the project's redb index, build the warm catalog, and drop the
/// storage handle so the single-open redb lock is released. A missing index
/// is created empty by `RedbStorage::open`, yielding an empty catalog.
///
/// # Errors
/// Propagates storage-open / catalog-build failures.
fn load_catalog(project_root: &Path) -> Result<WarmCatalog, DaemonError> {
    let storage = RedbStorage::open(&index_path(project_root))?;
    WarmCatalog::build(&storage, project_root.display().to_string())
}

/// Serve exactly one request/response exchange on an accepted connection.
/// A request carrying a newer redb revision than the warm catalog was built
/// from rebuilds the catalog (exclusive write lock) before dispatching.
fn serve_connection(
    stream: &mut Stream,
    catalog: &RwLock<WarmCatalog>,
    project_root: &Path,
) -> Result<(), DaemonError> {
    let payload = codec::read_frame(stream)?;
    let req = codec::decode_request(&payload)?;

    let stale = catalog
        .read()
        .expect("warm-catalog read lock")
        .is_stale(req.revision);
    if stale {
        // Rebuild under the write lock so the redb open here is serialized
        // against the live-update pump, which also opens redb only under this
        // same lock — single-open-per-process means the two opens must not
        // overlap or one races to `DatabaseAlreadyOpen` [src: tier-08 audit I1].
        // Re-check staleness under the lock: the pump (or another connection)
        // may have already refreshed the catalog between the read above and the
        // write acquisition.
        let mut guard = catalog.write().expect("warm-catalog write lock");
        if guard.is_stale(req.revision) {
            match load_catalog(project_root) {
                Ok(fresh) => *guard = fresh,
                Err(e) => {
                    // A transient refresh failure becomes a typed query-level
                    // error so a client distinguishes a stale-rebuild miss from
                    // daemon death (a dropped connection). The daemon keeps its
                    // last-good warm graph and stays alive [src: tier-07 audit F3].
                    drop(guard);
                    let resp = DaemonResponse::Error(format!("warm-graph refresh failed: {e}"));
                    return codec::write_frame(stream, &codec::encode_response(&resp)?);
                }
            }
        }
    }

    let resp = {
        let cat = catalog.read().expect("warm-catalog read lock");
        dispatch::dispatch(&cat, req.query)
    };
    codec::write_frame(stream, &codec::encode_response(&resp)?)
}

// ---- pidfile / residue helpers -------------------------------------------

/// Read and parse the pidfile, if present and well-formed.
fn read_pid(paths: &DaemonPaths) -> Option<Pid> {
    std::fs::read_to_string(&paths.pidfile)
        .ok()
        .and_then(|text| Pid::parse(&text))
}

/// Whether the on-disk pidfile names this very process.
fn pidfile_is_ours(paths: &DaemonPaths, own: Pid) -> bool {
    read_pid(paths) == Some(own)
}

/// Remove the socket and pidfile, ignoring "not found".
fn remove_residue(paths: &DaemonPaths) {
    let _ = std::fs::remove_file(&paths.socket);
    let _ = std::fs::remove_file(&paths.pidfile);
}

/// Ensure `.ariadne/` exists so the pidfile and socket have a home.
fn ensure_dir(paths: &DaemonPaths) -> Result<(), DaemonError> {
    if let Some(dir) = paths.pidfile.parent() {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Block until the daemon answers, or time out.
fn wait_until_up(paths: &DaemonPaths, timeout: Duration) -> Result<(), DaemonError> {
    let deadline = Instant::now() + timeout;
    while !is_alive(paths) {
        if Instant::now() >= deadline {
            return Err(DaemonError::Timeout(
                "daemon did not answer within the start window".to_owned(),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

/// Block until the daemon stops answering, or time out.
fn wait_until_down(paths: &DaemonPaths, timeout: Duration) -> Result<(), DaemonError> {
    let deadline = Instant::now() + timeout;
    while is_alive(paths) {
        if Instant::now() >= deadline {
            return Err(DaemonError::Timeout(
                "daemon still answering after stop signal".to_owned(),
            ));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Ok(())
}

// ---- public lifecycle API -------------------------------------------------

/// Run the daemon: reclaim any stale residue, claim the pidfile, build the
/// warm graph, bind the socket, and serve queries until `stop` removes the
/// pidfile. Blocks for the daemon's lifetime; returns once it has shut down
/// cleanly.
///
/// # Errors
/// Returns [`DaemonError::AlreadyRunning`] if a live daemon already holds the
/// socket, a [`DaemonError::Storage`] / [`DaemonError::Graph`] error if the
/// warm graph cannot be built, or an [`DaemonError::Io`] /
/// [`DaemonError::Protocol`] error if binding or framing fails.
pub fn serve(project_root: &Path) -> Result<(), DaemonError> {
    let paths = DaemonPaths::new(project_root);
    let Some(own) = claim_lifecycle(&paths)? else {
        return Err(DaemonError::AlreadyRunning {
            pid: read_pid(&paths).map_or(0, |p| p.0),
        });
    };

    // Build the warm graph before binding so the daemon answers queries the
    // instant it accepts a connection.
    let catalog = match load_catalog(project_root) {
        Ok(catalog) => RwLock::new(catalog),
        Err(e) => {
            let _ = std::fs::remove_file(&paths.pidfile);
            return Err(e);
        }
    };

    serve_loop(&catalog, &paths, project_root, own)
}

/// Run the daemon with a live update loop: the warm graph is kept current by
/// draining `events` (filesystem invalidations the composition root feeds from
/// the watcher) through the incremental re-derivation pipeline, while the
/// accept loop serves queries against the same warm catalog. Blocks for the
/// daemon's lifetime, identically to [`serve`]; the CLI wires the
/// `ariadne-watcher` to this entry point (the daemon never depends on the
/// watcher directly — strict hexagonal invariant) [src: tier-08 build notes;
/// ADR-0007].
///
/// # Errors
/// Same failure modes as [`serve`], plus warm-engine seeding failures.
pub fn serve_live(project_root: &Path, events: Receiver<Invalidation>) -> Result<(), DaemonError> {
    let paths = DaemonPaths::new(project_root);
    let Some(own) = claim_lifecycle(&paths)? else {
        return Err(DaemonError::AlreadyRunning {
            pid: read_pid(&paths).map_or(0, |p| p.0),
        });
    };

    // Build the warm engine (catalog + seeded salsa db) before binding.
    let engine = match LiveEngine::start(project_root) {
        Ok(engine) => engine,
        Err(e) => {
            let _ = std::fs::remove_file(&paths.pidfile);
            return Err(e);
        }
    };
    let catalog = engine.catalog_arc();
    let stop = Arc::new(AtomicBool::new(false));
    let pump = engine.spawn_pump(events, Arc::clone(&stop));

    let result = serve_loop(&catalog, &paths, project_root, own);

    // Tear the update thread down cleanly regardless of how the loop ended.
    stop.store(true, Ordering::Relaxed);
    let _ = pump.join();
    result
}

/// Whether this process is the re-executed detached daemon child (the
/// `RUN_ENV` marker is set). The CLI composition root uses this to decide
/// whether to wire the watcher and block in [`serve_live`], or to spawn a
/// detached child [src: tier-08 build notes].
#[must_use]
pub fn running_as_daemon_child() -> bool {
    std::env::var_os(RUN_ENV).is_some()
}

/// Reclaim stale residue and claim the pidfile for this process. Returns the
/// claimed [`Pid`], or `None` when a live daemon already holds the socket.
fn claim_lifecycle(paths: &DaemonPaths) -> Result<Option<Pid>, DaemonError> {
    ensure_dir(paths)?;
    match reclaim_decision(
        paths.pidfile.exists(),
        paths.socket.exists(),
        is_alive(paths),
    ) {
        ReclaimDecision::AlreadyRunning => return Ok(None),
        ReclaimDecision::Reclaim => remove_residue(paths),
        ReclaimDecision::Fresh => {}
    }
    let own = Pid::current();
    std::fs::write(&paths.pidfile, own.to_text())?;
    Ok(Some(own))
}

/// Bind the socket and serve queries against `catalog` until the pidfile is
/// removed (the shutdown signal), then remove residue. Shared by [`serve`] and
/// [`serve_live`]; the catalog they pass differs (cold-rebuild-on-staleness vs
/// live-updated) but the accept loop is identical.
fn serve_loop(
    catalog: &RwLock<WarmCatalog>,
    paths: &DaemonPaths,
    project_root: &Path,
    own: Pid,
) -> Result<(), DaemonError> {
    // Bind the socket; retry once after clearing a leftover socket file.
    let listener = match ListenerOptions::new()
        .name(socket_name(paths)?)
        .create_sync()
    {
        Ok(listener) => listener,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let _ = std::fs::remove_file(&paths.socket);
            match ListenerOptions::new()
                .name(socket_name(paths)?)
                .create_sync()
            {
                Ok(listener) => listener,
                Err(e) => {
                    let _ = std::fs::remove_file(&paths.pidfile);
                    return Err(DaemonError::from(e));
                }
            }
        }
        Err(e) => {
            let _ = std::fs::remove_file(&paths.pidfile);
            return Err(DaemonError::from(e));
        }
    };

    for conn in listener.incoming() {
        if let Ok(mut stream) = conn {
            // A malformed client must not kill the daemon; a transient refresh
            // failure is answered as a typed error frame, not a dropped
            // connection (see `serve_connection`).
            let _ = serve_connection(&mut stream, catalog, project_root);
        }
        // The pidfile vanishing (or being reassigned) is the shutdown signal.
        if !pidfile_is_ours(paths, own) {
            break;
        }
    }

    remove_residue(paths);
    Ok(())
}

/// Send a single query to the daemon and return its response. The thin
/// client embedded by every driving adapter that speaks to the daemon
/// (tier-09/10/16) [src: docs/adr/0015-daemon-mode-ipc.md].
///
/// # Errors
/// Returns [`DaemonError::Io`] if no daemon is reachable, or
/// [`DaemonError::Protocol`] on a malformed frame.
pub fn query(project_root: &Path, request: &DaemonRequest) -> Result<DaemonResponse, DaemonError> {
    round_trip(&DaemonPaths::new(project_root), request)
}

/// Send a single `Ping` and return the daemon's response.
///
/// # Errors
/// Returns [`DaemonError::Io`] if no daemon is reachable, or
/// [`DaemonError::Protocol`] on a malformed response frame.
pub fn ping(project_root: &Path) -> Result<DaemonResponse, DaemonError> {
    round_trip(&DaemonPaths::new(project_root), &DaemonRequest::ping())
}

/// Report whether a daemon is serving this project.
///
/// # Errors
/// Returns [`DaemonError::Protocol`] when the socket is reachable but the peer
/// speaks a malformed protocol. A connection failure is reported as
/// [`DaemonStatus::Stopped`], not an error.
pub fn status(project_root: &Path) -> Result<DaemonStatus, DaemonError> {
    let paths = DaemonPaths::new(project_root);
    match round_trip(&paths, &DaemonRequest::ping()) {
        Ok(DaemonResponse::Pong) => Ok(DaemonStatus::Running {
            pid: read_pid(&paths).map(|p| p.0),
        }),
        Ok(other) => Err(DaemonError::Protocol(format!(
            "status probe expected Pong, got {other:?}"
        ))),
        Err(DaemonError::Io(_)) => Ok(DaemonStatus::Stopped),
        Err(other) => Err(other),
    }
}

/// Stop the daemon: remove the pidfile (the shutdown signal), wake the accept
/// loop, and wait for the socket to be released. Idempotent.
///
/// # Errors
/// Returns [`DaemonError::Timeout`] if the daemon does not release the socket
/// within the shutdown window.
pub fn stop(project_root: &Path) -> Result<(), DaemonError> {
    let paths = DaemonPaths::new(project_root);
    if !is_alive(&paths) {
        remove_residue(&paths);
        return Ok(());
    }
    let _ = std::fs::remove_file(&paths.pidfile);
    // One connection wakes the blocking accept loop; the daemon answers, then
    // notices its pidfile is gone and exits.
    let _ = round_trip(&paths, &DaemonRequest::ping());
    wait_until_down(&paths, SHUTDOWN_TIMEOUT)?;
    remove_residue(&paths);
    Ok(())
}

/// Start a daemon for `project_root`, detached into the background, and wait
/// until it answers. When invoked as the re-executed child (the `RUN_ENV`
/// marker is set) this call *becomes* the daemon and blocks in [`serve`].
///
/// # Errors
/// Returns [`DaemonError::AlreadyRunning`] if a daemon is already serving,
/// [`DaemonError::Spawn`] if the detached process cannot be launched, or
/// [`DaemonError::Timeout`] if it never answers.
pub fn start(project_root: &Path) -> Result<StartReport, DaemonError> {
    let paths = DaemonPaths::new(project_root);
    ensure_dir(&paths)?;

    // Re-executed child: become the daemon. `serve` blocks until shutdown.
    if std::env::var_os(RUN_ENV).is_some() {
        serve(project_root)?;
        return Ok(StartReport {
            pid: Pid::current().0,
            spawned: false,
        });
    }

    match reclaim_decision(
        paths.pidfile.exists(),
        paths.socket.exists(),
        is_alive(&paths),
    ) {
        ReclaimDecision::AlreadyRunning => {
            return Err(DaemonError::AlreadyRunning {
                pid: read_pid(&paths).map_or(0, |p| p.0),
            });
        }
        ReclaimDecision::Reclaim => remove_residue(&paths),
        ReclaimDecision::Fresh => {}
    }

    // Pass an absolute root so the detached child is independent of cwd.
    let root = std::fs::canonicalize(project_root).map_err(DaemonError::from)?;
    let exe = std::env::current_exe().map_err(|e| DaemonError::Spawn(e.to_string()))?;
    let child = std::process::Command::new(exe)
        .arg("daemon")
        .arg("start")
        .arg(&root)
        .env(RUN_ENV, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| DaemonError::Spawn(e.to_string()))?;
    let pid = child.id();

    wait_until_up(&paths, STARTUP_TIMEOUT)?;
    Ok(StartReport { pid, spawned: true })
}
