//! Criterion benches for `WriteTxn::apply` at 1K / 10K / 100K edges and a
//! one-shot file-size + RSS memory probe at 10K edges. No CI gate this tier
//! [src: .claude/plans/ariadne-core/tier-02-storage.md step 10].

use ariadne_core::{
    Changeset, EdgeKey, EdgeKind, EdgeRecord, FileId, FileRecord, Lang, Span, Storage, SymbolId,
    WriteTxn,
};
use ariadne_storage::RedbStorage;
use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use sysinfo::{System, get_current_pid};
use tempfile::TempDir;

const FILES: u32 = 50;

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn build_changeset(edges: usize) -> Changeset {
    let mut cs = Changeset::new();
    for f in 1..=FILES {
        cs = cs.upsert_file(
            FileId::new(f).expect("nonzero"),
            FileRecord {
                path: format!("f{f}.rs"),
                lang: Lang::Rust,
                size: 0,
                blake3: [0u8; 32],
                mtime_ns: 0,
            },
        );
    }
    let kinds = [EdgeKind::Defines, EdgeKind::References, EdgeKind::Imports];
    let mut rng: u64 = 0xDEAD_BEEF_CAFE_F00D;
    for i in 0..edges {
        let src_n = (xorshift(&mut rng) % 1_000_000).max(1);
        let dst_n = (xorshift(&mut rng) % 1_000_000).max(1);
        let kind = kinds[i % 3];
        let file_n = u32::try_from(i % FILES as usize).expect("fits") + 1;
        let src = SymbolId::new(src_n).expect("nonzero");
        let dst = SymbolId::new(dst_n).expect("nonzero");
        let file = FileId::new(file_n).expect("nonzero");
        cs = cs.add_edge(
            EdgeKey { src, kind, dst },
            EdgeRecord {
                source_span: Span {
                    file,
                    byte_start: 0,
                    byte_end: 1,
                },
                evidence_lang: Lang::Rust,
                weight: 1,
            },
        );
    }
    cs
}

fn fresh() -> (RedbStorage, TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("index.redb");
    let storage = RedbStorage::open(&path).expect("open");
    (storage, dir, path)
}

fn bench_apply(c: &mut Criterion) {
    let mut group = c.benchmark_group("apply");
    for &n in &[1_000usize, 10_000, 100_000] {
        group.throughput(Throughput::Elements(n as u64));
        group.sample_size(if n >= 100_000 { 10 } else { 30 });
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (storage, dir, _path) = fresh();
                    let cs = build_changeset(n);
                    (storage, dir, cs)
                },
                |(storage, _dir, cs)| {
                    let txn = storage.begin_write().expect("begin");
                    txn.apply(&cs).expect("apply");
                },
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

fn memory_probe(_: &mut Criterion) {
    let n = 10_000usize;
    let (storage, _dir, path) = fresh();
    let cs = build_changeset(n);
    let mut sys = System::new_all();
    sys.refresh_all();
    let pid = get_current_pid().expect("pid");
    let pre = sys.process(pid).map_or(0, sysinfo::Process::memory);
    let txn = storage.begin_write().expect("begin");
    txn.apply(&cs).expect("apply");
    sys.refresh_all();
    let post = sys.process(pid).map_or(0, sysinfo::Process::memory);
    let file_size = std::fs::metadata(&path).map_or(0, |m| m.len());
    println!(
        "[memory_probe edges={n}] file_size_bytes={file_size} rss_pre={pre} rss_post={post} rss_delta={}",
        post.saturating_sub(pre)
    );
}

criterion_group!(benches, bench_apply, memory_probe);
criterion_main!(benches);
