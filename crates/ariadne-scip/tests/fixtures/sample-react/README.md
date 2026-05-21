# sample-react — ariadne-scip tier-06 ingest fixture

Minimal, license-clean React/Solid-style project used by
`crates/ariadne-scip/tests/ingest_react.rs` to exercise `.tsx`/`.jsx`
semantic ingest. No third-party dependencies — `tsconfig.json` sets
`jsx: "preserve"`, so the JSX needs no React runtime or `@types/react`.

`Button` is a `.tsx` component imported and used by `App` in another
`.tsx` file, giving a cross-file definition→reference pair. `legacy.jsx`
is a standalone `.jsx` document.

## Regenerating index.scip

`index.scip` is a real `scip-typescript` index, committed alongside the
sources (same approach as `tests/fixtures/sample.scip`). Regenerate it —
only when the fixture sources change — with:

```sh
npm install -g @sourcegraph/scip-typescript
cd crates/ariadne-scip/tests/fixtures/sample-react
scip-typescript index --cwd . --output index.scip
```

This is the same invocation `ScipTypescriptIndexer::run` issues.
