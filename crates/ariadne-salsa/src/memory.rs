//! Per-table memory probe for `AriadneDb` (tier-04 step 7; CLAUDE.md R1).
//!
//! `salsa = 0.26.2` exposes no per-table heap-size or table-enumeration API on
//! its public surface (src: <https://docs.rs/salsa/0.26.2/salsa/struct.Storage.html>
//! — public methods are `new`, `builder`, `into_zalsa_handle`, plus
//! `Clone`/`Default`). The plan authorizes a fallback for exactly that case:
//! `if absent in pinned version, fall back to counter + mem::size_of_val walk
//! and document the gap` (src: tier-04 plan step 7).
//!
//! [`AriadneDb::memory_report`](crate::AriadneDb::memory_report) implements that
//! walk: for every seeded file it recomputes (a salsa cache hit) each tracked
//! per-file query and sums the deep heap size — struct footprint plus the
//! `Vec`/`String` buffers each entry owns — into that query's table. The figure
//! is an approximation of resident bytes, not an allocator-exact total, but it
//! grows with the corpus and can therefore cross the 256 MiB ceiling, so
//! [`MemoryReport::over_budget`] enforces R1 for real (src: tier-04 plan
//! `exit_criteria` "warns if any table > 256MB"; CLAUDE.md R1).

use std::collections::BTreeMap;

use crate::derived::{
    CallRaw, DeclRaw, HookRaw, ImportRaw, RenderRaw, ScipFactsRaw, ScipOccurrenceRaw,
    ScipRelationshipRaw, SymbolFactsRaw, SyntacticFactsRaw,
};

/// 256MB. Tier-04 plan `exit_criteria` + R1 risk mitigation.
pub const TABLE_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

/// Snapshot of approximate per-tracked-table byte counts.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemoryReport {
    /// Table name → estimated byte count.
    pub tables: BTreeMap<&'static str, u64>,
}

impl MemoryReport {
    /// Sum across tables.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.tables.values().sum()
    }

    /// Tables whose estimated size exceeds [`TABLE_BUDGET_BYTES`].
    pub fn over_budget(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        self.tables
            .iter()
            .filter(|(_, b)| **b > TABLE_BUDGET_BYTES)
            .map(|(name, b)| (*name, *b))
    }

    // Build a report from the measured per-file table totals. The two stub
    // tables (`edges_for_file`, `blast_radius`) report 0 — see the
    // `AriadneDb::memory_report` doc for why neither is materialized by the
    // derivation flow. The inserted key set must equal [`TRACKED_TABLES`]; the
    // `report_lists_every_tracked_table` test guards that.
    pub(crate) fn from_table_bytes(
        syntactic_facts: u64,
        scip_facts: u64,
        symbols_for_file: u64,
    ) -> Self {
        // Build over the canonical table set so the report always lists every
        // tracked table; the two stub tables stay at 0.
        let tables = TRACKED_TABLES
            .iter()
            .map(|&name| {
                let bytes = match name {
                    "syntactic_facts" => syntactic_facts,
                    "scip_facts" => scip_facts,
                    "symbols_for_file" => symbols_for_file,
                    _ => 0,
                };
                (name, bytes)
            })
            .collect();
        Self { tables }
    }
}

/// The set of tracked-fn tables salsa allocates on the database. Adding a
/// new `#[salsa::tracked]` requires adding it here so the probe surface
/// stays in sync; the `report_lists_every_tracked_table` unit test guards the
/// registered set, while sync with the actual `#[salsa::tracked]` declarations
/// is currently hand-maintained until salsa exposes a runtime table
/// enumeration API.
pub(crate) const TRACKED_TABLES: &[&str] = &[
    "syntactic_facts",
    "scip_facts",
    "symbols_for_file",
    "edges_for_file",
    "blast_radius",
];

/// Deep heap size of one file's [`SyntacticFactsRaw`]: the struct itself plus
/// every `Vec` buffer and the `String` bytes each record owns.
pub(crate) fn syntactic_facts_bytes(f: &SyntacticFactsRaw) -> u64 {
    let base = std::mem::size_of::<SyntacticFactsRaw>() as u64;
    let decls = vec_buf::<DeclRaw>(f.decls.len())
        + f.decls
            .iter()
            .map(|d| str_heap(&d.kind) + str_heap(&d.name) + strvec_heap(&d.attributes))
            .sum::<u64>();
    let imports = vec_buf::<ImportRaw>(f.imports.len())
        + f.imports.iter().map(|i| str_heap(&i.path)).sum::<u64>();
    let calls = vec_buf::<CallRaw>(f.calls.len())
        + f.calls.iter().map(|c| str_heap(&c.callee)).sum::<u64>();
    let renders = vec_buf::<RenderRaw>(f.renders.len())
        + f.renders
            .iter()
            .map(|r| str_heap(&r.component))
            .sum::<u64>();
    let hooks = vec_buf::<HookRaw>(f.hooks.len())
        + f.hooks.iter().map(|h| str_heap(&h.callee)).sum::<u64>();
    base + decls + imports + calls + renders + hooks
}

