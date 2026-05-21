//! Tier-11 step 1: C++ registry coverage + syntactic-fact extraction.

mod common;

use ariadne_core::Lang;
use ariadne_parser::ParserRegistry;

#[test]
fn registry_supports_cpp() {
    assert!(
        ParserRegistry::new().supports(Lang::Cpp),
        "C++ grammar must be registered",
    );
}

#[test]
fn facts_cpp_sample() {
    let facts = common::facts_for(Lang::Cpp, "cpp/sample.cpp");
    assert!(!facts.decls.is_empty(), "expected >=1 C++ declaration");
    assert!(!facts.calls.is_empty(), "expected >=1 C++ call site");
}
