//! Public-surface extractor (block A, A2).
//!
//! [`public_surface`] parses a file's bytes and returns its public
//! declarations as owned [`PublicSymbol`]s — name, kind, visibility, and the
//! normalized declaration-header text. Base and head surfaces both run through
//! this same tree-sitter path so the API-surface diff compares like-for-like,
//! never mixing stored SCIP visibility into one side
//! [src: .claude/plans/intelligence-platform/block-a/plan.md D3/D4].

use ariadne_core::{Lang, PublicSymbol, Visibility};

use super::facts::{DeclKind, extract_syntactic_facts};
use super::parse_file;
use super::registry::ParserRegistry;
use crate::errors::ParserError;

/// Extract the public surface of `bytes` parsed as `lang`: every declaration
/// whose visibility is [`Visibility::Public`], as an owned [`PublicSymbol`]
/// carrying its normalized declaration-header text. Results are sorted by
/// `(name, kind)` so output is deterministic across runs.
///
/// Visibility comes from the same tree-sitter fact path the indexer uses;
/// grammars that expose no public-visibility modifier for a decl (Python, plain
/// C, C++ section-marked members) leave it [`Visibility::Unknown`] and the decl
/// is therefore excluded — the `Unknown`-as-public policy is tier-05's, not
/// this surface's [src: crates/ariadne-core/src/domain/types/visibility.rs ;
/// block-a plan.md D3].
///
/// # Errors
/// [`ParserError`] when `lang` is unsupported or the parse aborts.
pub fn public_surface(lang: Lang, bytes: &[u8]) -> Result<Vec<PublicSymbol>, ParserError> {
    let registry = ParserRegistry::new();
    let parsed = parse_file(lang, &registry, bytes, None, &[])?;
    let facts = extract_syntactic_facts(&parsed, bytes)?;

    let mut surface: Vec<PublicSymbol> = facts
        .decls
        .iter()
        .filter(|decl| decl.visibility == Visibility::Public)
        .map(|decl| PublicSymbol {
            name: decl.name.clone(),
            kind: kind_tag(&decl.kind).to_owned(),
            visibility: decl.visibility,
            signature: declaration_header(lang, bytes, decl.def_byte_range),
        })
        .collect();
    // Deterministic order: by (name, kind). The fact path already emits decls
    // in source order, but two refs' surfaces must compare position-free.
    surface.sort_by(|a, b| {
        (a.name.as_str(), a.kind.as_str()).cmp(&(b.name.as_str(), b.kind.as_str()))
    });
    Ok(surface)
}

/// Canonical kind tag for a [`DeclKind`], mirroring the CLI's symbol-kind
/// labels so a `PublicSymbol.kind` matches a stored `SymbolRecord.kind`
/// [src: crates/ariadne-cli/src/domain/mod.rs:623-636].
fn kind_tag(kind: &DeclKind) -> &str {
    match kind {
        DeclKind::Function => "function",
        DeclKind::Method => "method",
        DeclKind::Class => "class",
        DeclKind::Struct => "struct",
        DeclKind::Enum => "enum",
        DeclKind::Interface => "interface",
        DeclKind::Trait => "trait",
        DeclKind::TypeAlias => "type",
        DeclKind::Record => "record",
        DeclKind::Object => "object",
        DeclKind::Module => "module",
        DeclKind::Variable => "variable",
        DeclKind::Component => "component",
        DeclKind::Other(s) => s.as_str(),
    }
}

/// Where a declaration's body opens, per language family.
enum BodyDelimiter {
    /// Brace-family languages open the body at the first `{`.
    Brace,
    /// Python opens the suite at the trailing `:`.
    Colon,
}

/// Per-`Lang` body-open delimiter (BR1 heuristic). Brace-family languages —
/// Rust, Go, Java, Kotlin, C#, C, C++, JS, TS, Tsx, and the Vue/Svelte/Astro
/// injected `<script>` blocks — cut the header at the first `{`; Python cuts at
/// the trailing `:`. The slice is a documented heuristic: multi-line or
/// macro-heavy headers may slice imperfectly, which the per-fixture golden
/// tests bound [src: block-a plan.md step 5; risk BR1].
fn body_delimiter(lang: Lang) -> BodyDelimiter {
    match lang {
        Lang::Python => BodyDelimiter::Colon,
        _ => BodyDelimiter::Brace,
    }
}

