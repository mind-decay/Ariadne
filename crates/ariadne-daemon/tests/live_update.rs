//! tier-08 live update: a filesystem invalidation fed to `serve_live`
//! re-derives the affected file through Salsa and applies a delta to the warm
//! graph, so a query over the IPC socket reflects the edit within the update
//! window. The test feeds a `Receiver<Invalidation>` manually — no
//! `ariadne-watcher` dependency, keeping the strict hexagonal invariant
//! (driving adapters never depend on each other; the real
//! fs→`NotifyWatcher`→daemon path is covered by the manual self-index
//! verification) [src: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md
//! steps 1, 3, 4; tier-08 build notes].

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ariadne_core::{DaemonQuery, DaemonRequest, DaemonResponse, Invalidation};
use ariadne_daemon::DaemonStatus;

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

fn definition(root: &Path, symbol: &str) -> DaemonResponse {
    ariadne_daemon::query(
        root,
        &DaemonRequest {
            revision: 0,
            query: DaemonQuery::FindDefinition {
                symbol: symbol.to_owned(),
            },
        },
    )
    .expect("query")
}

fn references(root: &Path, symbol: &str) -> Vec<String> {
    let resp = ariadne_daemon::query(
        root,
        &DaemonRequest {
            revision: 0,
            query: DaemonQuery::FindReferences {
                symbol: symbol.to_owned(),
            },
        },
    )
    .expect("query");
    match resp {
        DaemonResponse::References(sites) => sites.into_iter().map(|s| s.caller_name).collect(),
        _ => Vec::new(),
    }
}

/// Poll until `symbol` is defined in the warm graph, or panic after `timeout`.
fn wait_for_definition(root: &Path, symbol: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(definition(root, symbol), DaemonResponse::Definition(_)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "symbol {symbol} not defined within {timeout:?}",
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Poll until `symbol` is no longer defined, or panic after `timeout`.
fn wait_for_absence(root: &Path, symbol: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if matches!(definition(root, symbol), DaemonResponse::Error(_)) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "symbol {symbol} still defined within {timeout:?}",
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Golden path: start `serve_live` on an empty index, feed a `Created` event
/// to index a file, then a `Modified` event after editing it on disk. Queries
/// over the socket observe the symbol churn (a renamed function) and the edge
/// re-resolution (the caller of `alpha` changes) without restarting the
/// daemon.
#[test]
fn live_edit_is_reflected_over_ipc() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");
    let file = root.join("a.rs");
    std::fs::write(&file, "fn alpha() {}\nfn beta() { alpha(); }\n").expect("write fixture");

    let (tx, rx) = mpsc::channel::<Invalidation>();
    let serve_root = root.clone();
    // No background re-walk in this tier-08 test: ignore the IndexLock handle.
    let handle = std::thread::spawn(move || ariadne_daemon::serve_live(&serve_root, rx, |_| {}));
    wait_until_running(&root, Duration::from_secs(5));

    // Index the initial file.
    tx.send(Invalidation::Created { path: file.clone() })
        .expect("send created");
    wait_for_definition(&root, "beta", Duration::from_secs(5));
    assert_eq!(
        references(&root, "alpha"),
        vec!["beta".to_owned()],
        "alpha must be called by beta before the edit",
    );

    // Edit on disk: rename `beta` to `gamma`.
    std::fs::write(&file, "fn alpha() {}\nfn gamma() { alpha(); }\n").expect("rewrite fixture");
    tx.send(Invalidation::Modified { path: file.clone() })
        .expect("send modified");
    wait_for_definition(&root, "gamma", Duration::from_secs(5));
    wait_for_absence(&root, "beta", Duration::from_secs(5));
    assert_eq!(
        references(&root, "alpha"),
        vec!["gamma".to_owned()],
        "the caller edge must re-resolve to gamma after the edit",
    );

    ariadne_daemon::stop(&root).expect("stop");
    drop(tx);
    handle
        .join()
        .expect("serve thread join")
        .expect("serve_live returns Ok after a clean stop");
}
