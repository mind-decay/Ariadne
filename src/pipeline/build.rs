use std::collections::BTreeMap;

use crate::detect::{detect_file_type, infer_arch_layer};
use crate::diagnostic::DiagnosticCollector;
use crate::model::*;
use crate::parser::ParserRegistry;

use super::read::FileContent;
use super::resolve::resolve_import;
use super::ParsedFile;

/// Build a ProjectGraph from parsed files.
/// This is the resolve_and_build stage per architecture.md.
pub fn resolve_and_build(
    parsed_files: &[ParsedFile],
    file_contents: &[FileContent],
    registry: &ParserRegistry,
    diagnostics: &DiagnosticCollector,
) -> ProjectGraph {
    // 1. Build FileSet from successfully-read files
    let file_set = FileSet::from_iter(file_contents.iter().map(|fc| fc.path.clone()));

    // 2-3. Detect FileType and ArchLayer per file, build nodes
    let mut nodes = BTreeMap::new();

    for fc in file_contents {
        let file_type = detect_file_type(&fc.path);
        let layer = infer_arch_layer(&fc.path);

        nodes.insert(
            fc.path.clone(),
            Node {
                file_type,
                layer,
                arch_depth: 0, // D-025: placeholder, computed in Phase 2
                lines: fc.lines,
                hash: fc.hash.clone(),
                exports: Vec::new(), // Populated below from parsed exports
                cluster: ClusterId::new(""), // Set after clustering
            },
        );
    }

    // Populate exports from parsed files
    for pf in parsed_files {
        if let Some(node) = nodes.get_mut(&pf.path) {
            let mut export_symbols: Vec<Symbol> = pf
                .exports
                .iter()
                .map(|e| Symbol::new(&e.name))
                .collect();
            export_symbols.sort();
            export_symbols.dedup();
            node.exports = export_symbols;
        }
    }

    // 4-7. Resolve imports and build edges
    let mut edges: Vec<Edge> = Vec::new();

    for pf in parsed_files {
        let source_file_type = nodes.get(&pf.path).map(|n| n.file_type);
        let extension = pf.path.extension().unwrap_or("");

        let resolver = match registry.resolver_for(extension) {
            Some(r) => r,
            None => continue,
        };

        // Process regular imports
        for import in &pf.imports {
            let resolved = match resolve_import(
                import,
                &pf.path,
                &file_set,
                resolver,
                diagnostics,
            ) {
                Some(r) => r,
                None => continue,
            };

            // 5. Classify edge type
            let edge_type = classify_edge_type(
                source_file_type,
                nodes.get(&resolved).map(|n| n.file_type),
                import.is_type_only,
                false, // not a re-export from import
            );

            let symbols: Vec<Symbol> = import.symbols.iter().map(|s| Symbol::new(s)).collect();

            edges.push(Edge {
                from: pf.path.clone(),
                to: resolved,
                edge_type,
                symbols,
            });
        }

        // Process re-exports (from RawExport with is_re_export=true and source)
        for export in &pf.exports {
            if export.is_re_export {
                if let Some(ref source_path) = export.source {
                    let re_export_import = crate::parser::RawImport {
                        path: source_path.clone(),
                        symbols: vec![export.name.clone()],
                        is_type_only: false,
                    };

                    if let Some(resolved) = resolve_import(
                        &re_export_import,
                        &pf.path,
                        &file_set,
                        resolver,
                        diagnostics,
                    ) {
                        edges.push(Edge {
                            from: pf.path.clone(),
                            to: resolved,
                            edge_type: EdgeType::ReExports,
                            symbols: vec![Symbol::new(&export.name)],
                        });
                    }
                }
            }
        }
    }

    // 6. Naming-convention test edge inference
    infer_test_edges_by_naming(&nodes, &file_set, &mut edges);

    // 7. Deduplicate edges: same (from, to, edge_type) → merge symbols
    deduplicate_edges(&mut edges);

    // Sort edges for determinism (D-006)
    edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then(a.to.cmp(&b.to))
            .then(a.edge_type.cmp(&b.edge_type))
    });

    ProjectGraph { nodes, edges }
}

/// Classify edge type based on source and target file types.
fn classify_edge_type(
    source_type: Option<FileType>,
    target_type: Option<FileType>,
    is_type_only: bool,
    _is_re_export: bool,
) -> EdgeType {
    // Test file importing source/typedef → tests edge
    if source_type == Some(FileType::Test) {
        if matches!(target_type, Some(FileType::Source) | Some(FileType::TypeDef)) {
            return EdgeType::Tests;
        }
    }

    // Type-only imports
    if is_type_only {
        return EdgeType::TypeImports;
    }

    EdgeType::Imports
}

/// Infer test edges by naming convention.
/// If `foo.test.ts` exists and `foo.ts` exists in same/parent directory → tests edge.
fn infer_test_edges_by_naming(
    nodes: &BTreeMap<CanonicalPath, Node>,
    file_set: &FileSet,
    edges: &mut Vec<Edge>,
) {
    // Collect test patterns
    let test_patterns = [
        (".test.ts", ".ts"),
        (".spec.ts", ".ts"),
        (".test.tsx", ".tsx"),
        (".spec.tsx", ".tsx"),
        (".test.js", ".js"),
        (".spec.js", ".js"),
        (".test.jsx", ".jsx"),
        (".spec.jsx", ".jsx"),
        ("_test.go", ".go"),
        ("_test.py", ".py"),
    ];

    for (path, node) in nodes {
        if node.file_type != FileType::Test {
            continue;
        }

        let path_str = path.as_str();

        for (test_suffix, source_suffix) in &test_patterns {
            if path_str.ends_with(test_suffix) {
                let source_path_str =
                    format!("{}{}", &path_str[..path_str.len() - test_suffix.len()], source_suffix);
                let source_path = CanonicalPath::new(&source_path_str);

                if file_set.contains(&source_path) {
                    // Check we don't already have this edge
                    let already_exists = edges.iter().any(|e| {
                        e.from == *path
                            && e.to == source_path
                            && e.edge_type == EdgeType::Tests
                    });
                    if !already_exists {
                        edges.push(Edge {
                            from: path.clone(),
                            to: source_path,
                            edge_type: EdgeType::Tests,
                            symbols: vec![],
                        });
                    }
                    break; // Found a match, don't try other patterns
                }
            }
        }
    }
}

/// Deduplicate edges: same (from, to, edge_type) → merge symbols (union, sorted).
fn deduplicate_edges(edges: &mut Vec<Edge>) {
    if edges.is_empty() {
        return;
    }

    // Sort by (from, to, edge_type) to group duplicates
    edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then(a.to.cmp(&b.to))
            .then(a.edge_type.cmp(&b.edge_type))
    });

    let mut deduped: Vec<Edge> = Vec::with_capacity(edges.len());

    for edge in edges.drain(..) {
        if let Some(last) = deduped.last_mut() {
            if last.from == edge.from && last.to == edge.to && last.edge_type == edge.edge_type {
                // Merge symbols
                for sym in edge.symbols {
                    if !last.symbols.contains(&sym) {
                        last.symbols.push(sym);
                    }
                }
                last.symbols.sort();
                continue;
            }
        }
        deduped.push(edge);
    }

    *edges = deduped;
}
