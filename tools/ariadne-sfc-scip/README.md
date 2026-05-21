# ariadne-sfc-scip

A SCIP indexer for Vue and Svelte single-file components. It is the bridge
`ariadne-scip`'s `ScipVueIndexer` / `ScipSvelteIndexer` drivers invoke — see
`docs/adr/0013-scip-sfc-bridge.md`.

No off-the-shelf SCIP indexer covers `.vue` or `.svelte`. This CLI builds a
TypeScript program over the SFC sources and walks it through the type checker,
remapping each occurrence back to the original SFC text:

- **Vue** (`--framework vue`): wraps `ts.createProgram` with
  [`@volar/typescript`]'s `proxyCreateProgram` and the `@vue/language-core`
  language plugin so `.vue` files become program-visible TypeScript — the same
  mechanism `vue-tsc` uses — and remaps via Volar's `Language.maps`.
- **Svelte** (`--framework svelte`): the Svelte tooling exposes no Volar
  `LanguagePlugin`, so each `.svelte` file is transpiled to TypeScript with
  `svelte2tsx`, the generated modules back a plain `ts.createProgram`, and
  occurrence ranges are remapped through `svelte2tsx`'s Source Map v3 output.

## Placement

This package lives **outside the Cargo workspace** and is built and vendored
separately. It is a Node CLI on PATH — like `scip-typescript` itself — and is
never linked into the `ariadne` binary, so the plan's no-Node-in-binary rule
(plan.md D5) holds.

## Build

```sh
npm ci          # install the exact pinned dependency set
npm run build   # tsc -> dist/index.js + dist/scip.js
```

`npm ci` is required (not `npm install`) so the committed `package-lock.json`
pins every transitive dependency exactly. After building, put `dist/index.js`
on PATH as `ariadne-sfc-scip` (e.g. `npm link`, or symlink the bin).

## Usage

```sh
ariadne-sfc-scip --framework <vue|svelte> --cwd <project-root> --output <out.scip>
```

`--cwd` must contain a `tsconfig.json` or `jsconfig.json` and at least one
`.vue` (or `.svelte`) file. The command writes a SCIP protobuf index to
`--output` and exits non-zero with a diagnostic on stderr if it cannot.

## Vendoring story

`scip-typescript` is a CLI, not a library: it builds its own `ts.Program` from
a tsconfig and exposes no seam for a pre-built program, so its indexer cannot be
reused. Rather than vendoring `scip-typescript`'s generated protobuf bindings
plus its `google-protobuf` runtime, this tool implements a self-contained
minimal SCIP emit: `src/scip.ts` is a hand-written protobuf writer for the
Index/Document/Occurrence/SymbolInformation subset `ariadne-scip` ingests, with
field numbers tracking `crates/ariadne-scip/proto/scip.proto` at the SHA in
`proto/SCIP_COMMIT`. The Svelte source-map decode (Base64-VLQ Source Map v3) is
likewise hand-written rather than adding a sourcemap-codec dependency. The
emitted bytes are validated end-to-end by the Rust `ingest_vue.rs` /
`ingest_svelte.rs` goldens, which decode them through the real `prost`-generated
`proto::Index`.

## Dependencies

Every dependency is pinned exactly (`.npmrc` sets `save-exact=true`):

- `typescript` — the compiler the program is built on.
- `@volar/typescript` — `proxyCreateProgram` + the `Language` mapping layer (Vue).
- `@vue/language-core` — the Vue SFC language plugin.
- `svelte2tsx` — transpiles `.svelte` to TypeScript with a source map (Svelte).
- `svelte` — `svelte2tsx`'s peer dependency; supplies the SFC parser.

A pin bump of `@volar/typescript`, `@vue/language-core`, or `svelte2tsx` is a
follow-up change that must re-generate the `sample-vue` / `sample-svelte`
fixtures, since the position-mapping internals are version-coupled.

[`@volar/typescript`]: https://www.npmjs.com/package/@volar/typescript
