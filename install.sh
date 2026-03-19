#!/usr/bin/env bash
set -euo pipefail

# Ariadne installer — downloads the correct binary from GitHub Releases
# Usage: curl -fsSL https://raw.githubusercontent.com/<org>/ariadne/master/install.sh | sh
#   or:  ./install.sh [--version v0.1.0]

REPO="anthropics/ariadne"
VERSION=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: install.sh [--version <tag>]"
            echo "  --version <tag>  Install a specific version (e.g., v0.1.0)"
            echo "  --help           Show this help"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Darwin)
        case "$ARCH" in
            x86_64) BINARY="ariadne-darwin-x64" ;;
            arm64)  BINARY="ariadne-darwin-arm64" ;;
            *)      echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    Linux)
        case "$ARCH" in
            x86_64)  BINARY="ariadne-linux-x64" ;;
            aarch64) BINARY="ariadne-linux-arm64" ;;
            *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "For Windows, download manually from:"
        echo "  https://github.com/$REPO/releases/latest"
        exit 1
        ;;
esac

# Determine install directory
if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
elif command -v sudo &>/dev/null && sudo -n true 2>/dev/null; then
    INSTALL_DIR="/usr/local/bin"
    USE_SUDO=1
else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "$INSTALL_DIR"
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        echo "Note: $INSTALL_DIR is not in your PATH."
        echo "Add it with: export PATH=\"$INSTALL_DIR:\$PATH\""
    fi
fi

# Check for existing installation
EXISTING=""
if command -v ariadne &>/dev/null; then
    EXISTING="$(ariadne --version 2>/dev/null || true)"
    echo "Found existing installation: $EXISTING"
fi

# Resolve version
if [ -z "$VERSION" ]; then
    VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" \
        | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || true)
    if [ -z "$VERSION" ]; then
        echo "Failed to determine latest release"
        exit 1
    fi
fi

if [ -n "$EXISTING" ] && echo "$EXISTING" | grep -q "${VERSION#v}"; then
    echo "Already up to date ($VERSION)"
    exit 0
fi

BASE_URL="https://github.com/$REPO/releases/download/$VERSION"

# Download binary
echo "Downloading $BINARY ($VERSION)..."
TMPDIR_DL=$(mktemp -d)
trap 'rm -rf "$TMPDIR_DL"' EXIT

HTTP_CODE=$(curl -sL -o "$TMPDIR_DL/$BINARY" -w "%{http_code}" "$BASE_URL/$BINARY")
if [ "$HTTP_CODE" != "200" ]; then
    echo "Download failed (HTTP $HTTP_CODE). URL: $BASE_URL/$BINARY"
    exit 1
fi

# Verify SHA-256 checksum
echo "Verifying checksum..."
HTTP_CODE=$(curl -sL -o "$TMPDIR_DL/$BINARY.sha256" -w "%{http_code}" "$BASE_URL/$BINARY.sha256")
if [ "$HTTP_CODE" = "200" ]; then
    EXPECTED=$(awk '{print $1}' "$TMPDIR_DL/$BINARY.sha256")
    if command -v sha256sum &>/dev/null; then
        ACTUAL=$(sha256sum "$TMPDIR_DL/$BINARY" | awk '{print $1}')
    elif command -v shasum &>/dev/null; then
        ACTUAL=$(shasum -a 256 "$TMPDIR_DL/$BINARY" | awk '{print $1}')
    else
        echo "Warning: no sha256sum or shasum found, skipping verification"
        ACTUAL="$EXPECTED"
    fi

    if [ "$EXPECTED" != "$ACTUAL" ]; then
        echo "Checksum verification failed!"
        echo "  Expected: $EXPECTED"
        echo "  Actual:   $ACTUAL"
        exit 1
    fi
    echo "Checksum OK"
else
    echo "Warning: checksum file not available, skipping verification"
fi

# Install
chmod +x "$TMPDIR_DL/$BINARY"
if [ "${USE_SUDO:-}" = "1" ]; then
    sudo mv "$TMPDIR_DL/$BINARY" "$INSTALL_DIR/ariadne"
else
    mv "$TMPDIR_DL/$BINARY" "$INSTALL_DIR/ariadne"
fi

echo "Installed ariadne ($VERSION) to $INSTALL_DIR/ariadne"

# Verify
if command -v ariadne &>/dev/null; then
    ariadne --version
fi
