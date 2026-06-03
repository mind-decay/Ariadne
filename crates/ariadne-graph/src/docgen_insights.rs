//! Deterministic project-overview insight helpers (tier-03). Each helper
//! synthesises one section of [`crate::docgen::for_project`] from the in-RAM
//! graph plus the git-history vectors — system-only signal, no raw metric
//! dumps. Pure `std::fmt::Write` into owned `String`s, ordered through
//! `BTreeMap`/`BTreeSet` and sorted vectors so the same revision renders
//! byte-identical output [src: .claude/plans/useful-docgen/tier-03 D2/D4/D5/D6].
//!
//! `write!` / `writeln!` into a `String` cannot fail, so every macro `Result`
//! is intentionally discarded with `let _ = …` (mirrors `docgen`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ariadne_core::{CoChangePair, FileChurn, Lang, ReadSnapshot, SymbolId};
use petgraph::Direction::{Incoming, Outgoing};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

use crate::build::GraphIndex;
use crate::co_change::{CoChangeConfig, CoChangeEdge, co_change_report};
use crate::coupling::{CouplingMetrics, ModuleSpec};
use crate::cycles::Cycle;
use crate::doc_model::{DocScope, LayerHint, crate_of};
use crate::docgen::purpose;
use crate::errors::GraphError;
use crate::heuristics::SymbolTable;
use crate::hotspot::{HotspotGrain, file_hotspots};
use crate::refactor::god_modules;

/// Cap on rows in any insight list (risk, clusters, god modules, coupling).
const LIST_N: usize = 10;
/// Representative SCC members printed before the "+N more" tail.
const REPR_MEMBERS: usize = 6;
/// Efferent-coupling threshold above which a crate is flagged a god module —
/// matches the daemon health tuning [src: daemon queries/health.rs:15].
const GOD_THRESHOLD: f32 = 15.0;

/// Crate bucket for a module name (a file path in production). Uses the
/// `crates/<name>/` prefix when present, else the path's first segment, so
/// every module maps to a deterministic crate key.
pub(crate) fn crate_key(name: &str) -> &str {
    crate_of(name).unwrap_or_else(|| name.split('/').next().unwrap_or(name))
}

/// Human label for a [`LayerHint`].
fn layer_label(layer: LayerHint) -> &'static str {
    match layer {
        LayerHint::Domain => "Domain",
        LayerHint::Adapter => "Adapter",
        LayerHint::Interior => "Interior",
    }
}

/// Crate-aware one-line role for a module (tier-04 step 2): the module name,
/// its owning crate (from the defining-file path), the hexagonal layer it sits
/// in, and the coupling-shape sentence (stable / volatile / intermediate)
/// shared with the project crate table [src: `docgen::purpose`].
pub(crate) fn module_role(name: &str, file_path: &str, metrics: &CouplingMetrics) -> String {
    let crate_name = crate_key(file_path);
    let layer = layer_label(LayerHint::of(file_path));
    format!(
        "`{name}` — crate `{crate_name}`, {layer} layer. {}",
        purpose(metrics)
    )
}

/// One-paragraph synopsis: scoped crate / layer counts, source symbol and edge
/// totals over the scoped set, and the languages present.
pub(crate) fn synopsis(
    graph: &GraphIndex,
    scoped: &[ModuleSpec],
    table: &SymbolTable,
    scope: &DocScope,
) -> String {
    let crates: BTreeSet<&str> = scoped.iter().map(|m| crate_key(&m.name)).collect();

    // LayerHint is not `Ord`/`Hash`; tally distinct layers in a fixed array.
    let mut layer_seen = [false; 3];
    for m in scoped {
        match LayerHint::of(&m.name) {
            LayerHint::Domain => layer_seen[0] = true,
            LayerHint::Adapter => layer_seen[1] = true,
            LayerHint::Interior => layer_seen[2] = true,
        }
    }
    let layer_count = layer_seen.iter().filter(|b| **b).count();

    let mut langs: BTreeSet<Lang> = BTreeSet::new();
    for path in table.file_paths() {
        if !scope.include(path) {
            continue;
        }
        if let Some(lang) = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(Lang::from_extension)
        {
            langs.insert(lang);
        }
    }
    let lang_list = if langs.is_empty() {
        "none".to_owned()
    } else {
        langs.iter().map(Lang::tag).collect::<Vec<_>>().join(", ")
    };

    let scoped_syms: BTreeSet<SymbolId> = scoped
        .iter()
        .flat_map(|m| m.members.iter().copied())
        .collect();
    let mut edges = 0usize;
    for er in graph.graph.edge_references() {
        if scoped_syms.contains(&graph.graph[er.source()])
            && scoped_syms.contains(&graph.graph[er.target()])
        {
            edges += 1;
        }
    }

    let mut md = String::new();
    let _ = writeln!(
        md,
        "{} crate(s) · {} layer(s) · {} source symbol(s) · {} dependency edge(s) · \
languages: {lang_list}.",
        crates.len(),
        layer_count,
        scoped_syms.len(),
        edges
    );
    md.push('\n');
    md
}