/// The normalized declaration-header text: the source slice from the
/// declaration's start up to (excluding) the language's body-open delimiter,
/// with internal whitespace runs collapsed to single spaces and the ends
/// trimmed. A decl with no delimiter in range (a bodyless `const`, a trait
/// method signature ending in `;`) takes its whole span [src: block-a plan.md
/// step 5].
fn declaration_header(lang: Lang, bytes: &[u8], range: (u32, u32)) -> String {
    let start = (range.0 as usize).min(bytes.len());
    let end = (range.1 as usize).min(bytes.len());
    let text = String::from_utf8_lossy(&bytes[start..end]);
    let header = match body_delimiter(lang) {
        // First `{` opens the body; everything before it is the header.
        BodyDelimiter::Brace => text.split_once('{').map_or(text.as_ref(), |(h, _)| h),
        // Trailing `:` opens a Python suite; earlier `:` are annotations.
        BodyDelimiter::Colon => text.rsplit_once(':').map_or(text.as_ref(), |(h, _)| h),
    };
    header.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use ariadne_core::{Lang, Visibility};

    use super::public_surface;

    /// Assert the surface of `src` parsed as `lang` is exactly `expected`
    /// (name, kind, signature) tuples, every entry [`Visibility::Public`], in
    /// the deterministic `(name, kind)` order `public_surface` guarantees.
    fn assert_surface(lang: Lang, src: &str, expected: &[(&str, &str, &str)]) {
        let got = public_surface(lang, src.as_bytes()).expect("surface");
        let tuples: Vec<(&str, &str, &str)> = got
            .iter()
            .map(|s| {
                assert_eq!(
                    s.visibility,
                    Visibility::Public,
                    "surface is public-only: {s:?}"
                );
                (s.name.as_str(), s.kind.as_str(), s.signature.as_str())
            })
            .collect();
        assert_eq!(tuples, expected, "surface mismatch for {lang:?}");
    }

    #[test]
    fn rust_surfaces_public_fn_header_and_excludes_private() {
        // TDD anchor (step 1): a `pub fn` surfaces with its header text; the
        // private `fn` is excluded.
        assert_surface(
            Lang::Rust,
            "pub fn alpha(x: u32) -> u32 { x }\nfn beta() {}\n",
            &[("alpha", "function", "pub fn alpha(x: u32) -> u32")],
        );
    }

    #[test]
    fn bodyless_decl_takes_its_whole_span() {
        // No body-open `{` in range → the whole def span is the header,
        // trailing `;` and all (BR1 fallback).
        assert_surface(
            Lang::Rust,
            "pub type Bag = std::vec::Vec<u32>;\n",
            &[("Bag", "type", "pub type Bag = std::vec::Vec<u32>;")],
        );
    }

    #[test]
    fn surface_is_sorted_by_name_then_kind() {
        // Source order (zeta, alpha) differs from the (name, kind) order the
        // surface must emit, proving the sort is applied for determinism.
        let got =
            public_surface(Lang::Rust, b"pub fn zeta() {}\npub fn alpha() {}\n").expect("surface");
        let names: Vec<&str> = got.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    /// One public + one private/non-exported decl per fixture language, asserting
    /// `public_surface` keeps exactly the public symbols with the correct header
    /// text and drops the rest. Covers the 15 fixture languages
    /// (`crates/ariadne-parser/fixtures/*`): rust, go, java, kotlin, csharp, c,
    /// cpp, javascript, typescript, react + solid (`.tsx`), python, vue, svelte,
    /// astro [src: block-a plan.md `exit_criteria`; risk BR2].
    #[test]
    fn per_language_public_surface_excludes_non_public() {
        assert_surface(
            Lang::Rust,
            "pub fn alpha(x: u32) -> u32 { x }\nfn beta() {}\npub struct Gamma { pub v: u32 }\n",
            &[
                ("Gamma", "struct", "pub struct Gamma"),
                ("alpha", "function", "pub fn alpha(x: u32) -> u32"),
            ],
        );
        // Go: exported-by-leading-uppercase; `beta` is unexported.
        assert_surface(
            Lang::Go,
            "package p\nfunc Alpha() int { return 1 }\nfunc beta() {}\n",
            &[("Alpha", "function", "func Alpha() int")],
        );
        assert_surface(
            Lang::Java,
            "public class C {\n  public int alpha() { return 1; }\n  private int beta() { return 2; }\n}\n",
            &[
                ("C", "class", "public class C"),
                ("alpha", "method", "public int alpha()"),
            ],
        );
        // Kotlin is public-by-default but the grammar only marks an explicit
        // `public` modifier; an unmodified decl stays `Unknown` (tier-05's
        // policy), so the snippet states `public` explicitly.
        assert_surface(
            Lang::Kotlin,
            "public fun alpha(): Int { return 1 }\nprivate fun beta(): Int { return 2 }\n",
            &[("alpha", "function", "public fun alpha(): Int")],
        );
        assert_surface(
            Lang::CSharp,
            "public class C {\n  public int Alpha() { return 1; }\n  private int Beta() { return 2; }\n}\n",
            &[
                ("Alpha", "method", "public int Alpha()"),
                ("C", "class", "public class C"),
            ],
        );
        // C has no public-visibility keyword; a free function is `Unknown` and a
        // `static` one is `Private`, so the surface is empty until tier-05's
        // `Unknown`-as-public policy [src: visibility.rs].
        assert_surface(
            Lang::C,
            "int alpha(void) { return 1; }\nstatic int beta(void) { return 2; }\n",
            &[],
        );
        // C++ section markers (`public:`/`private:`) bind to the enclosing
        // class, not to individual members, so only the class surfaces (BR1).
        assert_surface(
            Lang::Cpp,
            "class C {\npublic:\n  int alpha();\nprivate:\n  int beta();\n};\n",
            &[("C", "class", "class C")],
        );
        // JS/TS: `export` is Public; the header excludes the `export` keyword
        // because the def span is the declaration node, not the export wrapper.
        assert_surface(
            Lang::JavaScript,
            "export function alpha() { return 1; }\nfunction beta() { return 2; }\n",
            &[("alpha", "function", "function alpha()")],
        );
        assert_surface(
            Lang::TypeScript,
            "export function alpha(x: number): number { return x; }\nfunction beta(): void {}\n",
            &[("alpha", "function", "function alpha(x: number): number")],
        );
        // react + solid both parse as the `.tsx` grammar; an exported
        // JSX-returning function is classified `component`.
        assert_surface(
            Lang::Tsx,
            "export function Alpha() { return <div />; }\nfunction beta() {}\n",
            &[("Alpha", "component", "function Alpha()")],
        );
        assert_surface(
            Lang::Tsx,
            "export function Counter() { return <div />; }\nfunction helper() {}\n",
            &[("Counter", "component", "function Counter()")],
        );
        // Python exposes no visibility modifier; every top-level decl is
        // `Unknown`, so the surface is empty until tier-05's policy.
        assert_surface(
            Lang::Python,
            "def alpha(x: int) -> int:\n    return x\ndef _beta():\n    return 0\n",
            &[],
        );
        // Vue/Svelte/Astro: the `<script>`/frontmatter block re-parses as an
        // injected JS layer, so an exported function surfaces from it.
        assert_surface(
            Lang::Vue,
            "<script>\nexport function alpha() { return 1; }\nfunction beta() { return 2; }\n</script>\n<template><div></div></template>\n",
            &[("alpha", "function", "function alpha()")],
        );
        assert_surface(
            Lang::Svelte,
            "<script>\nexport function alpha() { return 1; }\nfunction beta() { return 2; }\n</script>\n",
            &[("alpha", "function", "function alpha()")],
        );
        assert_surface(
            Lang::Astro,
            "---\nexport function alpha() { return 1; }\nfunction beta() { return 2; }\n---\n<div></div>\n",
            &[("alpha", "function", "function alpha()")],
        );
    }
}