/// Deep heap size of one file's [`ScipFactsRaw`]: the struct itself plus the
/// occurrence buffer and the `String` symbol key each occurrence owns, plus the
/// relationship buffer and the two `String` keys each relationship owns.
pub(crate) fn scip_facts_bytes(f: &ScipFactsRaw) -> u64 {
    let base = std::mem::size_of::<ScipFactsRaw>() as u64;
    let occ = vec_buf::<ScipOccurrenceRaw>(f.occurrences.len())
        + f.occurrences
            .iter()
            .map(|o| str_heap(&o.symbol))
            .sum::<u64>();
    let rels = vec_buf::<ScipRelationshipRaw>(f.relationships.len())
        + f.relationships
            .iter()
            .map(|r| str_heap(&r.from) + str_heap(&r.to))
            .sum::<u64>();
    base + occ + rels
}

/// Deep heap size of one file's merged `symbols_for_file` output.
pub(crate) fn symbols_vec_bytes(v: &[SymbolFactsRaw]) -> u64 {
    vec_buf::<SymbolFactsRaw>(v.len())
        + v.iter()
            .map(|s| str_heap(&s.canonical_name) + str_heap(&s.kind) + strvec_heap(&s.attributes))
            .sum::<u64>()
}

/// Bytes a `Vec<T>` of `len` elements occupies for its element buffer.
fn vec_buf<T>(len: usize) -> u64 {
    (len * std::mem::size_of::<T>()) as u64
}

/// Heap bytes a `String`'s text buffer owns.
fn str_heap(s: &str) -> u64 {
    s.len() as u64
}

/// Heap bytes a `Vec<String>` owns: its pointer buffer plus each string's text.
fn strvec_heap(v: &[String]) -> u64 {
    vec_buf::<String>(v.len()) + v.iter().map(|s| s.len() as u64).sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_lists_every_tracked_table() {
        // A fresh database has no seeded files, so every table reports zero —
        // but the full registered set is still present and the budget is clear.
        let db = crate::AriadneDb::new();
        let report = db.memory_report();
        for table in TRACKED_TABLES {
            assert!(
                report.tables.contains_key(table),
                "missing table {table} in memory report"
            );
            assert_eq!(report.tables.get(table), Some(&0));
        }
        assert_eq!(report.total_bytes(), 0);
        assert!(report.over_budget().next().is_none());
    }

    #[test]
    fn over_budget_filters_correctly() {
        let mut tables = BTreeMap::new();
        tables.insert("a", TABLE_BUDGET_BYTES);
        tables.insert("b", TABLE_BUDGET_BYTES + 1);
        let report = MemoryReport { tables };
        let over: Vec<_> = report.over_budget().collect();
        assert_eq!(over, vec![("b", TABLE_BUDGET_BYTES + 1)]);
    }

    #[test]
    fn deep_size_counts_owned_buffers() {
        // The empty facts cost only the struct itself; adding a decl with owned
        // strings strictly increases the measured bytes — proving the walk
        // reaches into the `Vec`/`String` buffers rather than returning a
        // constant.
        let empty = SyntacticFactsRaw::default();
        assert_eq!(
            syntactic_facts_bytes(&empty),
            std::mem::size_of::<SyntacticFactsRaw>() as u64
        );

        let one = SyntacticFactsRaw {
            decls: vec![DeclRaw {
                kind: "function".to_owned(),
                name: "alpha".to_owned(),
                name_byte_range: (3, 8),
                def_byte_range: (0, 13),
                visibility_byte: 0,
                attributes: vec!["test".to_owned()],
                complexity: 0,
            }],
            ..Default::default()
        };
        assert!(syntactic_facts_bytes(&one) > syntactic_facts_bytes(&empty));

        let syms = [SymbolFactsRaw {
            canonical_name: "alpha".to_owned(),
            kind: "function".to_owned(),
            defining_file_raw: 1,
            defining_byte_range: (0, 13),
            visibility_byte: 0,
            attributes: Vec::new(),
            complexity: 0,
        }];
        assert!(symbols_vec_bytes(&syms) > symbols_vec_bytes(&[]));
    }
}
