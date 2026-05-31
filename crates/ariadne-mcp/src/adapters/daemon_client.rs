//! Thin IPC client to the warm daemon (RD6).
//!
//! The MCP server forwards each read query to the always-warm daemon over the
//! `<root>/.ariadne/daemon.sock` local socket instead of cold-reading redb and
//! rebuilding a graph per session. The frame is the same one the daemon hosts
//! — a 4-byte big-endian payload length followed by the postcard encoding of a
//! [`DaemonRequest`] / [`DaemonResponse`] — so this client is wire-compatible
//! with `ariadne-daemon`'s codec without depending on it (the two are sibling
//! driving adapters, barred from depending on each other; ADR-0015 accepts the
//! one-file client duplication) [src: docs/adr/0015-daemon-mode-ipc.md].
//!
//! Connection policy (risk R-B3): try the socket; if no daemon answers,
//! auto-spawn `ariadne daemon start <root>` and retry once; if it still does
//! not answer, return `None` so the caller falls back to the v1 cold path. A
//! daemon that *does* answer — including a query-level
//! [`DaemonResponse::Error`] — is a real answer and is returned as `Some`.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ariadne_core::{DaemonQuery, DaemonRequest, DaemonResponse};
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{GenericFilePath, Name, Stream};

/// Per-project state directory holding the socket (mirrors the daemon's
/// `DaemonPaths`).
const ARIADNE_DIR: &str = ".ariadne";
/// Socket file name within `.ariadne/` (mirrors the daemon's `DaemonPaths`).
const SOCKET_NAME: &str = "daemon.sock";
/// Environment switch that disables the auto-spawn step. Any value other than
/// `"0"` (or absence) leaves auto-spawn enabled. Tests set it to `"0"` so the
/// cold-fallback path is deterministic.
const AUTOSPAWN_ENV: &str = "ARIADNE_MCP_AUTOSPAWN";
/// How long to wait for an auto-spawned daemon to start answering.
const SPAWN_TIMEOUT: Duration = Duration::from_secs(10);
/// Poll cadence while waiting for the auto-spawned daemon to come up.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// A thin client to the warm daemon for one project root.
#[derive(Debug, Clone)]
pub struct DaemonClient {
    /// Project root, passed to `ariadne daemon start` on auto-spawn.
    root: PathBuf,
    /// `<root>/.ariadne/daemon.sock`.
    socket: PathBuf,
    /// Whether a missed socket may auto-spawn a daemon (see [`AUTOSPAWN_ENV`]).
    autospawn: bool,
}

impl DaemonClient {
    /// Build a client for `root`. Reads `AUTOSPAWN_ENV` once to decide
    /// whether a missed socket may auto-spawn a daemon.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let socket = root.join(ARIADNE_DIR).join(SOCKET_NAME);
        let autospawn = std::env::var_os(AUTOSPAWN_ENV).is_none_or(|v| v != "0");
        Self {
            root,
            socket,
            autospawn,
        }
    }

    /// Route `query` to the daemon, carrying the client's last-known redb
    /// `revision` so the daemon refreshes its warm graph when the client is
    /// ahead (tier-07 staleness handshake).
    ///
    /// Returns the daemon's [`DaemonResponse`], or `None` when no daemon is
    /// reachable after the auto-spawn attempt — the caller then answers from
    /// the cold path.
    #[must_use]
    pub fn try_query(&self, revision: u64, query: DaemonQuery) -> Option<DaemonResponse> {
        let request = DaemonRequest { revision, query };
        if let Ok(resp) = self.round_trip(&request) {
            return Some(resp);
        }
        if self.autospawn && self.spawn_and_wait() {
            if let Ok(resp) = self.round_trip(&request) {
                return Some(resp);
            }
        }
        None
    }

    /// Async wrapper around [`Self::try_query`] for the `#[tool]` handlers.
    ///
    /// [`Self::try_query`] is synchronous: it blocks on socket IO and, on a
    /// missing daemon, runs an auto-spawn poll loop that sleeps up to
    /// `SPAWN_TIMEOUT`. Calling it directly inside an async handler would
    /// pin a tokio worker thread for that whole duration, so a slow or absent
    /// daemon could stall the executor. Offloading the round-trip to
    /// [`tokio::task::spawn_blocking`] keeps the blocking work on the blocking
    /// pool, leaving runtime workers free to drive other tool futures. A join
    /// failure (the blocking task panicked or was cancelled) maps to `None`,
    /// so the caller still falls back to the cold path.
    #[must_use]
    pub async fn try_query_async(
        &self,
        revision: u64,
        query: DaemonQuery,
    ) -> Option<DaemonResponse> {
        let client = self.clone();
        tokio::task::spawn_blocking(move || client.try_query(revision, query))
            .await
            .unwrap_or(None)
    }

    /// Address the socket path as an `interprocess` name.
    fn name(&self) -> std::io::Result<Name<'static>> {
        self.socket.clone().to_fs_name::<GenericFilePath>()
    }

    /// Send one request and read one response over a fresh connection.
    fn round_trip(&self, request: &DaemonRequest) -> std::io::Result<DaemonResponse> {
        let mut stream = Stream::connect(self.name()?)?;
        write_frame(&mut stream, &encode(request)?)?;
        let payload = read_frame(&mut stream)?;
        decode(&payload)
    }

    /// Auto-spawn `ariadne daemon start <root>` (re-exec of the running
    /// binary, matching the daemon's own detached-start path) and poll until it
    /// answers the liveness handshake or [`SPAWN_TIMEOUT`] elapses.
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
            if matches!(
                self.round_trip(&DaemonRequest::ping()),
                Ok(DaemonResponse::Pong)
            ) {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Upper bound on an accepted frame payload, guarding a malformed length prefix
/// from demanding a huge allocation (mirrors the daemon codec's 64 MiB cap).
const MAX_FRAME: usize = 64 * 1024 * 1024;

/// Write a length-prefixed frame and flush it.
fn write_frame<W: Write>(w: &mut W, payload: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "frame too large"))?;
    w.write_all(&len.to_be_bytes())?;
    w.write_all(payload)?;
    w.flush()
}

/// Read one length-prefixed frame, rejecting an oversized length prefix.
fn read_frame<R: Read>(r: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame length exceeds cap",
        ));
    }
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload)?;
    Ok(payload)
}

/// Encode a request to its postcard payload.
fn encode(request: &DaemonRequest) -> std::io::Result<Vec<u8>> {
    postcard::to_stdvec(request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Decode a response payload.
fn decode(payload: &[u8]) -> std::io::Result<DaemonResponse> {
    postcard::from_bytes(payload)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
