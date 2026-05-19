//! Tier-03 step 11 — criterion perf gates.
//!
//! Targets [src: docs/adr/0005-tier-03-parse-slo-baseline.md `<decision>`]:
//! - cold parse 10 MB jQuery 3.7.1 fixture ≤ 700 ms (p50, Apple Silicon)
//! - single-token incremental re-parse ≤ 5 ms
//!
//! Tier-10 e2e re-verifies cold cost on real OSS repos against the 60 s
//! plan budget. Locally run with `cargo bench -p ariadne-parser`.

use ariadne_core::Lang;
use ariadne_parser::{ParserRegistry, TreeSitterParser};
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use tree_sitter::{InputEdit, Point};

const TEN_MB: usize = 10 * 1024 * 1024;
const JQUERY_FIXTURE: &[u8] = include_bytes!("../fixtures/javascript/jquery.js");

fn build_payload() -> Vec<u8> {
    // Use a real-world hand-written JS source (jQuery 3.7.1, MIT) replicated
    // until the buffer crosses 10 MB. Synthetic regular structures triggered
    // a pathologically dense AST shape that pushed cold-parse cost ~10x off
    // the realistic baseline; jQuery's varied syntax matches what we expect
    // out of OSS workloads on tier-10.
    let mut out = Vec::with_capacity(TEN_MB + JQUERY_FIXTURE.len());
    while out.len() < TEN_MB {
        out.extend_from_slice(JQUERY_FIXTURE);
    }
    out.truncate(TEN_MB);
    out
}

fn bench_cold(c: &mut Criterion) {
    let registry = ParserRegistry::new();
    let payload = build_payload();
    let mut group = c.benchmark_group("parse_cold");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));
    group.bench_function("javascript_10mb", |b| {
        b.iter(|| {
            let mut parser = TreeSitterParser::for_lang(Lang::JavaScript, &registry).unwrap();
            let tree = parser
                .parse_file(black_box(&payload), None, &[])
                .expect("parse ok");
            black_box(tree);
        });
    });
    group.finish();
}

fn bench_incremental(c: &mut Criterion) {
    let registry = ParserRegistry::new();
    let mut parser = TreeSitterParser::for_lang(Lang::JavaScript, &registry).unwrap();
    let original = build_payload();
    let cold_tree = parser.parse_file(&original, None, &[]).unwrap();

    let edit_pos = TEN_MB / 2;
    let mut edited = original.clone();
    edited.insert(edit_pos, b'1');
    let row = original[..edit_pos]
        .iter()
        .fold(0usize, |a, b| a + usize::from(*b == b'\n'));
    let line_start = original[..edit_pos]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |i| i + 1);
    let column = edit_pos - line_start;
    let edit = InputEdit {
        start_byte: edit_pos,
        old_end_byte: edit_pos,
        new_end_byte: edit_pos + 1,
        start_position: Point { row, column },
        old_end_position: Point { row, column },
        new_end_position: Point {
            row,
            column: column + 1,
        },
    };

    let mut group = c.benchmark_group("parse_incremental");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("javascript_single_char_edit", |b| {
        b.iter(|| {
            let tree = parser
                .parse_file(black_box(&edited), Some(&cold_tree), &[edit])
                .expect("incremental parse");
            black_box(tree);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_cold, bench_incremental);
criterion_main!(benches);
