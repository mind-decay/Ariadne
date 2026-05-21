; tier-04 syntactic-fact query — Svelte SFC host layer (tree-sitter-svelte-ng).
;
; A `.svelte` file's top level is HTML-shaped: `<script>`/`<style>` are
; elements, child components are custom-element tags (plan.md D5). This query
; runs against the Svelte host layer only; the `<script>` block's
; decls/imports/calls/hooks come from the injected JS/TS layer's own query.
; node-type reference:
;   tree-sitter-svelte-ng-1.0.2/src/node-types.json
;
;   @render.component  the `tag_name` of every opening / self-closing tag.
;                      facts.rs keeps only capitalised names as RenderSites
;                      (a custom component `<Child/>`); lower-case host tags
;                      (`div`, `h1`, and the `script`/`style` injection-host
;                      elements) are dropped by the same post-filter that
;                      classifies JSX tags. `end_tag` is not captured, so
;                      `<Child></Child>` yields one RenderSite.
;
; Svelte logic blocks (`{#each}`, `{#if}`) parse to `each_statement` /
; `if_statement` nodes with no `tag_name` child — they carry no
; render/hook capture and are not expressible with the tier-02 capture set,
; so they stay un-captured this tier (tier-04 step 7). Directives (`bind:`,
; `on:`, `use:`) stay visible as plain attributes (plan.md R-VueDir;
; docs/adr/0011-framework-grammars-injection.md).

(start_tag
  (tag_name) @render.component)

(self_closing_tag
  (tag_name) @render.component)
