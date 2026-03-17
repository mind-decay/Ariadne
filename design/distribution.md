# Distribution & Installation

## Versioning

Ariadne follows [Semantic Versioning](https://semver.org/):

- **MAJOR** (1.0.0 → 2.0.0): Breaking changes to graph.json/clusters.json schema, CLI interface changes that break scripts, removal of supported languages
- **MINOR** (0.1.0 → 0.2.0): New languages, new CLI flags, new output fields (backward-compatible additions), new algorithms (Phase 2)
- **PATCH** (0.1.0 → 0.1.1): Parser bug fixes, performance improvements, dependency updates

### Pre-1.0

During 0.x development, MINOR version bumps may include breaking changes. Graph schema carries a `"version"` field — consumers should check it.

### Output Schema Versioning

`graph.json` has a `"version": 1` field. Schema changes:
- **Adding fields** to nodes/edges: compatible (version stays). Consumers must tolerate unknown fields.
- **Removing/renaming fields**: incompatible → version bump to 2. Old consumers will fail to deserialize.
- **Adding new edge/node types**: compatible (version stays). Consumers may encounter unknown enum values.

**Migration path:** When schema version bumps, the previous version is documented in a migration guide. Ariadne does NOT support outputting old schema versions — consumers must update.

## Installation Methods

### 1. Cargo Install (developers with Rust)

```bash
cargo install ariadne
```

- Builds from source via crates.io
- Requires Rust toolchain (rustup)
- Binary installed to `~/.cargo/bin/ariadne`
- Update: `cargo install ariadne --force`

**Crates.io requirements:**
- Package name: `ariadne` (check availability — there may be name conflicts on crates.io, see Decision section below)
- Include: src/, Cargo.toml, LICENSE, README.md
- Exclude: tests/fixtures/ (large), benches/, .github/ (via `Cargo.toml [package] exclude`)

### 2. Prebuilt Binaries (GitHub Releases)

```bash
# One-liner install script (recommended)
curl -fsSL https://raw.githubusercontent.com/<org>/ariadne/master/install.sh | sh

# Or manual download per platform:
# macOS ARM (Apple Silicon)
curl -Lo ariadne https://github.com/<org>/ariadne/releases/latest/download/ariadne-darwin-arm64
chmod +x ariadne && sudo mv ariadne /usr/local/bin/

# macOS Intel
curl -Lo ariadne https://github.com/<org>/ariadne/releases/latest/download/ariadne-darwin-x64

# Linux x64
curl -Lo ariadne https://github.com/<org>/ariadne/releases/latest/download/ariadne-linux-x64

# Linux ARM64
curl -Lo ariadne https://github.com/<org>/ariadne/releases/latest/download/ariadne-linux-arm64

# Windows x64
curl -Lo ariadne.exe https://github.com/<org>/ariadne/releases/latest/download/ariadne-windows-x64.exe
```

**Targets (5):**

| Target triple | Binary name | OS |
|---------------|-------------|-------|
| `aarch64-apple-darwin` | `ariadne-darwin-arm64` | macOS ARM |
| `x86_64-apple-darwin` | `ariadne-darwin-x64` | macOS Intel |
| `x86_64-unknown-linux-gnu` | `ariadne-linux-x64` | Linux x64 |
| `aarch64-unknown-linux-gnu` | `ariadne-linux-arm64` | Linux ARM64 |
| `x86_64-pc-windows-msvc` | `ariadne-windows-x64.exe` | Windows x64 |

### 3. Install Script (`install.sh`)

A shell script in the repo root that:
1. Detects OS and architecture
2. Downloads the correct binary from latest GitHub Release
3. Verifies checksum (SHA-256)
4. Installs to `/usr/local/bin/` (or `~/.local/bin/` if no sudo)
5. Verifies installation: `ariadne info`

```bash
curl -fsSL https://raw.githubusercontent.com/<org>/ariadne/master/install.sh | sh
```

**Script behavior:**
- Detects: `uname -s` (Darwin/Linux), `uname -m` (arm64/x86_64)
- Falls back to manual instructions if detection fails
- Checks for existing installation and reports version
- `--version <tag>` flag for installing specific version
- Never overwrites without confirmation (unless piped to sh)

### 4. Future: Package Managers

Not in Phase 1 scope. Considered for later:

| Manager | Priority | Notes |
|---------|----------|-------|
| Homebrew | High | `brew install ariadne` — macOS/Linux, formula in homebrew-core or tap |
| Nix | Medium | Flake-based package |
| AUR | Medium | Arch Linux user repository |
| apt/deb | Low | Requires maintaining a PPA |
| Chocolatey | Low | Windows package manager |

## Release Process

### Trigger

A release is triggered by pushing a git tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

### CI Pipeline

```
Tag push v*
  → GitHub Actions release.yml
    → Matrix build (5 targets)
      → cargo test (must pass)
      → cargo build --release --target <target>
      → Strip binary (strip on Linux/macOS)
      → Generate SHA-256 checksum per binary
      → Create GitHub Release with:
        - 5 binaries
        - 5 checksum files (.sha256)
        - Auto-generated changelog (from commits since last tag)
```

### Pre-release Checklist

Before tagging:
1. All tests pass (`cargo test`)
2. Clippy clean (`cargo clippy -- -D warnings`)
3. Format clean (`cargo fmt --check`)
4. Benchmarks show no regression >20%
5. CHANGELOG.md updated
6. Version in Cargo.toml matches tag

### Checksum Verification

Each release includes SHA-256 checksums:

```
ariadne-darwin-arm64.sha256
ariadne-darwin-x64.sha256
ariadne-linux-x64.sha256
ariadne-linux-arm64.sha256
ariadne-windows-x64.sha256
```

Users can verify:
```bash
sha256sum -c ariadne-linux-x64.sha256
```

The install script verifies automatically.

## Updating

### Check for Updates

```bash
ariadne info
# ariadne v0.1.0
# ...

# Check latest on GitHub:
curl -s https://api.github.com/repos/<org>/ariadne/releases/latest | grep tag_name
```

Phase 2 consideration: `ariadne self-update` command that downloads the latest binary and replaces itself. Not in Phase 1.

### Update Methods

| Install method | Update command |
|---------------|----------------|
| cargo install | `cargo install ariadne --force` |
| install.sh | Re-run `install.sh` (downloads latest) |
| Manual download | Re-download from releases page |
| Homebrew (future) | `brew upgrade ariadne` |

### Output Compatibility on Update

When updating Ariadne, existing `.ariadne/` output may need regeneration:

1. **Patch update (0.1.0 → 0.1.1):** Output is compatible. No action needed.
2. **Minor update (0.1.0 → 0.2.0):** Output may have new fields. Old graph.json still valid but `ariadne build` recommended to get new data.
3. **Major update (schema version bump):** Old graph.json may not work with new `ariadne query`. Rerun `ariadne build`.

**Detection:** `ariadne build` reads existing graph.json version field. If schema version doesn't match, emits:
```
warn: existing graph.json has schema version 1, current is 2. Rebuilding.
```

## Files in Repository

| File | Purpose |
|------|---------|
| `install.sh` | Install script for prebuilt binaries |
| `CHANGELOG.md` | User-facing release notes |
| `LICENSE` | License file (MIT or Apache-2.0 — TBD) |
| `.github/workflows/release.yml` | Cross-compilation + release publishing |
| `.github/workflows/ci.yml` | Test + lint on every push/PR |

## Crate Name

**Potential conflict:** The name `ariadne` may already be taken on crates.io (there's a popular error reporting crate called `ariadne`). If so, alternatives:
- `ariadne-graph`
- `ariadne-cli`
- `ariadne-deps`

Check availability before first publish. The binary name can differ from the crate name via `[[bin]]` in Cargo.toml:
```toml
[package]
name = "ariadne-graph"

[[bin]]
name = "ariadne"
path = "src/main.rs"
```

This way `cargo install ariadne-graph` installs a binary called `ariadne`.
