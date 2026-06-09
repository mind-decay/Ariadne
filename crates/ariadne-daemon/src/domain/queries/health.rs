//! Architecture-health queries: `coupling_report`, `weak_spots`.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use ariadne_core::{
    CouplingReport, CouplingRow, CycleRow, DaemonResponse, FileId, SymbolId, SymbolSummary,
    Verbosity, WeakSpotsReport,
};
use ariadne_graph::economy::{self, Budget, Verbosity as EconVerbosity};
use ariadne_graph::{CouplingMetrics, DeadCodeConfig, ModuleSpec, roots::is_root};

use crate::domain::catalog::WarmCatalog;
use crate::domain::dispatch::summarize;

/// Efferent-coupling threshold above which a library file is a god module
/// (matches the v1 MCP tuning) [src: tier-14 step 8 dogfood].
const GOD_THRESHOLD: u32 = 15;

/// Project catalog symbols into one [`ModuleSpec`] per file, gated by an
/// optional path prefix.
pub(crate) fn build_modules(cat: &WarmCatalog, prefix: Option<&str>) -> Vec<ModuleSpec> {
    let mut by_file: BTreeMap<FileId, BTreeSet<SymbolId>> = BTreeMap::new();
    for (sid, meta) in &cat.symbols {
        by_file.entry(meta.file).or_default().insert(*sid);
    }
    let mut out = Vec::with_capacity(by_file.len());
    for (fid, members) in by_file {
        let Some(path) = cat.path_of(fid) else {
            continue;
        };
        if let Some(p) = prefix {
            if !path.starts_with(p) {
                continue;
            }
        }
        out.push(ModuleSpec {
            name: path.to_owned(),
            members,
            abstract_members: BTreeSet::new(),
        });
    }
    out
}

fn to_row(m: &CouplingMetrics) -> CouplingRow {
    CouplingRow {
        module: m.name.clone(),
        afferent: m.afferent,
        efferent: m.efferent,
        instability: m.instability,
        abstractness: m.abstractness,
        distance: m.distance,
    }
}

/// Per-file Martin coupling metrics filtered by `prefix`, capped to one page in
/// stable (Ca desc, module asc) order — the warm twin of the cold
/// `tools::coupling_report` handler, so their JSON is byte-identical (parity).
/// A malformed / stale cursor surfaces as a typed `DaemonResponse::InvalidInput`.
pub(crate) fn coupling_report(
    cat: &WarmCatalog,
    prefix: Option<&str>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let modules = build_modules(cat, prefix);
    let report = cat.graph.coupling_report(&modules);
    let rows: Vec<CouplingRow> = report.rows.iter().map(to_row).collect();
    let revision = u32::try_from(cat.revision).unwrap_or(u32::MAX);
    let decoded = match cursor
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::InvalidInput(err.to_string()),
    };
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded,
        verbosity: to_economy(verbosity),
    };
    let total = rows.len();
    let paged = economy::paginate(rows, cmp_row, &budget, revision, 0);
    let note = paged
        .next_cursor
        .as_ref()
        .map(|_| economy::truncation_note(paged.rows.len(), total, "modules"));
    DaemonResponse::Coupling(CouplingReport {
        rows: paged.rows,
        next_cursor: paged.next_cursor,
        note,
    })
}

/// Map the protocol verbosity onto the economy use case's verbosity.
fn to_economy(v: Verbosity) -> EconVerbosity {
    match v {
        Verbosity::Concise => EconVerbosity::Concise,
        Verbosity::Detailed => EconVerbosity::Detailed,
    }
}

/// Stable order for a coupling page (identical to the cold handler — keeps the
/// paths byte-identical, D4): afferent desc, then module path ascending.
fn cmp_row(a: &CouplingRow, b: &CouplingRow) -> Ordering {
    b.afferent
        .cmp(&a.afferent)
        .then_with(|| a.module.cmp(&b.module))
}

/// Whether `path` names a Cargo *library* compilation target — not an
/// integration test, bench, example, or build script. God-module detection
/// flags only library files (matches the v1 MCP exclusion).
fn is_library_target(path: &str) -> bool {
    if path.rsplit('/').next() == Some("build.rs") {
        return false;
    }
    !path
        .split('/')
        .any(|c| matches!(c, "tests" | "benches" | "examples"))
}

/// Cycles ∪ god modules ∪ dead-code candidates, filtered by `prefix`, each list
/// capped to one page sharing a single multi-list cursor and the dead rows
/// projected at `verbosity` — the warm twin of the cold `tools::weak_spots`
/// handler, so their JSON is byte-identical (parity). The economy cap + cursor
/// supersede the ad-hoc `MAX_DEAD` cap, so the dead-code remainder is reachable.
/// The dead-code pass excludes the per-language root set so `main`, exported
/// API, and test functions do not surface (tier-05 RD4). A malformed / stale
/// cursor surfaces as a typed `DaemonResponse::InvalidInput`.
// A linear handler: build the three lists, decode the cursor, paginate each, and
// assemble the report. The per-sublist sort/cap/note carries it over the line
// lint; the cold twin (`tools::weak_spots`) splits a `page` helper out, but the
// warm path returns `DaemonResponse` throughout, so an inline body reads clearer.
#[allow(clippy::too_many_lines)]
pub(crate) fn weak_spots(
    cat: &WarmCatalog,
    prefix: Option<&str>,
    limit: Option<u32>,
    cursor: Option<&str>,
    verbosity: Verbosity,
) -> DaemonResponse {
    let modules = build_modules(cat, prefix);
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
    let decoded = match cursor
        .map(|c| economy::Cursor::decode(c, revision))
        .transpose()
    {
        Ok(c) => c,
        Err(err) => return DaemonResponse::InvalidInput(err.to_string()),
    };
    let econ = to_economy(verbosity);
    let budget = Budget {
        limit: limit.map_or(economy::DEFAULT_PAGE, |l| l as usize),
        cursor: decoded,
        verbosity: econ,
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
    DaemonResponse::WeakSpots(WeakSpotsReport {
        cycles: cycles_page.rows,
        god_modules: gods_page.rows,
        dead_symbols: dead_page
            .rows
            .into_iter()
            .map(|s| project_dead(s, econ))
            .collect(),
        next_cursor,
        note,
    })
}

/// Stable order for the cycle page (identical to the cold handler, D4): by
/// first member, then cycle size.
fn cmp_cycle(a: &CycleRow, b: &CycleRow) -> Ordering {
    a.members
        .first()
        .cmp(&b.members.first())
        .then(a.members.len().cmp(&b.members.len()))
}

/// Stable order for the god-module page (identical to the cold handler, D4):
/// most-efferent first, then module path ascending.
fn cmp_god(a: &CouplingRow, b: &CouplingRow) -> Ordering {
    b.efferent.cmp(&a.efferent).then(a.module.cmp(&b.module))
}

/// Stable order for the dead-symbol page (identical to the cold handler, D4): by
/// file, then byte offset, then name. Read before any concise projection nulls
/// `byte_start`.
fn cmp_dead(a: &SymbolSummary, b: &SymbolSummary) -> Ordering {
    a.file
        .cmp(&b.file)
        .then(a.byte_start.cmp(&b.byte_start))
        .then(a.name.cmp(&b.name))
}

/// Drop a dead-symbol row's cryptic id/offset fields in concise verbosity (D3),
/// matching the cold handler.
fn project_dead(mut sym: SymbolSummary, verbosity: EconVerbosity) -> SymbolSummary {
    if matches!(verbosity, EconVerbosity::Concise) {
        sym.id = None;
        sym.byte_start = None;
        sym.byte_end = None;
    }
    sym
}
