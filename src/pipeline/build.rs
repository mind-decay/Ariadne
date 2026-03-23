use std::collections::BTreeMap;

use crate::detect::{detect_file_type, infer_arch_layer};
use crate::diagnostic::DiagnosticCollector;
use crate::model::workspace::WorkspaceInfo;
use crate::model::*;
use crate::parser::{ImportKind, ParserRegistry};

use super::read::FileContent;
use super::resolve::resolve_import;
use super::ParsedFile;

/// Options for the resolve stage.
pub struct ResolveOptions<'a> {
    pub workspace: Option<&'a WorkspaceInfo>,
    pub case_insensitive: bool,
    pub is_fsd: bool,
    pub rust_crate_name: Option<&'a str>,
}

/// Build a ProjectGraph from parsed files.
/// This is the resolve_and_build stage per architecture.md.
pub fn resolve_and_build(
    parsed_files: &[ParsedFile],
    file_contents: &[FileContent],
    registry: &ParserRegistry,
    diagnostics: &DiagnosticCollector,
    resolve_opts: &ResolveOptions,
) -> ProjectGraph {
    let workspace = resolve_opts.workspace;
    let case_insensitive = resolve_opts.case_insensitive;
    let is_fsd = resolve_opts.is_fsd;
    let rust_crate_name = resolve_opts.rust_crate_name;
    // 1. Build FileSet from successfully-read files
    let file_set = FileSet::from_iter(file_contents.iter().map(|fc| fc.path.clone()));

    // 2-3. Detect FileType and ArchLayer per file, build nodes
    let mut nodes = BTreeMap::new();

    for fc in file_contents {
        let file_type = detect_file_type(&fc.path);
        let (layer, fsd_layer) = infer_arch_layer(&fc.path, is_fsd);

        nodes.insert(
            fc.path.clone(),
            Node {
                file_type,
                layer,
                fsd_layer,
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
            let mut export_symbols: Vec<Symbol> =
                pf.exports.iter().map(|e| Symbol::new(&e.name)).collect();
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
            // Rewrite crate-name imports (e.g. `use ariadne_graph::foo` → `crate::foo`)
            let rewritten;
            let import_ref = if let Some(crate_name) = rust_crate_name {
                if let Some(rest) = import.path.strip_prefix(crate_name) {
                    let new_path = if let Some(suffix) = rest.strip_prefix("::") {
                        format!("crate::{}", suffix)
                    } else if rest.is_empty() {
                        "crate".to_string()
                    } else {
                        import.path.clone() // not a match (e.g. `ariadne_graphx::`)
                    };
                    rewritten = crate::parser::RawImport {
                        path: new_path,
                        symbols: import.symbols.clone(),
                        is_type_only: import.is_type_only,
                        kind: import.kind.clone(),
                    };
                    &rewritten
                } else {
                    import
                }
            } else {
                import
            };

            let resolved = match resolve_import(
                import_ref,
                &pf.path,
                &file_set,
                resolver,
                diagnostics,
                workspace,
                case_insensitive,
            ) {
                Some(r) => r,
                None => continue,
            };

            // 5. Classify edge type
            let edge_type = classify_edge_type(
                source_file_type,
                nodes.get(&resolved).map(|n| n.file_type),
                import.is_type_only,
                &import.kind,
            );

            let symbols: Vec<Symbol> = import.symbols.iter().map(Symbol::new).collect();

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
                        kind: ImportKind::Regular,
                    };

                    if let Some(resolved) = resolve_import(
                        &re_export_import,
                        &pf.path,
                        &file_set,
                        resolver,
                        diagnostics,
                        workspace,
                        case_insensitive,
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
    import_kind: &ImportKind,
) -> EdgeType {
    // Markdown link references → references edge (D-069)
    if matches!(import_kind, ImportKind::Link) {
        return EdgeType::References;
    }

    // Test file importing source/typedef → tests edge
    if source_type == Some(FileType::Test)
        && matches!(
            target_type,
            Some(FileType::Source) | Some(FileType::TypeDef)
        )
    {
        return EdgeType::Tests;
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
    // Suffix patterns: foo.test.ts → foo.ts, foo_test.go → foo.go
    let test_patterns: &[(&str, &str)] = &[
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

    // Prefix patterns: test_auth.py → auth.py (same directory)
    let prefix_patterns: &[(&str, &str)] = &[
        ("test_", ".py"), // test_auth.py → auth.py
    ];

    // Build HashSet from existing edges for O(1) dedup lookup (fully borrowed)
    let existing_edges: std::collections::HashSet<(&CanonicalPath, &CanonicalPath, EdgeType)> =
        edges
            .iter()
            .map(|e| (&e.from, &e.to, e.edge_type))
            .collect();

    let mut new_edges: Vec<Edge> = Vec::new();
    // Track new edges in a HashSet too, avoiding O(n²) scan on new_edges
    let mut new_edge_keys: std::collections::HashSet<(CanonicalPath, CanonicalPath, EdgeType)> =
        std::collections::HashSet::new();

    for (path, node) in nodes {
        if node.file_type != FileType::Test {
            continue;
        }

        let path_str = path.as_str();
        let mut found = false;

        // Try suffix patterns (e.g., foo.test.ts → foo.ts)
        for (test_suffix, source_suffix) in test_patterns {
            if let Some(stem) = path_str.strip_suffix(test_suffix) {
                let source_path_str = format!("{}{}", stem, source_suffix);
                let source_path = CanonicalPath::new(&source_path_str);

                if file_set.contains(&source_path) {
                    if !existing_edges.contains(&(path, &source_path, EdgeType::Tests))
                        && !new_edge_keys.contains(&(
                            path.clone(),
                            source_path.clone(),
                            EdgeType::Tests,
                        ))
                    {
                        new_edge_keys.insert((path.clone(), source_path.clone(), EdgeType::Tests));
                        new_edges.push(Edge {
                            from: path.clone(),
                            to: source_path,
                            edge_type: EdgeType::Tests,
                            symbols: vec![],
                        });
                    }
                    found = true;
                    break;
                }
            }
        }

        if found {
            continue;
        }

        // Try prefix patterns (e.g., test_auth.py → auth.py in same dir)
        let file_name = path.file_name();
        for (test_prefix, source_ext) in prefix_patterns {
            if file_name.starts_with(test_prefix) && file_name.ends_with(source_ext) {
                let source_name = &file_name[test_prefix.len()..];
                let source_path = match path.parent() {
                    Some(parent) => CanonicalPath::new(format!("{}/{}", parent, source_name)),
                    None => CanonicalPath::new(source_name),
                };

                if file_set.contains(&source_path) {
                    if !existing_edges.contains(&(path, &source_path, EdgeType::Tests))
                        && !new_edge_keys.contains(&(
                            path.clone(),
                            source_path.clone(),
                            EdgeType::Tests,
                        ))
                    {
                        new_edge_keys.insert((path.clone(), source_path.clone(), EdgeType::Tests));
                        new_edges.push(Edge {
                            from: path.clone(),
                            to: source_path,
                            edge_type: EdgeType::Tests,
                            symbols: vec![],
                        });
                    }
                    break;
                }
            }
        }
    }

    edges.extend(new_edges);
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
