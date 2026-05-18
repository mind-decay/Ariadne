---
tier_id: tier-03
title: tree-sitter parser pipeline (incremental edit, syntactic facts, CST cache)
deps: [tier-01, tier-02]
exit_criteria:
  - `ParserRegistry::for_lang(Lang) -> Option<&Parser>` returns a configured tree-sitter parser for each v1 lang.
  - `Parser::parse_file(content, prev_tree) -> Tree` performs incremental re-parse using `tree_sitter::InputEdit`.
  - `extract_syntactic_facts(&Tree, Lang) -> SyntacticFacts` emits canonical decls/imports/calls per language via tree-sitter queries.
  - Parse-cache round-trip via storage (serialize tree → redb → deserialize) is byte-stable.
  - Proptest: random edit sequence produces same Tree as full reparse (semantic equivalence: same root node count + same span fingerprint).
  - Criterion: parse a 10MB JS file in <100ms cold, <5ms incremental for single-token edit.
status: pending
---

<context>
Syntactic backbone for any file regardless of SCIP indexer availability. tree-sitter is incremental and language-agnostic [src: https://github.com/tree-sitter/tree-sitter]. Per-lang grammars are external crates.
</context>

<files>
- `crates/ariadne-parser/Cargo.toml` — `tree-sitter`, `tree-sitter-typescript`, `tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-java`, `tree-sitter-kotlin`, `tree-sitter-c-sharp`, `bincode`, workspace `thiserror`/`tracing`.
- `crates/ariadne-parser/src/lib.rs` — re-exports `ParserRegistry`, `Parser`, `SyntacticFacts`, `ParserError`.
- `crates/ariadne-parser/src/registry.rs` — `ParserRegistry::new()` builds one `tree_sitter::Language` per lang.
- `crates/ariadne-parser/src/incremental.rs` — `Parser::parse_file` (uses `set_language` + `Parser::parse_with_options` + `InputEdit`).
- `crates/ariadne-parser/src/queries/<lang>.scm` — tree-sitter query files (decls, imports, calls).
- `crates/ariadne-parser/src/facts.rs` — runs queries on the Tree, emits `SyntacticFacts { decls: Vec<Decl>, imports: Vec<Import>, calls: Vec<CallSite> }`.
- `crates/ariadne-parser/src/cache.rs` — serialize/deserialize tree-sitter tree to/from bytes via `Tree::serialize` (or `tree-sitter` blob API); store via ariadne-storage `ParseCache`.
- `crates/ariadne-parser/tests/incremental.rs` — proptest random edits equivalence.
- `crates/ariadne-parser/tests/facts_<lang>.rs` — golden insta snapshot per lang on fixture file.
- `crates/ariadne-parser/benches/parse.rs` — criterion cold + incremental.
- `crates/ariadne-parser/fixtures/<lang>/*` — small real-world snippets (single-file, public-domain or MIT-licensed).
</files>

<steps>
1. Add tree-sitter + grammar crates as workspace deps. Each grammar crate publishes a `language()` fn returning `tree_sitter::Language` [src: https://github.com/tree-sitter/tree-sitter/tree/master/lib/binding_rust].
2. `ParserRegistry`: a `HashMap<Lang, tree_sitter::Language>` populated at construction. Cloning is cheap (Language is `Arc`-like).
3. **Failing test first** (`tests/facts_typescript.rs`): assert that parsing fixture `fixtures/typescript/sample.ts` yields a `SyntacticFacts` whose insta-snapshot matches `tests/snapshots/facts_typescript__sample.snap`. Test fails until step 7.
4. `Parser` wrapper: holds a `tree_sitter::Parser` (per-thread; not Send). Provide `Parser::for_lang(lang, &registry) -> Parser`. Cap parse timeout at 5s via `Parser::set_timeout_micros` [src: https://docs.rs/tree-sitter/latest/tree_sitter/struct.Parser.html].
5. Incremental parse: signature `parse_file(content: &[u8], prev_tree: Option<&Tree>, edits: &[InputEdit]) -> Result<Tree>`. For first parse, `prev_tree = None`. For subsequent: apply `Tree::edit(edit)` for each edit, then `parser.parse(content, Some(&old_tree))` [src: https://github.com/tree-sitter/tree-sitter/blob/master/lib/binding_rust/README.md].
6. Write tree-sitter query files (`.scm`) for each lang capturing:
   - Declarations: function/class/method/struct/interface/enum/type-alias (per-lang node types).
   - Imports: import/use/require statements with raw module path.
   - Calls: identifier inside `call_expression` / `invocation_expression`.
   Query reference: https://tree-sitter.github.io/tree-sitter/using-parsers#pattern-matching-with-queries
7. `extract_syntactic_facts`: `tree_sitter::Query::new(lang, query_src)` + `QueryCursor::matches`. Map captures to typed `Decl`/`Import`/`CallSite` records with byte-spans. Emit `SyntacticFacts`.
8. Cache: `serialize(tree: &Tree) -> Vec<u8>` calls `tree_sitter::Tree::root_node().to_sexp()` — wait, no, use `Tree::included_ranges` + raw bytes is not stable. Instead: store raw content + lang in `ParseCache` and re-parse on cold load (parsing 1 file is fast; full-tree serialization is not part of tree-sitter's public stable API). Document this trade-off in `cache.rs` doc-comment.
9. Proptest (`tests/incremental.rs`): for a fixture file, generate N random `InputEdit` sequences; for each prefix, assert that `parse(content, Some(prev_tree), edits)` produces a Tree whose root S-expression equals `parse(content, None, &[])` [src: tree-sitter docs above].
10. Per-lang golden snapshots (`tests/facts_<lang>.rs`): parse fixture, snapshot `SyntacticFacts` debug repr via insta.
11. Criterion bench (`benches/parse.rs`): cold parse 10MB synthetic JS file (assert <100ms); incremental single-char edit on the same (assert <5ms). Numbers match published tree-sitter expectations (sub-ms incremental) [src: https://github.com/tree-sitter/tree-sitter].
12. Unsafe code: tree-sitter uses C FFI internally but the Rust binding wraps it. Permit `unsafe = "allow"` only inside `ariadne-parser` crate's `Cargo.toml` lints override.
</steps>

<verification>
- `cargo nextest run -p ariadne-parser` green for all 8 langs.
- `cargo bench -p ariadne-parser` reports cold ≤100ms/10MB JS, incremental ≤5ms; CI gate.
- Manual: parse a real-world file (e.g., `rust-analyzer/crates/parser/src/lib.rs`); facts include at least 1 decl per top-level item.
- Property suite: 100 random edit sequences pass; if any divergence, fail loud (do not weaken to "S-expr equality with tolerance").
</verification>

<rollback>
`git rm -r crates/ariadne-parser` + workspace member removal. No on-disk state owned by this tier (parse_cache table created in tier-02; tombstone via tier-02 changeset if needed).
</rollback>
