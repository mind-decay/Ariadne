//! Per-language ingest golden for Vue — `.vue` single-file components via the
//! `ariadne-sfc-scip` Volar bridge.
//!
//! Plan ref: `.claude/plans/js-framework-support/tier-07-scip-bridge-vue.md`.
//!
//! Like `ingest_react.rs`, this test ingests a *real* SCIP index committed at
//! `tests/fixtures/sample-vue/index.scip` (generation command in that
//! directory's `README.md`). Unlike React's `scip-typescript` index, the Vue
//! index is produced by the custom `tools/ariadne-sfc-scip` bridge: it proves
//! the bridge output ingests cleanly, that `.vue` documents carry attributed
//! occurrences, that each document's extension resolves to `Lang::Vue`, and
//! that a symbol defined in one `.vue` resolves to a reference in another.

mod common;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ariadne_core::Lang;
use ariadne_scip::{parse, proto};

use crate::common::summarize;

/// `SymbolRole::Definition` bit [src: `crates/ariadne-scip/proto/scip.proto`
/// — `Definition = 0x1`].
const DEFINITION_ROLE: i32 = 0x1;

/// Read the committed bridge-produced SCIP index for the `sample-vue` fixture.
fn fixture_bytes() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-vue/index.scip");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("committed SCIP fixture {} must read: {e}", path.display()))
}

/// Resolve a SCIP document's relative path to its [`Lang`] through the
/// canonical `ariadne_core::Lang::from_extension` table.
fn lang_for_relative_path(path: &str) -> Option<Lang> {
    Lang::from_extension(Path::new(path).extension()?.to_str()?)
}

/// Case-insensitive extension match on a SCIP document's relative path.
fn has_extension(path: &str, ext: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

/// Golden summary of the Vue ingest. Reuses the shared `summarize` helper so
/// the snapshot shape matches the sibling per-language ingest goldens.
#[test]
fn ingest_vue_summary() {
    let bytes = fixture_bytes();
    let summary = summarize(Lang::Vue, &bytes).expect("vue SCIP fixture must parse");
    insta::assert_snapshot!(summary);
}

/// Exit criteria: occurrences on `.vue` documents are attributed (not dropped),
/// and each document's extension resolves to `Lang::Vue`.
#[test]
fn vue_documents_are_attributed_not_dropped() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::Vue, &bytes).expect("vue SCIP fixture must parse");

    let vue_docs: Vec<&proto::Document> = doc
        .index
        .documents
        .iter()
        .filter(|d| has_extension(&d.relative_path, "vue"))
        .collect();

    assert!(
        !vue_docs.is_empty(),
        "fixture must contain at least one .vue document",
    );

    for d in &vue_docs {
        assert!(
            !d.occurrences.is_empty(),
            "occurrences on {} must be attributed, not dropped",
            d.relative_path,
        );
        assert_eq!(
            lang_for_relative_path(&d.relative_path),
            Some(Lang::Vue),
            ".vue document {} must attribute Lang::Vue",
            d.relative_path,
        );
    }
}

/// Exit criterion: a symbol defined in one `.vue` document and used in another
/// resolves to one symbol with a definition occurrence and a cross-document
/// reference occurrence — the `<script setup>` import edge across `.vue` files.
#[test]
fn cross_file_vue_definition_reference_resolves() {
    let bytes = fixture_bytes();
    let doc = parse(Lang::Vue, &bytes).expect("vue SCIP fixture must parse");

    let mut def_doc: HashMap<&str, &str> = HashMap::new();
    let mut ref_docs: HashMap<&str, Vec<&str>> = HashMap::new();
    for d in &doc.index.documents {
        if !has_extension(&d.relative_path, "vue") {
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
        "a .vue symbol must be defined in one document and referenced from another",
    );
}
