//! Per-language entry-point classifier — tier-05 RD4.
//!
//! Reads the tier-04 [`SymbolRecord`](ariadne_core::SymbolRecord) metadata
//! exposed by the caller — [`Lang`], [`Visibility`], attribute list,
//! kind tag and canonical name — and returns whether the symbol is a
//! root for the dead-code filter. The decision is per-language so that a
//! Go `Test*` function, a Rust `#[test]`, and a Java `@Test` method all
//! resolve via the same shape: visibility + attributes + a documented
//! name convention used only where the language has no stronger signal.
//!
//! The pure-domain placement is intentional: `ariadne-graph` already
//! depends on `ariadne-core` and on no driven adapter, so the classifier
//! has the metadata it needs without violating the hexagonal boundary
//! \[src: CLAUDE.md `<rules>`\].

use ariadne_core::{Lang, Visibility};

/// Returns `true` when `(lang, visibility, attributes, kind, name)`
/// describes a language-level entry point. Callers use the result to
/// build a [`crate::DeadCodeConfig::roots`] set; the dead-code filter
/// skips any symbol in that set before testing fan-in.
///
/// `attributes` are the bare identifier names emitted by the parser
/// (`"test"`, `"Override"`, `"pytest.fixture"`), not the surface
/// syntax. `kind` is the free-form kind tag carried on
/// `SymbolRecord::kind`.
#[must_use]
pub fn is_root(
    lang: Lang,
    visibility: Visibility,
    attributes: &[String],
    kind: &str,
    name: &str,
) -> bool {
    match lang {
        Lang::Rust => rust_root(visibility, attributes, name),
        Lang::Go => go_root(visibility, name),
        Lang::Python => python_root(attributes, name),
        Lang::JavaScript
        | Lang::TypeScript
        | Lang::Tsx
        | Lang::Vue
        | Lang::Svelte
        | Lang::Astro => js_root(visibility),
        Lang::Java | Lang::Kotlin => jvm_root(visibility, attributes, kind, name),
        Lang::CSharp => csharp_root(visibility, attributes, kind, name),
        Lang::C | Lang::Cpp => c_root(visibility, name),
        // `Lang::Other` plus future `#[non_exhaustive]` variants are
        // not roots until a per-language rule is added.
        _ => false,
    }
}

fn last_segment(name: &str) -> &str {
    name.rsplit("::")
        .next()
        .unwrap_or(name)
        .rsplit('.')
        .next()
        .unwrap_or(name)
}

/// Whether any attribute's leaf segment (after `::` or `.`) matches
/// `needle`. Handles path-qualified forms like `tokio::test`,
/// `serial_test::serial`, `pytest.mark.parametrize` — the parser emits
/// the joined name and the leaf is the framework's marker.
fn attr_leaf_matches(attributes: &[String], needle: &str) -> bool {
    attributes.iter().any(|a| last_segment(a) == needle)
}

fn rust_root(visibility: Visibility, attributes: &[String], name: &str) -> bool {
    if visibility == Visibility::Public {
        return true;
    }
    for attr in ["test", "bench", "no_mangle", "export_name"] {
        if attr_leaf_matches(attributes, attr) {
            return true;
        }
    }
    last_segment(name) == "main"
}

fn go_root(visibility: Visibility, name: &str) -> bool {
    // `Visibility::Public` already encodes the upper-case-identifier rule
    // (parser fallback in attach_visibility).
    if visibility == Visibility::Public {
        return true;
    }
    let leaf = last_segment(name);
    leaf == "main" || leaf.starts_with("Test") || leaf.starts_with("Benchmark")
}

fn python_root(attributes: &[String], name: &str) -> bool {
    // pytest collects test_*-prefixed functions; the framework convention
    // is the sole signal when there is no decorator.
    let leaf = last_segment(name);
    if leaf == "__main__" || leaf.starts_with("test_") || leaf.starts_with("Test") {
        return true;
    }
    for prefix in ["pytest.", "click.", "fastapi.", "app."] {
        if attributes.iter().any(|a| a.starts_with(prefix)) {
            return true;
        }
    }
    attributes.iter().any(|a| a == "fixture" || a == "task")
}

fn js_root(visibility: Visibility) -> bool {
    // `export` lowers to `Visibility::Public` (parser typescript.scm /
    // javascript.scm @visibility capture on `export_statement`).
    visibility == Visibility::Public
}

fn jvm_root(visibility: Visibility, attributes: &[String], kind: &str, name: &str) -> bool {
    for attr in ["Test", "ParameterizedTest", "RepeatedTest"] {
        if attr_leaf_matches(attributes, attr) {
            return true;
        }
    }
    if visibility == Visibility::Public {
        return true;
    }
    last_segment(name) == "main" && kind != "field"
}

fn csharp_root(visibility: Visibility, attributes: &[String], kind: &str, name: &str) -> bool {
    for attr in ["Fact", "Theory", "Test", "TestMethod"] {
        if attr_leaf_matches(attributes, attr) {
            return true;
        }
    }
    if visibility == Visibility::Public {
        return true;
    }
    last_segment(name) == "Main" && kind != "field" || last_segment(name) == "main"
}

fn c_root(visibility: Visibility, name: &str) -> bool {
    if last_segment(name) == "main" {
        return true;
    }
    visibility == Visibility::Public
}
