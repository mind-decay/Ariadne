//! Shared helpers for tier-03 fact / incremental tests.
//!
//! Lives under `tests/common/` so Cargo treats it as a module (not a
//! separate test binary). Each test file `mod common;` includes it.

use std::path::PathBuf;

use ariadne_core::Lang;
use ariadne_parser::{ParserRegistry, SyntacticFacts, extract_syntactic_facts, parse_file};

pub fn fixture(rel: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read fixture {}: {e}", path.display()))
}

pub fn facts_for(lang: Lang, rel: &str) -> SyntacticFacts {
    let source = fixture(rel);
    let registry = ParserRegistry::new();
    let parsed = parse_file(lang, &registry, &source, None, &[]).expect("parse ok");
    assert!(
        !parsed.host.1.root_node().has_error(),
        "fixture {rel} produced a tree-sitter parse error",
    );
    extract_syntactic_facts(&parsed, &source).expect("facts extraction")
}
