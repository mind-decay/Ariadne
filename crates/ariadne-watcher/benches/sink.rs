//! Tier-06 step 10: throughput of `apply_invalidation` through the
//! `AriadneDbSink` against a stub `AriadneDb`. Plan target: ≥10K events/s.
//!
//! The bench drives `Invalidation::Modified` events for 10K pre-created
//! files inside a tempdir, so the sink's IO path (read + blake3) is on
//! the hot loop alongside the salsa input setter chain.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use ariadne_core::Invalidation;
use ariadne_core::WatcherSink;
use ariadne_salsa::AriadneDb;
use ariadne_watcher::AriadneDbSink;
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use tempfile::TempDir;

const N: usize = 10_000;

fn setup() -> (TempDir, Vec<PathBuf>) {
    let tmp = TempDir::new().unwrap();
    let mut paths = Vec::with_capacity(N);
    for i in 0..N {
        let p = tmp.path().join(format!("f{i:05}.rs"));
        std::fs::write(&p, format!("// file {i}").as_bytes()).unwrap();
        paths.push(p);
    }
    (tmp, paths)
}

fn bench_sink_throughput(c: &mut Criterion) {
    let (_tmp, paths) = setup();
    let mut group = c.benchmark_group("sink_apply_invalidation");
    group.throughput(Throughput::Elements(N as u64));
    group.sample_size(10);
    group.bench_function("10k_modified_events", |b| {
        b.iter_batched(
            || {
                let db = Arc::new(Mutex::new(AriadneDb::new()));
                (AriadneDbSink::new(db), paths.clone())
            },
            |(mut sink, paths)| {
                for p in paths {
                    sink.apply_invalidation(black_box(&Invalidation::Modified { path: p }));
                }
            },
            criterion::BatchSize::SmallInput,
        );
    });
    group.finish();
}

criterion_group!(benches, bench_sink_throughput);
criterion_main!(benches);
