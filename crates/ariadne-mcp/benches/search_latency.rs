//! Tier-07 step 8 / R6 — `search_code` latency on a large synthetic catalog.
//!
//! Seeds a redb-backed `.ariadne/index.redb` with ~100K symbols across 500
//! files, builds the cold [`Catalog`] the tool reads, and times
//! `tools::search_code::handle` over the worst-case shapes: a broad
//! substring and a broad anchored regex that each match nearly every symbol
//! (so the collect + rank-sort path — the cost R6 bounds — runs at full
//! width before the default `limit` truncates). Aborts with a non-zero exit
//! code when the p95 exceeds the 100ms query SLO so CI can gate it.
//!
//! Custom harness (`harness = false`, no `criterion`) — mirrors the sibling
//! `cold_start` / `concurrent` benches, which deliberately avoid criterion
//! so the bench can *assert* a latency budget and exit non-zero, matching
//! this crate's bench convention and the tier-07 tech inventory.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use ariadne_core::{
    Changeset, FileId, FileRecord, Lang, Span, Storage, SymbolId, SymbolRecord, Visibility,
    WriteTxn,
};
use ariadne_mcp::AriadneServer;
use ariadne_mcp::tools::search_code as sc;
use ariadne_mcp::types::SearchCodeInput;
use ariadne_storage::RedbStorage;

const SYMBOL_COUNT: u64 = 100_000;
const FILE_COUNT: u32 = 500;
const BATCH: u64 = 10_000;
const ITERS: usize = 100;
const P95_LIMIT_MS: f64 = 100.0;

fn main() -> ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(run())
}

async fn run() -> ExitCode {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage_path = dir.path().join(".ariadne").join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open storage");
    seed_large(&storage);
    let revision = storage.revision().0;
    drop(storage);

    let server = AriadneServer::new(storage_path, dir.path().to_path_buf(), revision);
    // Force the lazy catalog build once so the timed loop measures query
    // latency, not the one-off cold build.
    let cat = server.catalog_arc().await;

    // Broad substring: "sym_0" matches every `sym_0XXXXX` name (~99K of
    // 100K), so the full match set is collected and rank-sorted each call.
    let substring = SearchCodeInput {
        query: "sym_0".into(),
        regex: false,
        path: None,
        kind: None,
        lang: None,
        visibility: None,
        limit: None,
    };
    // Broad anchored regex over the same set — the compiled-regex path.
    let regex = SearchCodeInput {
        query: "^sym_0".into(),
        regex: true,
        path: None,
        kind: None,
        lang: None,
        visibility: None,
        limit: None,
    };

    let mut samples: Vec<Duration> = Vec::with_capacity(ITERS * 2);
    for _ in 0..ITERS {
        let t0 = Instant::now();
        let _ = sc::handle(&cat, &substring).expect("substring search");
        samples.push(t0.elapsed());
        let t1 = Instant::now();
        let _ = sc::handle(&cat, &regex).expect("regex search");
        samples.push(t1.elapsed());
    }
    samples.sort();

    let p50 = percentile(&samples, 50);
    let p95 = percentile(&samples, 95);
    let p99 = percentile(&samples, 99);
    println!(
        "search-latency bench: symbols={SYMBOL_COUNT} iters={} samples={} \
         p50={:.3}ms p95={:.3}ms p99={:.3}ms budget={P95_LIMIT_MS:.1}ms",
        ITERS * 2,
        samples.len(),
        ms(p50),
        ms(p95),
        ms(p99),
    );

    if ms(p95) > P95_LIMIT_MS {
        eprintln!(
            "p95 {:.3}ms exceeds {P95_LIMIT_MS:.1}ms budget (R6)",
            ms(p95)
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn percentile(samples: &[Duration], pct: u32) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    // Nearest-rank with round-half-up.
    let last = samples.len() - 1;
    let scaled = last
        .saturating_mul(usize::try_from(pct).unwrap_or(0))
        .saturating_add(50);
    let idx = (scaled / 100).min(last);
    samples[idx]
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1_000.0
}

fn seed_large(storage: &RedbStorage) {
    let batches = SYMBOL_COUNT.div_ceil(BATCH);
    for batch in 0..batches {
        let mut cs = Changeset::new();
        let start = batch * BATCH + 1;
        let end = ((batch + 1) * BATCH).min(SYMBOL_COUNT);
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
                    canonical_name: format!("sym_{s:06}"),
                    kind: "function".into(),
                    defining_file: FileId::new(file).expect("nonzero file id"),
                    defining_span: Span {
                        file: FileId::new(file).expect("nonzero file id"),
                        byte_start: 0,
                        byte_end: 16,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                    complexity: 0,
                },
            );
        }
        let txn = storage.begin_write().expect("begin");
        txn.apply(&cs).expect("apply");
    }
}
