//! `weak_spots` — cycles ∪ god-modules ∪ dead-code top-N.
//!
//! Block 1 tier-03 caps each of the three lists independently behind ONE
//! opaque multi-list cursor and projects the dead-symbol rows at the requested
//! verbosity (concise default) — all via the shared `ariadne_graph::economy`
//! helper so the cold and warm paths stay byte-identical. The economy cap +
//! cursor supersede the ad-hoc `MAX_DEAD` constant, so the dead-code remainder
//! is now reachable [src: .claude/plans/data-fidelity-arc/block-1/plan.md
//! D1-D5].

use std::cmp::Ordering;
use std::collections::BTreeSet;

use ariadne_core::SymbolId;
use ariadne_graph::economy::{self, Budget, Verbosity};
use ariadne_graph::{DeadCodeConfig, roots::is_root};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::{coupling_report, summarize};
use crate::types::{
    CouplingRow, CycleRow, SymbolSummary, Verbosity as WireVerbosity, WeakSpotsInput,
    WeakSpotsOutput,
};

/// Efferent-coupling threshold above which a file-as-module is flagged a
/// god module. Raised from 8 to 15 after the tier-14 dogfood: on
/// Ariadne's own 202-file repo, the `is_library_target` exclusion still
/// left 25 modules flagged (efferent 9–30) — a dense bulk of ordinary
/// Rust modules at 9–15 and a 4-module tail at 17+. 15 cuts at the top
/// of that bulk so the signal is an actionable tail, not noise
/// [src: tier-14 step 8 dogfood].
const GOD_THRESHOLD: u32 = 15;

/// Whether `path` names a Cargo *library* compilation target — i.e. not
/// an integration test, bench, or example, and not a build script.
///
/// God-module detection only flags library files: a high efferent count
/// in `tests/`, `benches/`, or `examples/` is the expected shape of a
/// test, bench, or example fanning out to the API under exercise, not an
/// architecture smell. Classification follows Cargo's directory
/// convention [src: <https://doc.rust-lang.org/cargo/guide/project-layout.html>];
/// per-language target classification is future work.
fn is_library_target(path: &str) -> bool {
    if path.rsplit('/').next() == Some("build.rs") {
        return false;
    }
    !path
        .split('/')
        .any(|c| matches!(c, "tests" | "benches" | "examples"))
}

/// Aggregate architecture-risk findings (cycles, god modules, dead code)
/// filtered by `input.prefix`, each list capped to one page sharing a single
/// multi-list cursor and the dead rows projected at `input.verbosity`.
///
/// # Errors
/// Returns [`McpError::InvalidInput`] when `input.cursor` is malformed or was
/// minted against a different index revision.
pub fn handle(cat: &Catalog, input: &WeakSpotsInput) -> Result<WeakSpotsOutput, McpError> {
    let modules = coupling_report::build_modules(cat, input.prefix.as_deref());
    let coupling = cat.graph.coupling_report(&modules);
    let god_modules: Vec<CouplingRow> = coupling
        .rows
        .into_iter()
        .filter(|m| is_library_target(&m.name) && m.efferent > GOD_THRESHOLD)
        .map(|m| CouplingRow {
            module: m.name,
            afferent: m.afferent,
            efferent: m.efferent,
            instability: m.instability,
            abstractness: m.abstractness,
            distance: m.distance,
        })
        .collect();

    let cycles: Vec<CycleRow> = cat
        .graph
        .cycle_report()
        .cycles
        .into_iter()
        .map(|cycle| {
            let mut members: Vec<String> = cycle
                .members
                .into_iter()
                .filter_map(|sid| cat.meta_of(sid).map(|m| m.name.clone()))
                .collect();
            members.sort();
            CycleRow { members }
        })
        .collect();

    let mut roots: BTreeSet<SymbolId> = BTreeSet::new();
    for (id, meta) in &cat.symbols {
        if is_root(
            meta.lang,
            meta.visibility,
            &meta.attributes,
            &meta.kind,
            &meta.name,
        ) {
            roots.insert(*id);
        }
    }
    let cfg = DeadCodeConfig {
        roots,
        ..Default::default()
    };
    let dead_symbols: Vec<SymbolSummary> = cat
        .graph
        .dead_code(&cfg)
        .symbols
        .into_iter()
        .map(|d| summarize(cat, d.id))
        .collect();

    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let cursor = input
        .cursor
        .as_deref()
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
        .map_err(|e| McpError::InvalidInput(e.to_string()))?;
    Ok(page(
        cycles,
        god_modules,
        dead_symbols,
        cursor,
        input,
        revision,
    ))
}

