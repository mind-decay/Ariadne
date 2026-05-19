//! Criterion sanity bench for `IdEncode` round-trips
//! [src: .claude/plans/ariadne-core/tier-01-workspace.md step 8].

use ariadne_core::{IdEncode, SymbolId};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

fn bench_symbol_id_round_trip(c: &mut Criterion) {
    c.bench_function("symbol_id_round_trip_1m", |b| {
        b.iter(|| {
            let mut acc: u64 = 0;
            for v in 1u64..=1_000_000 {
                let id = SymbolId::new(v).expect("non-zero by construction");
                let bytes = id.to_bytes();
                let decoded = SymbolId::from_bytes(black_box(bytes)).expect("round-trip is total");
                acc = acc.wrapping_add(decoded.get());
            }
            black_box(acc)
        });
    });
}

criterion_group!(benches, bench_symbol_id_round_trip);
criterion_main!(benches);
