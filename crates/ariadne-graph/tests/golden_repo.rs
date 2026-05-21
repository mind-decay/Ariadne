//! Golden-repo snapshot tests. Builds a hand-crafted mini "repo"
//! (5 files, 8 symbols, mixed edges) directly via the `GraphIndex`
//! constructor surface, then snapshots each analytic output with
//! `insta` [src: tier-07 step 10].
//!
//! Insta review: `cargo insta review -p ariadne-graph`. New goldens are
//! committed by the spec-build session that produces them.

use std::collections::BTreeSet;

use ariadne_core::{FileId, SymbolId};
use ariadne_graph::{
    CouplingMetrics, CouplingReport, Cycle, CycleReport, DeadCodeConfig, DeadSymbol, EdgeKind,
    EdgeKindSet, GraphIndex, ModuleSpec, PlanFile,
};

fn fid(n: u32) -> FileId {
    FileId::new(n).expect("non-zero")
}

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("non-zero")
}

struct MiniRepo {
    graph: GraphIndex,
    file_of: std::collections::HashMap<SymbolId, FileId>,
    modules: Vec<ModuleSpec>,
}

fn mini_repo() -> MiniRepo {
    // Symbols: lib::main(1) → lib::router(2) → lib::handler(3)
    //          util::log(4) ← lib::handler(3)
    //          model::User(5) (struct, abstract); model::UserTrait(6)
    //          orphan::dead(7) — no callers, not exported
    //          tests::test_router(8) — calls 2
    let mut g = GraphIndex::new();
    for s in 1u64..=8 {
        g.add_symbol(sid(s));
    }
    g.add_edge(sid(1), sid(2), EdgeKind::Calls);
    g.add_edge(sid(2), sid(3), EdgeKind::Calls);
    g.add_edge(sid(3), sid(4), EdgeKind::Calls);
    g.add_edge(sid(3), sid(5), EdgeKind::TypeOf);
    g.add_edge(sid(6), sid(5), EdgeKind::Inherits);
    g.add_edge(sid(8), sid(2), EdgeKind::Calls);

    let file_of = [
        (sid(1), fid(1)), // lib/main.rs
        (sid(2), fid(1)),
        (sid(3), fid(2)), // lib/handler.rs
        (sid(4), fid(3)), // util/log.rs
        (sid(5), fid(4)), // model/user.rs
        (sid(6), fid(4)),
        (sid(7), fid(5)), // orphan/dead.rs
        (sid(8), fid(6)), // tests/router_test.rs
    ]
    .into_iter()
    .collect();

    let modules = vec![
        ModuleSpec {
            name: "lib".into(),
            members: BTreeSet::from([sid(1), sid(2), sid(3)]),
            abstract_members: BTreeSet::new(),
        },
        ModuleSpec {
            name: "util".into(),
            members: BTreeSet::from([sid(4)]),
            abstract_members: BTreeSet::new(),
        },
        ModuleSpec {
            name: "model".into(),
            members: BTreeSet::from([sid(5), sid(6)]),
            abstract_members: BTreeSet::from([sid(6)]),
        },
    ];

    MiniRepo {
        graph: g,
        file_of,
        modules,
    }
}

fn fmt_blast(label: &str, br: &ariadne_graph::BlastRadius) -> String {
    let must = ids(&br.must_touch);
    let may = ids(&br.may_touch);
    format!(
        "{label}: must={must:?} may={may:?} depth_used={}",
        br.depth_used
    )
}

fn ids(v: &[SymbolId]) -> Vec<u64> {
    v.iter().map(|s| s.get()).collect()
}

fn fmt_cycles(rep: &CycleReport) -> String {
    let lines: Vec<String> = rep
        .cycles
        .iter()
        .map(|Cycle { members }| format!("{:?}", ids(members)))
        .collect();
    lines.join("\n")
}

fn fmt_coupling(rep: &CouplingReport) -> String {
    rep.rows
        .iter()
        .map(
            |CouplingMetrics {
                 name,
                 afferent,
                 efferent,
                 instability,
                 abstractness,
                 distance,
             }| {
                format!(
                    "{name}: Ca={afferent} Ce={efferent} I={instability:.2} A={abstractness:.2} d={distance:.2}"
                )
            },
        )
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_dead(rep: &ariadne_graph::DeadCodeReport) -> String {
    rep.symbols
        .iter()
        .map(|DeadSymbol { id, reason }| format!("sid={} reason={reason}", id.get()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_plan(rep: &ariadne_graph::PlanAssist) -> String {
    rep.files
        .iter()
        .map(
            |PlanFile {
                 file,
                 why,
                 certainty,
             }| {
                format!(
                    "file={} certainty={:.3} why={:?}",
                    file.get(),
                    certainty,
                    ids(why)
                )
            },
        )
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn golden_blast_radius_user_struct() {
    let repo = mini_repo();
    let br = repo
        .graph
        .blast_radius(sid(5), 10, EdgeKindSet::ALL)
        .expect("sid(5) present");
    insta::assert_snapshot!(fmt_blast("model::User", &br));
}

#[test]
fn blast_radius_distinguishes_absent_from_resolved_empty() {
    // Two-symbol graph: A → B. `add_edge` auto-inserts both endpoints.
    // A has zero inbound edges; B has one.
    let mut g = GraphIndex::new();
    g.add_edge(sid(1), sid(2), EdgeKind::Calls);

    // A `SymbolId` never inserted into the node set is a graph-level
    // miss — "not analysed", reported as `None`.
    assert!(
        g.blast_radius(sid(99), 3, EdgeKindSet::ALL).is_none(),
        "absent symbol must return None"
    );

    // A present symbol with no inbound edges resolves to `Some` with an
    // empty radius — a true "no dependents" answer, distinct from `None`.
    let radius = g
        .blast_radius(sid(1), 3, EdgeKindSet::ALL)
        .expect("present symbol must return Some");
    assert!(
        radius.must_touch.is_empty(),
        "no dependents → empty must_touch"
    );
    assert!(
        radius.may_touch.is_empty(),
        "no dependents → empty may_touch"
    );
}

#[test]
fn golden_cycles_empty() {
    let repo = mini_repo();
    insta::assert_snapshot!("cycles", fmt_cycles(&repo.graph.cycle_report()));
}

#[test]
fn golden_coupling() {
    let repo = mini_repo();
    insta::assert_snapshot!(
        "coupling",
        fmt_coupling(&repo.graph.coupling_report(&repo.modules))
    );
}

#[test]
fn golden_dead_code_orphan() {
    let repo = mini_repo();
    let cfg = DeadCodeConfig {
        tests: BTreeSet::from([sid(8)]),
        entry_points: BTreeSet::from([sid(1)]),
        ..Default::default()
    };
    insta::assert_snapshot!("dead", fmt_dead(&repo.graph.dead_code(&cfg)));
}

#[test]
fn golden_plan_assist_router() {
    let repo = mini_repo();
    let lookup = |s: SymbolId| repo.file_of.get(&s).copied();
    let plan = repo.graph.plan_assist(sid(2), 10, &lookup);
    insta::assert_snapshot!("plan", fmt_plan(&plan));
}
