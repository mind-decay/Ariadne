//! Tier-09 step 9 — golden snapshots of the refactor suggestion lists,
//! one fixture per finding type (god module, cycle break, misplaced
//! symbol).
//!
//! Insta review: `cargo insta review -p ariadne-graph`.

mod support;

use ariadne_graph::{CycleBreakProposal, GodModuleFinding, MisplacedSymbol, refactor};

fn fmt_gods(v: &[GodModuleFinding]) -> String {
    v.iter()
        .map(|g| {
            let top: Vec<(u64, u32)> = g.top_outbound.iter().map(|(s, c)| (s.get(), *c)).collect();
            format!(
                "module={} Ce={} cohesion={:.4} top_outbound={top:?}\n  suggestion={}",
                g.module, g.efferent, g.cohesion, g.suggestion
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_props(v: &[CycleBreakProposal]) -> String {
    v.iter()
        .map(|p| {
            format!(
                "{} -> {} score={:.4}\n  rationale={}",
                p.from.get(),
                p.to.get(),
                p.score,
                p.rationale
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn fmt_misplaced(v: &[MisplacedSymbol]) -> String {
    v.iter()
        .map(|m| {
            format!(
                "symbol={} {} -> {} ratio={:.2}",
                m.symbol.get(),
                m.current_module,
                m.target_module,
                m.ratio
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn golden_god_modules() {
    let fx = support::core_fixture();
    let gods =
        refactor::god_modules(&fx.graph, &fx.snapshot, &fx.modules, 2.0).expect("god_modules scan");
    insta::assert_snapshot!("god_modules", fmt_gods(&gods));
}

/// The split suggestion must name an extractable *member* of the module
/// (D1), never an external callee, and re-keying the histogram must not
/// silently redefine `Ce` (it stays the count of distinct external
/// targets) [src: plan.md D1/D3, `exit_criteria`].
#[test]
fn god_module_split_names_a_member_and_pins_ce() {
    use std::collections::BTreeSet;

    let fx = support::core_fixture();
    let gods =
        refactor::god_modules(&fx.graph, &fx.snapshot, &fx.modules, 2.0).expect("god_modules scan");
    let finding = gods.first().expect("core qualifies as a god module");
    let module = support::module_named(&fx.modules, &finding.module);

    // D1: top_outbound[0] names a member, so "extract <member>" is coherent.
    let named = finding.top_outbound[0].0;
    assert!(
        module.members.contains(&named),
        "top_outbound[0] {named:?} must be a member of module {}",
        finding.module
    );

    // Ce = distinct external targets — recomputed straight from the fixture
    // so the re-keying cannot redefine it.
    let members: BTreeSet<u64> = module.members.iter().map(|s| s.get()).collect();
    let expected_ce = support::edges()
        .iter()
        .filter(|&&(src, dst, _)| members.contains(&src) && !members.contains(&dst))
        .map(|&(_, dst, _)| dst)
        .collect::<BTreeSet<u64>>()
        .len();
    assert_eq!(
        finding.efferent as usize, expected_ce,
        "efferent (Ce) must equal the count of distinct external targets"
    );
}

#[test]
fn golden_cycle_break() {
    let fx = support::core_fixture();
    let cycles = fx.graph.cycle_report();
    let scc = cycles.cycles.first().expect("fixture has one cycle");
    let props = refactor::cycle_break_proposals(&fx.graph, scc);
    insta::assert_snapshot!("cycle_break", fmt_props(&props));
}

#[test]
fn golden_misplaced_symbols() {
    let (graph, modules) = support::misplaced_fixture();
    let mis = refactor::misplaced_symbols(&graph, &modules);
    insta::assert_snapshot!("misplaced", fmt_misplaced(&mis));
}
