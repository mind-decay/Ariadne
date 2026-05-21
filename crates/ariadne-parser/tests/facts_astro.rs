//! Tier-04 step 1: Astro SFC multi-region parse + merged syntactic facts.
//!
//! An `.astro` file is an HTML-ish host skeleton with a leading `---`-fenced
//! TypeScript frontmatter block. Parsing it yields a
//! [`ariadne_parser::ParsedFile`] with an Astro host layer and one injected
//! TypeScript frontmatter layer; the merged facts must carry both the
//! frontmatter declarations/imports and the body's child-component render
//! sites, all with file-absolute byte spans (tier-04 exit criteria 1, 3-4).
//!
//! Pin note R-Astro-ts: `tree-sitter-astro-next` is pinned at `=0.1.1`, a
//! pre-1.0 grammar (plan.md R-Astro-ts). This fixture test exercises the
//! exact node types the injection + facts code depends on; a pin bump is a
//! follow-up that must amend docs/adr/0011-framework-grammars-injection.md
//! (tier-04 step 9).

mod common;

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, ParserRegistry, parse_file};

#[test]
fn registry_supports_astro() {
    assert!(
        ParserRegistry::new().supports(Lang::Astro),
        "Astro host grammar must be registered",
    );
}

#[test]
fn parsed_file_has_astro_host_and_one_frontmatter_layer() {
    let source = common::fixture("astro/sample.astro");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Astro, &registry, &source, None, &[]).expect("parse astro ok");

    assert_eq!(
        parsed.host.0,
        Lang::Astro,
        "host layer is the Astro grammar",
    );
    assert!(
        !parsed.host.1.root_node().has_error(),
        "astro fixture host tree has a parse error",
    );
    assert_eq!(
        parsed.injected.len(),
        1,
        "expected exactly one injected frontmatter layer; got {:?}",
        parsed
            .injected
            .iter()
            .map(|(lang, _)| *lang)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        parsed.injected[0].0,
        Lang::TypeScript,
        "the `---`-fenced frontmatter injects a TypeScript layer",
    );
    assert!(
        !parsed.injected[0].1.root_node().has_error(),
        "the frontmatter byte range must parse clean under the TS grammar",
    );
}

#[test]
fn merged_facts_cover_frontmatter_decls_and_body_renders() {
    let facts = common::facts_for(Lang::Astro, "astro/sample.astro");

    assert!(
        facts
            .decls
            .iter()
            .any(|d| d.name == "title" && matches!(d.kind, DeclKind::Variable)),
        "expected the frontmatter's `title` const decl; got {:?}",
        facts.decls,
    );
    assert!(
        facts
            .imports
            .iter()
            .any(|i| i.path.contains("Layout.astro")),
        "expected the `Layout.astro` import from the frontmatter; got {:?}",
        facts.imports,
    );
    assert!(
        facts.renders.iter().any(|r| r.component == "Layout"),
        "expected a RenderSite for `<Layout>` in the body; got {:?}",
        facts.renders,
    );
    assert!(
        facts.renders.iter().any(|r| r.component == "Card"),
        "expected a RenderSite for `<Card/>` in the body; got {:?}",
        facts.renders,
    );
    insta::assert_debug_snapshot!(facts);
}
