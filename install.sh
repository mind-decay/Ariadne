#!/usr/bin/env bash
set -euo pipefail

# Ariadne installer — downloads the correct binary from GitHub Releases
REPO="anthropics/ariadne"
INSTALL_DIR="${INSTALL_PATH:-/usr/local/bin}"

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
            x86_64) BINARY="ariadne-linux-x64" ;;
            *)      echo "Unsupported architecture: $ARCH"; exit 1 ;;
        esac
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

# Get latest release tag via GitHub API
LATEST=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')
if [ -z "$LATEST" ]; then
    echo "Failed to determine latest release"
    exit 1
fi

URL="https://github.com/$REPO/releases/download/$LATEST/$BINARY"

echo "Downloading $BINARY ($LATEST)..."
TMPFILE=$(mktemp)
HTTP_CODE=$(curl -sL -o "$TMPFILE" -w "%{http_code}" "$URL")
if [ "$HTTP_CODE" != "200" ]; then
    rm -f "$TMPFILE"
    echo "Download failed (HTTP $HTTP_CODE). URL: $URL"
    exit 1
fi

mv "$TMPFILE" "$INSTALL_DIR/ariadne"
chmod +x "$INSTALL_DIR/ariadne"
echo "Installed ariadne to $INSTALL_DIR/ariadne"
echo "Run 'ariadne --help' to get started"
