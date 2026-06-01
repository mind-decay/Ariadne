//! `ariadne index` — run the cold-index pipeline and commit to redb.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use anyhow::{Context, Result};
use ariadne_core::{FileId, ReadSnapshot, Storage, SymbolId};
use ariadne_daemon::IndexLock;
use ariadne_git::{HistoryOptions, walk_line_hunks, walk_since};
use ariadne_graph::{FileSymbolSpans, attribute_symbol_churn};
use ariadne_storage::RedbStorage;

use crate::config::Config;
use crate::domain::{index_path, run_index};

/// Chunk size for the symbol-churn snapshot scans — bounds the working set of
/// the file/symbol streams the same way the cold-index count scans do.
const CHURN_SCAN_CHUNK: usize = 65_536;

/// Load the project config, run the full cold pipeline, print the per-phase
/// timing breakdown + parse sub-phase breakdown on stderr, and the JSON-line
/// summary on stdout. `scip` gates the external SCIP indexers
/// [src: tier-12 steps 1-2; tier-13 step 1].
///
/// # Errors
/// Propagates config-load, walk, parse, and storage failures.
pub fn run(root: &Path, fresh: bool, scip: bool) -> Result<()> {
    let config = Config::load(root)?;
    let (summary, phases, parse_sub) = run_index(root, &config, fresh, scip)?;
    eprintln!(
        "[index] phases (ms): walk={} parse={} resolve={} commit={} scip={}",
        phases.walk, phases.parse, phases.resolve, phases.commit, phases.scip,
    );
    eprintln!(
        "[index] parse (ms, summed over workers): read={} parse={} extract={}",
        parse_sub.read, parse_sub.parse, parse_sub.extract,
    );
    // Cold `ariadne index` is the only redb opener in this process, so it needs
    // no cross-process serialization lock.
    refresh_history(root, &config, None)?;
    refresh_symbol_churn(root, &config)?;
    println!("{}", serde_json::to_string(&summary)?);
    Ok(())
}

/// Refresh the persisted Git-history derivation, keeping it current cheaply:
/// read the HEAD-oid watermark and walk only commits added since it
/// (`walk_since`). A valid watermark merges the delta into `CHURN`/`CO_CHANGE`
/// and advances the watermark atomically; an absent or unreachable one (first
/// run, or rewritten/force-pushed history) replaces with a full cold walk and
/// records the new watermark. Wired here at the composition root so the daemon
/// never depends on `ariadne-git` (RD7). A non-Git project is skipped — there
/// is no history to ingest, not a failure; genuine traversal errors propagate.
///
/// When called from the daemon's background re-walk, `lock` carries the
/// daemon's redb-open serialization handle, so the transient redb opens here
/// stay serialized against the pump and accept-loop opens (single-open per
/// process, tier-11a I1). The cold `ariadne index` path passes `None` — it is
/// the only opener. redb is opened only for the watermark read and the
/// merge/replace, never across the `gix` walk, which needs no storage (I2).
///
/// A bounded `history.depth` caps *every* walk, not just the first: an
/// incremental merge would append new commits onto an already-`depth`-deep
/// base, growing the effective window past `depth` over the daemon lifetime and
/// diverging from a `depth`-bounded cold walk. With `depth` set, this always
/// takes the full `replace_history` path so the bounded window stays exact
/// (divergence 0); the watermarked incremental path applies only to the
/// unbounded default (tier-11a I3).
///
/// # Errors
/// Propagates Git-walk and storage failures.
pub(crate) fn refresh_history(
    root: &Path,
    config: &Config,
    lock: Option<&IndexLock>,
) -> Result<()> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    let opts = HistoryOptions {
        depth: config.history.depth,
        max_files_per_commit: config.history.max_files_per_commit,
    };
    // I3: a bounded depth forces a full bounded walk every time; never merge.
    let force_full = opts.depth.is_some();

    // I2: read the watermark under a transient redb open, then drop the handle
    // before the walk. A forced-full refresh ignores the watermark entirely.
    let watermark = if force_full {
        None
    } else {
        with_index_lock(lock, || {
            let storage =
                RedbStorage::open(&index_path(root)).context("open redb index for history")?;
            storage
                .last_ingested_commit()
                .context("read history watermark")
        })?
    };

    // The `gix` walk holds no redb handle (I2).
    let walk = walk_since(root, &opts, watermark.as_deref()).context("walk git history")?;
    let Some(head) = walk.head_oid else {
        // Unborn HEAD: nothing to ingest.
        return Ok(());
    };
    // A valid watermark already at HEAD means no new commits — skip the write.
    if walk.incremental && watermark.as_deref() == Some(head.as_slice()) {
        return Ok(());
    }

    // I2: re-open redb only for the write, again under the serialization lock.
    with_index_lock(lock, || {
        let storage =
            RedbStorage::open(&index_path(root)).context("open redb index for history")?;
        if walk.incremental {
            storage
                .merge_history(&walk.report.churn, &walk.report.pairs, &head)
                .context("merge git history")
        } else {
            storage
                .replace_history(&walk.report.churn, &walk.report.pairs)
                .context("replace git history")?;
            storage
                .set_last_ingested_commit(&head)
                .context("set history watermark")
        }
    })?;

    eprintln!(
        "[index] history: {} files, {} co-change pairs ({})",
        walk.report.churn.len(),
        walk.report.pairs.len(),
        if walk.incremental {
            "incremental"
        } else {
            "full"
        },
    );
    Ok(())
}

