//! Table definitions + META keys for the redb adapter.

use redb::{MultimapTableDefinition, TableDefinition};

/// On-disk schema version. Bumps require a new tier + a registered
/// `vN -> vN+1` step in [`crate::domain::migration`] so existing databases
/// upgrade in place instead of rebuilding.
pub(crate) const SCHEMA_VERSION: u64 = 3;
pub(super) const KEY_SCHEMA_VERSION: &str = "schema_version";
pub(super) const KEY_REVISION: &str = "revision";

pub(super) const META: TableDefinition<'_, &str, u64> = TableDefinition::new("meta");
pub(super) const FILES: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("files");
pub(super) const SYMBOLS: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("symbols");
pub(super) const EDGES: TableDefinition<'_, &[u8], &[u8]> = TableDefinition::new("edges");
pub(super) const EDGES_BY_FILE: MultimapTableDefinition<'_, &[u8], &[u8]> =
    MultimapTableDefinition::new("edges_by_file");
