//! Thin client to the warm daemon for the CLI (RD6).
//!
//! Unlike the MCP server — a sibling driving adapter barred from depending on
//! `ariadne-daemon`, so it embeds its own `interprocess`/postcard transport
//! [src: crates/ariadne-mcp/src/adapters/daemon_client.rs] — the CLI *is* the
//! composition root (ADR-0007) and already depends on `ariadne-daemon`. It
//! therefore reuses the daemon's canonical transport through the public
//! [`ariadne_daemon::query`] / [`ariadne_daemon::ping`] entry points instead
//! of duplicating the codec. That keeps the duplicated client transport at
//! exactly one file (the MCP client), so ADR-0015's deferred `ariadne-ipc`
//! crate stays deferred — "warranted only if per-adapter client duplication
//! later exceeds one file" \[src: docs/adr/0015-daemon-mode-ipc.md
//! `<consequences>`\]. This is the tier-10 step-2 decision: reuse the daemon's
//! transport rather than add `interprocess` to the CLI.
//!
//! Connection policy mirrors the tier-09 MCP client (risk R-B3): try the
//! socket; if no daemon answers, auto-spawn `ariadne daemon start <root>` and
//! retry once; if it still does not answer, return `None` so the caller falls
//! back to the v1 cold in-process path. A daemon that *does* answer —
//! including a query-level [`DaemonResponse::Error`] — is a real answer and is
//! returned as `Some`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ariadne_core::{DaemonQuery, DaemonRequest, DaemonResponse};

/// Environment switch that disables the auto-spawn step. Any value other than
/// `"0"` (or absence) leaves auto-spawn enabled. Tests set it to `"0"` so the
/// cold-fallback path is deterministic — mirrors the MCP client's switch.
const AUTOSPAWN_ENV: &str = "ARIADNE_CLI_AUTOSPAWN";
/// How long to wait for an auto-spawned daemon to start answering.
const SPAWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Poll cadence while waiting for the auto-spawned daemon to come up.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// A thin client to the warm daemon for one project root.
#[derive(Debug, Clone)]
pub struct DaemonClient {
    /// Project root, passed to `ariadne daemon start` on auto-spawn and to the
    /// daemon transport to address `<root>/.ariadne/daemon.sock`.
    root: PathBuf,
    /// Whether a missed socket may auto-spawn a daemon (see [`AUTOSPAWN_ENV`]).
    autospawn: bool,
}

impl DaemonClient {
    /// Build a client for `root`. Reads [`AUTOSPAWN_ENV`] once to decide
    /// whether a missed socket may auto-spawn a daemon.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        let autospawn = std::env::var_os(AUTOSPAWN_ENV).is_none_or(|v| v != "0");
        Self {
            root: root.to_path_buf(),
            autospawn,
        }
    }

    /// Route `query` to the daemon and return its [`DaemonResponse`], or `None`
    /// when no daemon is reachable after the auto-spawn attempt — the caller
    /// then answers from the cold path.
    ///
    /// A one-shot CLI invocation has no persistent last-observed redb revision
    /// (unlike a long-lived MCP session, which tracks its catalog), so it sends
    /// `revision: 0` (never-stale): the tier-08 daemon's own watcher keeps its
    /// warm graph current, so `revision 0` returns the daemon's live state
    /// without forcing a refresh [src: crates/ariadne-core/src/domain/daemon/mod.rs:30-41].
    #[must_use]
    pub fn try_query(&self, query: DaemonQuery) -> Option<DaemonResponse> {
        let request = DaemonRequest { revision: 0, query };
        if let Ok(resp) = ariadne_daemon::query(&self.root, &request) {
            return Some(resp);
        }
        if self.autospawn && self.spawn_and_wait() {
            if let Ok(resp) = ariadne_daemon::query(&self.root, &request) {
                return Some(resp);
            }
        }
        None
    }

    /// Auto-spawn `ariadne daemon start <root>` (re-exec of the running
    /// binary, matching the MCP client's auto-spawn) and poll until it answers
    /// the liveness handshake or [`SPAWN_TIMEOUT`] elapses.
    fn spawn_and_wait(&self) -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let spawned = Command::new(exe)
            .arg("daemon")
            .arg("start")
            .arg(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_err() {
            return false;
        }
        let deadline = Instant::now() + SPAWN_TIMEOUT;
        loop {
            if matches!(ariadne_daemon::ping(&self.root), Ok(DaemonResponse::Pong)) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}