/// Run `f` holding the daemon's redb-open serialization lock when one is
/// supplied (the daemon re-walk path), or directly otherwise (the cold
/// `ariadne index` path, the sole opener) — so a daemon-scheduled re-walk
/// cannot race the pump or accept-loop redb opens (tier-11a I1).
fn with_index_lock<T>(lock: Option<&IndexLock>, f: impl FnOnce() -> Result<T>) -> Result<T> {
    match lock {
        Some(l) => l.with(f),
        None => f(),
    }
}

/// Recompute and persist per-symbol churn (tier-11b): walk the recent commits'
/// new-side `blob-diff` line hunks, attribute them to the symbol spans from the
/// committed snapshot via the pure `ariadne-graph` use-case, and replace the
/// `SYMBOL_CHURN` table. Wired here at the composition root — the symbol-agnostic
/// `ariadne-git` adapter and the symbol join in `ariadne-graph` are joined only
/// here (ADR-0019), so the daemon never depends on `ariadne-git` (RD7).
///
/// A non-Git project is skipped — there is no history to attribute, not a
/// failure; genuine traversal/storage errors propagate. The `gix` walk holds no
/// redb handle, matching `refresh_history`'s single-open-per-process discipline;
/// the cold `ariadne index` is the sole opener, so no cross-process lock is
/// needed [src: post-v1-roadmap tier-11b steps 3-5; tier-11a I1/I2].
///
/// # Errors
/// Propagates Git-walk and storage failures.
pub(crate) fn refresh_symbol_churn(root: &Path, config: &Config) -> Result<()> {
    if !root.join(".git").exists() {
        return Ok(());
    }
    // Collect each recent commit's new-side line hunks off any redb transaction.
    let commit_hunks =
        walk_line_hunks(root, config.history.symbol_churn_depth).context("walk git line hunks")?;

    // Paths changed anywhere in the window. Symbols in untouched files have zero
    // churn, so they need neither a disk read nor a line index.
    let changed: BTreeSet<&str> = commit_hunks
        .iter()
        .flatten()
        .map(|h| h.path.as_str())
        .collect();

    let storage =
        RedbStorage::open(&index_path(root)).context("open redb index for symbol churn")?;
    let symbol_lines = if changed.is_empty() {
        Vec::new()
    } else {
        build_symbol_lines(root, &storage, &changed).context("build symbol line index")?
    };

    let churn = attribute_symbol_churn(&symbol_lines, &commit_hunks);
    storage
        .replace_symbol_churn(&churn)
        .context("replace symbol churn")?;

    eprintln!(
        "[index] symbol churn: {} symbols across {} changed files",
        churn.len(),
        symbol_lines.len(),
    );
    Ok(())
}

