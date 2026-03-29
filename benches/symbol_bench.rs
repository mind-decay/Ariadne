mod helpers;

use std::collections::BTreeMap;

use criterion::{criterion_group, criterion_main, Criterion};

use ariadne_graph::algo::callgraph::CallGraph;
use ariadne_graph::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use ariadne_graph::model::symbol_index::SymbolIndex;
use ariadne_graph::model::*;
use ariadne_graph::parser::ParserRegistry;

/// Build a synthetic graph with symbols for benchmarking.
fn build_graph_with_symbols(node_count: usize, symbols_per_file: usize) -> ProjectGraph {
    let mut nodes = BTreeMap::new();
    let mut edges = Vec::new();

    for i in 0..node_count {
        let path = CanonicalPath::new(format!("src/module_{}/file_{}.ts", i % 30, i));

        let mut symbols = Vec::with_capacity(symbols_per_file);
        for s in 0..symbols_per_file {
            symbols.push(SymbolDef {
                name: format!("symbol_{}_{}", i, s),
                kind: match s % 5 {
                    0 => SymbolKind::Function,
                    1 => SymbolKind::Method,
                    2 => SymbolKind::Class,
                    3 => SymbolKind::Interface,
                    _ => SymbolKind::Const,
                },
                visibility: if s % 3 == 0 {
                    Visibility::Public
                } else {
                    Visibility::Private
                },
                span: LineSpan {
                    start: (s * 10) as u32 + 1,
                    end: (s * 10 + 9) as u32 + 1,
                },
                signature: Some(format!("function symbol_{}_{}(arg: Type): Result", i, s)),
                parent: if s % 5 == 1 {
                    Some(format!("Class_{}", i))
                } else {
                    None
                },
            });
        }
        symbols.sort();

        let export_symbols: Vec<Symbol> = (0..symbols_per_file.min(5))
            .map(|s| Symbol::new(format!("symbol_{}_{}", i, s)))
            .collect();

        nodes.insert(
            path,
            Node {
                file_type: FileType::Source,
                layer: ArchLayer::Unknown,
                fsd_layer: None,
                arch_depth: (i % 5) as u32,
                lines: (symbols_per_file * 10) as u32,
                hash: ContentHash::new(format!("{:016x}", i)),
                exports: export_symbols,
                cluster: ClusterId::new(format!("module_{}", i % 30)),
                symbols,
            },
        );

        // Add edges with symbol references
        if i > 0 {
            let from = CanonicalPath::new(format!("src/module_{}/file_{}.ts", i % 30, i));
            let to = CanonicalPath::new(format!(
                "src/module_{}/file_{}.ts",
                (i - 1) % 30,
                i - 1
            ));
            edges.push(Edge {
                from,
                to,
                edge_type: EdgeType::Imports,
                symbols: vec![Symbol::new(format!("symbol_{}_0", i - 1))],
            });
        }
    }

    ProjectGraph { nodes, edges }
}

// --- SymbolIndex benchmarks ---

fn bench_symbol_index_build(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);

    c.bench_function("symbol_index_build_3000x20", |b| {
        b.iter(|| SymbolIndex::build(&graph.nodes, &graph.edges))
    });
}

fn bench_symbol_search(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);

    c.bench_function("symbol_search_3000x20", |b| {
        b.iter(|| index.search("symbol_150", None))
    });
}

fn bench_symbol_search_with_kind(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);

    c.bench_function("symbol_search_kind_filter_3000x20", |b| {
        b.iter(|| index.search("symbol", Some(SymbolKind::Function)))
    });
}

fn bench_symbols_for_file(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);
    let path = CanonicalPath::new("src/module_0/file_0.ts");

    c.bench_function("symbols_for_file_lookup", |b| {
        b.iter(|| index.symbols_for_file(&path))
    });
}

// --- CallGraph benchmarks ---

fn bench_callgraph_build(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);

    c.bench_function("callgraph_build_3000", |b| {
        b.iter(|| CallGraph::build(&graph.edges, &index))
    });
}

fn bench_callers_of(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);
    let call_graph = CallGraph::build(&graph.edges, &index);
    let path = CanonicalPath::new("src/module_0/file_0.ts");

    c.bench_function("callers_of_lookup", |b| {
        b.iter(|| call_graph.callers_of(&path, "symbol_0_0"))
    });
}

fn bench_callees_of(c: &mut Criterion) {
    let graph = build_graph_with_symbols(3000, 20);
    let index = SymbolIndex::build(&graph.nodes, &graph.edges);
    let call_graph = CallGraph::build(&graph.edges, &index);
    let path = CanonicalPath::new("src/module_0/file_30.ts");

    c.bench_function("callees_of_lookup", |b| {
        b.iter(|| call_graph.callees_of(&path, "symbol_30_0"))
    });
}

// --- Symbol extraction benchmark (uses real parser) ---

fn bench_symbol_extraction_typescript(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    // Generate a TS file with functions, classes, interfaces, consts
    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!(
            "export function handler_{}(req: Request): Response {{ return new Response(); }}\n",
            i
        ));
    }
    for i in 0..10 {
        source.push_str(&format!(
            "export class Service_{} {{ method_a() {{}} method_b() {{}} }}\n",
            i
        ));
    }
    for i in 0..10 {
        source.push_str(&format!(
            "export interface IHandler_{} {{ handle(): void; }}\n",
            i
        ));
    }
    source.push_str("export const MAX_RETRIES = 3;\nexport const TIMEOUT_MS = 5000;\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("ts").unwrap();

    c.bench_function("symbol_extraction_typescript_72_symbols", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser, "ts", &CanonicalPath::new("bench.ts")).unwrap();
        });
    });
}

fn bench_symbol_extraction_rust(c: &mut Criterion) {
    let registry = ParserRegistry::with_tier1();

    let mut source = String::new();
    for i in 0..50 {
        source.push_str(&format!(
            "pub fn process_{}(input: &str) -> Result<(), Error> {{ Ok(()) }}\n",
            i
        ));
    }
    for i in 0..10 {
        source.push_str(&format!(
            "pub struct Config_{} {{ pub field: String }}\n",
            i
        ));
    }
    for i in 0..5 {
        source.push_str(&format!(
            "pub trait Handler_{} {{ fn handle(&self); }}\n",
            i
        ));
    }
    source.push_str("pub const MAX_SIZE: usize = 1024;\n");
    let bytes = source.into_bytes();

    let parser = registry.parser_for("rs").unwrap();

    c.bench_function("symbol_extraction_rust_66_symbols", |b| {
        b.iter(|| {
            registry.parse_source(&bytes, parser, "rs", &CanonicalPath::new("bench.rs")).unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_symbol_index_build,
    bench_symbol_search,
    bench_symbol_search_with_kind,
    bench_symbols_for_file,
    bench_callgraph_build,
    bench_callers_of,
    bench_callees_of,
    bench_symbol_extraction_typescript,
    bench_symbol_extraction_rust,
);
criterion_main!(benches);
