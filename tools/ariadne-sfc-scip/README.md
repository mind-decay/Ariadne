# ariadne-sfc-scip

A SCIP indexer for Vue single-file components. It is the bridge `ariadne-scip`'s
`ScipVueIndexer` driver invokes — see `docs/adr/0013-scip-sfc-bridge.md`.

No off-the-shelf SCIP indexer covers `.vue`. This CLI wraps `ts.createProgram`
with [`@volar/typescript`]'s `proxyCreateProgram` and the `@vue/language-core`
language plugin so `.vue` files become program-visible TypeScript — the same
mechanism `vue-tsc` uses. It walks every `.vue` source file through the
TypeScript type checker, remaps each occurrence from virtual-TS positions back
to the original SFC text via Volar's `Language.maps`, and emits a SCIP index.

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
ariadne-sfc-scip --framework vue --cwd <project-root> --output <out.scip>
```

`--cwd` must contain a `tsconfig.json` or `jsconfig.json` and at least one
`.vue` file. The command writes a SCIP protobuf index to `--output` and exits
non-zero with a diagnostic on stderr if it cannot.

## Vendoring story

`scip-typescript` is a CLI, not a library: it builds its own `ts.Program` from
a tsconfig and exposes no seam for a pre-built Volar-wrapped program, so its
indexer cannot be reused. Rather than vendoring `scip-typescript`'s generated
protobuf bindings plus its `google-protobuf` runtime, this tool implements a
self-contained minimal SCIP emit: `src/scip.ts` is a hand-written protobuf
writer for the Index/Document/Occurrence/SymbolInformation subset `ariadne-scip`
ingests, with field numbers tracking `crates/ariadne-scip/proto/scip.proto` at
the SHA in `proto/SCIP_COMMIT`. The emitted bytes are validated end-to-end by
the Rust `crates/ariadne-scip/tests/ingest_vue.rs` golden, which decodes them
through the real `prost`-generated `proto::Index`.

## Dependencies

Every dependency is pinned exactly (`.npmrc` sets `save-exact=true`):

- `typescript` — the compiler the program is built on.
- `@volar/typescript` — `proxyCreateProgram` + the `Language` mapping layer.
- `@vue/language-core` — the Vue SFC language plugin.

A pin bump of `@volar/typescript` or `@vue/language-core` is a follow-up change
that must re-generate the `sample-vue` fixture, since Volar's position-mapping
internals are version-coupled.

[`@volar/typescript`]: https://www.npmjs.com/package/@volar/typescript
