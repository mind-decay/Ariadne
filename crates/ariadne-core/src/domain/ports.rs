//! Port traits — hexagonal contracts ariadne-core declares for adapters to
//! implement. Tier-01 ships them as empty marker traits so the architecture
//! invariant test has real symbols to bind to; later tiers fill in the
//! actual signatures [src: docs/folder-layout.md `<adding-a-port>`].

/// Persistent storage port. Implemented by `ariadne-storage` (redb) in
/// tier-02.
pub trait Storage {}

/// Parsing port. Implemented by `ariadne-parser` (tree-sitter) in tier-03.
pub trait Parser {}

/// Semantic indexing port. Implemented by `ariadne-scip` in tier-05.
pub trait Indexer {}

/// File-system event sink port. Implemented by `ariadne-watcher` (notify)
/// in tier-06.
pub trait WatcherSink {}
