//! `plan_assist` — wraps `ariadne_graph::GraphIndex::plan_assist`,
//! converting `FileId` rows back to wire paths.

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::types::{PlanAssistInput, PlanAssistOutput, PlanFileRow};

const DEFAULT_MAX_FILES: u32 = 16;

/// Ranked plan-assist file list for `input.symbol`.
///
/// # Errors
/// Returns [`McpError::NotFound`] when `input.symbol` is unknown.
pub fn handle(cat: &Catalog, input: &PlanAssistInput) -> Result<PlanAssistOutput, McpError> {
    let id = cat
        .find_symbol(&input.symbol)
        .ok_or_else(|| McpError::NotFound(format!("symbol {}", input.symbol)))?;
    let max =
        usize::try_from(input.max_files.unwrap_or(DEFAULT_MAX_FILES).max(1)).unwrap_or(usize::MAX);
    let file_of = |sid: ariadne_core::SymbolId| cat.file_of(sid);
    let plan = cat.graph.plan_assist(id, max, &file_of);
    let mut rows = Vec::with_capacity(plan.files.len());
    for row in plan.files {
        let Some(path) = cat.path_of(row.file) else {
            continue;
        };
        let mut why: Vec<String> = row
            .why
            .into_iter()
            .filter_map(|s| cat.meta_of(s).map(|m| m.name.clone()))
            .collect();
        why.sort();
        rows.push(PlanFileRow {
            file: path.to_owned(),
            why,
            certainty: row.certainty,
        });
    }
    Ok(PlanAssistOutput { files: rows })
}
