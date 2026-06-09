//! Architecture-fitness engine (block A, A3).
//!
//! Productises the project's own `tests/architecture.rs` as config-driven
//! fitness functions: a declarative layer assignment + forbidden dependency
//! directions + cycle/coupling thresholds, checked against the live graph
//! \[src: `ArchUnit` `layeredArchitecture`
//! <https://www.baeldung.com/java-archunit-intro>;
//! .claude/plans/intelligence-platform/block-a/plan.md D5\].
//!
//! The engine is pure: it reuses the existing
//! [`cycle_report`](GraphIndex::cycle_report) and
//! [`coupling_report`](GraphIndex::coupling_report) analytics and adds no new
//! metric code (block-a plan.md BR5). Rule parsing + glob → layer resolution
//! happen at the composition root; this function receives the already-resolved
//! [`FitnessRules`] plus the symbol → file assignment the graph nodes lack
//! (graph nodes are bare `SymbolId`s — the file each lives in is a companion
//! input, mirroring how `coupling_report` takes its `ModuleSpec` members).
//!
//! Deterministic: every output collection is sorted, so the report is
//! byte-identical across runs on the same inputs (block-a plan.md
//! `<constraints>`).

use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{FileId, SymbolId};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::build::GraphIndex;
use crate::coupling::ModuleSpec;

/// Resolved architecture-fitness rules the engine checks against the graph.
///
/// Produced at the composition root from `ariadne-fitness.toml` (ADR-0028):
/// each layer's path globs are resolved against the indexed file paths to
/// build [`FitnessRules::layer_of`], the `forbid` rules become
/// [`FitnessRules::forbidden`] layer-name pairs, and the `[thresholds]` section
/// fixes the cycle / instability ceilings.
#[derive(Debug, Clone, Default)]
pub struct FitnessRules {
    /// Each indexed file's resolved layer name. Files matching no layer are
    /// absent and excluded from the dependency-direction check.
    pub layer_of: BTreeMap<FileId, String>,
    /// Forbidden `(from_layer, to_layer)` dependency directions: any inter-file
    /// edge whose endpoints resolve to such a pair is a violation.
    pub forbidden: Vec<(String, String)>,
    /// Maximum tolerated dependency cycles (SCCs of size ≥ 2). A graph with
    /// strictly more than this many cycles yields one [`Violation::Cycle`] per
    /// cycle.
    pub max_cycles: u32,
    /// Optional per-file instability (`I = Ce / (Ca + Ce)`) ceiling. A file
    /// whose instability strictly exceeds it is a [`Violation::Instability`].
    /// `None` disables the coupling check.
    pub max_instability: Option<f32>,
}

/// A single architecture-fitness violation.
#[derive(Debug, Clone, PartialEq)]
pub enum Violation {
    /// An inter-file dependency crossed a forbidden layer boundary. Reported
    /// once per `(from_file, to_file)` ordered pair.
    ForbiddenDependency {
        /// Resolved layer of the depending (source) file.
        from_layer: String,
        /// Resolved layer of the depended-on (target) file.
        to_layer: String,
        /// The depending (source) file.
        from_file: FileId,
        /// The depended-on (target) file.
        to_file: FileId,
    },
    /// A dependency cycle present when the cycle count exceeds `max_cycles`.
    Cycle {
        /// Sorted symbols participating in the cycle (from [`crate::Cycle`]).
        members: Vec<SymbolId>,
    },
    /// A file whose instability exceeded the configured ceiling.
    Instability {
        /// The over-coupled file.
        file: FileId,
        /// The file's measured instability `I = Ce / (Ca + Ce)`.
        instability: f32,
    },
}

/// Outcome of [`GraphIndex::fitness_check`]: the sorted violations and an
/// `ok` flag (`true` exactly when there are none).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FitnessReport {
    /// Every violation found, sorted deterministically (forbidden dependencies
    /// by `(from_file, to_file)`, then cycles by members, then instabilities by
    /// file).
    pub violations: Vec<Violation>,
    /// `true` when `violations` is empty — the architecture passes.
    pub ok: bool,
}

