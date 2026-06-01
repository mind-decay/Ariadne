//! redb schema-migration framework: ordered `vN -> vN+1` transform steps
//! that upgrade an on-disk database in place, replacing v1's
//! rebuild-on-mismatch behaviour [src: post-v1-roadmap plan.md RD2].
//!
//! [`MigrationRegistry`] holds the project's full ordered step chain;
//! [`MigrationRegistry::plan`] selects the contiguous sub-chain that advances
//! a given on-disk version up to the running binary's `SCHEMA_VERSION`. The
//! caller runs every selected step inside one redb `WriteTransaction`, so a
//! crash mid-migration leaves the file untouched at its original version
//! [src: <https://docs.rs/redb/4.1.0/redb/struct.WriteTransaction.html>].

use ariadne_core::{FileId, Span, SymbolRecord, Visibility};
use redb::{ReadableTable, TableDefinition, WriteTransaction};
use serde::{Deserialize, Serialize};

use crate::adapters::codec::{decode_value, encode_value};
use crate::errors::RedbStorageError;

/// One `from -> to` schema transform. `to` is always `from + 1`: the chain
/// advances a database exactly one version at a time.
///
/// `apply` mutates the data tables of an in-flight write transaction. It must
/// not open the `meta` table — the migration runner owns the `schema_version`
/// key and bumps it once the whole chain succeeds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MigrationStep {
    /// On-disk schema version this step upgrades from.
    pub from: u64,
    /// Schema version this step produces (`from + 1`).
    pub to: u64,
    /// Pure data transform applied to the open write transaction.
    pub apply: fn(&WriteTransaction) -> Result<(), RedbStorageError>,
}

/// Ordered registry of every schema-migration step the binary knows.
///
/// Steps are stored sorted and contiguous (`steps[i].to == steps[i + 1].from`);
/// each future format bump appends exactly one [`MigrationStep`].
#[derive(Debug, Clone)]
pub(crate) struct MigrationRegistry {
    steps: Vec<MigrationStep>,
}

impl MigrationRegistry {
    /// Build the registry with every migration step the binary ships.
    #[must_use]
    pub(crate) fn builtin() -> Self {
        Self {
            steps: vec![
                MigrationStep {
                    from: 1,
                    to: 2,
                    apply: migrate_v1_to_v2,
                },
                MigrationStep {
                    from: 2,
                    to: 3,
                    apply: migrate_v2_to_v3,
                },
                MigrationStep {
                    from: 3,
                    to: 4,
                    apply: migrate_v3_to_v4,
                },
                MigrationStep {
                    from: 4,
                    to: 5,
                    apply: migrate_v4_to_v5,
                },
                MigrationStep {
                    from: 5,
                    to: 6,
                    apply: migrate_v5_to_v6,
                },
            ],
        }
    }

    /// Return the contiguous step chain that advances `from` up to `to`, or
    /// `None` when `from >= to` or no registered path spans the gap.
    #[must_use]
    pub(crate) fn plan(&self, from: u64, to: u64) -> Option<&[MigrationStep]> {
        if from >= to {
            return None;
        }
        let start = self.steps.iter().position(|s| s.from == from)?;
        let chain = &self.steps[start..];
        let mut cursor = from;
        for (idx, step) in chain.iter().enumerate() {
            if step.from != cursor {
                return None;
            }
            cursor = step.to;
            if cursor == to {
                return Some(&chain[..=idx]);
            }
            if cursor > to {
                return None;
            }
        }
        None
    }
}

/// Identity migration `v1 -> v2`: the v1 and v2 on-disk layouts are
/// byte-identical, so no records are rewritten. Registered so the framework
/// runs end-to-end against a real path; future format bumps replace such
/// no-ops with concrete table transforms.
//
// The `Result` is fixed by `MigrationStep::apply`'s fn-pointer type —
// non-identity steps return `Err`, so the wrap is part of the contract.
#[allow(clippy::unnecessary_wraps)]
fn migrate_v1_to_v2(_txn: &WriteTransaction) -> Result<(), RedbStorageError> {
    Ok(())
}

/// `SYMBOLS` table definition — local mirror so the migration step owns
/// the postcard codec without leaking the adapter's table modules. Keeps
/// the schema-format constants colocated with the migration logic.
const SYMBOLS: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("symbols");

