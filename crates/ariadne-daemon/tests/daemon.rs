//! Integration tests for the daemon skeleton: a real `interprocess` local
//! socket carries `Ping`/`Pong`, the lifecycle creates and reclaims its
//! pidfile + socket under `.ariadne/`, and a stale pidfile/socket left by a
//! crashed daemon is reclaimed on the next start
//! [src: .claude/plans/post-v1-roadmap/tier-06-daemon-skeleton.md steps 1, 5].

use std::path::Path;
use std::time::{Duration, Instant};

use ariadne_core::DaemonResponse;
use ariadne_daemon::{DaemonPaths, DaemonStatus};

/// Spin until the daemon answers a `status` probe as `Running`, or panic
/// after `timeout`. The serve loop binds asynchronously on its own thread, so
/// tests must wait for readiness rather than assume it.
fn wait_until_running(root: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(
            ariadne_daemon::status(root).expect("status probe"),
            DaemonStatus::Running { .. }
        ) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "daemon did not reach Running within {timeout:?}",
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Golden path: serve on a background thread, `Ping` over the socket receives
/// `Pong`, `status` reports `Running`, then a clean `stop` tears down the
/// socket + pidfile and the serve loop returns `Ok`.
#[test]
fn ping_roundtrips_and_stop_is_clean() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");
    let paths = DaemonPaths::new(&root);

    let serve_root = root.clone();
    let handle = std::thread::spawn(move || ariadne_daemon::serve(&serve_root));

    wait_until_running(&root, Duration::from_secs(5));

    assert_eq!(
        ariadne_daemon::ping(&root).expect("ping"),
        DaemonResponse::Pong,
        "Ping over the local socket must receive Pong",
    );
    assert!(
        matches!(
            ariadne_daemon::status(&root).expect("status"),
            DaemonStatus::Running { .. }
        ),
        "status must report Running while the daemon serves",
    );

    ariadne_daemon::stop(&root).expect("stop");
    handle
        .join()
        .expect("serve thread join")
        .expect("serve returns Ok after a clean stop");

    assert!(!paths.socket.exists(), "socket file removed after stop");
    assert!(!paths.pidfile.exists(), "pidfile removed after stop");
    assert!(
        matches!(
            ariadne_daemon::status(&root).expect("status after stop"),
            DaemonStatus::Stopped
        ),
        "status must report Stopped once the daemon exits",
    );
}

/// A pidfile naming a dead PID alongside a dangling socket file (the residue
/// of a crashed daemon) is detected as stale on the next `serve` and
/// reclaimed: the live daemon rebinds the socket and overwrites the pidfile
/// with its own PID (risk R-B3).
#[test]
fn stale_pidfile_and_socket_are_reclaimed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");
    let paths = DaemonPaths::new(&root);

    // Plant a crashed daemon's residue: a bogus PID and a non-socket file
    // sitting at the socket path. No process is listening.
    std::fs::write(&paths.pidfile, "999999").expect("plant stale pidfile");
    std::fs::write(&paths.socket, b"stale").expect("plant stale socket file");

    let serve_root = root.clone();
    let handle = std::thread::spawn(move || ariadne_daemon::serve(&serve_root));

    wait_until_running(&root, Duration::from_secs(5));

    assert_eq!(
        ariadne_daemon::ping(&root).expect("ping after reclaim"),
        DaemonResponse::Pong,
        "the reclaimed daemon must answer Ping with Pong",
    );
    let live_pid = std::fs::read_to_string(&paths.pidfile).expect("read pidfile");
    assert_ne!(
        live_pid.trim(),
        "999999",
        "the stale pidfile must be overwritten with the live daemon's PID",
    );

    ariadne_daemon::stop(&root).expect("stop");
    handle
        .join()
        .expect("serve thread join")
        .expect("serve returns Ok after a clean stop");
}
