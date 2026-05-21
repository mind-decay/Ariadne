//! Tier-03 step 1: Vue SFC multi-region parse + merged syntactic facts.
//!
//! A `.vue` file is an HTML host skeleton plus an embedded JS/TS `<script>`
//! block. Parsing it yields a [`ariadne_parser::ParsedFile`] with an HTML
//! host layer and one injected TypeScript layer; the merged facts must
//! carry both the `<script>`-block declarations and the `<template>`'s
//! child-component render site, all with file-absolute byte spans.

mod common;

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, ParserRegistry, parse_file};

#[test]
fn registry_supports_vue() {
    assert!(
        ParserRegistry::new().supports(Lang::Vue),
        "Vue host grammar must be registered",
    );
}

#[test]
fn parsed_file_has_html_host_and_one_injected_layer() {
    let source = common::fixture("vue/sample.vue");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Vue, &registry, &source, None, &[]).expect("parse vue ok");

    assert_eq!(
        parsed.host.0,
        Lang::Vue,
        "host layer is the Vue/HTML grammar"
    );
    assert!(
        !parsed.host.1.root_node().has_error(),
        "vue fixture host tree has a parse error",
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
        "`<script setup lang=\"ts\">` injects a TypeScript layer",
    );
}

#[test]
fn script_lang_tsx_injects_a_tsx_layer() {
    let source = common::fixture("vue/script-tsx.vue");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Vue, &registry, &source, None, &[]).expect("parse vue ok");

    assert_eq!(
        parsed.injected.len(),
        1,
        "expected exactly one injected <script> layer",
    );
    assert_eq!(
        parsed.injected[0].0,
        Lang::Tsx,
        "`<script lang=\"tsx\">` must inject a Tsx layer, not TypeScript",
    );
    assert!(
        !parsed.injected[0].1.root_node().has_error(),
        "JSX in `<script lang=\"tsx\">` must parse clean under the TSX grammar",
    );
}

#[test]
fn two_scripts_collapse_into_one_injected_layer() {
    let source = common::fixture("vue/two-scripts.vue");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Vue, &registry, &source, None, &[]).expect("parse vue ok");

    assert_eq!(
        parsed.injected.len(),
        1,
        "`<script>` + `<script setup>` must collapse into one injected layer; got {:?}",
        parsed
            .injected
            .iter()
            .map(|(lang, _)| *lang)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        parsed.injected[0].0,
        Lang::Tsx,
        "the collapsed layer takes the most JSX-capable grammar any `<script>` \
         requests: `lang=\"tsx\"` + `lang=\"ts\"` escalates to Tsx",
    );
    assert!(
        !parsed.injected[0].1.root_node().has_error(),
        "both `<script>` byte ranges must parse clean under the collapsed grammar",
    );

    let facts = common::facts_for(Lang::Vue, "vue/two-scripts.vue");
    assert!(
        facts.decls.iter().any(|d| d.name == "cardName"),
        "expected `cardName` from the first `<script lang=\"tsx\">` block; got {:?}",
        facts.decls,
    );
    assert!(
        facts.decls.iter().any(|d| d.name == "bump"),
        "expected `bump` from the second `<script setup>` block; got {:?}",
        facts.decls,
    );
}

#[test]
fn merged_facts_cover_script_decls_and_template_render() {
    let facts = common::facts_for(Lang::Vue, "vue/sample.vue");

    assert!(
        facts.decls.iter().any(|d| d.name == "onSelect"),
        "expected the <script> block's `onSelect` decl; got {:?}",
        facts.decls,
    );
    assert!(
        facts
            .decls
            .iter()
            .any(|d| matches!(d.kind, DeclKind::Function | DeclKind::Variable)),
        "expected a <script>-block declaration; got {:?}",
        facts.decls,
    );
    assert!(
        facts.renders.iter().any(|r| r.component == "Child"),
        "expected a RenderSite for `<Child/>` in <template>; got {:?}",
        facts.renders,
    );
    insta::assert_debug_snapshot!(facts);
}