impl GraphIndex {
    /// Check the graph against `rules`, using `symbol_files` to resolve each
    /// graph node (a `SymbolId`) to the file — and thereby the layer — it lives
    /// in.
    ///
    /// Runs three independent checks, each reusing existing analytics:
    /// (a) every inter-file edge whose `(from_layer, to_layer)` is in
    /// [`FitnessRules::forbidden`] yields a [`Violation::ForbiddenDependency`]
    /// (deduped per file pair); (b) when [`cycle_report`](Self::cycle_report)
    /// finds more than `max_cycles` cycles, each becomes a [`Violation::Cycle`];
    /// (c) when `max_instability` is set, each file whose
    /// [`coupling_report`](Self::coupling_report) instability exceeds it yields
    /// a [`Violation::Instability`].
    ///
    /// Deterministic: the violations are sorted before returning, so re-runs on
    /// the same inputs are byte-identical.
    #[must_use]
    pub fn fitness_check(
        &self,
        symbol_files: &BTreeMap<SymbolId, FileId>,
        rules: &FitnessRules,
    ) -> FitnessReport {
        let mut violations = Vec::new();
        self.collect_forbidden_dependencies(symbol_files, rules, &mut violations);
        self.collect_cycle_violations(rules, &mut violations);
        self.collect_instability_violations(symbol_files, rules, &mut violations);
        violations.sort_by_key(order_key);
        let ok = violations.is_empty();
        FitnessReport { violations, ok }
    }

    /// (a) Dependency-direction: each inter-file edge whose endpoints resolve
    /// to a forbidden `(from_layer, to_layer)` pair, deduped per
    /// `(from_file, to_file)`.
    fn collect_forbidden_dependencies(
        &self,
        symbol_files: &BTreeMap<SymbolId, FileId>,
        rules: &FitnessRules,
        out: &mut Vec<Violation>,
    ) {
        if rules.forbidden.is_empty() {
            return;
        }
        let forbidden: BTreeSet<(&str, &str)> = rules
            .forbidden
            .iter()
            .map(|(f, t)| (f.as_str(), t.as_str()))
            .collect();
        let mut seen: BTreeSet<(FileId, FileId)> = BTreeSet::new();
        for er in self.graph.edge_references() {
            let src_sym = self.graph[er.source()];
            let dst_sym = self.graph[er.target()];
            let (Some(&from_file), Some(&to_file)) =
                (symbol_files.get(&src_sym), symbol_files.get(&dst_sym))
            else {
                continue;
            };
            if from_file == to_file {
                continue;
            }
            let (Some(from_layer), Some(to_layer)) =
                (rules.layer_of.get(&from_file), rules.layer_of.get(&to_file))
            else {
                continue;
            };
            if forbidden.contains(&(from_layer.as_str(), to_layer.as_str()))
                && seen.insert((from_file, to_file))
            {
                out.push(Violation::ForbiddenDependency {
                    from_layer: from_layer.clone(),
                    to_layer: to_layer.clone(),
                    from_file,
                    to_file,
                });
            }
        }
    }

    /// (b) Cycles: when the cycle count exceeds `max_cycles`, each cycle is a
    /// violation. Reuses [`cycle_report`](Self::cycle_report) (BR5 — no new
    /// metric code).
    fn collect_cycle_violations(&self, rules: &FitnessRules, out: &mut Vec<Violation>) {
        let report = self.cycle_report();
        let count = u32::try_from(report.cycles.len()).unwrap_or(u32::MAX);
        if count > rules.max_cycles {
            for cycle in report.cycles {
                out.push(Violation::Cycle {
                    members: cycle.members,
                });
            }
        }
    }

    /// (c) Coupling: each file whose instability exceeds `max_instability`.
    /// Reuses [`coupling_report`](Self::coupling_report) over one module per
    /// file (BR5 — no new metric code).
    fn collect_instability_violations(
        &self,
        symbol_files: &BTreeMap<SymbolId, FileId>,
        rules: &FitnessRules,
        out: &mut Vec<Violation>,
    ) {
        let Some(max) = rules.max_instability else {
            return;
        };
        let (modules, name_to_file) = modules_by_file(symbol_files);
        let report = self.coupling_report(&modules);
        for row in report.rows {
            if row.instability > max {
                if let Some(&file) = name_to_file.get(&row.name) {
                    out.push(Violation::Instability {
                        file,
                        instability: row.instability,
                    });
                }
            }
        }
    }
}

