mod helpers;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::parser::ParserRegistry;
use ariadne_graph::pipeline::{BuildPipeline, FsReader, FsWalker, WalkConfig};
use ariadne_graph::serial::json::JsonSerializer;

fn make_pipeline() -> BuildPipeline {
    BuildPipeline::new(
        Box::new(FsWalker::new()),
        Box::new(FsReader::new()),
        ParserRegistry::with_tier1(),
        Box::new(JsonSerializer),
    )
}

fn bench_build_100(c: &mut Criterion) {
    let project = helpers::generate_synthetic_project(100, 10, 5, "typescript");
    let pipeline = make_pipeline();

    c.bench_function("build_100_files", |b| {
        b.iter(|| {
            let output_dir = tempfile::tempdir().unwrap();
            pipeline
                .run_with_output(
                    project.path(),
                    WalkConfig::default(),
                    Some(output_dir.path()),
                    false,
                    false,
                    false,
                )
                .unwrap();
        });
    });
}

fn bench_build_1000(c: &mut Criterion) {
    let project = helpers::generate_synthetic_project(1000, 50, 10, "typescript");
    let pipeline = make_pipeline();

    c.bench_function("build_1000_files", |b| {
        b.iter(|| {
            let output_dir = tempfile::tempdir().unwrap();
            pipeline
                .run_with_output(
                    project.path(),
                    WalkConfig::default(),
                    Some(output_dir.path()),
                    false,
                    false,
                    false,
                )
                .unwrap();
        });
    });
}

fn bench_build_3000(c: &mut Criterion) {
    let project = helpers::generate_synthetic_project(3000, 100, 10, "typescript");
    let pipeline = make_pipeline();

    let mut group = c.benchmark_group("build_3000");
    group.sample_size(10);
    group.bench_function("build_3000_files", |b| {
        b.iter(|| {
            let output_dir = tempfile::tempdir().unwrap();
            pipeline
                .run_with_output(
                    project.path(),
                    WalkConfig::default(),
                    Some(output_dir.path()),
                    false,
                    false,
                    false,
                )
                .unwrap();
        });
    });
    group.finish();
}

criterion_group!(benches, bench_build_100, bench_build_1000, bench_build_3000);
criterion_main!(benches);
