//! Tier-01 golden + structured tests for the pure outline assembler over byte
//! fixtures (rust / typescript / javascript), plus focused cases for the gap
//! collapse, `max_symbols` cap, and the multi-line signature probe (R3).
//!
//! Insta review: `cargo insta review -p ariadne-graph`.

use ariadne_core::{Lang, Visibility};
use ariadne_graph::{Outline, OutlineOptions, OutlineRequest, OutlineSymbol, assemble};

const SAMPLE_RS: &str = include_str!("fixtures/outline/sample.rs");
const SAMPLE_TS: &str = include_str!("fixtures/outline/sample.ts");
const SAMPLE_JS: &str = include_str!("fixtures/outline/sample.js");

/// Byte span `[start, end)` of the item anchored at `anchor`: `start` is the
/// anchor offset; `end` is the matching `}` of the first `{` at/after the
/// anchor (balanced), or the anchor line's end for a brace-less item.
fn span(src: &str, anchor: &str) -> (u32, u32) {
    let off = |n: usize| u32::try_from(n).expect("offset fits u32");
    let start = src.find(anchor).expect("anchor present");
    let bytes = src.as_bytes();
    if let Some(rel) = src[start..].find('{') {
        let mut depth = 0usize;
        let mut i = start + rel;
        let end = loop {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break i + 1;
                    }
                }
                _ => {}
            }
            i += 1;
        };
        (off(start), off(end))
    } else {
        let nl = src[start..].find('\n').map_or(src.len(), |p| start + p);
        (off(start), off(nl))
    }
}

/// Build an [`OutlineSymbol`] from an anchor + identity.
fn sym(src: &str, anchor: &str, name: &str, kind: &str, vis: Visibility) -> OutlineSymbol {
    let (byte_start, byte_end) = span(src, anchor);
    OutlineSymbol {
        name: name.to_owned(),
        kind: kind.to_owned(),
        byte_start,
        byte_end,
        visibility: vis,
    }
}

/// Default options: hide private symbols, no symbol cap.
fn opts() -> OutlineOptions {
    OutlineOptions {
        include_private: false,
        max_symbols: 0,
    }
}

fn rust_request() -> OutlineRequest {
    let s = SAMPLE_RS;
    OutlineRequest {
        source: s.as_bytes().to_vec(),
        symbols: vec![
            sym(s, "pub fn greet", "greet", "function", Visibility::Public),
            sym(s, "fn helper", "helper", "function", Visibility::Unknown),
            sym(
                s,
                "pub struct Counter",
                "Counter",
                "struct",
                Visibility::Public,
            ),
            sym(s, "impl Counter", "Counter", "impl", Visibility::Unknown),
            sym(s, "pub fn new", "new", "method", Visibility::Public),
            sym(s, "pub fn bump", "bump", "method", Visibility::Public),
        ],
        lang: Lang::Rust,
        options: opts(),
    }
}

/// Source line count consistent with the assembler's line model.
fn total_lines(src: &str) -> u32 {
    u32::try_from(src.split_inclusive('\n').count()).unwrap()
}

/// Every source line is kept or elided exactly once, and the skeleton is
/// strictly smaller than the source for a multi-symbol file.
fn assert_invariants(out: &Outline, src: &str) {
    assert_eq!(
        out.kept_lines + out.elided_lines,
        total_lines(src),
        "kept + elided must account for every source line"
    );
    assert!(
        out.skeleton.len() < src.len(),
        "skeleton ({}) must be smaller than source ({})",
        out.skeleton.len(),
        src.len()
    );
}

