//! Tier-02 step 7: JSX (`.jsx`, JavaScript grammar) component facts.

mod common;

use ariadne_core::Lang;
use ariadne_parser::DeclKind;

#[test]
fn facts_jsx_react_sample() {
    let facts = common::facts_for(Lang::JavaScript, "react/sample.jsx");
    assert!(
        facts.decls.iter().any(|d| d.kind == DeclKind::Component),
        "expected >=1 Component decl; got {:?}",
        facts.decls,
    );
    assert!(!facts.renders.is_empty(), "expected >=1 RenderSite");
    assert!(!facts.hooks.is_empty(), "expected >=1 HookSite");
    insta::assert_debug_snapshot!(facts);
}
