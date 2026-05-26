//! Per-language ingest golden for Astro — `.astro` component-script
//! (frontmatter) semantics via the `ariadne-sfc-scip` bridge.
//!
//! Plan ref: `.claude/plans/post-v1-roadmap/tier-03-astro-semantic.md`.
//!
//! Like `ingest_svelte.rs`, this test ingests a *real* SCIP index committed at
//! `fixtures/astro/index.scip` (generation command in that directory's
//! `README.md`). The index is produced by the `ariadne-sfc-scip` bridge in
//! `--framework astro` mode: the bridge slices the TypeScript frontmatter
//! region between the `---` fences, type-checks it, and emits SCIP whose
//! occurrence ranges are shifted back onto the original `.astro` source. The
//! test proves the bridge output ingests cleanly, that `.astro` documents
//! carry attributed occurrences, that every remapped occurrence lands inside
//! the frontmatter span, and that the frontmatter yields a
//! definition→reference edge.

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ariadne_core::Lang;
use ariadne_scip::{parse, proto};

use crate::common::summarize;

/// `SymbolRole::Definition` bit [src: `crates/ariadne-scip/proto/scip.proto`
/// — `Definition = 0x1`].
const DEFINITION_ROLE: i32 = 0x1;

/// Root of the committed `.astro` fixture project.
fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/astro")
}

/// Read the committed bridge-produced SCIP index for the `.astro` fixture.
fn fixture_bytes() -> Vec<u8> {
    let path = fixture_root().join("index.scip");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("committed SCIP fixture {} must read: {e}", path.display()))
}

/// Case-insensitive extension match on a SCIP document's relative path.
fn has_extension(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// 0-based line indices of the opening and closing `---` frontmatter fences in
/// an `.astro` source file.
fn frontmatter_fences(source: &str) -> (usize, usize) {
    let mut fences = source
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim_end() == "---")
        .map(|(n, _)| n);
    let open = fences.next().expect("fixture .astro must open a --- fence");
    let close = fences
        .next()
        .expect("fixture .astro must close the --- fence");
    (open, close)
}

/// Golden summary of the Astro ingest. Reuses the shared `summarize` helper so
/// the snapshot shape matches the sibling per-language ingest goldens.
#[test]
fn ingest_astro_summary() {
    let bytes = fixture_bytes();
    let summary = summarize(Lang::Astro, &bytes).expect("astro SCIP fixture must parse");
    insta::assert_snapshot!(summary);
}

/// Exit criterion: each remapped occurrence on an `.astro` document lands
/// strictly between the frontmatter `---` fences — never in the opening fence,
/// the closing fence, or the template body.
#[test]
fn astro_occurrences_remap_inside_frontmatter_span() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::Astro, &bytes).expect("astro SCIP fixture must parse");

    let astro_docs: Vec<&proto::Document> = doc
        .index
        .documents
        .iter()
        .filter(|d| has_extension(&d.relative_path, "astro"))
        .collect();
    assert!(
        !astro_docs.is_empty(),
        "fixture must contain at least one .astro document",
    );

    for d in &astro_docs {
        let source = std::fs::read_to_string(fixture_root().join(&d.relative_path))
            .unwrap_or_else(|e| panic!("fixture source {} must read: {e}", d.relative_path));
        let (open, close) = frontmatter_fences(&source);
        assert!(
            !d.occurrences.is_empty(),
            "occurrences on {} must be attributed, not dropped",
            d.relative_path,
        );
        for occ in &d.occurrences {
            let start_line =
                usize::try_from(occ.range[0]).expect("occurrence start line is non-negative");
            let end_line =
                usize::try_from(occ.range[2]).expect("occurrence end line is non-negative");
            assert!(
                start_line > open && end_line < close,
                "occurrence on {} lines {start_line}..={end_line} must land inside the \
                 frontmatter span (fences at lines {open}, {close})",
                d.relative_path,
            );
        }
    }
}

/// Exit criterion: the `.astro` frontmatter yields at least one semantic edge —
/// a symbol carrying both a definition occurrence and a reference occurrence.
#[test]
fn astro_frontmatter_yields_definition_reference_edge() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::Astro, &bytes).expect("astro SCIP fixture must parse");

    let mut defined: HashSet<&str> = HashSet::new();
    let mut referenced: HashSet<&str> = HashSet::new();
    for d in &doc.index.documents {
        if !has_extension(&d.relative_path, "astro") {
            continue;
        }
        for occ in &d.occurrences {
            if occ.symbol_roles & DEFINITION_ROLE != 0 {
                defined.insert(&occ.symbol);
            } else {
                referenced.insert(&occ.symbol);
            }
        }
    }
    assert!(
        defined.intersection(&referenced).next().is_some(),
        "an .astro frontmatter symbol must carry both a definition and a reference occurrence",
    );
}
