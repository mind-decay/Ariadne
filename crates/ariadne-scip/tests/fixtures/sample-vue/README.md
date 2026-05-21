# sample-vue — ariadne-scip tier-07 ingest fixture

Minimal, license-clean Vue single-file-component project used by
`crates/ariadne-scip/tests/ingest_vue.rs` to exercise `.vue` semantic
ingest. No dependencies are installed — the `vue` entry in `package.json`
only exists so `ScipVueIndexer::detect` recognises the project; the bridge
resolves the `.vue` modules itself.

`Button.vue` declares `buttonName` in a plain `<script>` block; `Card.vue`
and `App.vue` import it through their `<script setup>` blocks, giving a
cross-`.vue` definition→reference pair. `Card` and `Button` are also used
as child components, so the index carries component-default references too.

## Regenerating index.scip

`index.scip` is produced by the `ariadne-sfc-scip` Volar bridge and
committed alongside the sources (same approach as `sample-react/index.scip`).
Regenerate it — only when the fixture sources change — with:

```sh
( cd tools/ariadne-sfc-scip && npm ci && npm run build )
node tools/ariadne-sfc-scip/dist/index.js \
  --framework vue \
  --cwd crates/ariadne-scip/tests/fixtures/sample-vue \
  --output crates/ariadne-scip/tests/fixtures/sample-vue/index.scip
```

This is the same invocation `ScipVueIndexer::run` issues against the
`ariadne-sfc-scip` binary on PATH.
