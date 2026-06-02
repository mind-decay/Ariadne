//! Block-C analytics queries: `hotspots`, `complexity`, `co_change`
//! (tier-15b).
//!
//! Each handler reads the warm catalog's Git-history vectors (`churn`,
//! `co_change`, `symbol_churn`) and per-symbol `complexity` from RAM, runs the
//! pure tier-13 `ariadne-graph` use case (or, for `complexity`, the handler-
//! side fold deferred there by tier-13 D2), and projects the result into the
//! core wire DTO. The logic is byte-identical to the cold MCP `tools::*`
//! handlers, so daemon-served and cold JSON match
//! [src: crates/ariadne-graph/src/hotspot.rs:102-150; co_change.rs:74-95].

use std::collections::BTreeMap;

use ariadne_core::{
    CoChangeEdge, CoChangeReport, ComplexityReport, ComplexityRow, DaemonResponse, Grain,
    HotspotReport, HotspotRow, SymbolId,
};
use ariadne_graph::{
    CoChangeConfig, HotspotGrain, HotspotReport as GraphHotspots, co_change_report, file_hotspots,
    symbol_hotspots,
};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

/// Whether `path` is in scope for an optional path prefix (`None` = all).
fn in_scope(path: &str, prefix: Option<&str>) -> bool {
    prefix.is_none_or(|p| path.starts_with(p))
}

/// Churn × complexity hotspots at `grain`, filtered by `prefix`.
pub(crate) fn hotspots(cat: &WarmCatalog, prefix: Option<&str>, grain: Grain) -> DaemonResponse {
    let report = match grain {
        Grain::File => {
            let mut file_complexity: BTreeMap<String, u32> = BTreeMap::new();
            for meta in cat.symbols.values() {
                if let Some(path) = cat.path_of(meta.file) {
                    *file_complexity.entry(path.to_owned()).or_insert(0) += meta.complexity;
                }
            }
            file_hotspots(&cat.churn, &file_complexity)
        }
        Grain::Symbol => {
            let symbol_complexity: BTreeMap<SymbolId, u32> = cat
                .symbols
                .iter()
                .map(|(id, m)| (*id, m.complexity))
                .collect();
            symbol_hotspots(&cat.symbol_churn, &symbol_complexity)
        }
    };
    DaemonResponse::Hotspots(HotspotReport {
        rows: project_hotspots(cat, report, prefix),
    })
}

/// Project a graph hotspot report into wire rows, dropping out-of-scope units.
fn project_hotspots(
    cat: &WarmCatalog,
    report: GraphHotspots,
    prefix: Option<&str>,
) -> Vec<HotspotRow> {
    report
        .entries
        .into_iter()
        .filter_map(|e| match e.grain {
            HotspotGrain::File { path } => in_scope(&path, prefix).then_some(HotspotRow {
                file: path,
                symbol: None,
                churn: e.churn,
                complexity: e.complexity,
                score: e.score,
            }),
            HotspotGrain::Symbol { symbol } => {
                let sym = summarize(cat, symbol);
                in_scope(&sym.file, prefix).then_some(HotspotRow {
                    file: String::new(),
                    symbol: Some(sym),
                    churn: e.churn,
                    complexity: e.complexity,
                    score: e.score,
                })
            }
        })
        .collect()
}

/// `McCabe` complexity ranking at `grain`, filtered by `prefix`. File grain sums
/// each file's symbol complexity (tier-13 D2 defers this fold to the root);
/// symbol grain carries each symbol's own value. Both rank complexity
/// descending, ties broken by key ascending.
pub(crate) fn complexity(cat: &WarmCatalog, prefix: Option<&str>, grain: Grain) -> DaemonResponse {
    let mut rows = match grain {
        Grain::File => {
            let mut by_file: BTreeMap<String, u32> = BTreeMap::new();
            for meta in cat.symbols.values() {
                let Some(path) = cat.path_of(meta.file) else {
                    continue;
                };
                if in_scope(path, prefix) {
                    *by_file.entry(path.to_owned()).or_insert(0) += meta.complexity;
                }
            }
            by_file
                .into_iter()
                .map(|(file, complexity)| ComplexityRow {
                    file,
                    symbol: None,
                    complexity,
                })
                .collect::<Vec<_>>()
        }
        Grain::Symbol => cat
            .symbols
            .iter()
            .filter(|(_, meta)| in_scope(cat.path_of(meta.file).unwrap_or(""), prefix))
            .map(|(id, meta)| ComplexityRow {
                file: String::new(),
                symbol: Some(summarize(cat, *id)),
                complexity: meta.complexity,
            })
            .collect::<Vec<_>>(),
    };
    rows.sort_by(|a, b| {
        b.complexity
            .cmp(&a.complexity)
            .then_with(|| key(a).cmp(&key(b)))
    });
    DaemonResponse::Complexity(ComplexityReport { rows })
}

/// Sort key for a complexity row: the file path (file grain) or the symbol id
/// rendered as a key (symbol grain). Ties after complexity break ascending.
fn key(row: &ComplexityRow) -> (String, u64) {
    (row.file.clone(), row.symbol.as_ref().map_or(0, |s| s.id))
}

/// Logical-coupling edges honoring the code-maat filters, filtered by
/// `prefix` (an edge is kept when either endpoint is in scope).
pub(crate) fn co_change(
    cat: &WarmCatalog,
    prefix: Option<&str>,
    min_revs: Option<u32>,
    min_shared_commits: Option<u32>,
    min_degree: Option<f32>,
) -> DaemonResponse {
    let cfg = resolve_cfg(min_revs, min_shared_commits, min_degree);
    let report = co_change_report(&cat.churn, &cat.co_change, &cfg);
    let edges = report
        .edges
        .into_iter()
        .filter(|e| in_scope(&e.a, prefix) || in_scope(&e.b, prefix))
        .map(|e| CoChangeEdge {
            a: e.a,
            b: e.b,
            shared_commits: e.shared_commits,
            degree: e.degree,
        })
        .collect();
    DaemonResponse::CoChange(CoChangeReport { edges })
}

