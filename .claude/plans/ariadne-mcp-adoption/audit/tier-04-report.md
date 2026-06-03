---
tier_id: tier-04
audited: 2026-06-03
verdict: PASS
commit: 93b2ed8fbbf9f9c05a01aaf29d2fc90ad5a905fe
---

<scope>
Tier-04 — non-blocking `PreToolUse` advisory that steers symbol-shaped
`Grep`/`Glob` patterns toward the Ariadne navigation tools, installed by
`ariadne setup`. Reviewed the working-tree diff on top of HEAD `93b2ed8`
scoped to the tier's `<files>` (the deliverable is uncommitted; the diff is
working-tree vs HEAD):

- `crates/ariadne-cli/src/commands/setup.rs` — embedded advisor template
  (`ADVISOR_HOOK`), `write_advisor_script`, `install_session_start_hook` →
  `install_hooks` rename, and `merge_settings_json` extended to deep-merge a
  `hooks.PreToolUse` advisory entry alongside the existing SessionStart merge.
- `.claude/hooks/ariadne-grep-advisor.sh` — the installed (dogfooded) classifier.
- `crates/ariadne-cli/tests/advisory.rs` — new: classification + never-deny/ask +
  settings-merge + idempotency.
- `crates/ariadne-cli/tests/setup.rs` — adjusted the SessionStart test's
  PreToolUse assertion from whole-array equality to membership (the array now
  also holds the advisory sibling).
- `.claude/settings.json` — gained the `Grep|Glob|Read` advisory entry; the Bash
  audit-gate entry retained.

No `Cargo.toml`/`Cargo.lock` change — no new dependency (pure `sh` script;
`serde_json`/`tempfile` already in tree).
</scope>

<checks_run>
- `cargo nextest run -p ariadne-cli` → **34/34 pass**, incl. the 5 new advisory
  tests (`advisor_nudges_symbol_shaped_grep`, `advisor_defers_quoted_log_string`,
  `advisor_defers_non_source_paths`, `advisor_never_denies_or_asks`,
  `setup_installs_pretooluse_advisory_preserving_audit_gate`) and the still-green
  `setup_installs_session_start_hook_preserving_existing_hooks`.
- `cargo clippy -p ariadne-cli --all-targets --all-features -- -D warnings` → clean.
- `cargo fmt --all --check` → clean.
- `cargo test --test architecture` → `architecture_invariants_hold` pass (no
  hexagonal violation; change is config + `ariadne-cli` composition root).
- **External schema verified** against the Claude Code hooks doc: `permissionDecision`
  enum is `allow`/`deny`/`ask`/`defer` — `defer` "defers to the normal permission
  flow" (the script's pass-through is valid, not an error); `additionalContext` is a
  valid PreToolUse field inserted "next to the tool result" [src:
  https://code.claude.com/docs/en/hooks].
- **Real run** of the installed `.claude/hooks/ariadne-grep-advisor.sh`: symbol
  `Catalog` → `allow` + context naming `find_definition`/`find_references`/`list_symbols`;
  `crate::commands::setup` → `allow`; `"failed to connect to daemon"` → `defer`,
  no context; `**/*.md` → `defer`; `Read README.md` → `defer`; `foo.*bar` (regex
  meta) → `defer`; empty + non-JSON → `defer`. Every output is valid JSON, exit 0.
- **Byte parity**: ran `ariadne setup` into a fresh temp dir; its
  `ariadne-grep-advisor.sh` is **byte-identical** to the dogfooded repo copy
  (`diff` → IDENTICAL), and the emitted `settings.json` carries the advisory
  PreToolUse entry. The repo script (mode `rwxr-xr-x`) is what `setup` produces.
- Read all changed files end-to-end; cross-checked the `tool_input.pattern` field
  name against the Grep tool input and confirmed `additionalContext` length (~400
  chars, well under the 10k cap, R4).
</checks_run>

<findings>
| id | category | severity | location | problem | fix |
|----|----------|----------|----------|---------|-----|
| F1 | correctness | INFO | `setup.rs:154` / `ariadne-grep-advisor.sh:49` | A single fixed `additionalContext` names all three nav tools for both shapes; plan `<steps>` 2 suggested differentiating "the specific tool for the shape (definition vs references)". Non-blocking — the text is factual, the binding exit criterion ("naming the matching Ariadne tool") is met, and naming all relevant tools is defensible. | Optionally branch the message: `::`-path/CamelCase → lead with `find_definition`; bare lower identifier → lead with `find_references`/`list_symbols`. |
| F2 | correctness | INFO | `ariadne-grep-advisor.sh:47-48` | A bare common English word (`error`, `TODO`, `todo`) is identifier-shaped and triggers a nudge even when it is free-text search — the residual false-positive class R5 accepts. Advisory and non-breaking, and tier-05 is the measurement gate; flagged only so the noise floor is on record. | If tier-05 shows noise, add a short stopword set or a min-length/has-`_`-or-`::`-or-CamelCase floor before nudging. |
</findings>

<verdict>
**PASS** — 0 FAIL, 2 INFO.

All four exit criteria independently verified:
1. The hook returns `permissionDecision:"allow"` + `additionalContext` naming
   Ariadne nav tools **only** for symbol-shaped `Grep`/`Glob` patterns — confirmed
   by running the installed script on a bare identifier and a `::`-path.
2. Non-symbol queries (whitespace phrases, quoted log strings, `*.md` globs,
   non-code paths, `Read`) pass through with `defer` and no `additionalContext` —
   confirmed by real run and the `advisor_defers_*` tests.
3. `ariadne setup` installs the script and the PreToolUse entry idempotently and
   preserves the Bash audit-gate — confirmed by the byte-idempotent merge test
   (seeded with an unsorted Bash entry) and fresh-temp-dir byte parity.
4. This repo's `settings.json` carries the advisory entry and a real symbol grep
   produces the injected suggestion — confirmed by directly exercising the
   dogfooded script.

D5 honored without exception: the classifier emits only `allow` or `defer`, never
`deny`/`ask` — proven across symbol/phrase/glob/path/empty/garbage by
`advisor_never_denies_or_asks`, so it can never block a legitimate search. Fail-open
on every unexpected input (empty/non-JSON → `defer`, exit 0). JSON is safe without
`jq`: the injected context is a fixed quote-free/backslash-free string and `printf
'%s'` avoids format-injection; `serde_json` parses every output in tests. No new
dependency, no hexagonal violation (the advisor is a config artifact owned by the
`ariadne-cli` composition root; the matcher is a sibling array entry, leaving the
audit-gate untouched).
</verdict>

<next_steps>
Verdict is PASS; no tier steps require redo. F1/F2 are optional, non-blocking polish
best deferred to tier-05's measurement signal (R2/R5): only tighten the heuristic or
shape-specialize the message if the adoption eval shows false-positive noise.
</next_steps>

<sources>
- [Claude Code hooks — PreToolUse permissionDecision enum (allow/deny/ask/defer) + additionalContext](https://code.claude.com/docs/en/hooks)
- [Claude Code MCP — 10k additionalContext cap](https://code.claude.com/docs/en/mcp)
- [Google eng-practices — reviewer standard (ship code health, not perfection)](https://google.github.io/eng-practices/review/reviewer/standard.html)
- Repo: tier-04 `<exit_criteria>`/`<steps>`/`<verification>`; plan.md D5/D6/R5; setup.rs; advisory.rs; .claude/settings.json; .claude/hooks/ariadne-grep-advisor.sh.
</sources>
