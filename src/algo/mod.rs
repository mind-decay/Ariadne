pub mod blast_radius;
pub mod centrality;
pub mod compress;
pub mod delta;
pub mod louvain;
pub mod pagerank;
pub mod scc;
pub mod spectral;
pub mod stats;
pub mod subgraph;
pub mod topo_sort;

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{CanonicalPath, Edge};

/// Filter edges to architectural types only (imports + re_exports + type_imports).
/// Excludes tests edges per D-034.
pub fn is_architectural(edge: &Edge) -> bool {
    edge.edge_type.is_architectural()
}

/// Round to 4 decimal places — standardized float determinism utility (D-049).
pub fn round4(v: f64) -> f64 {
    (v * 10000.0).round() / 10000.0
}

/// Build forward and reverse adjacency indices from edges, filtered by predicate.
/// Deduplicates neighbors to ensure each (from, to) pair appears at most once,
/// which is required for correct Brandes centrality computation.
#[allow(clippy::type_complexity)]
pub fn build_adjacency(
    edges: &[Edge],
    filter: fn(&Edge) -> bool,
) -> (
    BTreeMap<&CanonicalPath, Vec<&CanonicalPath>>,
    BTreeMap<&CanonicalPath, Vec<&CanonicalPath>>,
) {
    let mut forward_set: BTreeMap<&CanonicalPath, BTreeSet<&CanonicalPath>> = BTreeMap::new();
    let mut reverse_set: BTreeMap<&CanonicalPath, BTreeSet<&CanonicalPath>> = BTreeMap::new();
    for edge in edges {
        if filter(edge) {
            forward_set.entry(&edge.from).or_default().insert(&edge.to);
            reverse_set.entry(&edge.to).or_default().insert(&edge.from);
        }
    }
    let forward = forward_set
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();
    let reverse = reverse_set
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().collect()))
        .collect();
    (forward, reverse)
}
