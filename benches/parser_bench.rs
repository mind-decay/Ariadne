#[allow(dead_code)]
mod helpers;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::hash::hash_content;
use ariadne_graph::parser::ParserRegistry;

fn bench_parse_typescript(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    // Generate a TS file with 50 imports
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!(
            "import {{ item_{} }} from './module_{}';\n",
            i, i
        ));
    }
    source.push_str("export const result = 42;\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("ts").unwrap();

    c.bench_function("parse_typescript_50_imports", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser).unwrap();
        });
    });
}

fn bench_parse_go(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    let mut source = String::from("package main\n\nimport (\n");
    for i in 0..30 {
        source.push_str(&format!("\t\"github.com/example/pkg_{}\"\n", i));
    }
    source.push_str(")\n\nfunc main() {}\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("go").unwrap();

    c.bench_function("parse_go_30_imports", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser).unwrap();
        });
    });
}

fn bench_parse_python(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    let mut source = String::new();
    for i in 0..40 {
        source.push_str(&format!("from module_{} import item_{}\n", i, i));
    }
    source.push_str("result = 42\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("py").unwrap();

    c.bench_function("parse_python_40_imports", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser).unwrap();
        });
    });
}

fn bench_hash_1mb(c: &mut Criterion) {
    let data = vec![b'x'; 1_048_576]; // 1MB of data

    c.bench_function("hash_1mb", |b| {
        b.iter(|| {
            hash_content(&data);
        });
    });
}

criterion_group!(
    benches,
    bench_parse_typescript,
    bench_parse_go,
    bench_parse_python,
    bench_hash_1mb
);
criterion_main!(benches);
