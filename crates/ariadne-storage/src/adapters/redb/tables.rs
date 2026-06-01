//! Table definitions + META keys for the redb adapter.

use redb::{MultimapTableDefinition, TableDefinition};

/// On-disk schema version. Bumps require a new tier + a registered
/// `vN -> vN+1` step in [`crate::domain::migration`] so existing databases
/// upgrade in place instead of rebuilding.
pub(crate) const SCHEMA_VERSION: u64 = 6;
pub(super) const KEY_SCHEMA_VERSION: &str = "schema_version";
pub(super) const KEY_REVISION: &str = "revision";
/// Key in [`HISTORY_META`] holding the HEAD-commit oid of the last Git-history
/// ingest — the incremental watermark (tier-11a).
pub(super) const KEY_LAST_INGESTED_COMMIT: &str = "last_ingested_commit";

pub(super) const META: TableDefinition<'_, &str, u64> = TableDefinition::new("meta");
pub(super) const FILES: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("files");
pub(super) const SYMBOLS: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("symbols");
pub(super) const EDGES: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("edges");
pub(super) const EDGES_BY_FILE: MultimapTableDefinition<'_, &[u8], &[u8]> =
    MultimapTableDefinition::new("edges_by_file");
/// Per-file Git-history churn (tier-11): path bytes -> postcard `FileChurn`.
pub(super) const CHURN: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("churn");
/// Unordered file-pair co-change (tier-11): ordered-pair key -> postcard
/// `CoChangePair`.
pub(super) const CO_CHANGE: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("co_change");
/// Byte-valued history metadata (tier-11a): currently the
/// [`KEY_LAST_INGESTED_COMMIT`] watermark. `META` is `&str -> u64`, so a
/// 20/32-byte commit oid needs this separate `&str -> &[u8]` table.
pub(super) const HISTORY_META: TableDefinition<'_, &str, &[u8]> =
    TableDefinition::new("history_meta");
/// Per-symbol Git-history churn (tier-11b): 8-byte big-endian `SymbolId` ->
/// postcard `SymbolChurn`. Written by the `ariadne-graph` attribution use-case
/// wired at the CLI composition root [src: post-v1-roadmap plan.md RD7 +
/// tier-11b step 5].
pub(super) const SYMBOL_CHURN: TableDefinition<'_, &[u8], &[u8]> =
    TableDefinition::new("symbol_churn");