/// Frozen v2 `SymbolRecord` layout (`canonical_name`, `kind`,
/// `defining_file`, `defining_span` — postcard order matches the field
/// order in `ariadne_core::SymbolRecord` at schema v2). v3 appends
/// `visibility` and `attributes`; postcard is non-self-describing
/// [src: <https://postcard.jamesmunns.com/wire-format>] — struct field
/// count and names are not on the wire — so a v3 decode of a v2 body
/// would read past the buffer. This frozen struct captures the v2 prefix
/// exactly.
#[derive(Debug, Serialize, Deserialize)]
struct SymbolRecordV2 {
    canonical_name: String,
    kind: String,
    defining_file: FileId,
    defining_span: Span,
}

/// v2 → v3: re-encode every `SYMBOLS` body so each record carries the new
/// `visibility` + `attributes` fields with their default values
/// (`Visibility::Unknown`, empty `Vec`). Keys are untouched. The whole
/// pass runs inside the caller's single `WriteTransaction`, so a failure
/// before commit leaves the file at v2 [src: post-v1-roadmap plan.md RD10
/// + tier-04 step 4].
fn migrate_v2_to_v3(txn: &WriteTransaction) -> Result<(), RedbStorageError> {
    // Collect first, then re-insert, so the iterator over the table is
    // dropped before the mutating reopen — redb forbids interleaving a
    // live iterator with a mutating `insert` on the same table.
    let collected: Vec<(Vec<u8>, SymbolRecordV2)> = {
        let table = txn.open_table(SYMBOLS)?;
        let mut out = Vec::new();
        for entry in table.iter()? {
            let (key, value) = entry?;
            let key_bytes = key.value().to_vec();
            let v2: SymbolRecordV2 = decode_value(value.value())?;
            out.push((key_bytes, v2));
        }
        out
    };

    let mut table = txn.open_table(SYMBOLS)?;
    for (key_bytes, v2) in collected {
        let v3 = SymbolRecord {
            canonical_name: v2.canonical_name,
            kind: v2.kind,
            defining_file: v2.defining_file,
            defining_span: v2.defining_span,
            visibility: Visibility::Unknown,
            attributes: Vec::new(),
        };
        let encoded = encode_value(&v3)?;
        table.insert(key_bytes.as_slice(), encoded.as_slice())?;
    }
    Ok(())
}

/// `CHURN` / `CO_CHANGE` table definitions — local mirrors so the migration
/// step owns the table names without leaking the adapter's table module (same
/// pattern as the [`SYMBOLS`] mirror above). Names match
/// [`crate::adapters::redb::tables`].
const CHURN: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("churn");
const CO_CHANGE: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("co_change");

/// v3 → v4: create the per-file churn + file-pair co-change tables in place so
/// a pre-existing v3 database gains the tier-11 Git-history tables without a
/// rebuild. Purely additive — no existing record is read or rewritten; opening
/// a table inside the write transaction creates it when absent
/// [src: post-v1-roadmap plan.md RD7 + tier-11 step 8].
fn migrate_v3_to_v4(txn: &WriteTransaction) -> Result<(), RedbStorageError> {
    txn.open_table(CHURN)?;
    txn.open_table(CO_CHANGE)?;
    Ok(())
}

/// `HISTORY_META` table definition — local mirror so the migration step owns
/// the table name without leaking the adapter's table module (same pattern as
/// the [`SYMBOLS`] / [`CHURN`] mirrors above). Name matches
/// [`crate::adapters::redb::tables`].
const HISTORY_META: TableDefinition<'_, &str, &[u8]> = TableDefinition::new("history_meta");

/// v4 → v5: create the byte-valued `HISTORY_META` table in place so a
/// pre-existing v4 database gains the tier-11a incremental-history watermark
/// store without a rebuild. Purely additive — no existing record is read or
/// rewritten; opening a table inside the write transaction creates it when
/// absent [src: post-v1-roadmap tier-11a-incremental-history.md step 2].
fn migrate_v4_to_v5(txn: &WriteTransaction) -> Result<(), RedbStorageError> {
    txn.open_table(HISTORY_META)?;
    Ok(())
}

/// `SYMBOL_CHURN` table definition — local mirror so the migration step owns
/// the table name without leaking the adapter's table module (same pattern as
/// the [`SYMBOLS`] / [`HISTORY_META`] mirrors above). Name matches
/// [`crate::adapters::redb::tables`].
const SYMBOL_CHURN: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("symbol_churn");

