#!/usr/bin/env bash
# remote-install.sh — Bootstrap Ariadne from a remote curl invocation.
# Usage: curl -fsSL https://raw.githubusercontent.com/mind-decay/Ariadne/master/remote-install.sh | bash
#
# Clones the repo into a temp directory, runs the real install.sh, then cleans up.

set -euo pipefail

REPO_URL="https://github.com/mind-decay/Ariadne.git"
BRANCH="master"

# Allow override via env var
ARIADNE_REPO_URL="${ARIADNE_REPO_URL:-$REPO_URL}"
ARIADNE_BRANCH="${ARIADNE_BRANCH:-$BRANCH}"

# ── Prerequisite: git ─────────────────────────────────────────────────
if ! command -v git &>/dev/null; then
  echo "[ERROR] git is required to install Ariadne."
  exit 1
fi

# ── Clone into temp directory ─────────────────────────────────────────
TMPDIR_ARIADNE="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_ARIADNE"' EXIT

echo "Cloning Ariadne..."
git clone --depth 1 --branch "$ARIADNE_BRANCH" "$ARIADNE_REPO_URL" "$TMPDIR_ARIADNE" 2>&1 | tail -1

# ── Delegate to real installer ────────────────────────────────────────
exec bash "$TMPDIR_ARIADNE/install.sh"
