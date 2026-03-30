#!/usr/bin/env bash
set -euo pipefail

# Ariadne installer
#
# Two modes:
#   1. From cloned repo (via remote-install.sh): detects Cargo.toml, builds from source
#   2. Standalone: downloads pre-built binary from GitHub Releases
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mind-decay/Ariadne/master/remote-install.sh | bash
#   ./install.sh [--version v0.1.0] [--from-source]

REPO="mind-decay/Ariadne"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VERSION=""
FORCE_SOURCE=0

# ── Banner ────────────────────────────────────────────────────────────
echo "======================================="
echo "  Installing Ariadne"
echo "======================================="
echo ""

# ── Parse arguments ───────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --from-source)
            FORCE_SOURCE=1
            shift
            ;;
        --help|-h)
            echo "Usage: install.sh [--version <tag>] [--from-source]"
            echo "  --version <tag>   Install a specific version (e.g., v0.1.0)"
            echo "  --from-source     Force build from source even if binaries are available"
            echo "  --help            Show this help"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# ── Determine install directory ───────────────────────────────────────
determine_install_dir() {
    # If ariadne already exists somewhere in PATH, install there to avoid
    # shadow binaries (e.g., stale copy in ~/.cargo/bin vs new in /usr/local/bin).
    local existing_path
    existing_path="$(command -v ariadne 2>/dev/null || true)"
    if [ -n "$existing_path" ]; then
        INSTALL_DIR="$(dirname "$existing_path")"
        if [ ! -w "$INSTALL_DIR" ]; then
            USE_SUDO=1
        fi
        echo "  Updating existing installation in $INSTALL_DIR"
        return
    fi

    if [ -w /usr/local/bin ]; then
        INSTALL_DIR="/usr/local/bin"
    elif command -v sudo &>/dev/null && sudo -n true 2>/dev/null; then
        INSTALL_DIR="/usr/local/bin"
        USE_SUDO=1
    else
        INSTALL_DIR="${HOME}/.local/bin"
        mkdir -p "$INSTALL_DIR"
        if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
            echo "  Note: $INSTALL_DIR is not in your PATH."
            echo "  Add it with: export PATH=\"$INSTALL_DIR:\$PATH\""
        fi
    fi
}

# ── Install binary to destination ─────────────────────────────────────
install_binary() {
    local src="$1"
    local dest="$INSTALL_DIR/ariadne"
    chmod +x "$src"
    # Remove existing binary first to avoid "text file busy" on some systems,
    # then copy new binary (cp, not mv, so source cache stays intact).
    if [ "${USE_SUDO:-}" = "1" ]; then
        sudo rm -f "$dest"
        sudo cp "$src" "$dest"
    else
        rm -f "$dest"
        cp "$src" "$dest"
    fi
    rm -f "$src"
    echo "[OK] Installed to $dest"
}

# ── Mode 1: Build from source ────────────────────────────────────────
install_from_source() {
    echo "  Building from source..."

    if ! command -v cargo &>/dev/null; then
        echo "[ERROR] cargo not found. Install Rust: https://rustup.rs"
        exit 1
    fi

    cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" 2>&1 | tail -5

    local binary_path="$SCRIPT_DIR/target/release/ariadne"
    if [[ ! -f "$binary_path" ]]; then
        echo "[ERROR] Build succeeded but binary not found at $binary_path"
        exit 1
    fi

    # Copy (not move) so the build cache remains intact
    local tmp_binary
    tmp_binary="$(mktemp)"
    cp "$binary_path" "$tmp_binary"
    install_binary "$tmp_binary"
}

# ── Mode 2: Download pre-built binary ────────────────────────────────
install_from_release() {
    echo "  Downloading pre-built binary..."

    # Detect platform
    local os arch binary
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin)
            case "$arch" in
                x86_64) binary="ariadne-darwin-x64" ;;
                arm64)  binary="ariadne-darwin-arm64" ;;
                *)      echo "[ERROR] Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64)  binary="ariadne-linux-x64" ;;
                aarch64) binary="ariadne-linux-arm64" ;;
                *)       echo "[ERROR] Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        *)
            echo "[ERROR] Unsupported OS: $os"
            echo "  For Windows, download manually from:"
            echo "  https://github.com/$REPO/releases/latest"
            exit 1
            ;;
    esac

    # Resolve version
    if [ -z "$VERSION" ]; then
        VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" \
            | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/' || true)
        if [ -z "$VERSION" ]; then
            echo "[ERROR] No releases found. Install from source instead:"
            echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/master/remote-install.sh | bash"
            exit 1
        fi
    fi

    local base_url="https://github.com/$REPO/releases/download/$VERSION"

    # Download
    echo "  Downloading $binary ($VERSION)..."
    local tmpdir
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    local http_code
    http_code=$(curl -sL -o "$tmpdir/$binary" -w "%{http_code}" "$base_url/$binary")
    if [ "$http_code" != "200" ]; then
        echo "[ERROR] Download failed (HTTP $http_code)"
        echo "  URL: $base_url/$binary"
        echo ""
        echo "  No release binaries available. Falling back to source build..."
        install_from_source
        return
    fi

    # Verify SHA-256 checksum
    echo "  Verifying checksum..."
    http_code=$(curl -sL -o "$tmpdir/$binary.sha256" -w "%{http_code}" "$base_url/$binary.sha256")
    if [ "$http_code" = "200" ]; then
        local expected actual
        expected=$(awk '{print $1}' "$tmpdir/$binary.sha256")
        if command -v sha256sum &>/dev/null; then
            actual=$(sha256sum "$tmpdir/$binary" | awk '{print $1}')
        elif command -v shasum &>/dev/null; then
            actual=$(shasum -a 256 "$tmpdir/$binary" | awk '{print $1}')
        else
            echo "  Warning: no sha256sum or shasum found, skipping verification"
            actual="$expected"
        fi

        if [ "$expected" != "$actual" ]; then
            echo "[ERROR] Checksum verification failed!"
            echo "  Expected: $expected"
            echo "  Actual:   $actual"
            exit 1
        fi
        echo "  Checksum OK"
    else
        echo "  Warning: checksum file not available, skipping verification"
    fi

    install_binary "$tmpdir/$binary"
}

# ── Check for existing installation ───────────────────────────────────
if command -v ariadne &>/dev/null; then
    existing="$(ariadne --version 2>/dev/null || true)"
    echo "  Found existing installation: $existing"
fi

# ── Determine install directory ───────────────────────────────────────
determine_install_dir

# ── Choose mode ───────────────────────────────────────────────────────
# If we're running from inside the repo (Cargo.toml exists next to us),
# build from source. Otherwise, download a pre-built binary.

if [[ "$FORCE_SOURCE" = "1" ]]; then
    install_from_source
elif [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    install_from_source
else
    install_from_release
fi

# ── Verify ────────────────────────────────────────────────────────────
echo ""
if command -v ariadne &>/dev/null; then
    echo "======================================="
    echo "  Ariadne $(ariadne --version 2>/dev/null || echo '') installed"
    echo "======================================="
    echo ""
    echo "  Next: cd <project> && ariadne build ."
else
    echo "======================================="
    echo "  Ariadne installed"
    echo "======================================="
    echo ""
    echo "  Binary location: $INSTALL_DIR/ariadne"
    echo "  Next: cd <project> && ariadne build ."
fi
echo ""
