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
