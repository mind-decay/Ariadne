//! redb-backed implementation of `ariadne_core::Storage`. Tier-02 wires up
//! the real database, schema, and codecs; tier-01 only fixes the type and
//! its trait binding so the architecture invariant has a concrete impl
//! [src: .claude/plans/ariadne-core/tier-02-storage.md].

use ariadne_core::Storage;

/// Placeholder redb-backed storage. Real implementation arrives in tier-02.
#[derive(Debug, Default)]
pub struct RedbStorage;

impl Storage for RedbStorage {}
