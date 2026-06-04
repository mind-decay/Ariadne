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
}

/// Classify `path` into a [`DocKind`] by deterministic string match in a
/// fixed priority order: Vendored → Generated → Fixture → Test → Source.
/// All matching is pure string work; no IO.
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
    } else {
        DocKind::Source
    }
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
/// [src: plan.md tier-05; CLAUDE.md `<architecture>`].
#[must_use]
pub fn symbol_role(kind: &str, file: &str) -> String {
    let layer = LayerHint::of(file).word();
    match crate_of(file) {
        Some(name) => format!("{kind} in the {name} {layer} layer"),
        None => format!("{kind} in the {layer} layer"),
    }
}
