# ADR-0032: warm-path invalid-cursor error-code parity

<status>
Accepted
Date: 2026-06-09
Decider: claude
</status>

<context>
ADR-0029/0030/0031 established that an un-honorable pagination cursor
(malformed, stale revision, or — for the diff tools — stale changeset
fingerprint) surfaces as a `CursorError`, which the MCP spec says to "handle
gracefully" by mapping to a JSON-RPC `invalid_params` (−32602) so a client tells
a bad argument from a server fault and re-queries [src:
https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination].

Every cursored tool has two serving paths that must agree byte-for-byte
(parity): the **cold** path (`ariadne-mcp` `tools::*`) and the **warm** path
(`ariadne-daemon` handlers, projected back through the MCP server). The cold
path already honors the spec: a `CursorError` becomes
`McpError::InvalidInput` → `invalid_params` [src:
crates/ariadne-mcp/src/errors.rs:40-46]. The warm path did not: every warm
handler returned a `CursorError` as the generic `DaemonResponse::Error(String)`,
which `project_daemon` maps to JSON-RPC `internal_error` (−32603) [src:
crates/ariadne-mcp/src/server.rs project_daemon]. The error *message* was already
identical (both call `CursorError::to_string()`); only the envelope code
diverged. The tier-04 audit flagged this as INFO finding F1 — a pre-existing
arc-wide divergence across all ten cursored warm handlers (find_references,
blast_radius, coupling_report, weak_spots, refactor_suggestions, hotspots,
complexity, co_change, diff_blast_radius, affected_tests) [src:
.claude/plans/data-fidelity-arc/block-1/audit/tier-04-report.md F1].
</context>

<decision>
Add a typed `DaemonResponse::InvalidInput(String)` arm to the daemon protocol,
distinct from the existing `DaemonResponse::Error(String)`. Every warm cursor
decode site returns the `CursorError` as `InvalidInput` instead of `Error`; the
ten sites are exactly the `Cursor::decode` / `DiffCursor::decode` calls (the
`Ok(c) => c` match arm), never the storage / not-found `Error` sites.

The three exhaustive consumers map it:
- `ariadne-mcp` `project_daemon` → `ErrorData::invalid_params` (−32602),
  byte-identical to the cold path's `McpError::InvalidInput`.
- `ariadne-cli` `query` and `affected-tests` → `bail!("{msg}")`, exactly like the
  `Error` arm, since the CLI has no JSON-RPC envelope (only the message matters).

`DaemonResponse::Error` keeps its meaning — query-level failures (symbol / file /
module not found, backend read errors) — and keeps mapping to `internal_error`,
matching the cold path's `McpError::NotFound`/`McpError::Storage`.
</decision>

<rationale>
- **Reliability / maintainability (parity):** the warm path now produces the
  same JSON-RPC error code as the cold path for the same caller-input fault, so
  cold == warm == CLI holds for error envelopes as it already did for success
  payloads. A client's "bad cursor → re-query without it" branch fires
  identically on either route [src: MCP pagination spec; tier-04 F1].
- **Maintainability (typed, not stringly):** distinguishing caller-input faults
  from server faults at the protocol type — rather than by sniffing the message
  string at the adapter — keeps the mapping total and compiler-checked. A new
  cursored handler that returns `InvalidInput` gets the right code for free.
- **Minimality (no behavior change beyond the code):** the message text is
  unchanged (shared `CursorError::to_string()`), the success path is untouched,
  and `Error` retains its `internal_error` mapping, so no not-found / storage
  contract shifts.
</rationale>

<alternatives>
- **Leave the divergence (status quo):** rejected — a stale cursor reads as a
  server bug (−32603) on the warm path, defeating the spec's "handle invalid
  cursors gracefully" and breaking cold/warm parity that the rest of the arc
  upholds. `[src: tier-04 F1; MCP pagination spec]`
- **Sniff the message string in `project_daemon`** (map `Error` whose text starts
  with "malformed"/"stale … cursor" to `invalid_params`): rejected — brittle,
  couples the adapter to error wording, and is not compiler-checked. `[src:
  reviewer standard: prefer typed over stringly-typed]`
- **Scope the fix to only the two tier-04 tools:** rejected — would leave eight
  other cursored tools on −32603, an internally inconsistent daemon worse than
  the uniform status quo. The audit explicitly scoped F1 arc-wide, "across all
  cursor tools." `[src: tier-04 F1 fix column]`
</alternatives>

<consequences>
- `DaemonResponse` gains an `InvalidInput(String)` variant — a new protocol
  revision. The protocol is in-workspace and single-binary, and the daemon
  restarts on a revision change, so no old daemon speaks the old shape (mirrors
  ADR-0031's protocol-revision reasoning).
- All three exhaustive `DaemonResponse` consumers (`project_daemon`, CLI `query`,
  CLI `affected-tests`) carry an `InvalidInput` arm; a future variant addition is
  caught by the compiler at these sites.
- A warm cursor-decode site that returns the generic `Error` for a `CursorError`
  (regressing the −32602 mapping) is off-limits without superseding this ADR.
</consequences>

<sources>
- `[src: .claude/plans/data-fidelity-arc/block-1/audit/tier-04-report.md F1]`
- `[src: docs/adr/0029-response-economy-cursor-verbosity.md ; 0030 ; 0031]`
- `[src: crates/ariadne-mcp/src/errors.rs:40-46 (McpError::into_rmcp)]`
- `[src: crates/ariadne-core/src/domain/daemon/response.rs (DaemonResponse)]`
- `[src: https://modelcontextprotocol.io/specification/2025-06-18/server/utilities/pagination]`
- `[src: https://google.github.io/eng-practices/review/reviewer/standard.html]`
</sources>
</content>
</invoke>
