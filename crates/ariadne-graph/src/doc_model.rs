//! Deterministic doc-layer source scoping plus crate / layer grouping.
//!
//! [`classify`] buckets a repository path into a [`DocKind`] using pure
//! string heuristics — no IO, no graph access — so the same path always
//! yields the same kind. [`DocScope`] decides which modules a generated
//! doc *reports* (default: `Source` only); it is a doc-layer filter and
//! never touches the graph, so `find_references` / `blast_radius` on a
//! fixture symbol still resolve [src: plan.md D3; tier-01-doc-scope-model].

/// Source-kind bucket for a repository path, in descending exclusion
/// priority: a path matching several markers takes the first kind below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    /// First-party source code — the only kind kept by the default scope.
    Source,
    /// Test or benchmark scaffolding (`/tests/`, `/benches/`, `_test.`).
    Test,
    /// Test fixtures / sample inputs (`/fixtures/`).
    Fixture,
    /// Vendored third-party code (`node_modules/`, `*.min.js`).
    Vendored,
    /// Build-generated code (`target/`, `*.pb.rs`).
    Generated,
    /// Project metadata, not first-party source: manifests / lock / config
    /// (`Cargo.toml`, `*.lock`, `*.json`, `*.yaml`) and the `.claude/` plan
    /// and audit tree. Kept out of the default scope so co-change /
    /// risk surfaces report code coupling, not manifest churn.
    Config,
}

/// Classify `path` into a [`DocKind`] by deterministic string match in a
/// fixed priority order: Vendored → Generated → Fixture → Test → Config →
/// Source. All matching is pure string work; no IO.
#[must_use]
pub fn classify(path: &str) -> DocKind {
    if path.contains("node_modules/") || path.ends_with(".min.js") {
        DocKind::Vendored
    } else if path.starts_with("target/") || path.contains("/target/") || path.ends_with(".pb.rs") {
        DocKind::Generated
    } else if path.contains("/fixtures/") || path.starts_with("fixtures/") {
        DocKind::Fixture
    } else if path.contains("/tests/")
        || path.starts_with("tests/")
        || path.contains("/benches/")
        || path.starts_with("benches/")
        || path.contains("_test.")
        || path.ends_with("/tests.rs")
    {
        DocKind::Test
    } else if path.contains(".claude/") || is_config_ext(path) {
        DocKind::Config
    } else {
        DocKind::Source
    }
}

/// True when `path`'s file extension marks project metadata (manifest, lock,
/// or config): `toml` / `lock` / `json` / `yaml` / `yml`, matched
/// case-insensitively. Used by [`classify`] to bucket such paths as
/// [`DocKind::Config`].
fn is_config_ext(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            ["toml", "lock", "json", "yaml", "yml"]
                .iter()
                .any(|ext| e.eq_ignore_ascii_case(ext))
        })
}

/// Doc-layer module filter. The default keeps only [`DocKind::Source`]
/// paths; `extra_excludes` are additional substring excludes layered on
/// top (CLI-configurable in tier-06). Never applied to the graph — it is
/// purely a reporting filter [src: plan.md D3].
#[derive(Debug, Clone, Default)]
pub struct DocScope {
    /// Additional substring excludes layered atop the `Source`-only default.
    pub extra_excludes: Vec<String>,
}

impl DocScope {
    /// True when `path` should appear in generated docs: it must classify
    /// as [`DocKind::Source`] and contain none of `extra_excludes`.
    #[must_use]
    pub fn include(&self, path: &str) -> bool {
        classify(path) == DocKind::Source
            && !self
                .extra_excludes
                .iter()
                .any(|ex| path.contains(ex.as_str()))
    }
}

/// Crate a path belongs to, taken as the first segment after a `crates/`
/// prefix (`crates/<name>/…` → `<name>`). Returns `None` for paths not
/// under `crates/`.
#[must_use]
pub fn crate_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("crates/")?;
    match rest.split('/').next() {
        Some(name) if !name.is_empty() => Some(name),
        _ => None,
    }
}

/// Hexagonal layer a path sits in, inferred from `src/domain` vs
/// `src/adapters` path segments \[src: CLAUDE.md `<architecture>`\].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerHint {
    /// Domain interior — `src/domain/…`.
    Domain,
    /// Driven or driving adapter — `src/adapters/…`.
    Adapter,
    /// Crate interior — neither domain nor adapter (façade, errors, …).
    Interior,
}

/// Crate names whose flat `src/` layout (no `src/domain` segment) hides their
/// domain-interior role from the [`LayerHint::of`] path heuristic. Pinned from
/// CLAUDE.md `<architecture>`: `ariadne-core` (types + ports), `ariadne-graph`
/// (analytics use cases), and `ariadne-salsa` (incremental query DB) are the
/// domain interior \[src: CLAUDE.md `<architecture>`\].
const DOMAIN_INTERIOR_CRATES: [&str; 3] = ["ariadne-core", "ariadne-graph", "ariadne-salsa"];

/// Hexagonal layer for `path`, applying the domain-interior crate override
/// before the [`LayerHint::of`] path heuristic: a file in a flat-`src` domain
/// crate reports [`LayerHint::Domain`]; every other path falls back to the
/// path-segment heuristic \[src: CLAUDE.md `<architecture>`\].
#[must_use]
pub fn layer_of(path: &str) -> LayerHint {
    match crate_of(path) {
        Some(name) if DOMAIN_INTERIOR_CRATES.contains(&name) => LayerHint::Domain,
        _ => LayerHint::of(path),
    }
}

impl LayerHint {
    /// Infer the [`LayerHint`] for `path` from its layer path segment.
    #[must_use]
    pub fn of(path: &str) -> Self {
        if path.contains("src/domain") {
            LayerHint::Domain
        } else if path.contains("src/adapters") {
            LayerHint::Adapter
        } else {
            LayerHint::Interior
        }
    }

    /// The lowercase word naming this layer, used in role one-liners.
    #[must_use]
    fn word(self) -> &'static str {
        match self {
            LayerHint::Domain => "domain",
            LayerHint::Adapter => "adapter",
            LayerHint::Interior => "interior",
        }
    }
}

/// One-line role descriptor for a symbol: its `kind` situated in the
/// hexagonal layer (and crate, when under `crates/`) of its defining `file`.
/// Derived purely from `kind` + the path's coupling shape, so the cold and
/// warm `doc_for` paths compute the identical string (parity)
/// \[src: plan.md tier-05; CLAUDE.md `<architecture>`\].
#[must_use]
pub fn symbol_role(kind: &str, file: &str) -> String {
    let layer = LayerHint::of(file).word();
    match crate_of(file) {
        Some(name) => format!("{kind} in the {name} {layer} layer"),
        None => format!("{kind} in the {layer} layer"),
    }
}
