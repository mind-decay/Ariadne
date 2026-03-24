mod helpers;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::hash::hash_content;
use ariadne_graph::parser::ParserRegistry;

fn bench_parse_typescript(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    // Generate a TS file with 50 imports
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!("import {{ item_{} }} from './module_{}';\n", i, i));
    }
    source.push_str("export const result = 42;\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("ts").unwrap();

    c.bench_function("parse_typescript_50_imports", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser, "ts").unwrap();
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
            registry.parse_source(&bytes, parser, "go").unwrap();
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
            registry.parse_source(&bytes, parser, "py").unwrap();
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

fn bench_clustering_3000(c: &mut Criterion) {
    use ariadne_graph::cluster::assign_clusters;
    use ariadne_graph::model::*;
    use std::collections::BTreeMap;

    // Build a synthetic ProjectGraph with 3000 nodes across 100 directories
    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();

    for i in 0..3000 {
        let dir = format!("dir_{}", i % 100);
        let path = CanonicalPath::new(format!("{}/file_{}.ts", dir, i));
        nodes.insert(
            path.clone(),
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                fsd_layer: None,
                arch_depth: 0,
                lines: 50,
                hash: ContentHash::new(format!("{:016x}", i)),
                exports: vec![],
                cluster: ClusterId::new(String::new()),
                symbols: Vec::new(),
            },
        );

        // Add some edges
        if i > 0 {
            let from = CanonicalPath::new(format!("dir_{}/file_{}.ts", i % 100, i));
            let to = CanonicalPath::new(format!("dir_{}/file_{}.ts", (i - 1) % 100, i - 1));
            edges.push(Edge {
                from,
                to,
                edge_type: EdgeType::Imports,
                symbols: vec![],
            });
        }
    }

    let graph = ProjectGraph { nodes, edges };

    c.bench_function("clustering_3000_nodes", |b| {
        b.iter(|| {
            assign_clusters(&graph);
        });
    });
}

fn bench_serialization_3000(c: &mut Criterion) {
    use ariadne_graph::serial::{GraphOutput, NodeOutput};
    use std::collections::BTreeMap;

    // Build a synthetic GraphOutput with 3000 nodes
    let mut nodes = BTreeMap::new();
    for i in 0..3000 {
        nodes.insert(
            format!("dir_{}/file_{}.ts", i % 100, i),
            NodeOutput {
                file_type: "source".to_string(),
                layer: "unknown".to_string(),
                fsd_layer: None,
                arch_depth: 0,
                lines: 50,
                hash: format!("{:016x}", i),
                exports: vec![format!("item_{}", i)],
                cluster: format!("dir_{}", i % 100),
                symbols: Vec::new(),
            },
        );
    }

    let mut edges = Vec::new();
    for i in 1..3000 {
        edges.push((
            format!("dir_{}/file_{}.ts", i % 100, i),
            format!("dir_{}/file_{}.ts", (i - 1) % 100, i - 1),
            "imports".to_string(),
            vec![format!("item_{}", i - 1)],
        ));
    }

    let graph_output = GraphOutput {
        version: 1,
        project_root: "/tmp/test".to_string(),
        node_count: 3000,
        edge_count: edges.len(),
        nodes,
        edges,
        generated: None,
    };

    c.bench_function("serialization_3000_nodes", |b| {
        b.iter(|| {
            serde_json::to_string(&graph_output).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_parse_typescript,
    bench_parse_go,
    bench_parse_python,
    bench_hash_1mb,
    bench_clustering_3000,
    bench_serialization_3000
);
criterion_main!(benches);
