//! Pure daemon lifecycle policy.
//!
//! Where the pidfile and socket live under `.ariadne/`, how a PID is parsed
//! and rendered, what `status` reports, and whether a leftover pidfile/socket
//! should be reclaimed — all decided here without touching the filesystem,
//! the network, or any process. The IO that acts on these decisions lives in
//! [`crate::adapters::ipc`].

use std::path::{Path, PathBuf};

/// Directory under a project root that holds Ariadne's per-project state.
const ARIADNE_DIR: &str = ".ariadne";
/// Pidfile name within `.ariadne/`. Holds the live daemon's PID as text.
const PIDFILE_NAME: &str = "daemon.pid";
/// Local-socket name within `.ariadne/`. A Unix domain socket on Unix; the
/// `interprocess` adapter maps it to the platform primitive.
const SOCKET_NAME: &str = "daemon.sock";

/// Filesystem locations the daemon manages for one project, derived purely
/// from the project root. The socket name is rooted in `.ariadne/` so each
/// project gets an isolated daemon
/// [src: .claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md step 4].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonPaths {
    /// Pidfile path: `<root>/.ariadne/daemon.pid`.
    pub pidfile: PathBuf,
    /// Local-socket path: `<root>/.ariadne/daemon.sock`.
    pub socket: PathBuf,
}

impl DaemonPaths {
    /// Derive the pidfile + socket paths for a daemon rooted at `project_root`.
    #[must_use]
    pub fn new(project_root: &Path) -> Self {
        let dir = project_root.join(ARIADNE_DIR);
        Self {
            pidfile: dir.join(PIDFILE_NAME),
            socket: dir.join(SOCKET_NAME),
        }
    }
}

/// A process identifier read from or written to the pidfile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pid(pub u32);

impl Pid {
    /// This process's PID. The serve loop records it so `stop` and a competing
    /// `start` can tell whose pidfile they are looking at.
    #[must_use]
    pub fn current() -> Self {
        Self(std::process::id())
    }

    /// Parse a pidfile's text body. Surrounding whitespace is ignored; any
    /// non-numeric content yields `None` (a corrupt pidfile is treated like a
    /// dead one — see [`reclaim_decision`]).
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        text.trim().parse::<u32>().ok().map(Self)
    }

    /// Render for writing to the pidfile.
    #[must_use]
    pub fn to_text(self) -> String {
        self.0.to_string()
    }
}

/// What `status` reports to a client about a project's daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonStatus {
    /// A daemon answered the liveness handshake. `pid` is the pidfile's value
    /// when readable, else `None` (running but pidfile missing/corrupt).
    Running {
        /// PID recorded in the pidfile, if it could be read and parsed.
        pid: Option<u32>,
    },
    /// No daemon answered the handshake.
    Stopped,
}

/// What a starting daemon should do about the existing on-disk state, decided
/// from two observations: whether a pidfile is present, and whether a daemon
/// answered the liveness handshake on the socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReclaimDecision {
    /// No live daemon and no residue — bind a fresh socket straight away.
    Fresh,
    /// A pidfile and/or socket remain but nothing answers — the residue of a
    /// crashed daemon. Remove both, then bind (risk R-B3).
    Reclaim,
    /// A daemon already answered the handshake — refuse to start a second one.
    AlreadyRunning,
}

/// Decide the start-up action from liveness + residue observations.
///
/// Liveness is established by the `Ping`/`Pong` handshake, not by inspecting
/// the PID: a dead process cannot answer, so a present pidfile with no live
/// responder is exactly the stale case to reclaim
/// [src: .claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md step 5;
///  plan.md risk R-B3].
#[must_use]
pub fn reclaim_decision(
    pidfile_present: bool,
    socket_present: bool,
    alive: bool,
) -> ReclaimDecision {
    if alive {
        ReclaimDecision::AlreadyRunning
    } else if pidfile_present || socket_present {
        ReclaimDecision::Reclaim
    } else {
        ReclaimDecision::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_root_under_ariadne_dir() {
        let paths = DaemonPaths::new(Path::new("/projects/demo"));
        assert_eq!(
            paths.pidfile,
            PathBuf::from("/projects/demo/.ariadne/daemon.pid")
        );
        assert_eq!(
            paths.socket,
            PathBuf::from("/projects/demo/.ariadne/daemon.sock")
        );
    }

    #[test]
    fn pid_parse_trims_and_rejects_garbage() {
        assert_eq!(Pid::parse(" 4321\n"), Some(Pid(4321)));
        assert_eq!(Pid::parse("not-a-pid"), None);
        assert_eq!(Pid::parse(""), None);
        assert_eq!(Pid(7).to_text(), "7");
    }

    #[test]
    fn decision_is_alive_then_reclaim_then_fresh() {
        // A live responder wins regardless of residue.
        assert_eq!(
            reclaim_decision(true, true, true),
            ReclaimDecision::AlreadyRunning
        );
        // Residue with no responder is stale.
        assert_eq!(
            reclaim_decision(true, false, false),
            ReclaimDecision::Reclaim
        );
        assert_eq!(
            reclaim_decision(false, true, false),
            ReclaimDecision::Reclaim
        );
        // No residue, no responder: clean start.
        assert_eq!(
            reclaim_decision(false, false, false),
            ReclaimDecision::Fresh
        );
    }
}
