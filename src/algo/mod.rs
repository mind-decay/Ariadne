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

/// Pre-built adjacency index with forward/reverse maps and degree counts.
/// Built once, passed to all graph algorithms — eliminates redundant edge scans.
pub struct AdjacencyIndex<'a> {
    pub forward: BTreeMap<&'a CanonicalPath, Vec<&'a CanonicalPath>>,
    pub reverse: BTreeMap<&'a CanonicalPath, Vec<&'a CanonicalPath>>,
    pub out_degree: BTreeMap<&'a CanonicalPath, usize>,
    pub in_degree: BTreeMap<&'a CanonicalPath, usize>,
}

impl<'a> AdjacencyIndex<'a> {
    pub fn build(edges: &'a [Edge], filter: fn(&Edge) -> bool) -> Self {
        let mut forward_set: BTreeMap<&CanonicalPath, BTreeSet<&CanonicalPath>> = BTreeMap::new();
        let mut reverse_set: BTreeMap<&CanonicalPath, BTreeSet<&CanonicalPath>> = BTreeMap::new();
        for edge in edges {
            if filter(edge) {
                forward_set.entry(&edge.from).or_default().insert(&edge.to);
                reverse_set.entry(&edge.to).or_default().insert(&edge.from);
            }
        }
        let forward: BTreeMap<_, Vec<_>> = forward_set
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();
        let reverse: BTreeMap<_, Vec<_>> = reverse_set
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();
        let out_degree = forward.iter().map(|(k, v)| (*k, v.len())).collect();
        let in_degree = reverse.iter().map(|(k, v)| (*k, v.len())).collect();
        Self {
            forward,
            reverse,
            out_degree,
            in_degree,
        }
    }
}
