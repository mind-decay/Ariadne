//! Language-injection engine: derive embedded JS/TS layers from a host tree.
//!
//! A `.vue` SFC is an HTML host skeleton with an embedded `<script>` block.
//! tree-sitter language injection re-parses chosen byte ranges of the full
//! file with a second grammar: `Parser::set_included_ranges` restricts the
//! parse to those ranges, and because the parse still runs over the *full*
//! file bytes the injected sub-tree's node offsets are already file-absolute
//! — no manual remap
//! [src: <https://tree-sitter.github.io/tree-sitter/3-syntax-highlighting.html>].
//!
//! Multiple `<script>` blocks (a Vue SFC may carry `<script>` plus
//! `<script setup>`) collapse into a *single* injected layer: one
//! included-ranges set, one JS/TS grammar. The layer takes the most
//! JSX-/type-capable grammar any `<script>` declares — `Tsx` for
//! `lang="tsx"`, `TypeScript` for `lang="ts"`, else `JavaScript`
//! [src: tier-03 step 5; docs/adr/0011-framework-grammars-injection.md].
//!
//! A Svelte SFC shares the Vue host shape — its `<script>` block is a
//! `script_element` with a `raw_text` content child — so the same
//! `<script>`-element walk derives its injected JS/TS layer. An Astro file
//! instead fences a TypeScript frontmatter block between leading `---`
//! tokens; the `tree-sitter-astro-next` grammar exposes it as a dedicated
//! `frontmatter` node whose `frontmatter_js_block` child is the embedded TS
//! [src: tier-04 steps 4-5; docs/adr/0011-framework-grammars-injection.md].

use std::ops::ControlFlow;
use std::time::Instant;

use ariadne_core::Lang;
use tree_sitter::{Node, ParseOptions, ParseState, Parser as TsParser, Range};

use super::Tree;
use super::incremental::{DEADLINE_SAMPLE_EVERY, PARSE_TIMEOUT};
use super::registry::ParserRegistry;
use crate::errors::ParserError;

/// Derive and parse the injected layers of a host `(Lang, Tree)`.
///
/// Returns an empty vector for any host grammar with no injection rule
/// (every single-grammar language) — the [`super::ParsedFile`] host-only
/// degenerate case.
///
/// # Errors
/// Propagates [`ParserError::UnsupportedLang`] /
/// [`ParserError::LanguageAssign`] / [`ParserError::IncludedRanges`] /
/// [`ParserError::ParseAborted`] from the injected-layer parse.
pub(crate) fn injected_layers(
    host_lang: Lang,
    host: &Tree,
    source: &[u8],
    registry: &ParserRegistry,
) -> Result<Vec<(Lang, Tree)>, ParserError> {
    let plan = injection_plan(host_lang, host, source);
    let mut layers = Vec::with_capacity(plan.len());
    for (lang, ranges) in plan {
        layers.push((lang, parse_injected(lang, &ranges, source, registry)?));
    }
    Ok(layers)
}

/// Derive the `(injected Lang, included byte ranges)` plan for a host tree.
/// Vue and Svelte inject their `<script>` blocks; Astro injects its
/// `---`-fenced frontmatter; every other host returns an empty plan.
fn injection_plan(host_lang: Lang, host: &Tree, source: &[u8]) -> Vec<(Lang, Vec<Range>)> {
    match host_lang {
        // A Vue or Svelte SFC's top level is HTML-shaped: each `<script>` is
        // a `script_element` whose `raw_text` child is the embedded JS/TS.
        // The two grammars share the node-type names, so one walk serves both
        // [src: tree-sitter-svelte-ng / tree-sitter-html node-types.json].
        Lang::Vue | Lang::Svelte => script_injection_plan(host, source),
        // An `.astro` file fences its TypeScript frontmatter in a dedicated
        // `frontmatter` node whose `frontmatter_js_block` child is the
        // embedded TS [src: tree-sitter-astro-next node-types.json +
        // queries/injections.scm].
        Lang::Astro => frontmatter_injection_plan(host),
        _ => Vec::new(),
    }
}

