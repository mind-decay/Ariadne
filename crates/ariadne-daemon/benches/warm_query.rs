//! Warm-query SLO bench (tier-10 step 4, exit criterion #2).
//!
//! Seeds a redb-backed fixture index, starts a real `ariadne-daemon` serving
//! the warm in-RAM graph over its `interprocess` local socket, then times
//! `blast_radius` queries answered directly off the warm graph. RD6 tightens
//! the query budget to p95 < 10 ms (vs the 100 ms v1 cold SLO): the bench
//! hard-gates a 100-sample p95 against that budget before handing the same
//! round-trip to criterion for its standard measurement, so a regression past
//! 10 ms fails the bench loudly rather than silently widening the budget
//! [src: .claude/plans/post-v1-roadmap/plan.md RD6;
//!  .claude/plans/post-v1-roadmap/tier-10-cli-daemon-client-slo.md step 4].
//!
//! The bench round-trips through the daemon transport (`ariadne_daemon::query`)
//! rather than a child process, isolating warm-graph + IPC cost from process
//! spawn — the warm path Claude and the CLI actually hit once a daemon is up.

use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use ariadne_core::{
    Changeset, DaemonQuery, DaemonRequest, DaemonResponse, EdgeKey, EdgeKind, EdgeRecord, FileId,
    FileRecord, Lang, Span, Storage, SymbolId, SymbolRecord, Visibility, WriteTxn,
};
use ariadne_daemon::DaemonStatus;
use ariadne_storage::RedbStorage;
use criterion::{Criterion, criterion_group, criterion_main};

/// Symbols in the fixture graph — large enough that a reverse-BFS does real
/// work, small enough to seed quickly.
const SYMBOL_COUNT: u64 = 2_000;
/// Files the symbols are spread across.
const FILE_COUNT: u32 = 100;
/// Samples backing the p95 gate.
const SAMPLES: usize = 100;
/// RD6 warm-query budget: p95 < 10 ms.
const P95_BUDGET: Duration = Duration::from_millis(10);

fn warm_query_bench(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");
    seed(&index_path(&root));

    let handle = spawn(&root);

    // --- p95 gate (RD6) ----------------------------------------------------
    let mut samples = Vec::with_capacity(SAMPLES);
    for n in 0..SAMPLES {
        let symbol = symbol_name(u64::try_from(n).unwrap_or(0) % SYMBOL_COUNT + 1);
        let started = Instant::now();
        let resp = ariadne_daemon::query(&root, &blast_request(symbol.clone()))
            .expect("warm daemon answers blast_radius");
        samples.push(started.elapsed());
        assert!(
            matches!(resp, DaemonResponse::BlastRadius(_)),
            "expected a BlastRadius answer for {symbol}, got {resp:?}",
        );
    }
    samples.sort_unstable();
    let p95 = percentile(&samples, 95);
    println!(
        "warm_query bench: samples={SAMPLES} p50={:.3}ms p95={:.3}ms p99={:.3}ms (budget {:.1}ms)",
        ms(percentile(&samples, 50)),
        ms(p95),
        ms(percentile(&samples, 99)),
        ms(P95_BUDGET),
    );
    assert!(
        p95 < P95_BUDGET,
        "warm query p95 {p95:?} exceeds the {P95_BUDGET:?} RD6 budget",
    );

    // --- criterion measurement of one warm round-trip ----------------------
    let mut tick: u64 = 0;
    c.bench_function("warm_query_blast_radius", |b| {
        b.iter(|| {
            tick = tick.wrapping_add(1);
            let symbol = symbol_name(tick % SYMBOL_COUNT + 1);
            std::hint::black_box(
                ariadne_daemon::query(&root, &blast_request(symbol)).expect("warm daemon answers"),
            )
        });
    });

    shutdown(&root, handle);
}

/// `<root>/.ariadne/index.redb`.
fn index_path(root: &Path) -> PathBuf {
    root.join(".ariadne").join("index.redb")
}

