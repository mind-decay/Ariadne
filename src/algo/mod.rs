pub mod blast_radius;
pub mod centrality;
pub mod scc;
pub mod stats;
pub mod subgraph;
pub mod topo_sort;

use std::collections::BTreeMap;

use crate::model::{CanonicalPath, Edge, EdgeType};

/// Filter edges to architectural types only (imports + re_exports + type_imports).
/// Excludes tests edges per D-034.
pub fn is_architectural(edge: &Edge) -> bool {
    matches!(
        edge.edge_type,
        EdgeType::Imports | EdgeType::ReExports | EdgeType::TypeImports
    )
}

/// Build forward and reverse adjacency indices from edges, filtered by predicate.
pub fn build_adjacency<'a>(
    edges: &'a [Edge],
    filter: fn(&Edge) -> bool,
) -> (
    BTreeMap<&'a CanonicalPath, Vec<&'a CanonicalPath>>,
    BTreeMap<&'a CanonicalPath, Vec<&'a CanonicalPath>>,
) {
    let mut forward: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    let mut reverse: BTreeMap<&CanonicalPath, Vec<&CanonicalPath>> = BTreeMap::new();
    for edge in edges {
        if filter(edge) {
            forward.entry(&edge.from).or_default().push(&edge.to);
            reverse.entry(&edge.to).or_default().push(&edge.from);
        }
    }
    (forward, reverse)
}