/// `<script>`-element injection plan shared by Vue and Svelte hosts: collapse
/// every `<script>` block's `raw_text` content into one injected JS/TS layer.
fn script_injection_plan(host: &Tree, source: &[u8]) -> Vec<(Lang, Vec<Range>)> {
    let mut scripts = Vec::new();
    collect_script_elements(host.root_node(), &mut scripts);

    let mut ranges = Vec::new();
    let mut lang = Lang::JavaScript;
    for script in scripts {
        let Some(range) = raw_text_range(script) else {
            continue; // empty `<script></script>` — nothing to inject
        };
        // The collapsed layer takes the most JSX-/type-capable grammar any
        // `<script>` requests: `Tsx` > `TypeScript` > `JavaScript`
        // [src: plan.md D2; tier-03 step 5].
        lang = match (lang, script_injected_lang(script, source)) {
            (Lang::Tsx, _) | (_, Lang::Tsx) => Lang::Tsx,
            (Lang::TypeScript, _) | (_, Lang::TypeScript) => Lang::TypeScript,
            _ => Lang::JavaScript,
        };
        ranges.push(range);
    }
    if ranges.is_empty() {
        return Vec::new();
    }
    // `set_included_ranges` requires earliest-to-latest, non-overlapping
    // ranges; distinct `<script>` blocks never overlap, so a start-byte sort
    // is sufficient (src: tree-sitter `set_included_ranges` docs).
    ranges.sort_by_key(|r| r.start_byte);
    vec![(lang, ranges)]
}

/// Astro frontmatter injection plan: the leading `---`-fenced block parses to
/// a `frontmatter` node whose `frontmatter_js_block` child holds the embedded
/// TypeScript. That child's content range becomes a single
/// [`Lang::TypeScript`] injected layer; a file with no frontmatter yields an
/// empty plan (the host-only degenerate case). A well-formed `.astro` file
/// has at most one `frontmatter` node, but the walk tolerates more — every
/// `frontmatter_js_block` range joins the one TypeScript layer.
fn frontmatter_injection_plan(host: &Tree) -> Vec<(Lang, Vec<Range>)> {
    let mut frontmatter = Vec::new();
    collect_frontmatter_elements(host.root_node(), &mut frontmatter);

    let mut ranges: Vec<Range> = frontmatter
        .iter()
        .filter_map(|node| frontmatter_js_block_range(*node))
        .collect();
    if ranges.is_empty() {
        return Vec::new();
    }
    // `set_included_ranges` requires earliest-to-latest, non-overlapping
    // ranges (src: tree-sitter `set_included_ranges` docs).
    ranges.sort_by_key(|r| r.start_byte);
    vec![(Lang::TypeScript, ranges)]
}

/// Recursively collect every `frontmatter` node under `node`.
fn collect_frontmatter_elements<'t>(node: Node<'t>, out: &mut Vec<Node<'t>>) {
    if node.kind() == "frontmatter" {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_frontmatter_elements(child, out);
    }
}

/// Byte+point range of a `frontmatter` node's `frontmatter_js_block` child —
/// the embedded TypeScript between the `---` fences. `None` for an empty
/// `frontmatter` (no `frontmatter_js_block` child).
fn frontmatter_js_block_range(frontmatter: Node<'_>) -> Option<Range> {
    let mut cursor = frontmatter.walk();
    frontmatter
        .children(&mut cursor)
        .find(|n| n.kind() == "frontmatter_js_block")
        .map(|n| n.range())
}

