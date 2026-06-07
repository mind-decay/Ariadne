//! `affected_tests` cold path — runs the `ariadne_graph` `affected_tests` use
//! case against the cold [`Catalog`] (Block A, A1).
//!
//! The git diff (`ariadne_git::diff`) runs in the server handler before this
//! (both the daemon and cold paths need its `hunks` + `changed_paths`); this
//! module classifies the catalog's symbols into the test-root set, joins the
//! line hunks to the indexed symbol spans, and returns the test subset of the
//! changed seeds ∪ their reverse-reachable closure. The cold catalog carries
//! the symbol spans but not the per-file `blake3`, so `storage` supplies the
//! byte-offset validity guard; the shared `spans_from` builder drops a file
//! whose on-disk bytes diverged from the index (→ unresolved, never a wrong
//! seed), mirroring the cold `diff_blast` tool (tier-15c D3).

use std::collections::BTreeMap;
use std::path::Path;

use ariadne_core::{FileId, LineHunk, ReadSnapshot, Storage, SymbolId};
use ariadne_graph::{
    EdgeKindSet, FileSpanSource, TestRootInput, classify_test_symbols, spans_from,
};

use crate::catalog::Catalog;
use crate::errors::McpError;
use crate::tools::summarize;
use crate::types::{AffectedTestsOutput, EdgeKindFilter};

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

/// Compute the tests a changeset reaches over the cold catalog. `hunks` +
/// `changed_paths` come from the already-run `ariadne_git::diff`; the test-root
/// set is classified on the fly from the catalog's per-symbol metadata (the
/// same pure classifier the warm projection precomputes), so the cold and warm
/// answers match. `storage` supplies each changed file's indexed `blake3`, and
/// `root` anchors the on-disk reads.
///
/// # Errors
/// Propagates storage snapshot failures from the `blake3` lookup.
// Each argument is a distinct, plan-mandated facet of the cold join — the cold
// catalog, the storage handle (per-file `blake3`), the project root (on-disk
// reads), the changeset's hunks + changed-path list, and the v1 depth/kind
// filter — the same shape `GraphIndex::affected_tests` is allowed for.
#[allow(clippy::too_many_arguments)]
pub fn handle<S: Storage>(
    cat: &Catalog,
    storage: &S,
    root: &Path,
    hunks: &[LineHunk],
    changed_paths: &[String],
    depth: Option<u8>,
    kinds: Option<&[EdgeKindFilter]>,
) -> Result<AffectedTestsOutput, McpError> {
    let depth = depth.unwrap_or(DEFAULT_DEPTH).max(1);
    let set = filter_to_set(kinds.unwrap_or(&[]));

    let test_roots = classify_test_symbols(cat.symbols.iter().map(|(id, m)| TestRootInput {
        id: *id,
        lang: m.lang,
        path: cat.path_of(m.file).unwrap_or(""),
        kind: &m.kind,
        name: &m.name,
        attributes: &m.attributes,
    }));

    let spans = spans_from(collect_span_sources(cat, storage, root, changed_paths)?);
    let report = cat
        .graph
        .affected_tests(&spans, hunks, changed_paths, &test_roots, depth, set);

    Ok(AffectedTestsOutput {
        tests: report
            .tests
            .into_iter()
            .map(|x| summarize(cat, x))
            .collect(),
        seeds: report
            .seeds
            .into_iter()
            .map(|x| summarize(cat, x))
            .collect(),
        unresolved: report.unresolved,
    })
}

/// Build the per-file span sources for the changed paths: each changed file's
/// indexed `blake3` (read transiently from `storage`), its symbols' defining
/// byte spans (from the cold catalog), and its current on-disk bytes under
/// `root`. A changed path with no indexed symbols is skipped — it owns no seed,
/// so it surfaces as `unresolved`. Mirrors the cold `diff_blast` tool's join.
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
