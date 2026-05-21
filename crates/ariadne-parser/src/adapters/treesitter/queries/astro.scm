; tier-04 syntactic-fact query — Astro SFC host layer (tree-sitter-astro-next).
;
; An `.astro` file's body is HTML-shaped: elements, and capitalised tags are
; child components (a `<Layout>`, an imported `.astro`/framework component).
; This query runs against the Astro host layer only; the `---`-fenced
; frontmatter's decls/imports/calls come from the injected TypeScript
; layer's own query.
; node-type reference:
;   tree-sitter-astro-next-0.1.1/src/node-types.json
;
;   @render.component  the `tag_name` of every opening / self-closing tag.
;                      facts.rs keeps only capitalised names as RenderSites
;                      (a child component `<Card/>`); lower-case host tags
;                      (`div`, `h1`, and the `script`/`style` injection-host
;                      elements) are dropped by the same post-filter that
;                      classifies JSX tags. `end_tag` is not captured, so
;                      `<Layout></Layout>` yields one RenderSite.
;
; Astro expression interpolations (`{expr}`) parse to `html_interpolation`
; nodes whose embedded JS is opaque `permissible_text`; they carry no
; render/hook capture and stay un-captured this tier (tier-04 step 7).

(start_tag
  (tag_name) @render.component)

(self_closing_tag
  (tag_name) @render.component)
