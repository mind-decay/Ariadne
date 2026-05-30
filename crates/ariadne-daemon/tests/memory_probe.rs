//! Daemon warm-graph memory probe (tier-10 step 5; CLAUDE.md R1).
//!
//! The daemon owns the warm in-RAM graph, so — per the standing per-tier rule
//! that every tier touching Salsa or the in-RAM graph reports `memory_report()`
//! deltas — this probe builds a warm graph through the live re-derivation
//! engine, then asserts (a) the populated tables carry real, non-zero per-table
//! bytes — proving the probe measures resident memory rather than a stub — and
//! (b) no warm-graph table exceeds the 256 MiB per-table budget. It reuses the
//! tier-04 mechanism (`AriadneDb::memory_report`, a `mem::size_of_val` deep walk
//! over the memoized per-file query outputs) surfaced on the daemon as
//! [`LiveEngine::memory_report`]. The complementary
//! 4 GiB daemon-RSS ceiling on the 100K-file workload is measured by the
//! `ariadne-e2e` warm SLO stage (`slo.rs`), which can probe a real daemon
//! process's RSS [src: .claude/plans/post-v1-roadmap/plan.md `<risks>` R1;
//!  .claude/plans/post-v1-roadmap/tier-10-cli-daemon-client-slo.md step 5].

use ariadne_core::Invalidation;
use ariadne_daemon::LiveEngine;
use ariadne_salsa::TABLE_BUDGET_BYTES;

/// Files seeded into the warm graph for the probe. Enough that every tracked
/// derivation table holds real data, fast enough for the unit suite.
const PROBE_FILES: usize = 64;

#[test]
fn warm_graph_tables_stay_within_the_per_table_budget() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");

    // Write a small multi-symbol corpus on disk.
    let mut paths = Vec::with_capacity(PROBE_FILES);
    for i in 0..PROBE_FILES {
        let path = root.join(format!("f{i:03}.rs"));
        std::fs::write(&path, source(i)).expect("write fixture file");
        paths.push(path);
    }

    // Build the warm graph through the live engine: re-derive each file so the
    // Salsa tables (`symbols_for_file`, `edges_for_file`, `syntactic_facts`, …)
    // are populated exactly as a running daemon's would be.
    let mut engine = LiveEngine::start(root).expect("start live engine");
    for path in &paths {
        engine
            .apply(&Invalidation::Created { path: path.clone() })
            .expect("re-derive seeded file into the warm graph");
    }

    let report = engine.memory_report();
    print_report(&report);

    assert!(
        !report.tables.is_empty(),
        "memory report listed no tables — the probe surface is empty",
    );

    // The probe must measure real resident bytes, not a constant stub: a warm
    // graph of 64 multi-symbol files necessarily holds parsed facts and derived
    // symbols, so those tables are strictly positive. A vacuous probe (the
    // pre-fix zero-baseline) would fail here.
    let syntactic = report.tables.get(&"syntactic_facts").copied().unwrap_or(0);
    let symbols = report.tables.get(&"symbols_for_file").copied().unwrap_or(0);
    assert!(
        syntactic > 0 && symbols > 0,
        "warm graph populated but per-table probe measured zero bytes \
         (syntactic_facts={syntactic}, symbols_for_file={symbols}): {report:?}",
    );

    let over: Vec<_> = report.over_budget().collect();
    assert!(
        over.is_empty(),
        "warm-graph tables over the {TABLE_BUDGET_BYTES}-byte per-table budget (R1): {over:?}",
    );
}

/// A tiny Rust source unit with a couple of inter-referencing functions, so
/// each file contributes symbols and an edge to the warm graph.
fn source(i: usize) -> String {
    format!("fn alpha_{i}() {{ beta_{i}(); }}\nfn beta_{i}() {{}}\n")
}

/// Print the per-table report in the same shape as `ariadne mem`, so the
/// audit can read the warm-graph deltas off the test log.
fn print_report(report: &ariadne_salsa::MemoryReport) {
    println!("warm-graph table         estimated_bytes");
    for (name, bytes) in &report.tables {
        println!("  {name:<22} {bytes}");
    }
    println!("  {:<22} {}", "total", report.total_bytes());
}