/// Build one [`ModuleSpec`] per file (named by the file's numeric id so the
/// coupling rows map back to a [`FileId`] without leaking paths into the pure
/// engine) plus the name → file reverse map.
fn modules_by_file(
    symbol_files: &BTreeMap<SymbolId, FileId>,
) -> (Vec<ModuleSpec>, BTreeMap<String, FileId>) {
    let mut by_file: BTreeMap<FileId, BTreeSet<SymbolId>> = BTreeMap::new();
    for (&sid, &fid) in symbol_files {
        by_file.entry(fid).or_default().insert(sid);
    }
    let mut modules = Vec::with_capacity(by_file.len());
    let mut name_to_file = BTreeMap::new();
    for (fid, members) in by_file {
        let name = fid.get().to_string();
        name_to_file.insert(name.clone(), fid);
        modules.push(ModuleSpec {
            name,
            members,
            abstract_members: BTreeSet::new(),
        });
    }
    (modules, name_to_file)
}

/// Total-order sort key making the violation list deterministic. Variant rank
/// first (forbidden < cycle < instability), then variant-specific ids.
fn order_key(v: &Violation) -> (u8, u64, u64, Vec<u64>) {
    match v {
        Violation::ForbiddenDependency {
            from_file, to_file, ..
        } => (
            0,
            u64::from(from_file.get()),
            u64::from(to_file.get()),
            Vec::new(),
        ),
        Violation::Cycle { members } => (1, 0, 0, members.iter().map(|m| m.get()).collect()),
        Violation::Instability { file, .. } => (2, u64::from(file.get()), 0, Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EdgeKind;

    fn sid(n: u64) -> SymbolId {
        SymbolId::new(n).expect("nonzero symbol id")
    }

    fn fid(n: u32) -> FileId {
        FileId::new(n).expect("nonzero file id")
    }

    /// Map two symbols to a `core` file (1) and one symbol to an `adapter`
    /// file (2): symbols 1,2 ∈ core.rs, symbol 3 ∈ adapter.rs.
    fn core_adapter_files() -> BTreeMap<SymbolId, FileId> {
        BTreeMap::from([(sid(1), fid(1)), (sid(2), fid(1)), (sid(3), fid(2))])
    }

    fn layered_rules(forbidden: Vec<(&str, &str)>) -> FitnessRules {
        FitnessRules {
            layer_of: BTreeMap::from([(fid(1), "core".to_owned()), (fid(2), "adapter".to_owned())]),
            forbidden: forbidden
                .into_iter()
                .map(|(f, t)| (f.to_owned(), t.to_owned()))
                .collect(),
            max_cycles: u32::MAX,
            max_instability: None,
        }
    }

    /// A `core`-layer file depending on an `adapter`-layer file, with a rule
    /// forbidding `core → adapter`, yields exactly one violation (exit
    /// criterion #2 / tier step 1).
    #[test]
    fn forbidden_layer_edge_yields_exactly_one_violation() {
        let mut g = GraphIndex::new();
        // sid(1) ∈ core calls sid(3) ∈ adapter — a forbidden core → adapter edge.
        g.add_edge(sid(1), sid(3), EdgeKind::Calls);
        let rules = layered_rules(vec![("core", "adapter")]);

        let report = g.fitness_check(&core_adapter_files(), &rules);

        assert!(!report.ok, "a forbidden edge must fail the check");
        assert_eq!(report.violations.len(), 1, "exactly one violation");
        assert_eq!(
            report.violations[0],
            Violation::ForbiddenDependency {
                from_layer: "core".to_owned(),
                to_layer: "adapter".to_owned(),
                from_file: fid(1),
                to_file: fid(2),
            },
        );
    }

    /// The same forbidden rule, but the only edge runs the *allowed* direction
    /// (`adapter → core`): a clean graph yields no violations (exit criterion
    /// #2).
    #[test]
    fn clean_graph_yields_no_violations() {
        let mut g = GraphIndex::new();
        // sid(3) ∈ adapter calls sid(1) ∈ core — the allowed direction.
        g.add_edge(sid(3), sid(1), EdgeKind::Calls);
        let rules = layered_rules(vec![("core", "adapter")]);

        let report = g.fitness_check(&core_adapter_files(), &rules);

        assert!(report.ok, "the allowed direction must pass");
        assert!(report.violations.is_empty());
    }

    /// Two forbidden inter-file edges between the *same* file pair collapse to a
    /// single violation (per-`(from_file, to_file)` dedup): the count stays
    /// meaningful (exit criterion #2 "exactly one violation" for one forbidden
    /// dependency).
    #[test]
    fn multiple_edges_in_one_file_pair_dedupe_to_one_violation() {
        let mut g = GraphIndex::new();
        // Both core symbols (1, 2) depend on the adapter symbol (3): two edges,
        // one forbidden core.rs → adapter.rs file dependency.
        g.add_edge(sid(1), sid(3), EdgeKind::Calls);
        g.add_edge(sid(2), sid(3), EdgeKind::Imports);
        let rules = layered_rules(vec![("core", "adapter")]);

        let report = g.fitness_check(&core_adapter_files(), &rules);

        assert_eq!(report.violations.len(), 1, "one file-pair → one violation");
    }

    /// The cycle threshold trips correctly: a 2-cycle passes at `max_cycles: 1`
    /// and fails (one violation per cycle) at `max_cycles: 0` (exit criterion
    /// #2 "thresholds trip correctly").
    #[test]
    fn cycle_threshold_trips_correctly() {
        let mut g = GraphIndex::new();
        // A ↔ B cycle (one SCC of size 2).
        g.add_edge(sid(1), sid(2), EdgeKind::Calls);
        g.add_edge(sid(2), sid(1), EdgeKind::Calls);
        let files = BTreeMap::from([(sid(1), fid(1)), (sid(2), fid(1))]);

        let mut rules = FitnessRules {
            max_cycles: 1,
            ..FitnessRules::default()
        };
        assert!(
            g.fitness_check(&files, &rules).ok,
            "one cycle is within max_cycles=1",
        );

        rules.max_cycles = 0;
        let report = g.fitness_check(&files, &rules);
        assert_eq!(report.violations.len(), 1, "the cycle trips max_cycles=0");
        assert!(matches!(report.violations[0], Violation::Cycle { .. }));
    }

    /// The instability ceiling trips correctly: a file pointing entirely
    /// outward (`I = 1.0`) violates `max_instability: 0.5`, and disabling the
    /// ceiling (`None`) reports nothing (exit criterion #2).
    #[test]
    fn instability_threshold_trips_correctly() {
        let mut g = GraphIndex::new();
        // File 1's symbol depends on file 2's symbol: file 1 is fully unstable
        // (Ce=1, Ca=0 → I=1.0); file 2 is fully stable (I=0.0).
        g.add_edge(sid(1), sid(2), EdgeKind::Calls);
        let files = BTreeMap::from([(sid(1), fid(1)), (sid(2), fid(2))]);

        let rules = FitnessRules {
            max_instability: Some(0.5),
            ..FitnessRules::default()
        };
        let report = g.fitness_check(&files, &rules);
        assert_eq!(report.violations.len(), 1, "the unstable file trips 0.5");
        assert_eq!(
            report.violations[0],
            Violation::Instability {
                file: fid(1),
                instability: 1.0,
            },
        );

        let disabled = FitnessRules {
            max_instability: None,
            ..FitnessRules::default()
        };
        assert!(
            g.fitness_check(&files, &disabled).ok,
            "a None ceiling disables the coupling check",
        );
    }

    /// Re-running on the same inputs is byte-identical (determinism constraint).
    #[test]
    fn report_is_deterministic() {
        let mut g = GraphIndex::new();
        g.add_edge(sid(1), sid(3), EdgeKind::Calls);
        g.add_edge(sid(2), sid(3), EdgeKind::Calls);
        let files = core_adapter_files();
        let rules = layered_rules(vec![("core", "adapter")]);

        let a = g.fitness_check(&files, &rules);
        let b = g.fitness_check(&files, &rules);
        assert_eq!(a, b, "re-run must be identical");
    }
}
