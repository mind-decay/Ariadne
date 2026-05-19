//! "What files must I touch to change symbol X?" — ranked file list.
//!
//! Pipeline (tier-07 step 9):
//! 1. BFS over the reversed graph from the queried symbol, filtered to
//!    `Calls | Imports | TypeOf | Inherits`. Min-hop distances drive the
//!    reachable set.
//! 2. Cooper et al. `simple_fast` on the same reversed graph: each
//!    predecessor's dominance depth = hops up the immediate-dominator
//!    chain to the queried symbol.
//! 3. Group predecessors by `FileId` via a caller-supplied resolver and
//!    rank files by `Σ 1 / dominance_depth(symbol)`.
//! 4. Cap at `max_files`.

use std::collections::{BTreeMap, VecDeque};

use ariadne_core::{FileId, SymbolId};
use petgraph::Direction::Incoming;
use petgraph::algo::dominators::{Dominators, simple_fast};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, Reversed};

use crate::build::{EdgeKindSet, GraphIndex};

/// One row of [`PlanAssist`].
#[derive(Debug, Clone)]
pub struct PlanFile {
    /// File the row is about.
    pub file: FileId,
    /// Per-symbol reasons collected during the walk.
    pub why: Vec<SymbolId>,
    /// Rank score; higher = stronger reason to touch.
    pub certainty: f32,
}

/// Ranked list of files implicated by changing the queried symbol.
#[derive(Debug, Clone, Default)]
pub struct PlanAssist {
    /// Ranked rows; first row has the highest certainty.
    pub files: Vec<PlanFile>,
}

impl GraphIndex {
    /// Compute the ranked plan-assist file list.
    #[must_use]
    pub fn plan_assist(
        &self,
        symbol: SymbolId,
        max_files: usize,
        file_of: &dyn Fn(SymbolId) -> Option<FileId>,
    ) -> PlanAssist {
        let Some(&start) = self.index.get(&symbol) else {
            return PlanAssist::default();
        };
        let kinds = EdgeKindSet::CALLS
            | EdgeKindSet::IMPORTS
            | EdgeKindSet::TYPE_OF
            | EdgeKindSet::INHERITS;

        let mut reachable = fxhash::FxHashSet::default();
        reachable.insert(start);
        let mut q: VecDeque<NodeIndex> = VecDeque::new();
        q.push_back(start);
        while let Some(node) = q.pop_front() {
            for er in self.graph.edges_directed(node, Incoming) {
                if !kinds.contains(er.weight().kind.to_flag()) {
                    continue;
                }
                let src = er.source();
                if reachable.insert(src) {
                    q.push_back(src);
                }
            }
        }
        reachable.remove(&start);
        if reachable.is_empty() {
            return PlanAssist::default();
        }

        let doms = simple_fast(Reversed(&self.graph), start);
        let mut buckets: BTreeMap<FileId, (Vec<SymbolId>, f32)> = BTreeMap::new();
        for ix in &reachable {
            let sid = self.graph[*ix];
            let Some(file) = file_of(sid) else {
                continue;
            };
            let depth = dominance_depth(&doms, *ix, start);
            let entry = buckets.entry(file).or_insert_with(|| (Vec::new(), 0.0));
            entry.0.push(sid);
            entry.1 += inv_depth(depth);
        }

        let mut files: Vec<PlanFile> = buckets
            .into_iter()
            .map(|(file, (mut why, certainty))| {
                why.sort();
                PlanFile {
                    file,
                    why,
                    certainty,
                }
            })
            .collect();
        files.sort_by(|a, b| {
            b.certainty
                .partial_cmp(&a.certainty)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.file.get().cmp(&b.file.get()))
        });
        files.truncate(max_files);
        PlanAssist { files }
    }
}

fn dominance_depth(doms: &Dominators<NodeIndex>, node: NodeIndex, root: NodeIndex) -> Option<u32> {
    if node == root {
        return Some(0);
    }
    let mut hops = 0u32;
    let mut cursor = node;
    while let Some(parent) = doms.immediate_dominator(cursor) {
        hops = hops.saturating_add(1);
        if parent == root {
            return Some(hops);
        }
        cursor = parent;
    }
    None
}

#[allow(clippy::cast_possible_truncation)]
fn inv_depth(depth: Option<u32>) -> f32 {
    match depth {
        Some(d) if d > 0 => (1.0_f64 / f64::from(d)) as f32,
        _ => 0.0,
    }
}