/// v5 → v6: create the per-symbol churn table in place so a pre-existing v5
/// database gains the tier-11b symbol-churn store without a rebuild. Purely
/// additive — no existing record is read or rewritten; opening a table inside
/// the write transaction creates it when absent [src: post-v1-roadmap
/// tier-11b-symbol-churn-attribution.md step 5].
fn migrate_v5_to_v6(txn: &WriteTransaction) -> Result<(), RedbStorageError> {
    txn.open_table(SYMBOL_CHURN)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MigrationRegistry, MigrationStep, WriteTransaction};
    use crate::errors::RedbStorageError;

    // Matches `MigrationStep::apply`'s fn-pointer type; the wrap is required.
    #[allow(clippy::unnecessary_wraps)]
    fn noop(_txn: &WriteTransaction) -> Result<(), RedbStorageError> {
        Ok(())
    }

    fn registry_with(links: &[(u64, u64)]) -> MigrationRegistry {
        MigrationRegistry {
            steps: links
                .iter()
                .map(|&(from, to)| MigrationStep {
                    from,
                    to,
                    apply: noop,
                })
                .collect(),
        }
    }

    #[test]
    fn plan_returns_full_contiguous_chain() {
        let reg = registry_with(&[(1, 2), (2, 3), (3, 4)]);
        let chain = reg.plan(1, 4).expect("path exists");
        assert_eq!(chain.len(), 3);
        assert_eq!((chain[0].from, chain[2].to), (1, 4));
    }

    #[test]
    fn plan_returns_subchain_when_target_below_max() {
        let reg = registry_with(&[(1, 2), (2, 3), (3, 4)]);
        let chain = reg.plan(2, 3).expect("subpath exists");
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (2, 3));
    }

    #[test]
    fn plan_rejects_missing_start_version() {
        let reg = registry_with(&[(2, 3)]);
        assert!(reg.plan(1, 3).is_none());
    }

    #[test]
    fn plan_rejects_gap_in_chain() {
        let reg = registry_with(&[(1, 2), (3, 4)]);
        assert!(reg.plan(1, 4).is_none());
    }

    #[test]
    fn plan_rejects_non_advancing_range() {
        let reg = registry_with(&[(1, 2)]);
        assert!(reg.plan(2, 2).is_none(), "equal versions");
        assert!(reg.plan(3, 1).is_none(), "backwards range");
    }

    #[test]
    fn builtin_registry_covers_v1_to_v2() {
        let chain = MigrationRegistry::builtin()
            .plan(1, 2)
            .expect("v1 -> v2 path")
            .to_vec();
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (1, 2));
    }

    #[test]
    fn builtin_registry_covers_v2_to_v3() {
        let chain = MigrationRegistry::builtin()
            .plan(2, 3)
            .expect("v2 -> v3 path")
            .to_vec();
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (2, 3));
    }

    #[test]
    fn builtin_registry_covers_v1_to_v3() {
        let chain = MigrationRegistry::builtin()
            .plan(1, 3)
            .expect("v1 -> v3 path")
            .to_vec();
        assert_eq!(chain.len(), 2);
        assert_eq!(
            (chain[0].from, chain[0].to, chain[1].from, chain[1].to),
            (1, 2, 2, 3),
        );
    }

    #[test]
    fn builtin_registry_covers_v3_to_v4() {
        let chain = MigrationRegistry::builtin()
            .plan(3, 4)
            .expect("v3 -> v4 path")
            .to_vec();
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (3, 4));
    }

    #[test]
    fn builtin_registry_covers_v1_to_v4() {
        let chain = MigrationRegistry::builtin()
            .plan(1, 4)
            .expect("v1 -> v4 path")
            .to_vec();
        assert_eq!(chain.len(), 3);
        assert_eq!((chain[0].from, chain[2].to), (1, 4));
    }

    #[test]
    fn builtin_registry_covers_v4_to_v5() {
        let chain = MigrationRegistry::builtin()
            .plan(4, 5)
            .expect("v4 -> v5 path")
            .to_vec();
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (4, 5));
    }

    #[test]
    fn builtin_registry_covers_v1_to_v5() {
        let chain = MigrationRegistry::builtin()
            .plan(1, 5)
            .expect("v1 -> v5 path")
            .to_vec();
        assert_eq!(chain.len(), 4);
        assert_eq!((chain[0].from, chain[3].to), (1, 5));
    }

    #[test]
    fn builtin_registry_covers_v5_to_v6() {
        let chain = MigrationRegistry::builtin()
            .plan(5, 6)
            .expect("v5 -> v6 path")
            .to_vec();
        assert_eq!(chain.len(), 1);
        assert_eq!((chain[0].from, chain[0].to), (5, 6));
    }

    #[test]
    fn builtin_registry_covers_v1_to_v6() {
        let chain = MigrationRegistry::builtin()
            .plan(1, 6)
            .expect("v1 -> v6 path")
            .to_vec();
        assert_eq!(chain.len(), 5);
        assert_eq!((chain[0].from, chain[4].to), (1, 6));
    }
}
