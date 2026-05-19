//! `file_summary` — per-file symbol roll-up plus fan-in/out totals and
//! a top-5 dependent-file table derived from the storage edge index.

use std::collections::BTreeMap;

use ariadne_core::{FileId, ReadSnapshot, Storage};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{DependencyRow, FileQuery, FileSummaryOutput};

const TOP_DEPS: usize = 5;

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
        let outgoing = snap.outgoing_edges(*sid).map_err(McpError::Storage)?;
        for (_, rec) in outgoing {
            let target_file = rec.source_span.file;
            if target_file == file_id {
                continue;
            }
            *dep_counts.entry(target_file).or_insert(0) += 1;
        }
    }
    symbols.sort_by_key(|a| a.byte_start);

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
    })
}
