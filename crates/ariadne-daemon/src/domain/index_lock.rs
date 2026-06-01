//! Redb-open serialization handle for composition-root background work.
//!
//! The daemon keeps a single redb handle per process: the live-update pump and
//! the accept loop's staleness rebuild each open redb *only* while holding the
//! warm-catalog write lock, so the two opens are serialized and can never
//! collide (`DatabaseAlreadyOpen`) [src: tier-08 audit I1;
//! crates/ariadne-daemon/src/domain/live.rs; crates/ariadne-daemon/src/adapters/ipc.rs].
//!
//! tier-11a adds a third opener in the same process — the CLI composition
//! root's periodic Git-history re-walk. It lives outside the daemon (the daemon
//! never depends on `ariadne-git`, RD7), so it cannot reach the catalog lock
//! directly. [`IndexLock`] hands that one lock to the composition root as an
//! opaque handle, so the re-walk's transient redb open stays serialized against
//! the pump and accept-loop opens — closing the I1 race
//! [src: .claude/plans/post-v1-roadmap/audit/tier-11a-report.md I1].

use std::sync::{Arc, RwLock};

use crate::domain::catalog::WarmCatalog;

/// Opaque guard over the daemon's redb-open serialization point (the warm
/// catalog write lock). Held by the composition root's background re-walk so
/// its redb access is serialized with the daemon's own opens (single-open per
/// process). Cloning shares the same underlying lock.
#[derive(Clone)]
pub struct IndexLock(Arc<RwLock<WarmCatalog>>);

impl IndexLock {
    /// Wrap the warm-catalog lock the pump and accept loop already serialize on.
    pub(crate) fn new(catalog: Arc<RwLock<WarmCatalog>>) -> Self {
        Self(catalog)
    }

    /// Run `f` while holding the redb-open serialization lock, returning its
    /// result. A composition-root redb open wrapped in `with` cannot overlap
    /// the pump or accept-loop opens, which take the same lock.
    ///
    /// # Panics
    /// Panics if the warm-catalog lock is poisoned (a prior holder panicked) —
    /// the same failure mode the pump and accept loop already `expect`.
    pub fn with<T>(&self, f: impl FnOnce() -> T) -> T {
        let _guard = self.0.write().expect("warm-catalog write lock");
        f()
    }
}

impl std::fmt::Debug for IndexLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexLock").finish_non_exhaustive()
    }
}
