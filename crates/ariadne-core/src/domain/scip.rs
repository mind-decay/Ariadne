//! Pure SCIP fact types crossing the `ariadne-scip` → `ariadne-core` →
//! `ariadne-salsa` boundary (scip-driven-edges tier-01, plan D2).
//!
//! `ariadne-scip::extract_facts` decodes the SCIP protobuf at the composition
//! root and reduces each occurrence to the edge signal carried here; salsa
//! never sees prost. These types are the `Update`-free, dependency-free mirror
//! of that signal — the salsa input layer keeps its own `salsa::Update` mirror
//! (`ariadne_salsa::ScipFactsRaw`) and converts at the composition root, exactly
//! as `ariadne_parser::SyntacticFacts` crosses into `ariadne_salsa::SyntacticFactsRaw`
//! [src: crates/ariadne-salsa/src/inputs.rs module header; plan D2].

use serde::{Deserialize, Serialize};

/// One SCIP occurrence reduced to the edge signal: a globally-resolved symbol
/// key, the byte range it spans in the file, and the `symbol_roles` bitset.
///
/// `symbol` is the *normalized* key — `ariadne-scip` runs each occurrence's raw
/// SCIP symbol string through `normalize_scip_symbol` and stores its stable
/// canonical id, so equivalent encodings (backtick-escaped vs plain) key equal
/// [src: crates/ariadne-scip/src/normalize/mod.rs:160-162; plan D3]. `byte_range`
/// is `(byte_start, byte_end)` in the file's source bytes — SCIP ranges are
/// line/character pairs, converted to bytes at extraction so they map onto the
/// tree-sitter symbols' byte spans [src: crates/ariadne-scip/proto/scip.proto:645-675].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScipOccurrence {
    /// Normalized symbol key (canonical-symbol id hex).
    pub symbol: String,
    /// `(byte_start, byte_end)` of this occurrence in the file's source bytes.
    pub byte_range: (u32, u32),
    /// SCIP `SymbolRole` bitset: `Definition = 0x1`, `Import = 0x2`,
    /// `WriteAccess = 0x4`, `ReadAccess = 0x8`, …
    /// [src: crates/ariadne-scip/proto/scip.proto:521-543].
    pub roles: u32,
}

/// One SCIP `SymbolInformation.relationships` entry reduced to the edge signal:
/// the two normalized symbol keys it relates and which relationship flags are
/// set. `from` is the owning symbol (`SymbolInformation.symbol`), `to` the
/// related symbol (`Relationship.symbol`); both are run through
/// `normalize_scip_symbol` so they key the same global `scip_symbol → SymbolId`
/// map the occurrences build [src: crates/ariadne-scip/proto/scip.proto:462-499;
/// plan D3, T3]. Only the two edge-bearing flags are kept: `is_implementation`
/// (Find implementations, → graph `Overrides`) and `is_type_definition` (Go to
/// type definition, → graph `TypeOf`) [src: scip.proto:489-499].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScipRelationship {
    /// Normalized key of the owning symbol (the relationship's `from` endpoint).
    pub from: String,
    /// Normalized key of the related symbol (the relationship's `to` endpoint).
    pub to: String,
    /// SCIP `is_implementation`: `from` implements / overrides / inherits `to`.
    pub is_implementation: bool,
    /// SCIP `is_type_definition`: `from`'s type is `to`.
    pub is_type_definition: bool,
}

/// All SCIP occurrences extracted for one file, plus the content hash the facts
/// were indexed at. The hash is the D4 coverage key: a file is "covered" — its
/// edges come from SCIP, not the tree-sitter resolver — only while its current
/// content hash still matches `indexed_hash`, so a live edit drops the file back
/// to the precise resolver until SCIP re-runs [src: plan D4].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScipFacts {
    /// Occurrences in this file, in extraction order.
    pub occurrences: Vec<ScipOccurrence>,
    /// Relationships declared on this file's symbols (scip-driven-edges T3).
    pub relationships: Vec<ScipRelationship>,
    /// blake3 of the file content the SCIP indexer saw, matching
    /// `FileRecord::blake3` while the file is unedited.
    pub indexed_hash: [u8; 32],
}