/// Map the MCP-facing verbosity onto the economy use case's verbosity.
fn to_economy(v: WireVerbosity) -> Verbosity {
    match v {
        WireVerbosity::Concise => Verbosity::Concise,
        WireVerbosity::Detailed => Verbosity::Detailed,
    }
}

/// Stable order for the cycle page: by first member, then cycle size (D4).
fn cmp_cycle(a: &CycleRow, b: &CycleRow) -> Ordering {
    a.members
        .first()
        .cmp(&b.members.first())
        .then(a.members.len().cmp(&b.members.len()))
}

/// Stable order for the god-module page: most-efferent first, then module path
/// ascending (D4).
fn cmp_god(a: &CouplingRow, b: &CouplingRow) -> Ordering {
    b.efferent.cmp(&a.efferent).then(a.module.cmp(&b.module))
}

/// Stable order for the dead-symbol page: by file, then byte offset, then name
/// (D4). Read before any concise projection nulls `byte_start`.
fn cmp_dead(a: &SymbolSummary, b: &SymbolSummary) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.name.cmp(&b.name))
}

/// Drop a dead-symbol row's cryptic id/offset fields in concise verbosity (D3).
fn project(mut sym: SymbolSummary, verbosity: Verbosity) -> SymbolSummary {
    if matches!(verbosity, Verbosity::Concise) {
        sym.id = None;
        sym.byte_start = None;
        sym.byte_end = None;
    }
    sym
}

/// Sort, cap, project, and steer the three lists behind one multi-list cursor.
/// Shared shape with the warm daemon handler so their JSON is byte-identical
/// (parity).
// Each parameter is a distinct sublist or page input; bundling them would only
// add indirection.
#[allow(clippy::too_many_arguments)]
fn page(
    cycles: Vec<CycleRow>,
    god_modules: Vec<CouplingRow>,
    dead_symbols: Vec<SymbolSummary>,
    cursor: Option<economy::Cursor>,
    input: &WeakSpotsInput,
    revision: u32,
) -> WeakSpotsOutput {
    let verbosity = to_economy(input.verbosity);
    let budget = Budget {
        limit: input.limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor,
        verbosity,
    };
    let total_cycles = cycles.len();
    let total_gods = god_modules.len();
    let total_dead = dead_symbols.len();
    let cycles_page = economy::paginate_sublist(cycles, cmp_cycle, &budget, 0);
    let gods_page = economy::paginate_sublist(god_modules, cmp_god, &budget, 1);
    let dead_page = economy::paginate_sublist(dead_symbols, cmp_dead, &budget, 2);
    let next_cursor = economy::multi_cursor(
        &[
            (cycles_page.next_offset, cycles_page.remainder),
            (gods_page.next_offset, gods_page.remainder),
            (dead_page.next_offset, dead_page.remainder),
        ],
        revision,
    );
    let mut truncated = Vec::new();
    if cycles_page.remainder {
        truncated.push((cycles_page.rows.len(), total_cycles, "cycles"));
    }
    if gods_page.remainder {
        truncated.push((gods_page.rows.len(), total_gods, "god_modules"));
    }
    if dead_page.remainder {
        truncated.push((dead_page.rows.len(), total_dead, "dead_symbols"));
    }
    let note = next_cursor
        .as_ref()
        .map(|_| economy::multi_truncation_note(&truncated));
    WeakSpotsOutput {
        cycles: cycles_page.rows,
        god_modules: gods_page.rows,
        dead_symbols: dead_page
            .rows
            .into_iter()
            .map(|s| project(s, verbosity))
            .collect(),
        next_cursor,
        note,
    }
}
