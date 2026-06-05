//! Criterion micro-bench for the Salsa edit cycle.
//!
//! Tier-04 plan step 10: a single-file edit must re-derive `symbols_for_file`
//! for the edited file within the budget while unrelated files hit the
//! Salsa cache. The fixture is intentionally small (1000 files) — tier-10
//! re-measures against the 100k-file SLO [src:
//! .claude/plans/ariadne-core/tier-04-salsa.md step 10].

use std::hint::black_box;

use ariadne_salsa::{
    AriadneDb, FileContentInput, FileMetadataInput, ProjectConfigInput, SyntacticFactsInput,
    SyntacticFactsRaw, symbols_for_file,
};
use criterion::{Criterion, criterion_group, criterion_main};
use salsa::{Durability, Setter};

const FILE_COUNT: usize = 1000;

struct Fixture {
    content: FileContentInput,
    facts: SyntacticFactsInput,
}

fn seed(db: &AriadneDb, i: usize) -> Fixture {
    let path = format!("/repo/src/f{i}.rs");
    let content = FileContentInput::builder(path, seed_bytes(i), seed_hash(i))
        .durability(Durability::LOW)
        .new(db);
    let _ = FileMetadataInput::builder("rust".into(), 0, 0)
        .durability(Durability::LOW)
        .new(db);
    let facts = SyntacticFactsInput::builder(SyntacticFactsRaw::default())
        .durability(Durability::LOW)
        .new(db);
    Fixture { content, facts }
}

fn bench_single_file_edit(c: &mut Criterion) {
    let mut db = AriadneDb::new();
    let _cfg = ProjectConfigInput::new(
        &db,
        "/repo".into(),
        vec!["rust".into()],
        Vec::<String>::new(),
    );
    let fixtures: Vec<Fixture> = (0..FILE_COUNT).map(|i| seed(&db, i)).collect();

    // Warm the cache so the unrelated-file lookups become cache hits.
    for f in &fixtures {
        black_box(symbols_for_file(&db, f.content, f.facts));
    }

    c.bench_function("edit_target_file_symbols", |b| {
        let mut tick: u64 = 0;
        b.iter(|| {
            tick = tick.wrapping_add(1);
            let target = &fixtures[0];
            let mut payload = seed_bytes(0);
            payload.push(b'/');
            payload.extend_from_slice(tick.to_string().as_bytes());
            target
                .content
                .set_content(&mut db)
                .with_durability(Durability::LOW)
                .to(payload);
            black_box(symbols_for_file(&db, target.content, target.facts));
        });
    });

    c.bench_function("untouched_file_symbols_cache_hit", |b| {
        b.iter(|| {
            for f in fixtures.iter().skip(1).take(8) {
                black_box(symbols_for_file(&db, f.content, f.facts));
            }
        });
    });
}

fn seed_bytes(i: usize) -> Vec<u8> {
    format!("fn item_{i}() {{}}\n").into_bytes()
}

fn seed_hash(i: usize) -> [u8; 32] {
    let mut h = [0u8; 32];
    h[..8].copy_from_slice(&(i as u64).to_be_bytes());
    h
}

criterion_group!(benches, bench_single_file_edit);
criterion_main!(benches);