/// Crate-level architecture table: one row per scoped crate with its dominant
/// layer and coupling-shape role, preceded by the sidecar SVG reference (D4).
pub(crate) fn architecture_section(graph: &GraphIndex, scoped: &[ModuleSpec]) -> String {
    let mut members: BTreeMap<String, BTreeSet<SymbolId>> = BTreeMap::new();
    let mut layer_votes: BTreeMap<String, [u32; 3]> = BTreeMap::new();
    for m in scoped {
        let key = crate_key(&m.name).to_owned();
        members
            .entry(key.clone())
            .or_default()
            .extend(m.members.iter().copied());
        let votes = layer_votes.entry(key).or_insert([0; 3]);
        match LayerHint::of(&m.name) {
            LayerHint::Domain => votes[0] += 1,
            LayerHint::Adapter => votes[1] += 1,
            LayerHint::Interior => votes[2] += 1,
        }
    }
    let specs: Vec<ModuleSpec> = members
        .into_iter()
        .map(|(name, members)| ModuleSpec {
            name,
            members,
            abstract_members: BTreeSet::new(),
        })
        .collect();
    let coupling = graph.coupling_report(&specs);

    let mut md = String::from("![architecture](codebase-overview.svg)\n\n");
    md.push_str("| Crate | Layer | Role |\n| --- | --- | --- |\n");
    for row in &coupling.rows {
        let votes = layer_votes.get(&row.name).copied().unwrap_or([0; 3]);
        let _ = writeln!(
            md,
            "| `{}` | {} | {} |",
            row.name,
            layer_label(dominant_layer(votes)),
            purpose(row)
        );
    }
    md.push('\n');
    md
}

/// The layer with the most member files; ties break Domain < Adapter < Interior.
fn dominant_layer(votes: [u32; 3]) -> LayerHint {
    let kinds = [LayerHint::Domain, LayerHint::Adapter, LayerHint::Interior];
    let mut best = 0usize;
    for i in 1..3 {
        if votes[i] > votes[best] {
            best = i;
        }
    }
    kinds[best]
}

/// Symbol-edge boundary violations (D5): domain→adapter, core→non-core, and
/// cross-crate adapter→adapter edges, over source-scoped endpoints only.
pub(crate) fn boundary_violations(
    graph: &GraphIndex,
    table: &SymbolTable,
    scope: &DocScope,
) -> String {
    let mut viols: BTreeSet<(String, String, &'static str)> = BTreeSet::new();
    for er in graph.graph.edge_references() {
        let (src, dst) = (graph.graph[er.source()], graph.graph[er.target()]);
        let (sp, dp) = (table.path(src), table.path(dst));
        if sp.is_empty() || dp.is_empty() || !scope.include(sp) || !scope.include(dp) {
            continue;
        }
        if let Some(reason) = classify_violation(
            crate_of(sp),
            crate_of(dp),
            LayerHint::of(sp),
            LayerHint::of(dp),
        ) {
            viols.insert((
                format!("`{}`", table.name(src)),
                format!("`{}`", table.name(dst)),
                reason,
            ));
        }
    }

    let mut md = String::new();
    if viols.is_empty() {
        md.push_str("No symbol-level boundary violations detected.\n\n");
        return md;
    }
    for (src, dst, reason) in viols.iter().take(LIST_N) {
        let _ = writeln!(md, "- {src} → {dst} — {reason}");
    }
    if viols.len() > LIST_N {
        let _ = writeln!(md, "- … and {} more.", viols.len() - LIST_N);
    }
    md.push('\n');
    md
}

/// Classify one symbol edge against the hexagonal invariants; `None` = clean.
fn classify_violation(
    src_crate: Option<&str>,
    dst_crate: Option<&str>,
    src_layer: LayerHint,
    dst_layer: LayerHint,
) -> Option<&'static str> {
    if src_layer == LayerHint::Domain && dst_layer == LayerHint::Adapter {
        return Some("domain → adapter");
    }
    if src_crate == Some("ariadne-core") && dst_crate.is_some() && dst_crate != Some("ariadne-core")
    {
        return Some("core → non-core crate");
    }
    if src_layer == LayerHint::Adapter
        && dst_layer == LayerHint::Adapter
        && src_crate.is_some()
        && dst_crate.is_some()
        && src_crate != dst_crate
    {
        return Some("adapter → adapter cross-crate");
    }
    None
}

