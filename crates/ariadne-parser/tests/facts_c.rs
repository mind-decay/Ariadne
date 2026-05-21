//! Tier-11 step 1: C registry coverage + syntactic-fact extraction.

mod common;

use ariadne_core::Lang;
use ariadne_parser::ParserRegistry;

#[test]
fn registry_supports_c() {
    assert!(
        ParserRegistry::new().supports(Lang::C),
        "C grammar must be registered",
    );
}

#[test]
fn facts_c_sample() {
    let facts = common::facts_for(Lang::C, "c/sample.c");
    assert!(!facts.decls.is_empty(), "expected >=1 C declaration");
    assert!(!facts.calls.is_empty(), "expected >=1 C call site");
}
