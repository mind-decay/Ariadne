//! Shared test helpers for the per-language ingest goldens.
//!
//! The six non-Rust SCIP indexers (`scip-typescript`, `scip-python`,
//! `scip-java`, `scip-clang`, `scip-dotnet`, `lsif-go` + `scip` CLI) are
//! not on PATH in the build environment that ships tier-05 (see
//! `tier-05-scip-ingest.md` deviations). Per-lang goldens therefore do
//! NOT shell out: they synthesize a minimal `proto::Index` per language,
//! prost-encode it, then exercise the public surface (the `parse` free
//! function and `normalize_scip_symbol`) on the decoded bytes — which
//! is exactly the contract `ScipDocInput` consumers (tier-04+ salsa)
//! see in production. The driver `run()` subprocess paths are covered
//! separately by `tests/ingest_plan.rs` via injected stub drivers, and
//! by the rust-analyzer round-trip fixture (`tests/roundtrip.rs`).

#![allow(dead_code)] // each integration test only pulls a subset of helpers.

use std::collections::BTreeMap;

use ariadne_core::Lang;
use ariadne_scip::{CanonicalSymbol, ScipDoc, ScipError, normalize_scip_symbol, parse, proto};
use prost::Message as _;
use std::fmt::Write as _;

/// Per-symbol blueprint for the synthetic index.
pub struct SymBp {
    pub raw: &'static str,
    pub occurrences: u32,
    pub relationships: u32,
}

/// Build a deterministic `proto::Index` for a language. One Document is
/// emitted with the requested symbols + per-symbol occurrence/relationship
/// counts; bytes round-trip through prost so consumers see exactly the
/// same wire format an external indexer would emit.
pub fn synth_bytes(
    tool: &str,
    relative_path: &str,
    document_language: &str,
    symbols: &[SymBp],
) -> Vec<u8> {
    let scip_symbols: Vec<proto::SymbolInformation> = symbols
        .iter()
        .map(|sym| proto::SymbolInformation {
            symbol: sym.raw.to_owned(),
            relationships: (0..sym.relationships)
                .map(|i| proto::Relationship {
                    symbol: format!("{}#rel-{i}", sym.raw),
                    is_reference: true,
                    is_implementation: false,
                    is_type_definition: false,
                    is_definition: false,
                })
                .collect(),
            ..Default::default()
        })
        .collect();

    let occurrences: Vec<proto::Occurrence> = symbols
        .iter()
        .flat_map(|sym| {
            let end_col = i32::try_from(sym.raw.len().min(i32::MAX as usize))
                .expect("clamped to i32::MAX above");
            (0..sym.occurrences).map(move |line| {
                let line_i32 = i32::try_from(line).unwrap_or(i32::MAX);
                proto::Occurrence {
                    range: vec![line_i32, 0, line_i32, end_col],
                    symbol: sym.raw.to_owned(),
                    ..Default::default()
                }
            })
        })
        .collect();

    let document = proto::Document {
        language: document_language.to_owned(),
        relative_path: relative_path.to_owned(),
        occurrences,
        symbols: scip_symbols,
        ..Default::default()
    };

    let index = proto::Index {
        metadata: Some(proto::Metadata {
            version: proto::ProtocolVersion::UnspecifiedProtocolVersion as i32,
            tool_info: Some(proto::ToolInfo {
                name: tool.to_owned(),
                version: "test".to_owned(),
                arguments: Vec::new(),
            }),
            project_root: "file:///synth".to_owned(),
            text_document_encoding: proto::TextEncoding::Utf8 as i32,
        }),
        documents: vec![document],
        external_symbols: Vec::new(),
    };

    let mut buf = Vec::with_capacity(256);
    index
        .encode(&mut buf)
        .expect("synthetic index must encode cleanly");
    buf
}

/// Decode `bytes` via the public `parse` free fn (the same call site any
/// salsa-derived consumer makes), then render a deterministic text
/// summary suitable for `insta::assert_snapshot!`. Plain-text snapshot
/// avoids dragging serde/yaml into ariadne-scip's test deps.
pub fn summarize(lang: Lang, bytes: &[u8]) -> Result<String, ScipError> {
    let doc = parse(lang, bytes)?;
    let top = top_symbols(&doc, 5);
    let ids = normalized_ids(&doc)?;
    let mut out = String::new();
    let _ = writeln!(out, "lang: {}", doc.lang.tag());
    let _ = writeln!(out, "docs: {}", doc.index.documents.len());
    let _ = writeln!(out, "symbols: {}", total_symbols(&doc));
    let _ = writeln!(out, "occurrences: {}", total_occurrences(&doc));
    let _ = writeln!(out, "relationships: {}", total_relationships(&doc));
    out.push_str("top_symbols_by_occurrence:\n");
    for entry in &top {
        let _ = writeln!(out, "  - {} ({} occ)", entry.canonical, entry.occurrences);
    }
    out.push_str("normalized_symbol_ids:\n");
    for id in &ids {
        let _ = writeln!(out, "  - {id}");
    }
    Ok(out)
}

pub struct TopSymbol {
    pub canonical: String,
    pub occurrences: usize,
}

fn total_symbols(doc: &ScipDoc) -> usize {
    doc.index
        .documents
        .iter()
        .map(|d| d.symbols.len())
        .sum::<usize>()
        + doc.index.external_symbols.len()
}

fn total_occurrences(doc: &ScipDoc) -> usize {
    doc.index
        .documents
        .iter()
        .map(|d| d.occurrences.len())
        .sum()
}

fn total_relationships(doc: &ScipDoc) -> usize {
    doc.index
        .documents
        .iter()
        .flat_map(|d| d.symbols.iter())
        .map(|s| s.relationships.len())
        .sum::<usize>()
        + doc
            .index
            .external_symbols
            .iter()
            .map(|s| s.relationships.len())
            .sum::<usize>()
}

fn top_symbols(doc: &ScipDoc, n: usize) -> Vec<TopSymbol> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for d in &doc.index.documents {
        for occ in &d.occurrences {
            *counts.entry(occ.symbol.clone()).or_insert(0) += 1;
        }
    }
    let mut ranked: Vec<(String, usize)> = counts.into_iter().collect();
    // Sort by (-count, symbol) so the snapshot is deterministic regardless
    // of insertion order.
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(n)
        .map(|(canonical, occurrences)| TopSymbol {
            canonical,
            occurrences,
        })
        .collect()
}

fn normalized_ids(doc: &ScipDoc) -> Result<Vec<String>, ScipError> {
    let mut out: Vec<String> = Vec::new();
    for d in &doc.index.documents {
        for sym in &d.symbols {
            let canon: CanonicalSymbol = normalize_scip_symbol(&sym.symbol)?;
            out.push(canon.id().to_hex());
        }
    }
    out.sort();
    Ok(out)
}
