//! Tier-02 steps 1 & 7: TSX registry coverage + React/Solid component facts.

mod common;

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, ParserRegistry};

#[test]
fn registry_supports_tsx() {
    assert!(
        ParserRegistry::new().supports(Lang::Tsx),
        "TSX grammar must be registered",
    );
}

#[test]
fn facts_tsx_react_sample() {
    let facts = common::facts_for(Lang::Tsx, "react/sample.tsx");
    assert!(
        facts.decls.iter().any(|d| d.kind == DeclKind::Component),
        "expected >=1 Component decl; got {:?}",
        facts.decls,
    );
    // Audit F1: an arrow-function component (`const Badge = () => <jsx/>`) is
    // captured as `@def.variable`; the component post-filter must still retag
    // it `Component`, matching plan step 5's `lexical_declaration` intent.
    assert_eq!(
        facts
            .decls
            .iter()
            .find(|d| d.name == "Badge")
            .map(|d| &d.kind),
        Some(&DeclKind::Component),
        "arrow-function component Badge must be classified Component; got {:?}",
        facts.decls,
    );
    assert!(!facts.renders.is_empty(), "expected >=1 RenderSite");
    assert!(!facts.hooks.is_empty(), "expected >=1 HookSite");
    insta::assert_debug_snapshot!(facts);
}

#[test]
fn facts_tsx_solid_sample() {
    let facts = common::facts_for(Lang::Tsx, "solid/sample.tsx");
    assert!(
        facts.decls.iter().any(|d| d.kind == DeclKind::Component),
        "expected >=1 Component decl; got {:?}",
        facts.decls,
    );
    assert!(!facts.renders.is_empty(), "expected >=1 RenderSite");
    assert!(
        facts.hooks.iter().any(|h| h.callee == "createSignal"),
        "expected a createSignal HookSite; got {:?}",
        facts.hooks,
    );
    insta::assert_debug_snapshot!(facts);
}