/// A `blast_radius` request for `symbol` (revision 0 — never-stale, as a
/// one-shot client sends).
fn blast_request(symbol: String) -> DaemonRequest {
    DaemonRequest {
        revision: 0,
        query: DaemonQuery::BlastRadius {
            symbol,
            depth: Some(3),
            kinds: None,
            limit: None,
            cursor: None,
            verbosity: ariadne_core::Verbosity::Concise,
        },
    }
}

/// `sym_000001`-style canonical name for symbol id `n`.
fn symbol_name(n: u64) -> String {
    format!("sym_{n:06}")
}

/// Spawn `ariadne_daemon::serve` on a thread and block until it answers.
fn spawn(root: &Path) -> JoinHandle<Result<(), ariadne_daemon::DaemonError>> {
    let serve_root = root.to_path_buf();
    let handle = thread::spawn(move || ariadne_daemon::serve(&serve_root));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if matches!(
            ariadne_daemon::status(root).expect("status probe"),
            DaemonStatus::Running { .. }
        ) {
            return handle;
        }
        assert!(Instant::now() < deadline, "daemon did not start within 10s");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Stop the daemon and join the serve thread cleanly.
fn shutdown(root: &Path, handle: JoinHandle<Result<(), ariadne_daemon::DaemonError>>) {
    ariadne_daemon::stop(root).expect("stop daemon");
    handle
        .join()
        .expect("serve thread join")
        .expect("serve returns Ok");
}

/// Seed a chain-graph fixture into the redb at `db_path`. Applies in five
/// commits to stay under redb's per-write byte ceiling (mirrors the MCP
/// `concurrent` bench's `seed_large`).
fn seed(db_path: &Path) {
    let storage = RedbStorage::open(db_path).expect("open storage");
    let chunk = SYMBOL_COUNT / 5;
    for batch in 0..5u64 {
        let mut cs = Changeset::new();
        let start = batch * chunk + 1;
        let end = ((batch + 1) * chunk).min(SYMBOL_COUNT);
        if batch == 0 {
            for f in 1..=FILE_COUNT {
                cs = cs.upsert_file(
                    FileId::new(f).expect("nonzero file id"),
                    FileRecord {
                        path: format!("src/mod_{f:03}.rs"),
                        lang: Lang::Rust,
                        size: 4096,
                        blake3: [0u8; 32],
                        mtime_ns: 0,
                    },
                );
            }
        }
        for s in start..=end {
            let s32 = u32::try_from(s).expect("symbol index fits in u32");
            let file = ((s32 - 1) % FILE_COUNT) + 1;
            cs = cs.upsert_symbol(
                SymbolId::new(s).expect("nonzero symbol id"),
                SymbolRecord {
                    canonical_name: symbol_name(s),
                    kind: "function".into(),
                    defining_file: FileId::new(file).expect("nonzero file id"),
                    defining_span: Span {
                        file: FileId::new(file).expect("nonzero file id"),
                        byte_start: s32.wrapping_mul(8) % 32_000,
                        byte_end: s32.wrapping_mul(8).wrapping_add(4) % 32_000,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                    complexity: 0,
                },
            );
            // Chain edge: every symbol references the next, so a reverse-BFS
            // from `sym_n` walks back through its predecessors.
            if s < end {
                cs = cs.add_edge(
                    EdgeKey {
                        src: SymbolId::new(s).expect("nonzero symbol id"),
                        kind: EdgeKind::References,
                        dst: SymbolId::new(s + 1).expect("nonzero symbol id"),
                    },
                    EdgeRecord {
                        source_span: Span {
                            file: FileId::new(file).expect("nonzero file id"),
                            byte_start: 0,
                            byte_end: 4,
                        },
                        evidence_lang: Lang::Rust,
                        weight: 1,
                    },
                );
            }
        }
        let txn = storage.begin_write().expect("begin write");
        txn.apply(&cs).expect("apply changeset");
    }
}

/// Nearest-rank percentile with round-half-up over a pre-sorted slice.
fn percentile(sorted: &[Duration], pct: u32) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let last = sorted.len() - 1;
    let scaled = last
        .saturating_mul(usize::try_from(pct).unwrap_or(0))
        .saturating_add(50);
    sorted[(scaled / 100).min(last)]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}

criterion_group!(benches, warm_query_bench);
criterion_main!(benches);
