//! Salsa `#[tracked]` derived queries (tier-04 step 4).
//!
//! Adaptations from the plan letter:
//!
//! * `parse_tree` was dropped as a separately tracked query because
//!   `tree_sitter::Tree` does not implement `salsa::Update` and a safe
//!   newtype wrapper is not possible — user-approved in this build session.
//! * Per the architecture invariant (ariadne-salsa may not depend on
//!   ariadne-parser/ariadne-scip; [src: tests/architecture.rs]),
//!   parsing and SCIP ingest are pushed into a driver layer that writes
//!   results back through the salsa input setters in later tiers. For
//!   tier-04 these queries return empty stubs — the cache-hit invariant
//!   and the per-revision query graph are what matter at this tier.
//!
//! All tracked-return types use only `Update`-friendly fields (`String`,
//! `u64`, `Vec<…>`, `[u8; N]`, derived structs with `salsa::Update`).

use std::sync::Arc;

use crate::inputs::{FileContentInput, ScipDocInput};

/// Syntactic facts pulled from a parsed file. Mirrors
/// `ariadne_parser::SyntacticFacts` but uses only `Update`-friendly types
/// — the driver layer (tier-06+) converts at the boundary.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SyntacticFactsRaw {
    /// Declarations in source order.
    pub decls: Vec<DeclRaw>,
    /// Imports in source order.
    pub imports: Vec<ImportRaw>,
    /// Call sites in source order.
    pub calls: Vec<CallRaw>,
}

/// Declaration record (driver-faced, `Update`-safe).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct DeclRaw {
    /// Free-form kind tag (mirrors `ariadne_parser::DeclKind` tag suffix).
    pub kind: String,
    /// Identifier text.
    pub name: String,
    /// `(byte_start, byte_end)` of the name node.
    pub name_byte_range: (u32, u32),
    /// `(byte_start, byte_end)` of the declaration node.
    pub def_byte_range: (u32, u32),
}

/// Import record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ImportRaw {
    /// Raw module path text.
    pub path: String,
    /// `(byte_start, byte_end)` of the import-path node.
    pub byte_range: (u32, u32),
}

/// Call-site record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallRaw {
    /// Callee identifier text.
    pub callee: String,
    /// `(byte_start, byte_end)` of the callee identifier.
    pub byte_range: (u32, u32),
}

/// Symbol record. The salsa-internal mirror of `ariadne_core::SymbolRecord`,
/// using only `Update`-friendly fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SymbolFactsRaw {
    /// Canonical symbol name.
    pub canonical_name: String,
    /// Free-form kind tag.
    pub kind: String,
    /// Defining file id, encoded as raw `u32`. Drivers convert to
    /// `ariadne_core::FileId` at the boundary.
    pub defining_file_raw: u32,
    /// `(byte_start, byte_end)` of the defining occurrence.
    pub defining_byte_range: (u32, u32),
}

/// Edge record (salsa-internal mirror).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct EdgeFactsRaw {
    /// Source symbol id (raw `u64`).
    pub src_raw: u64,
    /// Edge kind: 0=Defines, 1=References, 2=Imports — mirrors
    /// `ariadne_core::EdgeKind::to_byte`.
    pub kind_byte: u8,
    /// Destination symbol id (raw `u64`).
    pub dst_raw: u64,
    /// `(file_raw, byte_start, byte_end)` evidence span.
    pub evidence_span: (u32, u32, u32),
    /// Tier-07 coupling weight; tier-04 leaves it 0.
    pub weight: u32,
}

/// Syntactic facts for a file. Tier-04 stub: returns empty facts. The
/// driver layer (later tier) overrides this query path by populating
/// `FileContentInput`s and a separate facts-input source.
#[salsa::tracked]
pub fn syntactic_facts(db: &dyn salsa::Database, file: FileContentInput) -> Arc<SyntacticFactsRaw> {
    // Bind the input to the query so cache invalidation tracks content
    // changes. We touch length only — actual parsing is a driver concern.
    let _ = file.content(db).len();
    let _ = file.path(db);
    Arc::new(SyntacticFactsRaw::default())
}

/// SCIP-derived symbols for a file. Tier-04 stub returns empty until tier-05
/// wires the scip ingest pipeline.
#[salsa::tracked]
pub fn scip_symbols(db: &dyn salsa::Database, scip: ScipDocInput) -> Arc<Vec<SymbolFactsRaw>> {
    let _ = scip.raw_proto(db);
    let _ = scip.path(db);
    Arc::new(Vec::new())
}

/// Merged symbols for a file: syntactic facts decls become symbols, then
/// SCIP records take precedence per `canonical_name`. Tier-04 stubs both
/// upstreams so this returns an empty vector, but the dependency edges
/// are recorded by salsa so cache-hit / cache-miss behaviour can be
/// exercised by tests + benches.
#[salsa::tracked]
pub fn symbols_for_file(
    db: &dyn salsa::Database,
    file: FileContentInput,
    scip: ScipDocInput,
) -> Arc<Vec<SymbolFactsRaw>> {
    let facts = syntactic_facts(db, file);
    let scip_syms = scip_symbols(db, scip);
    let mut out: Vec<SymbolFactsRaw> = facts
        .decls
        .iter()
        .map(|d| SymbolFactsRaw {
            canonical_name: d.name.clone(),
            kind: d.kind.clone(),
            defining_file_raw: 0,
            defining_byte_range: d.def_byte_range,
        })
        .collect();
    // SCIP precedence per canonical_name.
    for s in scip_syms.iter() {
        if let Some(slot) = out
            .iter_mut()
            .find(|o| o.canonical_name == s.canonical_name)
        {
            *slot = s.clone();
        } else {
            out.push(s.clone());
        }
    }
    Arc::new(out)
}

/// Per-file edges. Tier-04 stub returns empty.
#[salsa::tracked]
pub fn edges_for_file(
    db: &dyn salsa::Database,
    file: FileContentInput,
    _scip: ScipDocInput,
) -> Arc<Vec<EdgeFactsRaw>> {
    let _ = file.content(db).len();
    Arc::new(Vec::new())
}

/// Blast-radius stub. Real algorithm lands in tier-07; tier-04 needs the
/// query node so dependency tracking is in place. The `_depth` arg is held
/// as a value rather than a salsa input so callers can drive recursion
/// limits without an extra revision bump.
#[salsa::tracked]
pub fn blast_radius(db: &dyn salsa::Database, sym_raw: u64, _depth: u8) -> Arc<Vec<u64>> {
    let _ = db;
    let _ = sym_raw;
    Arc::new(Vec::new())
}
