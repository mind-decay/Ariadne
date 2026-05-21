; tier-03 syntactic-fact query — Vue SFC host layer (tree-sitter-html grammar).
;
; A `.vue` SFC's top level is valid HTML: `<template>`/`<script>`/`<style>`
; are elements, child components are custom-element tags (plan.md D1). This
; query runs against the HTML host layer only; the `<script>` block's
; decls/imports/calls/hooks come from the injected JS/TS layer's own query.
; node-type reference:
;   https://github.com/tree-sitter/tree-sitter-html/blob/master/src/node-types.json
;
;   @render.component  the `tag_name` of every opening / self-closing tag.
;                      facts.rs keeps only capitalised names as RenderSites
;                      (a custom component `<Child/>`); lower-case host tags
;                      (`div`, `template`, and the `script`/`style`
;                      injection-host elements) are dropped by the same
;                      post-filter that classifies JSX tags. `end_tag` is not
;                      captured, so `<Child></Child>` yields one RenderSite.
;
; Vue directives (`v-if`, `:prop`, `@event`) stay visible as plain HTML
; attributes; capturing them as typed facts is deferred to a tier-04
; follow-up (plan.md R-VueDir; docs/adr/0011-framework-grammars-injection.md).

(start_tag
  (tag_name) @render.component)

(self_closing_tag
  (tag_name) @render.component)