/// Cycle clusters ranked by member count: each lists its size, representative
/// members, and the lowest-`(src, dst)` member edge as a suggested cut.
pub(crate) fn cycle_clusters(graph: &GraphIndex, table: &SymbolTable) -> String {
    let report = graph.cycle_report();
    let mut md = String::new();
    if report.cycles.is_empty() {
        md.push_str("No dependency cycles detected.\n\n");
        return md;
    }
    let mut clusters: Vec<&Cycle> = report.cycles.iter().collect();
    clusters.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(a.members.cmp(&b.members))
    });
    let _ = writeln!(
        md,
        "{} dependency cluster(s) detected.\n",
        report.cycles.len()
    );
    for cluster in clusters.iter().take(LIST_N) {
        let shown: Vec<String> = cluster
            .members
            .iter()
            .take(REPR_MEMBERS)
            .map(|s| format!("`{}`", table.name(*s)))
            .collect();
        let extra = cluster.members.len().saturating_sub(REPR_MEMBERS);
        let tail = if extra > 0 {
            format!(" +{extra} more")
        } else {
            String::new()
        };
        let _ = writeln!(
            md,
            "- {} members ({}{tail}) — suggested cut: {}",
            cluster.members.len(),
            shown.join(", "),
            suggested_cut(graph, cluster, table)
        );
    }
    md.push('\n');
    md
}

/// The lowest-(src id, dst id) directed edge between two cluster members,
/// rendered as a "from arrow to" pair; "none" when the SCC has no intra edge.
fn suggested_cut(graph: &GraphIndex, cluster: &Cycle, table: &SymbolTable) -> String {
    let members: BTreeSet<SymbolId> = cluster.members.iter().copied().collect();
    let mut best: Option<(SymbolId, SymbolId)> = None;
    for &from in &cluster.members {
        let Some(&ix) = graph.index.get(&from) else {
            continue;
        };
        for er in graph.graph.edges_directed(ix, Outgoing) {
            let to = graph.graph[er.target()];
            if !members.contains(&to) {
                continue;
            }
            let cand = (from, to);
            if best.is_none_or(|b| cand < b) {
                best = Some(cand);
            }
        }
    }
    best.map_or_else(
        || "none".to_owned(),
        |(f, t)| format!("`{}` → `{}`", table.name(f), table.name(t)),
    )
}

/// Risk hot-spots: churn × complexity over source-scoped files, top [`LIST_N`].
/// Empty churn degrades to an explicit history-unavailable line (D6).
pub(crate) fn risk_hotspots(table: &SymbolTable, churn: &[FileChurn], scope: &DocScope) -> String {
    let mut md = String::new();
    if churn.is_empty() {
        md.push_str("_Git history unavailable — risk hot-spots need per-file churn._\n\n");
        return md;
    }
    let complexity = file_complexity_map(table);
    let report = file_hotspots(churn, &complexity);
    let mut rows: Vec<(&str, u32, u32, f32)> = Vec::new();
    for entry in &report.entries {
        let HotspotGrain::File { path } = &entry.grain else {
            continue;
        };
        if !scope.include(path) {
            continue;
        }
        rows.push((path.as_str(), entry.churn, entry.complexity, entry.score));
        if rows.len() >= LIST_N {
            break;
        }
    }
    if rows.is_empty() {
        md.push_str("_No source-scoped files carry Git history._\n\n");
        return md;
    }
    md.push_str("| File | Churn | Complexity | Risk |\n| --- | --- | --- | --- |\n");
    for (path, churn, complexity, score) in rows {
        let _ = writeln!(md, "| `{path}` | {churn} | {complexity} | {score:.2} |");
    }
    md.push('\n');
    md
}

/// Fold per-file complexity by summing each file's symbols' `McCabe`
/// complexity, keyed by defining-file path [src: daemon analytics.rs:35-40].
pub(crate) fn file_complexity_map(table: &SymbolTable) -> BTreeMap<String, u32> {
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    for (id, rec) in table.symbols() {
        let path = table.path(*id);
        if path.is_empty() {
            continue;
        }
        *map.entry(path.to_owned()).or_insert(0) += rec.complexity;
    }
    map
}

