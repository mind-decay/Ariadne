#!/usr/bin/env sh
# Ariadne PreToolUse advisor — for a symbol-shaped Grep/Glob pattern, returns
# permissionDecision:"allow" plus additionalContext naming the Ariadne tool that
# answers it in one call; every other call defers untouched. Installed by
# `ariadne setup`; do not edit by hand. Advisory by construction: it emits only
# "allow" or "defer", NEVER "deny"/"ask", so it can never block a legitimate
# search (D5). Any unexpected input defers (fail-open; precision over recall, R5)
# [src: plan.md D5, R5; https://code.claude.com/docs/en/hooks PreToolUse].

set -u

# Defer: let the tool call through unchanged, with no added context. This is the
# only output besides a precise symbol-shaped match below.
defer() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"defer"}}\n'
  exit 0
}

# Nudge: allow the call and inject $1 as advisory additionalContext, then exit.
# Each message is a fixed quote-free/backslash-free string, so it interpolates
# into the JSON safely without jq.
nudge() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow","additionalContext":"%s"}}\n' "$1"
  exit 0
}

# stdin carries the PreToolUse payload {"tool_name":...,"tool_input":{...}}. An
# empty or unreadable payload defers.
PAYLOAD=$(cat 2>/dev/null) || defer
[ -n "$PAYLOAD" ] || defer

# Extract the tool name (no jq: a flat string field). Anything we cannot read
# cleanly leaves TOOL empty and defers below.
TOOL=$(printf '%s' "$PAYLOAD" | sed -n 's/.*"tool_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

# Only Grep/Glob carry a search pattern worth classifying. Read takes a file
# path, never a symbol query (and a bare filename like `Makefile` would look
# identifier-shaped), so Read — and any other tool — defers.
case "$TOOL" in
  Grep|Glob) : ;;
  *) defer ;;
esac

# Extract the search pattern up to the first quote. A value containing an
# (escaped) quote — i.e. a quoted phrase — truncates to something the identifier
# test below rejects, so it defers. Intended.
QUERY=$(printf '%s' "$PAYLOAD" | sed -n 's/.*"pattern"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
[ -n "$QUERY" ] || defer

# Two factual messages, one per shape (audit F1): a definition-shaped query (a
# `::`-path or a CamelCase type) leads with find_definition; a snake_case query
# leads with find_references/list_symbols. Both name all three nav tools and stay
# quote- and backslash-free so they interpolate without jq [src: plan.md D5].
DEF_CTX="Ariadne's read-only semantic graph can resolve this symbol in one call: find_definition jumps straight to where it is defined, find_references then lists every call site across files, and list_symbols searches symbol names by substring or kind. The graph captures cross-file edges a text grep misses; consider the Ariadne MCP tools before scanning text."
REF_CTX="Ariadne's read-only semantic graph can resolve this symbol in one call: find_references lists every call site across files and list_symbols searches symbol names by substring or kind, while find_definition locates the definition. The graph captures cross-file edges a text grep misses; consider the Ariadne MCP tools before scanning text."

# Symbol-shaped heuristic with a structural floor (precision over recall, R5).
# The pattern must first be a bare identifier or a `::`-path; whitespace phrases,
# quoted strings, regex metacharacters, globs and file paths (with `/` or `.`)
# fail both and defer. A bare identifier then nudges ONLY if it carries a code
# signal — a `::` path, a `_` (snake_case), or a case mix (CamelCase). A bare
# all-lowercase or all-caps word with none of these (error, TODO, render) is
# free-text-shaped and defers: the residual false-positive class R5 trades away
# [src: audit F2].
if printf '%s' "$QUERY" | grep -Eq '^[A-Za-z_][A-Za-z0-9_]*(::[A-Za-z_][A-Za-z0-9_]*)+$'; then
  # Shape A — `::`-separated path (crate::mod::Type): definition-lead.
  nudge "$DEF_CTX"
elif printf '%s' "$QUERY" | grep -Eq '^[A-Za-z_][A-Za-z0-9_]*$'; then
  # Shapes B/C — a bare identifier. Apply the structural floor.
  if printf '%s' "$QUERY" | grep -q '_'; then
    # Shape C — snake_case (has `_`): references-lead.
    nudge "$REF_CTX"
  elif printf '%s' "$QUERY" | grep -Eq '[A-Z]' && printf '%s' "$QUERY" | grep -Eq '[a-z]'; then
    # Shape B — CamelCase / mixed case: definition-lead.
    nudge "$DEF_CTX"
  fi
  # else: bare all-lowercase or all-caps word, no code signal — fall through.
fi

defer
