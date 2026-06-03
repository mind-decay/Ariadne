#!/usr/bin/env bash
# PreToolUse hook on Bash. Gates `git commit` / `git push` on the latest
# spec-audit verdict: blocks ONLY when that verdict is FAIL. A stale PASS (HEAD
# advanced past the audited commit) or a missing/incomplete verdict no longer
# blocks — gating those over-blocked routine commits made between audits and was
# the dominant source of spurious denials. Allows when no audit-state.json
# exists yet.
#
# Fires ONLY on commands that actually invoke git commit/push: a separator-
# delimited segment whose first word is `git` (bare or a path) with `commit`/
# `push` as its subcommand. Commands that merely mention the words inside an
# echo/printf/grep/heredoc no longer trip the gate — the prior substring match
# (`*"git commit"*`) false-positived on those and spammed blocks
# [src: https://code.claude.com/docs/en/hooks "matcher filters on tool name"].
#
# Blocks via PreToolUse JSON `permissionDecision:"deny"` + reason — the
# documented way to prevent a tool call cleanly, vs a raw `exit 2` that surfaces
# as a `hook error` line [src: https://code.claude.com/docs/en/hooks PreToolUse
# output]. Fail-open: any internal error allows the command rather than emitting
# a non-blocking transcript error.

set -u

INPUT=$(cat)
CMD=$(printf '%s' "$INPUT" | jq -r '.tool_input.command // ""' 2>/dev/null || printf '')

# Emit "yes" when CMD invokes `git <want>` (want = commit|push) in any
# command segment. Splits on shell separators, strips leading VAR=val
# assignments, requires the first word to be `git`/`*/git`, then takes the
# first non-flag token as the subcommand (skipping `-C dir` / `-c k=v`).
invokes_git() {
  printf '%s' "$CMD" \
    | sed -E 's/&&|\|\|/\n/g; s/[|;]/\n/g' \
    | awk -v want="$1" '
        {
          sub(/^[ \t]+/, "")
          while ($0 ~ /^[A-Za-z_][A-Za-z0-9_]*=[^ \t]+[ \t]+/) \
            sub(/^[A-Za-z_][A-Za-z0-9_]*=[^ \t]+[ \t]+/, "")
          n = split($0, w, /[ \t]+/)
          if (n < 2) next
          if (w[1] != "git" && w[1] !~ /\/git$/) next
          for (i = 2; i <= n; i++) {
            if (w[i] == "-C" || w[i] == "-c") { i++; continue }
            if (substr(w[i], 1, 1) == "-") continue
            if (w[i] == want) found = 1
            break
          }
        }
        END { if (found) print "yes" }'
}

GATED=""
[ -n "$(invokes_git commit)" ] && GATED="commit"
[ -z "$GATED" ] && [ -n "$(invokes_git push)" ] && GATED="push"
[ -n "$GATED" ] || exit 0

PLANS_DIR=".claude/plans"
[ -d "$PLANS_DIR" ] || exit 0

LATEST=$(find "$PLANS_DIR" -type f -name audit-state.json -print0 2>/dev/null \
  | xargs -0 ls -t 2>/dev/null | head -n 1 || true)
[ -n "$LATEST" ] || exit 0

VERDICT=$(jq -r '.verdict // "MISSING"' "$LATEST" 2>/dev/null || printf 'MISSING')
REPORT=$(jq -r '.report // ""' "$LATEST" 2>/dev/null || printf '')
SLUG_DIR=$(dirname "$LATEST")

# Deny the tool call with a reason Claude can act on, then exit 0 (the JSON,
# not the exit code, carries the decision for PreToolUse).
deny() {
  jq -n --arg r "$1" '{
    hookSpecificOutput: {
      hookEventName: "PreToolUse",
      permissionDecision: "deny",
      permissionDecisionReason: $r
    }
  }'
  exit 0
}

if [ "$VERDICT" = "FAIL" ]; then
  deny "spec-audit FAIL for $SLUG_DIR. See $SLUG_DIR/$REPORT. Re-run /spec-build then /spec-audit before the git $GATED."
fi

exit 0
