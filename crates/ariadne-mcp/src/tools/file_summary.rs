//! `file_summary` — per-file symbol roll-up plus fan-in/out totals, a
//! top-5 dependent-file table, and the component-graph neighbourhood of
//! any `Component` symbols the file defines (ADR-0012).

use std::collections::BTreeMap;

use ariadne_core::{EdgeKind, FileId, ReadSnapshot, Storage, SymbolId};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{ComponentRow, DependencyRow, FileQuery, FileSummaryOutput};

const TOP_DEPS: usize = 5;

/// Free-form kind tag carried by component symbols (`DeclKind::Component`
/// and the synthesized SFC component) [src: crates/ariadne-cli/src/domain/mod.rs].
const COMPONENT_KIND: &str = "component";

/// Build the [`FileSummaryOutput`] for the file at `input.path`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.path` is not indexed, or
/// [`McpError::Storage`] when the snapshot scan fails.
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    input: &FileQuery,
) -> Result<FileSummaryOutput, McpError> {
    let file_id = *cat
        .path_to_id
        .get(&input.path)
        .ok_or_else(|| McpError::NotFound(format!("file {}", input.path)))?;
    let snap = storage.snapshot().map_err(McpError::Storage)?;

    let mut symbols = Vec::new();
    let mut components = Vec::new();
    let mut fan_in: u32 = 0;
    let mut fan_out: u32 = 0;
    let mut dep_counts: BTreeMap<FileId, u32> = BTreeMap::new();

    for (sid, meta) in &cat.symbols {
        if meta.file != file_id {
            continue;
        }
        symbols.push(summarize(cat, *sid));
        let fi = u32::try_from(cat.graph.fan_in(*sid)).unwrap_or(u32::MAX);
        let fo = u32::try_from(cat.graph.fan_out(*sid)).unwrap_or(u32::MAX);
        fan_in = fan_in.saturating_add(fi);
        fan_out = fan_out.saturating_add(fo);

        let is_component = meta.kind == COMPONENT_KIND;
        let mut renders = Vec::new();
        let mut hooks = Vec::new();
        let outgoing = snap.outgoing_edges(*sid).map_err(McpError::Storage)?;
        for (key, rec) in &outgoing {
            let target_file = rec.source_span.file;
            if target_file != file_id {
                *dep_counts.entry(target_file).or_insert(0) += 1;
            }
            if is_component {
                match key.kind {
                    EdgeKind::Renders => renders.push(dst_name(cat, key.dst)),
                    EdgeKind::UsesHook => hooks.push(dst_name(cat, key.dst)),
                    _ => {}
                }
            }
        }
        if is_component {
            renders.sort();
            hooks.sort();
            components.push(ComponentRow {
                component: meta.name.clone(),
                renders,
                hooks,
            });
        }
    }
    symbols.sort_by_key(|a| a.byte_start);
    components.sort_by(|a, b| a.component.cmp(&b.component));

    let mut deps: Vec<DependencyRow> = dep_counts
        .into_iter()
        .filter_map(|(fid, edges)| {
            cat.path_of(fid).map(|p| DependencyRow {
                file: p.to_owned(),
                edges,
            })
        })
        .collect();
    deps.sort_by(|a, b| b.edges.cmp(&a.edges).then(a.file.cmp(&b.file)));
    deps.truncate(TOP_DEPS);

    Ok(FileSummaryOutput {
        path: input.path.clone(),
        symbols,
        fan_in,
        fan_out,
        top_dependencies: deps,
        components,
    })
}

/// Canonical name of an edge destination symbol; `<unknown>` when the id
/// is absent from the catalog (matches the `summarize` placeholder).
fn dst_name(cat: &Catalog, dst: SymbolId) -> String {
    cat.meta_of(dst)
        .map_or_else(|| String::from("<unknown>"), |m| m.name.clone())
}