/// Build the per-file attribution input for the changed paths: group each
/// changed file's symbols (with their defining byte spans) by file and pair
/// them with the file's HEAD line index, read from the on-disk bytes.
///
/// The bytes are read here at the composition root rather than threaded out of
/// the parse — within one index run the working tree is the parse-time content,
/// so the line index is identical; a `blake3` guard skips any file whose working
/// tree has since diverged from the indexed revision, where the persisted byte
/// offsets would no longer be valid [src: tier-11b step 5].
fn build_symbol_lines(
    root: &Path,
    storage: &RedbStorage,
    changed: &BTreeSet<&str>,
) -> Result<Vec<FileSymbolSpans>> {
    let snap = storage.snapshot().context("snapshot for symbol churn")?;

    // Changed path -> FileId, and FileId -> indexed content hash (the validity
    // guard for byte offsets against the on-disk bytes).
    let mut file_of_path: HashMap<String, FileId> = HashMap::new();
    let mut hash_of_file: HashMap<FileId, [u8; 32]> = HashMap::new();
    for chunk in snap.iter_files(CHURN_SCAN_CHUNK)? {
        for (id, rec) in chunk.context("scan files for symbol churn")? {
            if changed.contains(rec.path.as_str()) {
                hash_of_file.insert(id, rec.blake3);
                file_of_path.insert(rec.path, id);
            }
        }
    }

    // Group changed files' symbols by FileId. iter_symbols is the only scan that
    // yields the SymbolId (the redb key); symbols_in_file drops it.
    let mut symbols_of_file: HashMap<FileId, Vec<(SymbolId, u32, u32)>> = HashMap::new();
    for chunk in snap.iter_symbols(CHURN_SCAN_CHUNK)? {
        for (sid, rec) in chunk.context("scan symbols for symbol churn")? {
            if hash_of_file.contains_key(&rec.defining_file) {
                symbols_of_file.entry(rec.defining_file).or_default().push((
                    sid,
                    rec.defining_span.byte_start,
                    rec.defining_span.byte_end,
                ));
            }
        }
    }
    drop(snap);

    let mut out = Vec::new();
    for (path, id) in file_of_path {
        let Some(symbols) = symbols_of_file.remove(&id) else {
            continue;
        };
        let Ok(content) = std::fs::read(root.join(&path)) else {
            continue;
        };
        if blake3::hash(&content).as_bytes() != &hash_of_file[&id] {
            continue;
        }
        out.push(FileSymbolSpans {
            path,
            line_starts: line_starts(&content),
            symbols,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Byte offset of each line's first byte (line 1 at offset 0): the HEAD line
/// index the symbol-churn use-case converts byte spans against. A trailing
/// newline yields a final start at `content.len()`, which is harmless — no
/// symbol byte offset maps there.
fn line_starts(content: &[u8]) -> Vec<u32> {
    let mut starts = vec![0u32];
    for (idx, &byte) in content.iter().enumerate() {
        if byte == b'\n' {
            if let Ok(next) = u32::try_from(idx + 1) {
                starts.push(next);
            }
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;

    use ariadne_git::walk_history;

    /// Run a git command in `repo`, isolated from any ambient git config so the
    /// fixture is reproducible on any host (matching `tests/incremental_history.rs`).
    fn git(repo: &Path, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(repo)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@x")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@x")
            .args(args)
            .output()
            .expect("spawn git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr),
        );
    }

    fn commit(repo: &Path, path: &str, content: &str) {
        std::fs::write(repo.join(path), content).expect("write fixture");
        // Stage only the named file: `refresh_history` creates `.ariadne/` in
        // the repo, and `add -A` would sweep the redb into the fixture history.
        git(repo, &["add", path]);
        git(repo, &["commit", "-m", "c", "--no-gpg-sign"]);
    }

    /// I3: with `history.depth = Some(N)`, repeated refreshes keep the persisted
    /// window exactly the last N commits — byte-identical to a single depth-N
    /// cold walk over the current HEAD — instead of growing past N as the
    /// watermarked incremental merge would
    /// [src: .claude/plans/post-v1-roadmap/audit/tier-11a-report.md I3].
    #[test]
    fn bounded_depth_window_stays_exact_across_incremental_refresh() {
        let repo = tempfile::tempdir().expect("tempdir");
        let p = repo.path();
        git(p, &["init", "-b", "main"]);
        commit(p, "a.txt", "1");
        commit(p, "b.txt", "1");

        let mut config = Config::detect(p);
        config.history.depth = Some(2);

        refresh_history(p, &config, None).expect("first refresh");

        // Two more commits push the oldest two out of a depth-2 window.
        commit(p, "c.txt", "1");
        commit(p, "d.txt", "1");
        refresh_history(p, &config, None).expect("second refresh");

        // Expected: a single depth-2 cold walk over the current HEAD.
        let opts = HistoryOptions {
            depth: Some(2),
            max_files_per_commit: config.history.max_files_per_commit,
        };
        let expected_dir = tempfile::tempdir().expect("tempdir");
        let expected =
            RedbStorage::open(&expected_dir.path().join("index.redb")).expect("open expected redb");
        let report = walk_history(p, &opts).expect("cold walk");
        expected
            .replace_history(&report.churn, &report.pairs)
            .expect("replace expected");

        let actual = RedbStorage::open(&index_path(p)).expect("open actual redb");
        assert_eq!(
            actual.all_churn().expect("actual churn"),
            expected.all_churn().expect("expected churn"),
            "bounded depth-2 churn must equal a single depth-2 cold walk, not grow past N",
        );
        assert_eq!(
            actual.all_co_change().expect("actual co_change"),
            expected.all_co_change().expect("expected co_change"),
        );
    }
}