#[test]
fn rust_skeleton_golden() {
    let out = assemble(&rust_request());
    insta::assert_snapshot!("rust_skeleton", out.skeleton);
    assert_invariants(&out, SAMPLE_RS);

    // Private filter: the bare `fn helper` is dropped, body and all.
    assert!(
        !out.skeleton.contains("helper"),
        "private fn must be hidden"
    );
    // Nesting: methods render under the impl, indented; the long body folds.
    assert!(out.skeleton.contains("impl Counter {"));
    assert!(
        out.skeleton
            .contains("    pub fn bump(&mut self) -> u32 { … 3 lines }"),
        "nested method body folds with its exact line count"
    );
    // Fold count for the public function body.
    let greet = out
        .symbols
        .iter()
        .find(|e| e.name == "greet")
        .expect("greet");
    assert_eq!(greet.body_lines, 5);
    assert!(greet.has_body);
    let bump = out.symbols.iter().find(|e| e.name == "bump").expect("bump");
    assert_eq!(bump.body_lines, 3);
    // The index lists exactly the retained symbols (helper excluded).
    let names: Vec<&str> = out.symbols.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["greet", "Counter", "Counter", "new", "bump"]);
}

#[test]
fn typescript_skeleton_golden() {
    let s = SAMPLE_TS;
    let req = OutlineRequest {
        source: s.as_bytes().to_vec(),
        symbols: vec![
            sym(
                s,
                "export function add",
                "add",
                "function",
                Visibility::Public,
            ),
            sym(
                s,
                "interface Hidden",
                "Hidden",
                "interface",
                Visibility::Unknown,
            ),
            sym(s, "export const TWO", "TWO", "const", Visibility::Public),
        ],
        lang: Lang::TypeScript,
        options: opts(),
    };
    let out = assemble(&req);
    insta::assert_snapshot!("typescript_skeleton", out.skeleton);
    assert_invariants(&out, s);
    // Block doc comment captured byte-faithfully; non-exported interface hidden.
    assert!(out.skeleton.contains(" * Adds two numbers."));
    assert!(!out.skeleton.contains("interface Hidden"));
    assert!(
        out.skeleton
            .contains("export function add(a: number, b: number): number { … 3 lines }")
    );
    assert!(out.skeleton.contains("export const TWO = 2;"));
}

#[test]
fn javascript_skeleton_golden() {
    let s = SAMPLE_JS;
    let req = OutlineRequest {
        source: s.as_bytes().to_vec(),
        symbols: vec![
            sym(
                s,
                "export function double",
                "double",
                "function",
                Visibility::Public,
            ),
            sym(
                s,
                "function secret",
                "secret",
                "function",
                Visibility::Unknown,
            ),
            sym(s, "export const NAME", "NAME", "const", Visibility::Public),
        ],
        lang: Lang::JavaScript,
        options: opts(),
    };
    let out = assemble(&req);
    insta::assert_snapshot!("javascript_skeleton", out.skeleton);
    assert_invariants(&out, s);
    assert!(!out.skeleton.contains("secret"));
    assert!(out.skeleton.contains("// Doubles its argument."));
    assert!(
        out.skeleton
            .contains("export function double(n) { … 3 lines }")
    );
}

#[test]
fn include_private_keeps_the_private_symbol() {
    let mut req = rust_request();
    req.options.include_private = true;
    let out = assemble(&req);
    assert!(out.skeleton.contains("fn helper(x: u32) -> u32"));
    let names: Vec<&str> = out.symbols.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"helper"));
}

#[test]
fn deterministic_across_runs() {
    let req = rust_request();
    let a = assemble(&req);
    let b = assemble(&req);
    assert_eq!(a.skeleton, b.skeleton);
    assert_eq!(a.symbols, b.symbols);
}

#[test]
fn large_gap_collapses_to_a_marker() {
    let src = "fn a() {}\nx1\nx2\nx3\nx4\nx5\nx6\nx7\nx8\nx9\nx10\n\nfn b() {}\n";
    let req = OutlineRequest {
        source: src.as_bytes().to_vec(),
        symbols: vec![
            sym(src, "fn a", "a", "function", Visibility::Public),
            sym(src, "fn b", "b", "function", Visibility::Public),
        ],
        lang: Lang::Rust,
        options: opts(),
    };
    let out = assemble(&req);
    assert!(
        out.skeleton.contains("// … 11 lines elided"),
        "{}",
        out.skeleton
    );
    assert!(!out.skeleton.contains("x5"));
    assert_eq!(out.kept_lines + out.elided_lines, total_lines(src));
}

