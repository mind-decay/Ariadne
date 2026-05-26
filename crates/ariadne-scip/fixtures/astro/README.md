# astro — ariadne-scip tier-03 ingest fixture

Minimal, license-clean Astro project used by
`crates/ariadne-scip/tests/ingest_astro.rs` to exercise `.astro` component-script
(frontmatter) semantic ingest. No dependencies are installed — the `astro` entry
in `package.json` only exists so `ScipAstroIndexer::detect` recognises the
project; the bridge slices and type-checks the frontmatter itself.

`Page.astro` carries a `---`-fenced TypeScript frontmatter that imports `siteName`
from `src/util.ts` and declares `heading` and `banner`. `heading` is both defined
and referenced inside the frontmatter, giving a definition→reference edge; every
occurrence remaps back onto the original `.astro` source between the fences.

## Regenerating index.scip

`index.scip` is produced by the `ariadne-sfc-scip` bridge in `--framework astro`
mode and committed alongside the sources (same approach as
`tests/fixtures/sample-svelte/index.scip`). Regenerate it — only when the fixture
sources change — with:

```sh
( cd tools/ariadne-sfc-scip && npm ci && npm run build )
node tools/ariadne-sfc-scip/dist/index.js \
  --framework astro \
  --cwd crates/ariadne-scip/fixtures/astro \
  --output crates/ariadne-scip/fixtures/astro/index.scip
```

This is the same invocation `ScipAstroIndexer::run` issues against the
`ariadne-sfc-scip` binary on PATH.