/// Re-parse `ranges` of `source` with `lang`'s grammar as an injected layer.
///
/// The parse runs under the same wall-clock [`PARSE_TIMEOUT`] guard as the
/// host parse, so a pathological `<script>` region cannot parse unbounded
/// [src: incremental.rs progress-callback deadline].
fn parse_injected(
    lang: Lang,
    ranges: &[Range],
    source: &[u8],
    registry: &ParserRegistry,
) -> Result<Tree, ParserError> {
    let language = registry
        .language(lang)
        .ok_or(ParserError::UnsupportedLang(lang))?;
    let mut parser = TsParser::new();
    parser
        .set_language(language)
        .map_err(|err| ParserError::LanguageAssign { lang, source: err })?;
    parser
        .set_included_ranges(ranges)
        .map_err(|err| ParserError::IncludedRanges { lang, source: err })?;
    // Mirror the host parse's throttled wall-clock deadline: tree-sitter's
    // progress callback fires very frequently, so only sample `Instant::now`
    // every Nth tick. Overrun fraction ≤ 1 / DEADLINE_SAMPLE_EVERY.
    let deadline = Instant::now() + PARSE_TIMEOUT;
    let mut ticks: u32 = 0;
    let mut on_progress = |_: &ParseState| -> ControlFlow<()> {
        ticks = ticks.wrapping_add(1);
        if ticks % DEADLINE_SAMPLE_EVERY == 0 && Instant::now() >= deadline {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let options = ParseOptions::new().progress_callback(&mut on_progress);
    let len = source.len();
    let mut read = |i: usize, _| -> &[u8] { if i < len { &source[i..] } else { &[] } };
    parser
        .parse_with_options(&mut read, None, Some(options))
        .ok_or(ParserError::ParseAborted { lang })
}

/// Recursively collect every `script_element` node under `node`.
fn collect_script_elements<'t>(node: Node<'t>, out: &mut Vec<Node<'t>>) {
    if node.kind() == "script_element" {
        out.push(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_script_elements(child, out);
    }
}

/// Byte+point range of a `script_element`'s `raw_text` content child.
fn raw_text_range(script: Node<'_>) -> Option<Range> {
    let mut cursor = script.walk();
    script
        .children(&mut cursor)
        .find(|n| n.kind() == "raw_text")
        .map(|n| n.range())
}

/// Injected `Lang` for a `<script>` element, read from its `start_tag`'s
/// `lang` attribute: `lang="tsx"` → [`Lang::Tsx`] (JSX needs the TSX grammar
/// — the `<T>x` cast ambiguity, plan.md D2), `lang="ts"` →
/// [`Lang::TypeScript`], absent or any other value → [`Lang::JavaScript`].
fn script_injected_lang(script: Node<'_>, source: &[u8]) -> Lang {
    let mut cursor = script.walk();
    let Some(start_tag) = script
        .children(&mut cursor)
        .find(|n| n.kind() == "start_tag")
    else {
        return Lang::JavaScript;
    };
    let mut tag_cursor = start_tag.walk();
    for attr in start_tag.children(&mut tag_cursor) {
        if attr.kind() == "attribute" && attribute_name(attr, source).as_deref() == Some("lang") {
            return match attribute_value(attr, source).as_deref() {
                Some("tsx") => Lang::Tsx,
                Some("ts") => Lang::TypeScript,
                _ => Lang::JavaScript,
            };
        }
    }
    Lang::JavaScript
}

/// Text of an `attribute` node's `attribute_name` child.
fn attribute_name(attr: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = attr.walk();
    attr.children(&mut cursor)
        .find(|n| n.kind() == "attribute_name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(str::to_owned)
}

/// Unquoted text of an `attribute` node's value (`attribute_value` directly,
/// or the `attribute_value` nested inside a `quoted_attribute_value`).
fn attribute_value(attr: Node<'_>, source: &[u8]) -> Option<String> {
    let mut cursor = attr.walk();
    for child in attr.children(&mut cursor) {
        match child.kind() {
            "attribute_value" => return child.utf8_text(source).ok().map(str::to_owned),
            "quoted_attribute_value" => {
                let mut inner = child.walk();
                return Some(
                    child
                        .children(&mut inner)
                        .find(|n| n.kind() == "attribute_value")
                        .and_then(|n| n.utf8_text(source).ok())
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
            _ => {}
        }
    }
    None
}
