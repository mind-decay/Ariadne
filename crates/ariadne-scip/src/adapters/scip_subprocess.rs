//! Subprocess + protobuf implementation of `ariadne_core::Indexer`. Tier-05
//! wires `rust-analyzer --scip`, `scip-typescript`, `scip-python`, …, and
//! the `lsif-go` → SCIP fallback.

use ariadne_core::Indexer;

/// Placeholder SCIP subprocess indexer. Real implementation arrives in
/// tier-05.
#[derive(Debug, Default)]
pub struct ScipSubprocessIndexer;

impl Indexer for ScipSubprocessIndexer {}
