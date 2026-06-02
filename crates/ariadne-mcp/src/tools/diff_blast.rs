//! `diff_blast_radius` cold path — runs the `ariadne_graph` `diff_blast` use
//! case against the cold [`Catalog`].
//!
//! The git diff (`ariadne_git::diff`) runs in the server handler before this
//! (both the daemon and cold paths need its `hunks` + `changed_paths`); this
//! module joins those line hunks to the indexed symbol spans and folds the
//! per-seed blast radius into the deduped report. The cold catalog carries the
//! symbol spans but not the per-file `blake3`, so `storage` supplies the
//! byte-offset validity guard; the shared `spans_from` builder drops a file
//! whose on-disk bytes diverged from the index (→ unresolved, never a wrong
//! seed) [src: .claude/plans/post-v1-roadmap/tier-15c-diff-blast-radius-tool.md
//!  step 5; D3].

use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{FileId, LineHunk, ReadSnapshot, Storage, SymbolId};
use ariadne_graph::{EdgeKindSet, FileSpanSource, spans_from};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{DiffBlastOutput, DiffSeedRow, EdgeKindFilter};

const DEFAULT_DEPTH: u8 = 3;

fn filter_to_set(filter: &[EdgeKindFilter]) -> EdgeKindSet {
    if filter.is_empty() {
        return EdgeKindSet::ALL;
    }
    let mut set = EdgeKindSet::empty();
    for f in filter {
        set |= match f {
            EdgeKindFilter::Calls => EdgeKindSet::CALLS,
            EdgeKindFilter::Imports => EdgeKindSet::IMPORTS,
            EdgeKindFilter::TypeOf => EdgeKindSet::TYPE_OF,
            EdgeKindFilter::Defines => EdgeKindSet::DEFINES,
            EdgeKindFilter::Overrides => EdgeKindSet::OVERRIDES,
            EdgeKindFilter::Reads => EdgeKindSet::READS,
            EdgeKindFilter::Writes => EdgeKindSet::WRITES,
            EdgeKindFilter::Inherits => EdgeKindSet::INHERITS,
        };
    }
    set
}

/// Compute the diff-aware blast radius over the cold catalog. `hunks` +
/// `changed_paths` come from the already-run `ariadne_git::diff`; `storage`
/// supplies each changed file's indexed `blake3`, and `root` anchors the
/// on-disk reads.
///
/// # Errors
/// Propagates storage snapshot failures from the `blake3` lookup.
// Each argument is a distinct, plan-mandated facet of the cold join: the cold
// catalog (graph + spans), the storage handle (per-file `blake3`), the project
// root (on-disk reads), the changeset's hunks + changed-path list, and the v1
// depth/kind filter. Bundling them would add an input type the tier does not
// call for — the same shape `GraphIndex::diff_blast` is allowed for (tier-14).
#[allow(clippy::too_many_arguments)]
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    root: &Path,
    hunks: &[LineHunk],
    changed_paths: &[String],
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
) -> Result<DiffBlastOutput, McpError> {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));
    let spans = spans_from(collect_span_sources(cat, storage, root, changed_paths)?);
    let report = cat
        .graph
        .diff_blast(&spans, hunks, changed_paths, depth, set);

    Ok(DiffBlastOutput {
        seeds: report
            .seeds
            .into_iter()
            .map(|s| DiffSeedRow {
                symbol: summarize(cat, s.symbol),
                must_touch: s
                    .must_touch
                    .into_iter()
                    .map(|x| summarize(cat, x))
                    .collect(),
                may_touch: s.may_touch.into_iter().map(|x| summarize(cat, x)).collect(),
                depth_used: s.depth_used,
            })
            .collect(),
        must_touch: report
            .must_touch
            .into_iter()
            .map(|x| summarize(cat, x))
            .collect(),
        may_touch: report
            .may_touch
            .into_iter()
            .map(|x| summarize(cat, x))
            .collect(),
        unresolved: report.unresolved,
    })
}

/// Build the per-file span sources for the changed paths: each changed file's
/// indexed `blake3` (read transiently from `storage`), its symbols' defining
/// byte spans (from the cold catalog, which carries them), and its current
/// on-disk bytes under `root`. A changed path with no indexed symbols is
/// skipped — it owns no seed, so it surfaces as `unresolved`. Mirrors the CLI
/// `build_symbol_lines` + daemon `collect_span_sources` shape (tier-15c D3).
fn collect_span_sources<S: Storage>(
    cat: &Catalog,
    storage: &S,
    root: &Path,
    changed_paths: &[String],
) -> Result<Vec<FileSpanSource>, McpError> {
    let snap = storage.snapshot().map_err(McpError::Storage)?;

    let mut hash_of_file: BTreeMap<FileId, [u8; 32]> = BTreeMap::new();
    let mut file_of_path: BTreeMap<String, FileId> = BTreeMap::new();
    for path in changed_paths {
        if let Some(&fid) = cat.path_to_id.get(path) {
            if let Some(rec) = snap.file(fid).map_err(McpError::Storage)? {
                hash_of_file.insert(fid, rec.blake3);
                file_of_path.insert(path.clone(), fid);
            }
        }
    }

    let mut symbols_of_file: BTreeMap<FileId, Vec<(SymbolId, u32, u32)>> = BTreeMap::new();
    for (sid, meta) in &cat.symbols {
        if hash_of_file.contains_key(&meta.file) {
            symbols_of_file.entry(meta.file).or_default().push((
                *sid,
                meta.byte_start,
                meta.byte_end,
            ));
        }
    }

    let mut sources = Vec::new();
    for (path, fid) in file_of_path {
        let Some(symbols) = symbols_of_file.remove(&fid) else {
            continue;
        };
        let Ok(content) = std::fs::read(root.join(&path)) else {
            continue;
        };
        sources.push(FileSpanSource {
            blake3: hash_of_file[&fid],
            path,
            symbols,
            content,
        });
    }
    Ok(sources)
}
