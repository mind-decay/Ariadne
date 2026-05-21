# sample-svelte — ariadne-scip tier-08 ingest fixture

Minimal, license-clean Svelte single-file-component project used by
`crates/ariadne-scip/tests/ingest_svelte.rs` to exercise `.svelte` semantic
ingest. No dependencies are installed — the `svelte` entry in `package.json`
only exists so `ScipSvelteIndexer::detect` recognises the project; the bridge
resolves the `.svelte` modules itself.

`Button.svelte` declares `buttonName` in a `<script module>` block; `Card.svelte`
and `App.svelte` import it through their instance `<script>` blocks, giving a
cross-`.svelte` definition→reference pair. `Card` and `Button` are also used
as child components, so the index carries component-default references too.

## Regenerating index.scip

`index.scip` is produced by the `ariadne-sfc-scip` Svelte bridge and committed
alongside the sources (same approach as `sample-vue/index.scip`). Regenerate it
— only when the fixture sources change — with:

```sh
( cd tools/ariadne-sfc-scip && npm ci && npm run build )
node tools/ariadne-sfc-scip/dist/index.js \
  --framework svelte \
  --cwd crates/ariadne-scip/tests/fixtures/sample-svelte \
  --output crates/ariadne-scip/tests/fixtures/sample-svelte/index.scip
```

This is the same invocation `ScipSvelteIndexer::run` issues against the
`ariadne-sfc-scip` binary on PATH.
