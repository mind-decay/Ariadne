//! Per-language ingest golden for React/Solid — `.tsx` / `.jsx` via
//! `scip-typescript`.
//!
//! Plan ref: `.claude/plans/js-framework-support/tier-06-scip-jsx-tsx.md`.
//!
//! Unlike the synthetic per-language goldens (`ingest_typescript.rs` et al.),
//! this test ingests a *real* `scip-typescript` index committed at
//! `tests/fixtures/sample-react/index.scip` (generation command in that
//! directory's `README.md`). It proves four tier exit criteria end-to-end:
//! `.tsx`/`.jsx` documents survive ingest with their occurrences attributed,
//! a cross-file definition→reference pair resolves, every `.tsx` symbol
//! round-trips through the canonical-symbol normalizer unchanged, and each
//! document's relative-path extension maps unambiguously onto its `Lang`.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ariadne_core::Lang;
use ariadne_scip::{normalize_scip_symbol, parse, proto};

use crate::common::summarize;

/// `SymbolRole::Definition` bit [src: `crates/ariadne-scip/proto/scip.proto`
/// line 526 — `Definition = 0x1`].
const DEFINITION_ROLE: i32 = 0x1;

/// Read the committed real `scip-typescript` index for the `sample-react`
/// fixture project.
fn fixture_bytes() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-react/index.scip");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("committed SCIP fixture {} must read: {e}", path.display()))
}

/// Resolve a SCIP document's relative path to its [`Lang`] through the
/// canonical `ariadne_core::Lang::from_extension` table — the same table
/// `ariadne_cli::lang_for_path` resolves through. Sharing the one map means a
/// future remap of the extension→`Lang` table is caught by this test rather
/// than silently missed (I1 audit follow-up).
fn lang_for_relative_path(path: &str) -> Option<Lang> {
    Lang::from_extension(Path::new(path).extension()?.to_str()?)
}

/// Case-insensitive extension match on a SCIP document's relative path.
/// Used instead of `str::ends_with(".tsx")`, which trips clippy's
/// `case_sensitive_file_extension_comparisons` lint.
fn has_extension(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Golden summary of the React ingest. Reuses the shared `summarize` helper
/// so the snapshot shape matches the sibling per-language ingest goldens.
#[test]
fn ingest_react_summary() {
    let bytes = fixture_bytes();
    let summary = summarize(Lang::TypeScript, &bytes).expect("react SCIP fixture must parse");
    insta::assert_snapshot!(summary);
}

/// Exit criteria 2 + 3: occurrences on `.tsx`/`.jsx` documents are attributed
/// (not dropped), and each document's extension resolves to the right `Lang`.
#[test]
fn tsx_and_jsx_documents_are_attributed_not_dropped() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::TypeScript, &bytes).expect("react SCIP fixture must parse");

    let tsx_docs: Vec<&proto::Document> = doc
        .index
        .documents
        .iter()
        .filter(|d| has_extension(&d.relative_path, "tsx"))
        .collect();
    let jsx_docs: Vec<&proto::Document> = doc
        .index
        .documents
        .iter()
        .filter(|d| has_extension(&d.relative_path, "jsx"))
        .collect();

    assert!(
        !tsx_docs.is_empty(),
        "fixture must contain at least one .tsx document",
    );
    assert!(
        !jsx_docs.is_empty(),
        "fixture must contain at least one .jsx document",
    );

    for d in tsx_docs.iter().chain(jsx_docs.iter()) {
        assert!(
            !d.occurrences.is_empty(),
            "occurrences on {} must be attributed, not dropped",
            d.relative_path,
        );
    }

    for d in &tsx_docs {
        assert_eq!(
            lang_for_relative_path(&d.relative_path),
            Some(Lang::Tsx),
            ".tsx document {} must attribute Lang::Tsx",
            d.relative_path,
        );
    }
    for d in &jsx_docs {
        assert_eq!(
            lang_for_relative_path(&d.relative_path),
            Some(Lang::JavaScript),
            ".jsx document {} must attribute Lang::JavaScript",
            d.relative_path,
        );
    }
}

/// Exit criterion 2: a `.tsx` component defined in one document and used in
/// another resolves to one symbol with a definition occurrence and a
/// cross-document reference occurrence.
#[test]
fn cross_file_tsx_definition_reference_resolves() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::TypeScript, &bytes).expect("react SCIP fixture must parse");

    let mut def_doc: HashMap<&str, &str> = HashMap::new();
    let mut ref_docs: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &doc.index.documents {
        if !has_extension(&d.relative_path, "tsx") {
            continue;
        }
        for occ in &d.occurrences {
            if occ.symbol.starts_with("local ") {
                continue;
            }
            if occ.symbol_roles & DEFINITION_ROLE != 0 {
                def_doc.insert(&occ.symbol, &d.relative_path);
            } else {
                ref_docs
                    .entry(&occ.symbol)
                    .or_default()
                    .push(&d.relative_path);
            }
        }
    }

    let cross = def_doc.iter().find(|(sym, def_path)| {
        ref_docs
            .get(*sym)
            .is_some_and(|refs| refs.iter().any(|r| r != *def_path))
    });
    assert!(
        cross.is_some(),
        "a .tsx symbol must be defined in one document and referenced from another",
    );
}

/// Tier step 4: every `scip-typescript` symbol on a `.tsx` document parses
/// through the canonical-symbol normalizer and yields a stable id — SCIP
/// symbol descriptors are language-agnostic, so `normalize/grammar.rs` needs
/// no TSX-specific arm.
#[test]
fn tsx_symbols_round_trip_through_normalize() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::TypeScript, &bytes).expect("react SCIP fixture must parse");

    let tsx_doc = doc
        .index
        .documents
        .iter()
        .find(|d| has_extension(&d.relative_path, "tsx"))
        .expect("fixture must contain a .tsx document");

    let mut checked = 0usize;
    for sym in &tsx_doc.symbols {
        let canon = normalize_scip_symbol(&sym.symbol)
            .unwrap_or_else(|e| panic!("normalize rejected TSX symbol {:?}: {e}", sym.symbol));
        assert_eq!(
            canon.id(),
            normalize_scip_symbol(&sym.symbol).unwrap().id(),
            "TSX symbol id must be stable across re-normalization",
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "fixture .tsx document must declare at least one symbol",
    );
}
