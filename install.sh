#!/usr/bin/env bash
set -euo pipefail

# Ariadne installer — downloads the correct binary from GitHub Releases
REPO="your-org/ariadne"  # Update with actual repo
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

# Get latest release tag
LATEST=$(curl -sI "https://github.com/$REPO/releases/latest" | grep -i location | sed 's/.*tag\///' | tr -d '\r\n')
if [ -z "$LATEST" ]; then
    echo "Failed to determine latest release"
    exit 1
fi

URL="https://github.com/$REPO/releases/download/$LATEST/$BINARY"

echo "Downloading $BINARY ($LATEST)..."
curl -sL "$URL" -o "$INSTALL_DIR/ariadne"
chmod +x "$INSTALL_DIR/ariadne"
echo "Installed ariadne to $INSTALL_DIR/ariadne"
echo "Run 'ariadne --help' to get started"
