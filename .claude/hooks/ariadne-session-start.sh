#!/usr/bin/env sh
# Ariadne SessionStart hook — injects the project digest as factual
# `additionalContext` before the first prompt (<=10k chars). Installed by
# `ariadne setup`; do not edit by hand. Fail-open: any failure prints a minimal
# factual fallback and exits 0, never a non-zero exit or malformed JSON (both
# surface as a `hook error` and defeat the bootstrap)
# [src: https://code.claude.com/docs/en/hooks SessionStart; plan.md D3b/D3c].

set -u

# Absolute `ariadne` path resolved by `ariadne setup` at install time (mirrors
# the `.mcp.json` `command` entry), so the hook works when `ariadne` is not on
# PATH.
BIN='/Users/minddecay/.local/bin/ariadne'

# Claude Code exports CLAUDE_PROJECT_DIR into the hook environment; fall back to
# the current directory when the script is run by hand.
DIR="${CLAUDE_PROJECT_DIR:-.}"

# Minimal factual fallback, phrased as project state rather than an instruction
# (out-of-band imperative text trips prompt-injection defenses) [src: plan.md D3].
FALLBACK="Ariadne's read-only semantic graph is configured for this project. The Ariadne MCP tools answer symbol, reference, impact, and architecture questions in one call where grep and Read take many; project_status reports whether the index is current."

# Run the digest, capturing stdout. A missing binary or a non-zero exit leaves
# DIGEST empty, which falls back below.
DIGEST=""
if [ -x "$BIN" ]; then
  DIGEST=$("$BIN" digest "$DIR" 2>/dev/null) || DIGEST=""
fi
[ -n "$DIGEST" ] || DIGEST="$FALLBACK"

# Build the JSON with jq so quotes, backslashes, and newlines in the digest are
# escaped correctly; hand-interpolating the payload into a literal {...} is the
# parse-failure bug class [src: plan.md D3a]. Without jq the hook is a silent
# no-op (exit 0) rather than a malformed-JSON `hook error`.
command -v jq >/dev/null 2>&1 || exit 0
jq -n --arg ctx "$DIGEST" \
  '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$ctx}}'
exit 0
