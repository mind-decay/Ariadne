//! Tier-08 step 10 — concurrent-tool-call latency probe.
//!
//! Builds a 10K-symbol redb-backed fixture index, opens an
//! [`AriadneServer`], spawns 8 tokio tasks each calling
//! `blast_radius` + `list_symbols` 100 times, and measures per-call
//! latency. Aborts with a non-zero exit code when the p95 exceeds
//! 100ms — gates the bench in CI per the tier's `<verification>`
//! line 2 ("cargo bench p95 ≤100ms per tool call under 8-way
//! concurrency").
//!
//! The bench drives the per-tool helper functions directly rather than
//! going through stdio framing. This isolates the analytics cost from
//! transport overhead (which Claude Code already amortizes per session
//! via the rmcp runtime) and matches what `criterion` can measure
//! without spawning a child process per iteration.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, Span, Storage, SymbolId,
    SymbolRecord, Visibility, WriteTxn,
};
use ariadne_mcp::tools::{blast_radius as br, list_symbols as ls};
use ariadne_mcp::types::{BlastRadiusInput, ListSymbolsInput};
use ariadne_mcp::{AriadneServer, Catalog};
use ariadne_storage::RedbStorage;

const SYMBOL_COUNT: u64 = 10_000;
const FILE_COUNT: u32 = 200;
const TASKS: usize = 8;
const PER_TASK_ITERS: usize = 100;
const P95_LIMIT_MS: f64 = 100.0;

fn main() -> ExitCode {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(TASKS)
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(run())
}

async fn run() -> ExitCode {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage_path = dir.path().join("index.redb");
    let storage = RedbStorage::open(&storage_path).expect("open storage");
    seed_large(&storage);
    let catalog =
        Catalog::build(&storage, dir.path().to_string_lossy().into_owned()).expect("catalog");
    drop(storage);
    let server = AriadneServer::new(storage_path, catalog);

    let mut handles = Vec::with_capacity(TASKS);
    for task_id in 0..TASKS {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            let mut samples = Vec::with_capacity(PER_TASK_ITERS * 2);
            let cat = server.catalog_arc();
            let ls_input = ListSymbolsInput {
                query: "sym_0".into(),
                kind: None,
                limit: Some(32),
            };
            for i in 0..PER_TASK_ITERS {
                let target = u64::try_from(task_id * PER_TASK_ITERS + i).unwrap_or(0)
                    % (SYMBOL_COUNT - 1)
                    + 1;
                let br_input = BlastRadiusInput {
                    symbol: format!("sym_{target:06}"),
                    depth: Some(2),
                    kinds: None,
                };
                let t0 = Instant::now();
                let _ = br::handle(&cat, &br_input);
                samples.push(t0.elapsed());
                let t1 = Instant::now();
                let _ = ls::handle(&cat, &ls_input);
                samples.push(t1.elapsed());
            }
            samples
        }));
    }

    let mut all: Vec<Duration> = Vec::new();
    for h in handles {
        all.extend(h.await.expect("task join"));
    }
    all.sort();
    let p50 = percentile(&all, 50);
    let p95 = percentile(&all, 95);
    let p99 = percentile(&all, 99);
    let total = all.len();

    println!(
        "concurrent bench: tasks={TASKS} per_task={PER_TASK_ITERS} samples={total} \
         p50={:.3}ms p95={:.3}ms p99={:.3}ms",
        ms(p50),
        ms(p95),
        ms(p99),
    );

    if ms(p95) > P95_LIMIT_MS {
        eprintln!("p95 {:.3}ms exceeds {P95_LIMIT_MS:.1}ms budget", ms(p95));
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
    // Apply in 5 commits to stay under redb's per-write byte ceiling.
    let chunk = SYMBOL_COUNT / 5;
    for batch in 0..5u64 {
        let mut cs = Changeset::new();
        let start = batch * chunk + 1;
        let end = ((batch + 1) * chunk).min(SYMBOL_COUNT);
        if batch == 0 {
            for f in 1..=FILE_COUNT {
                cs = cs.upsert_file(
                    FileId::new(f).unwrap(),
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
                SymbolId::new(s).unwrap(),
                SymbolRecord {
                    canonical_name: format!("sym_{s:06}"),
                    kind: "function".into(),
                    defining_file: FileId::new(file).unwrap(),
                    defining_span: Span {
                        file: FileId::new(file).unwrap(),
                        byte_start: s32.wrapping_mul(8) % 32_000,
                        byte_end: s32.wrapping_mul(8).wrapping_add(4) % 32_000,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                },
            );
            // Sparse edge graph: every symbol points to the next.
            if s < end {
                cs = cs.add_edge(
                    EdgeKey {
                        src: SymbolId::new(s).unwrap(),
                        kind: EdgeKind::References,
                        dst: SymbolId::new(s + 1).unwrap(),
                    },
                    EdgeRecord {
                        source_span: Span {
                            file: FileId::new(file).unwrap(),
                            byte_start: 0,
                            byte_end: 4,
                        },
                        evidence_lang: Lang::Rust,
                        weight: 1,
                    },
                );
            }
        }
        let txn = storage.begin_write().expect("begin");
        txn.apply(&cs).expect("apply");
    }
}
