---
block_id: block-b
title: Block B — broaden reach into four products
arc: intelligence-platform
order: 2
deps: [block-a]
status: seed   # seed → expand via /spec-plan into tiers
expand_with: /spec-plan .claude/plans/intelligence-platform/block-b-reach-products.md
---

<context>
Seed plan, not a tier set — scopes Block B generally for a later `/spec-plan`. Shared constraints/tech: `.claude/plans/intelligence-platform/plan.md`.
Problem: the graph is headless (MCP stdio + CLI only). Block B puts human- and CI-facing surfaces on it and ships the four products the user picked, all local-first, every surface a thin daemon client.
Success: a developer opens the explorer in a browser and navigates the self-index; the HTTP API answers every endpoint on loopback; `ariadne review` emits a correct PR report on a real branch and a GitHub Action posts it; `ariadne fitness check` gates CI.
Depends on Block A: the review bot composes A1 (affected-tests) + A2 (api-diff) + A3 (fitness); the explorer surfaces A3 violations as a panel. Run B's `/spec-plan` after A's tiers land.
Scope (in): local HTTP/JSON API; web codebase explorer; PR review & risk bot; fitness dashboard surface. Scope (out): auth/multi-tenant/hosted (local-first); a JS build toolchain (AD3).
</context>

<candidate_capabilities>
Likely tiers, general terms only.

**B1 — Local read-only HTTP/JSON API (→ explorer backend + reach enabler).**
A driving adapter `ariadne-http` on axum 0.8.9 — a thin daemon client (mirrors `daemon_client`) exposing the warm catalog queries as JSON over `127.0.0.1`, off by default, started by a subcommand [src: https://docs.rs/crate/axum/latest — `Router`, `State`, `Json`; AD4]. Also serves the explorer static assets via `tower_http::services::ServeDir` with `ServeFile` SPA fallback [src: 0.6.11 — https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs ; https://benw.is/posts/serving-static-files-with-axum]. Read-only, pure-Rust, single binary, loopback (constraints).

**B2 — Web codebase explorer (→ product: interactive explorer; also fitness dashboard).**
Static assets served by B1, reusing the existing deterministic SVG emitters for the graph views — project overview (`architecture_svg`), module drill-down (`module_svg`), diagram (`render_svg`) — with minimal vanilla JS for pan/zoom/click and JSON fetches for symbol detail, blast-radius overlay, hotspots heat, and an A3 fitness-violations panel [src: `crates/ariadne-graph/src/docgen.rs`, `crates/ariadne-graph/src/diagram.rs`; AD3 — no Node build]. No new viz dependency.

**B3 — PR review & risk bot (→ product: PR-risk bot).**
`ariadne review <base>..<head>` composes existing `diff_blast_radius` + `hotspots`/`complexity` with A1/A2/A3 into one structured report (markdown + JSON): blast radius, semver verdict, affected tests, risk hotspots on changed files, fitness regressions. A composite GitHub Action (repo YAML) runs the CLI on `pull_request` and posts the markdown via `gh pr comment` (upsert/sticky) under `permissions: pull-requests: write` [src: https://docs.github.com/actions ; https://github.com/cli/cli/issues/8374]. CLI does all analysis locally; the Action is thin glue (AR5).

**B4 — Fitness dashboard surface (→ product: architecture-fitness dashboard).**
Mostly composition: A3's `fitness check` is the CI gate; B2 renders the violations + coupling/cycle health as an explorer panel. May add a `fitness report --format json|md` surface for dashboards. No new engine.
</candidate_capabilities>

<existing_assets>
- Deterministic SVG emitters `architecture_svg` / `module_svg` / `render_svg`, golden-tested [src: `crates/ariadne-graph/src/docgen.rs`, `diagram.rs`; tests `*_svg_is_deterministic_and_well_formed`].
- `daemon_client` thin-client pattern to reuse for the HTTP adapter [src: `crates/ariadne-cli/src/adapters/daemon_client.rs`].
- Warm catalog (`WarmCatalog`) projecting all analytics the API exposes [src: `crates/ariadne-daemon/src/domain/catalog.rs`].
- Existing `diff_blast_radius`, `hotspots`, `complexity` use-cases the review bot composes [src: existing MCP tools, recon].
</existing_assets>

<open_questions>
Resolve in the `/spec-plan` expansion:
- B1: endpoint set + JSON schema (mirror MCP tools 1:1, or a smaller explorer-shaped surface?); does it live in a new `ariadne-http` crate or a CLI subcommand module? (lean new driving-adapter crate per hexagonal rule).
- B1: how `ariadne-http` stays a thin daemon client without depending on `ariadne-daemon` (adapter isolation) — composition-root wiring, ADR like ADR-0007/0015.
- B2: interactivity boundary — how much vanilla JS is acceptable before it becomes a build step; is a single vendored, pinned JS file allowed as a static asset?
- B3: report format/threshold policy (what makes a PR "high risk"); sticky-comment strategy (one upserted comment vs append); fork-PR permission handling (`pull_request_target` caveats) [src: GH Actions docs].
- B4: dashboard scope — explorer panel only, or also a static HTML report artifact for CI?
</open_questions>

<verification_intent>
Real-run gates (type-check alone does not count [src: CLAUDE.md `<rules>`]): launch the HTTP host, curl each endpoint, assert JSON; open the explorer in a browser against the self-index and walk overview→module→symbol→blast-radius→fitness; run `ariadne review` on a seeded branch and diff the report against expectation; run the Action in a test repo and confirm the comment posts. Each tier TDD.
</verification_intent>

<sources>
- axum / static serving: https://docs.rs/crate/axum/latest ; https://github.com/tokio-rs/axum/blob/main/examples/static-file-server/src/main.rs
- tower-http ServeDir: https://docs.rs/crate/tower-http/latest ; https://benw.is/posts/serving-static-files-with-axum
- GitHub Actions PR comment: https://docs.github.com/actions ; https://github.com/cli/cli/issues/8374
- Existing SVG emitters: `crates/ariadne-graph/src/docgen.rs`, `crates/ariadne-graph/src/diagram.rs`
- Arc master + inherited constraints: .claude/plans/intelligence-platform/plan.md
</sources>