/// Single-file churn × complexity risk line for the module's defining file
/// (tier-04 step 6). Reuses `file_hotspots` over the *whole* churn set so the
/// score is the file's repo-relative risk, mirroring the project risk table.
/// Empty git history degrades to an explicit history-unavailable line (D6).
pub(crate) fn risk_line(file_path: &str, churn: &[FileChurn], table: &SymbolTable) -> String {
    if churn.is_empty() {
        return "_Git history unavailable — risk needs per-file churn._".to_owned();
    }
    let complexity = file_complexity_map(table);
    let report = file_hotspots(churn, &complexity);
    for entry in &report.entries {
        let HotspotGrain::File { path } = &entry.grain else {
            continue;
        };
        if path == file_path {
            return format!(
                "Churn {} × complexity {} → risk {:.2} (repo-relative).",
                entry.churn, entry.complexity, entry.score
            );
        }
    }
    "_No Git history recorded for this file._".to_owned()
}

/// Refactor & change-coupling: structural god modules plus co-changing file
/// pairs with no static edge (hidden coupling). Empty git history degrades the
/// coupling half to an explicit history-unavailable line (D6).
///
/// # Errors
/// Propagates [`GraphError::Storage`] from `god_modules`' snapshot scan.
#[allow(clippy::too_many_arguments)]
pub(crate) fn change_coupling(
    graph: &GraphIndex,
    snap: &dyn ReadSnapshot,
    scoped: &[ModuleSpec],
    churn: &[FileChurn],
    co_change: &[CoChangePair],
    table: &SymbolTable,
) -> Result<String, GraphError> {
    let mut md = String::new();

    let gods = god_modules(graph, snap, scoped, GOD_THRESHOLD)?;
    md.push_str("**God modules.** ");
    if gods.is_empty() {
        md.push_str("None detected.\n\n");
    } else {
        md.push('\n');
        for god in gods.iter().take(LIST_N) {
            let _ = writeln!(
                md,
                "- `{}` — Ce {}, cohesion {:.2}. {}",
                god.module, god.efferent, god.cohesion, god.suggestion
            );
        }
        md.push('\n');
    }

    md.push_str("**Hidden change-coupling.** ");
    if churn.is_empty() || co_change.is_empty() {
        md.push_str("_Git history unavailable — change-coupling needs co-change data._\n");
        return Ok(md);
    }
    let report = co_change_report(churn, co_change, &CoChangeConfig::default());
    let path_syms = path_symbols(table);
    let hidden: Vec<&CoChangeEdge> = report
        .edges
        .iter()
        .filter(|e| !structurally_linked(graph, &path_syms, &e.a, &e.b))
        .take(LIST_N)
        .collect();
    if hidden.is_empty() {
        md.push_str("None — every co-changing pair shares a structural edge.\n");
        return Ok(md);
    }
    md.push('\n');
    for edge in hidden {
        let _ = writeln!(
            md,
            "- `{}` ⇄ `{}` — {} shared commit(s), degree {:.2}",
            edge.a, edge.b, edge.shared_commits, edge.degree
        );
    }
    Ok(md)
}

/// Map each defining-file path to the set of symbols it defines.
fn path_symbols(table: &SymbolTable) -> BTreeMap<String, BTreeSet<SymbolId>> {
    let mut map: BTreeMap<String, BTreeSet<SymbolId>> = BTreeMap::new();
    for id in table.symbols().keys() {
        let path = table.path(*id);
        if path.is_empty() {
            continue;
        }
        map.entry(path.to_owned()).or_default().insert(*id);
    }
    map
}

/// True when any graph edge connects a symbol of file `a` to a symbol of file
/// `b` (either direction) — i.e. the co-change pair is *not* hidden.
fn structurally_linked(
    graph: &GraphIndex,
    path_syms: &BTreeMap<String, BTreeSet<SymbolId>>,
    a: &str,
    b: &str,
) -> bool {
    let (Some(syms_a), Some(syms_b)) = (path_syms.get(a), path_syms.get(b)) else {
        return false;
    };
    for &s in syms_a {
        let Some(&ix) = graph.index.get(&s) else {
            continue;
        };
        for er in graph.graph.edges_directed(ix, Outgoing) {
            if syms_b.contains(&graph.graph[er.target()]) {
                return true;
            }
        }
        for er in graph.graph.edges_directed(ix, Incoming) {
            if syms_b.contains(&graph.graph[er.source()]) {
                return true;
            }
        }
    }
    false
}
