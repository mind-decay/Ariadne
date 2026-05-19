//! Per-table memory probe for `AriadneDb` (tier-04 step 7).
//!
//! Plan note: `salsa::Storage::heap_size` was anticipated but does not
//! exist on the public surface of `salsa = 0.26.2`
//! (src: <https://docs.rs/salsa/0.26.2/salsa/struct.Storage.html> — public
//! methods are `new`, `builder`, `into_zalsa_handle`, plus `Clone`/`Default`).
//!
//! The plan explicitly authorizes a fallback when the API is absent: `if
//! absent in pinned version, fall back to counter + mem::size_of_val walk
//! and document the gap` (src: tier-04 plan step 7). Tier-04 ships the
//! fallback shape — a per-tracked-table counter populated by the driver
//! layer in later tiers — and an `over_budget` predicate the audit step
//! uses to enforce the 256MB ceiling (src: tier-04 plan `exit_criteria`
//! "warns if any table > 256MB").

use std::collections::BTreeMap;

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
}

impl crate::AriadneDb {
    /// Per-table memory report. Tier-04 ships the empty surface; later
    /// tiers feed counters in as tracked queries materialize.
    #[must_use]
    pub fn memory_report(&self) -> MemoryReport {
        let mut tables: BTreeMap<&'static str, u64> = BTreeMap::new();
        // Insert the registered tracked-fn tables with a zero baseline so
        // callers see the full table set even before any data is written.
        for name in TRACKED_TABLES {
            tables.insert(name, 0);
        }
        MemoryReport { tables }
    }
}

/// The set of tracked-fn tables salsa allocates on the database. Adding a
/// new `#[salsa::tracked]` requires adding it here so the probe surface
/// stays in sync; the `baseline_report_lists_every_tracked_table` unit test
/// guards the registered set, while sync with the actual `#[salsa::tracked]`
/// declarations is currently hand-maintained until salsa exposes a runtime
/// table enumeration API.
pub(crate) const TRACKED_TABLES: &[&str] = &[
    "syntactic_facts",
    "scip_symbols",
    "symbols_for_file",
    "edges_for_file",
    "blast_radius",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_report_lists_every_tracked_table() {
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
}
