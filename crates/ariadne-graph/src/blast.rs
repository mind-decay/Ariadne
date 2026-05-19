//! Blast-radius analysis. Reverse-BFS filtered by edge kind, plus
//! Cooper et al. immediate dominators (`simple_fast`) to rank "must
//! touch" vs "may touch" callers
//! [src: <https://docs.rs/petgraph/latest/petgraph/algo/dominators/fn.simple_fast.html>].

use std::collections::VecDeque;

use ariadne_core::SymbolId;
use petgraph::Direction::Incoming;
use petgraph::algo::dominators::simple_fast;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, Reversed};
use smallvec::SmallVec;

use crate::build::{EdgeKindSet, GraphIndex};

/// Result of [`GraphIndex::blast_radius`].
///
/// `must_touch` are direct reverse-1-hop predecessors of the queried
/// symbol (every caller funnel point). `may_touch` are deeper
/// predecessors reachable transitively through the filtered edge kinds.
/// `depth_used` is the deepest hop level any reported predecessor sits
/// at.
#[derive(Debug, Clone, Default)]
pub struct BlastRadius {
    /// First-hop callers / direct dependents.
    pub must_touch: Vec<SymbolId>,
    /// Transitive callers beyond the first hop.
    pub may_touch: Vec<SymbolId>,
    /// Largest hop depth in the returned set.
    pub depth_used: u8,
}

impl GraphIndex {
    /// Incoming-edge count for `symbol`. Returns 0 for unknown symbols.
    #[must_use]
    pub fn fan_in(&self, symbol: SymbolId) -> usize {
        self.index
            .get(&symbol)
            .map_or(0, |ix| self.graph.edges_directed(*ix, Incoming).count())
    }

    /// Outgoing-edge count for `symbol`. Returns 0 for unknown symbols.
    #[must_use]
    pub fn fan_out(&self, symbol: SymbolId) -> usize {
        self.index.get(&symbol).map_or(0, |ix| {
            self.graph
                .edges_directed(*ix, petgraph::Direction::Outgoing)
                .count()
        })
    }

    /// Compute the blast radius of `symbol`. See [`BlastRadius`] for the
    /// must/may distinction. Returns an empty radius for unknown
    /// symbols.
    #[must_use]
    pub fn blast_radius(&self, symbol: SymbolId, depth: u8, kinds: EdgeKindSet) -> BlastRadius {
        let Some(&start) = self.index.get(&symbol) else {
            return BlastRadius::default();
        };
        let preds = self.reverse_bfs(start, depth, kinds);
        if preds.is_empty() {
            return BlastRadius::default();
        }
        let doms = simple_fast(Reversed(&self.graph), start);
        let mut must = SmallVec::<[SymbolId; 8]>::new();
        let mut may = Vec::with_capacity(preds.len());
        let mut depth_used = 0u8;
        for &(ix, d) in &preds {
            depth_used = depth_used.max(d);
            let sid = self.graph[ix];
            if doms.immediate_dominator(ix) == Some(start) {
                must.push(sid);
            } else {
                may.push(sid);
            }
        }
        let mut must_vec: Vec<SymbolId> = must.into_vec();
        must_vec.sort();
        may.sort();
        BlastRadius {
            must_touch: must_vec,
            may_touch: may,
            depth_used,
        }
    }

    fn reverse_bfs(&self, start: NodeIndex, depth: u8, kinds: EdgeKindSet) -> Vec<(NodeIndex, u8)> {
        let mut out = Vec::new();
        let mut visited = fxhash::FxHashSet::default();
        visited.insert(start);
        let mut q: VecDeque<(NodeIndex, u8)> = VecDeque::new();
        q.push_back((start, 0));
        while let Some((node, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            for er in self.graph.edges_directed(node, Incoming) {
                if !kinds.contains(er.weight().kind.to_flag()) {
                    continue;
                }
                let src = er.source();
                if visited.insert(src) {
                    out.push((src, d + 1));
                    q.push_back((src, d + 1));
                }
            }
        }
        out
    }
}