#[test]
fn max_symbols_caps_and_notes_the_tail() {
    let src = "pub fn one() {}\npub fn two() {}\npub fn three() {}\n";
    let req = OutlineRequest {
        source: src.as_bytes().to_vec(),
        symbols: vec![
            sym(src, "pub fn one", "one", "function", Visibility::Public),
            sym(src, "pub fn two", "two", "function", Visibility::Public),
            sym(src, "pub fn three", "three", "function", Visibility::Public),
        ],
        lang: Lang::Rust,
        options: OutlineOptions {
            include_private: false,
            max_symbols: 2,
        },
    };
    let out = assemble(&req);
    assert!(out.skeleton.contains("one"));
    assert!(out.skeleton.contains("two"));
    assert!(!out.skeleton.contains("three"));
    assert!(
        out.skeleton
            .contains("// … 1 more symbol elided (max_symbols=2)"),
        "{}",
        out.skeleton
    );
    assert_eq!(out.symbols.len(), 2);
    assert_eq!(out.kept_lines + out.elided_lines, total_lines(src));
}

#[test]
fn capped_container_drops_its_children_from_the_index() {
    // A retained child of a `max_symbols`-capped top-level container must not
    // surface in the index, since the parent (and thus the child) is elided
    // from the skeleton (INFO-3).
    let src = "pub fn first() {\n    1\n}\nimpl Thing {\n    pub fn method_a(&self) -> u32 {\n        1\n    }\n}\n";
    let req = OutlineRequest {
        source: src.as_bytes().to_vec(),
        symbols: vec![
            sym(src, "pub fn first", "first", "function", Visibility::Public),
            sym(src, "impl Thing", "Thing", "impl", Visibility::Unknown),
            sym(
                src,
                "pub fn method_a",
                "method_a",
                "method",
                Visibility::Public,
            ),
        ],
        lang: Lang::Rust,
        options: OutlineOptions {
            include_private: false,
            max_symbols: 1,
        },
    };
    let out = assemble(&req);
    // Only the uncapped container renders; the capped one and its child vanish.
    assert!(out.skeleton.contains("first"));
    assert!(!out.skeleton.contains("Thing"), "{}", out.skeleton);
    assert!(!out.skeleton.contains("method_a"), "{}", out.skeleton);
    // The index agrees with the skeleton: no child of the capped container.
    let names: Vec<&str> = out.symbols.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["first"], "capped child must not be indexed");
    assert!(
        out.skeleton
            .contains("// … 1 more symbol elided (max_symbols=1)"),
        "{}",
        out.skeleton
    );
    assert_eq!(out.kept_lines + out.elided_lines, total_lines(src));
}

#[test]
fn multiline_signature_is_kept_then_folded() {
    let src = "pub fn wide(\n    a: u32,\n    b: u32,\n) -> u32 {\n    let s = a + b;\n    s\n}\n";
    let req = OutlineRequest {
        source: src.as_bytes().to_vec(),
        symbols: vec![sym(
            src,
            "pub fn wide",
            "wide",
            "function",
            Visibility::Public,
        )],
        lang: Lang::Rust,
        options: opts(),
    };
    let out = assemble(&req);
    // The whole multi-line signature survives; the body folds to a marker.
    assert!(out.skeleton.contains("pub fn wide("));
    assert!(out.skeleton.contains("    a: u32,"));
    assert!(out.skeleton.contains("    b: u32,"));
    assert!(
        out.skeleton.contains(") -> u32 { … 3 lines }"),
        "{}",
        out.skeleton
    );
}
