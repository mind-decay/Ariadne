//! Tier-04 step 1: Svelte SFC multi-region parse + merged syntactic facts.
//!
//! A `.svelte` file is an HTML-ish host skeleton plus an embedded JS/TS
//! `<script>` block. Parsing it yields a [`ariadne_parser::ParsedFile`] with
//! a Svelte host layer and one injected TypeScript layer; the merged facts
//! must carry both the `<script>`-block declarations and the template's
//! child-component render site, all with file-absolute byte spans
//! (tier-04 exit criteria 1-2, 4).

mod common;

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, ParserRegistry, parse_file};

#[test]
fn registry_supports_svelte() {
    assert!(
        ParserRegistry::new().supports(Lang::Svelte),
        "Svelte host grammar must be registered",
    );
}

#[test]
fn parsed_file_has_svelte_host_and_one_injected_layer() {
    let source = common::fixture("svelte/sample.svelte");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Svelte, &registry, &source, None, &[]).expect("parse svelte ok");

    assert_eq!(
        parsed.host.0,
        Lang::Svelte,
        "host layer is the Svelte grammar",
    );
    assert!(
        !parsed.host.1.root_node().has_error(),
        "svelte fixture host tree has a parse error",
    );
    assert_eq!(
        parsed.injected.len(),
        1,
        "expected exactly one injected <script> layer; got {:?}",
        parsed
            .injected
            .iter()
            .map(|(lang, _)| *lang)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        parsed.injected[0].0,
        Lang::TypeScript,
        "`<script lang=\"ts\">` injects a TypeScript layer",
    );
    assert!(
        !parsed.injected[0].1.root_node().has_error(),
        "the `<script lang=\"ts\">` byte range must parse clean under the TS grammar",
    );
}

#[test]
fn merged_facts_cover_script_decls_and_template_render() {
    let facts = common::facts_for(Lang::Svelte, "svelte/sample.svelte");

    assert!(
        facts.decls.iter().any(|d| d.name == "increment"),
        "expected the <script> block's `increment` function decl; got {:?}",
        facts.decls,
    );
    assert!(
        facts
            .decls
            .iter()
            .any(|d| d.name == "count" && matches!(d.kind, DeclKind::Variable)),
        "expected the <script> block's `count` variable decl; got {:?}",
        facts.decls,
    );
    assert!(
        facts
            .imports
            .iter()
            .any(|i| i.path.contains("Child.svelte")),
        "expected the `./Child.svelte` import from <script>; got {:?}",
        facts.imports,
    );
    assert!(
        facts.renders.iter().any(|r| r.component == "Child"),
        "expected a RenderSite for `<Child/>` in the template; got {:?}",
        facts.renders,
    );
    insta::assert_debug_snapshot!(facts);
}