/// Resolve the three optional thresholds against `CoChangeConfig::default()`.
fn resolve_cfg(
    min_revs: Option<u32>,
    min_shared_commits: Option<u32>,
    min_degree: Option<f32>,
) -> CoChangeConfig {
    let d = CoChangeConfig::default();
    CoChangeConfig {
        min_revs: min_revs.unwrap_or(d.min_revs),
        min_shared_commits: min_shared_commits.unwrap_or(d.min_shared_commits),
        min_degree: min_degree.unwrap_or(d.min_degree),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ariadne_core::{
        Changeset, CoChangePair, FileChurn, FileId, FileRecord, Lang, Span, Storage, SymbolChurn,
        SymbolRecord, Visibility, WriteTxn,
    };
    use ariadne_storage::RedbStorage;

    fn fid(n: u32) -> FileId {
        FileId::new(n).expect("nonzero file id")
    }

    fn sid(n: u64) -> SymbolId {
        SymbolId::new(n).expect("nonzero symbol id")
    }

    /// Seed the Block-C analytics fixture (mirrors the cold MCP
    /// `seed_analytics_project`) and build a warm catalog over it.
    fn warm() -> WarmCatalog {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage =
            RedbStorage::open(&dir.path().join(".ariadne").join("index.redb")).expect("open redb");
        let mut cs = Changeset::new();
        for (id, path) in [(1u32, "src/alpha.rs"), (2, "src/beta.rs")] {
            cs = cs.upsert_file(
                fid(id),
                FileRecord {
                    path: path.into(),
                    lang: Lang::Rust,
                    size: 128,
                    blake3: [u8::try_from(id).expect("fits u8"); 32],
                    mtime_ns: i128::from(id),
                },
            );
        }
        for (id, name, file, complexity) in
            [(1u64, "crate::alpha", 1u32, 7u32), (2, "crate::beta", 2, 3)]
        {
            cs = cs.upsert_symbol(
                sid(id),
                SymbolRecord {
                    canonical_name: name.into(),
                    kind: "function".into(),
                    defining_file: fid(file),
                    defining_span: Span {
                        file: fid(file),
                        byte_start: 0,
                        byte_end: 64,
                    },
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                    complexity,
                },
            );
        }
        storage
            .begin_write()
            .expect("begin")
            .apply(&cs)
            .expect("apply");
        storage
            .replace_history(
                &[
                    FileChurn {
                        path: "src/alpha.rs".into(),
                        commits: 9,
                        author_keys: vec![[1u8; 8]],
                        last_changed_ns: 100,
                    },
                    FileChurn {
                        path: "src/beta.rs".into(),
                        commits: 4,
                        author_keys: vec![[1u8; 8], [2u8; 8]],
                        last_changed_ns: 200,
                    },
                ],
                &[CoChangePair {
                    a: "src/alpha.rs".into(),
                    b: "src/beta.rs".into(),
                    count: 3,
                }],
            )
            .expect("replace history");
        storage
            .replace_symbol_churn(&[
                SymbolChurn {
                    symbol: sid(1),
                    commits: 5,
                },
                SymbolChurn {
                    symbol: sid(2),
                    commits: 2,
                },
            ])
            .expect("replace symbol churn");
        WarmCatalog::build(&storage, "/p".to_owned()).expect("build")
    }

    /// File-grain hotspots rank alpha (churn 9 × Σ-complexity 7, score 1.0)
    /// above beta, reading churn + per-symbol complexity from the warm catalog.
    #[test]
    fn hotspots_file_grain_ranks_alpha_first() {
        let DaemonResponse::Hotspots(report) = hotspots(&warm(), None, Grain::File) else {
            panic!("expected Hotspots");
        };
        assert_eq!(report.rows[0].file, "src/alpha.rs");
        assert!((report.rows[0].score - 1.0).abs() < f32::EPSILON);
        assert_eq!(report.rows[1].file, "src/beta.rs");
    }

    /// Symbol-grain complexity ranks each symbol's own `McCabe`, descending.
    #[test]
    fn complexity_symbol_grain_ranks_descending() {
        let DaemonResponse::Complexity(report) = complexity(&warm(), None, Grain::Symbol) else {
            panic!("expected Complexity");
        };
        let names: Vec<(&str, u32)> = report
            .rows
            .iter()
            .map(|r| {
                (
                    r.symbol.as_ref().expect("symbol").name.as_str(),
                    r.complexity,
                )
            })
            .collect();
        assert_eq!(names, vec![("crate::alpha", 7), ("crate::beta", 3)]);
    }

    /// `co_change` honors the thresholds: defaults exclude the fixture pair
    /// (beta has 4 < 5 revisions), lowered thresholds surface the alpha↔beta
    /// edge with the expected support.
    #[test]
    fn co_change_honors_thresholds() {
        let cat = warm();
        let DaemonResponse::CoChange(empty) = co_change(&cat, None, None, None, None) else {
            panic!("expected CoChange");
        };
        assert!(empty.edges.is_empty(), "defaults exclude the fixture pair");

        let DaemonResponse::CoChange(report) = co_change(&cat, None, Some(1), Some(1), Some(0.0))
        else {
            panic!("expected CoChange");
        };
        assert_eq!(report.edges.len(), 1);
        assert_eq!(report.edges[0].a, "src/alpha.rs");
        assert_eq!(report.edges[0].b, "src/beta.rs");
        assert_eq!(report.edges[0].shared_commits, 3);
    }
}
