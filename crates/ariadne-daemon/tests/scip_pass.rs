//! scip-driven-edges tier-04: the daemon runs SCIP as a background/idle pass.
//!
//! A batch of out-of-band `ScipFacts` pushed over the `serve_live` SCIP channel
//! (the hand-back the composition root drives) is folded into the warm graph on
//! the pump thread — recovering a precise cross-crate edge the tree-sitter
//! resolver abstains on (ADR-0025) — while queries served concurrently over the
//! socket never block (exit #3). The facts are fed synthetically, so the test
//! needs no external indexer binary and stays hermetic, exactly as
//! `live_update.rs` feeds `Invalidation`s without `ariadne-watcher`
//! [src: docs/adr/0026-default-on-out-of-band-scip.md;
//!  .claude/plans/scip-driven-edges/tier-04-default-on-out-of-band-ingest.md step 4].

use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use ariadne_core::{
    DaemonQuery, DaemonRequest, DaemonResponse, Invalidation, ScipFacts, ScipOccurrence,
};
use ariadne_daemon::{DaemonStatus, ScipFactsBatch};

/// SCIP `SymbolRole::Definition` bit [src: crates/ariadne-scip/proto/scip.proto:526].
const SCIP_DEFINITION: u32 = 0x1;

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

/// Callers of `symbol` (names of the symbols whose edges point at it), via the
/// warm `FindReferences` query — the same projection `live_update.rs` asserts on.
fn references(root: &Path, symbol: &str) -> Vec<String> {
    let resp = ariadne_daemon::query(
        root,
        &DaemonRequest {
            revision: 0,
            query: DaemonQuery::FindReferences {
                symbol: symbol.to_owned(),
                limit: None,
                cursor: None,
                verbosity: ariadne_core::Verbosity::Concise,
            },
        },
    )
    .expect("query");
    match resp {
        DaemonResponse::References(report) => report
            .references
            .into_iter()
            .map(|s| s.caller_name)
            .collect(),
        _ => Vec::new(),
    }
}

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

/// The headline: a background SCIP pass recovers a cross-crate edge the resolver
/// drops, and the warm graph reflects it without any query blocking.
#[test]
fn background_scip_pass_recovers_cross_crate_edge_without_blocking_queries() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");

    // Crate B defines `connect`; crate A calls it method-style with no import.
    // The precise resolver abstains on the cross-crate method callee (ADR-0025),
    // so before SCIP there is no `user -> connect` edge.
    let b_dir = root.join("crates/b/src");
    let a_dir = root.join("crates/a/src");
    std::fs::create_dir_all(&b_dir).expect("mkdir b");
    std::fs::create_dir_all(&a_dir).expect("mkdir a");
    let b_file = b_dir.join("lib.rs");
    let a_file = a_dir.join("lib.rs");
    let b_src = "fn connect() {}\n";
    let a_src = "fn user() { s.connect(); }\n";
    std::fs::write(&b_file, b_src).expect("write b");
    std::fs::write(&a_file, a_src).expect("write a");

    // Drive `serve_live`, capturing the SCIP-facts sender it hands to `on_ready`.
    let (tx_out, rx_out) = mpsc::channel::<Sender<ScipFactsBatch>>();
    let (inv_tx, inv_rx) = mpsc::channel::<Invalidation>();
    let serve_root = root.clone();
    let handle = std::thread::spawn(move || {
        ariadne_daemon::serve_live(&serve_root, inv_rx, move |_lock, scip_tx| {
            let _ = tx_out.send(scip_tx);
        })
    });
    wait_until_running(&root, Duration::from_secs(5));
    let scip_tx = rx_out
        .recv_timeout(Duration::from_secs(5))
        .expect("on_ready hands back the SCIP sender");

    // Index both files, then confirm the resolver abstains pre-SCIP.
    inv_tx
        .send(Invalidation::Created {
            path: b_file.clone(),
        })
        .expect("send b");
    inv_tx
        .send(Invalidation::Created {
            path: a_file.clone(),
        })
        .expect("send a");
    wait_for_definition(&root, "user", Duration::from_secs(5));
    wait_for_definition(&root, "connect", Duration::from_secs(5));
    assert!(
        references(&root, "connect").is_empty(),
        "the precise resolver must abstain on the cross-crate method call before SCIP runs",
    );

    // Out-of-band SCIP batch: `connect` defined in B (def occurrence inside its
    // span), referenced from A's `user` body, both under one global symbol key.
    // Each file's indexed hash echoes its on-disk content hash → both covered (D4).
    let batch: ScipFactsBatch = vec![
        (
            "crates/b/src/lib.rs".to_owned(),
            ScipFacts {
                occurrences: vec![ScipOccurrence {
                    symbol: "scip:connect".to_owned(),
                    byte_range: (3, 10), // `connect` name in "fn connect() {}"
                    roles: SCIP_DEFINITION,
                }],
                relationships: Vec::new(),
                indexed_hash: *blake3::hash(b_src.as_bytes()).as_bytes(),
            },
        ),
        (
            "crates/a/src/lib.rs".to_owned(),
            ScipFacts {
                occurrences: vec![ScipOccurrence {
                    symbol: "scip:connect".to_owned(),
                    byte_range: (12, 21), // `s.connect` inside `user`'s body
                    roles: 0,
                }],
                relationships: Vec::new(),
                indexed_hash: *blake3::hash(a_src.as_bytes()).as_bytes(),
            },
        ),
    ];
    scip_tx.send(batch).expect("send scip batch");

    // The pump folds the batch on its own thread. Poll until the warm graph
    // surfaces the recovered edge; every poll is a live query answered while the
    // background pass runs — proving queries never block on the SCIP fold.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if references(&root, "connect") == vec!["user".to_owned()] {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background SCIP pass did not fold `user -> connect` into the warm graph in time",
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    ariadne_daemon::stop(&root).expect("stop");
    drop(inv_tx);
    handle
        .join()
        .expect("serve thread join")
        .expect("serve_live returns Ok after a clean stop");
}
