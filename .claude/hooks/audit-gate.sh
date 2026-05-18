#!/usr/bin/env bash
# PreToolUse hook on Bash. Blocks `git commit` / `git push` when the latest
# spec-audit verdict is FAIL, or when HEAD has advanced past the audited commit.
# Allows when no spec lifecycle has produced an audit-state.json yet.
# Exit code 2 blocks the tool call and surfaces stderr to Claude [src: https://code.claude.com/docs/en/hooks-guide].

set -euo pipefail

INPUT=$(cat)
CMD=$(printf '%s' "$INPUT" | jq -r '.tool_input.command // ""')

case "$CMD" in
  *"git commit"*|*"git push"*) ;;
  *) exit 0 ;;
esac

PLANS_DIR=".claude/plans"
[ -d "$PLANS_DIR" ] || exit 0

LATEST=$(find "$PLANS_DIR" -type f -name audit-state.json -print0 2>/dev/null \
  | xargs -0 ls -t 2>/dev/null | head -n 1 || true)
[ -n "$LATEST" ] || exit 0

VERDICT=$(jq -r '.verdict // "MISSING"' "$LATEST")
REPORT=$(jq -r '.report // ""' "$LATEST")
AUDITED_COMMIT=$(jq -r '.audited_commit // ""' "$LATEST")
SLUG_DIR=$(dirname "$LATEST")

if [ "$VERDICT" = "FAIL" ]; then
  echo "spec-audit FAIL for $SLUG_DIR. See $SLUG_DIR/$REPORT. Re-run /spec-build then /spec-audit before committing." >&2
  exit 2
fi

if [ "$VERDICT" != "PASS" ]; then
  echo "spec-audit verdict is '$VERDICT' in $LATEST. Re-run /spec-audit." >&2
  exit 2
fi

HEAD_SHA=$(git rev-parse HEAD 2>/dev/null || echo "")
if [ -n "$HEAD_SHA" ] && [ -n "$AUDITED_COMMIT" ] && [ "$AUDITED_COMMIT" != "$HEAD_SHA" ]; then
  echo "spec-audit PASS is for commit $AUDITED_COMMIT but HEAD is $HEAD_SHA. Re-run /spec-audit on current HEAD." >&2
  exit 2
fi

exit 0
