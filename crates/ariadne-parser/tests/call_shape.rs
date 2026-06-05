//! R1 completion: each grammar's call sub-patterns map to the right
//! [`CallKind`]. A `@call.free` capture is a bare identifier, `@call.method` a
//! receiver/member call, `@call.path` a scoped/qualified call. The salsa
//! resolver gates its cross-crate fallback on this shape, so a mislabel in a
//! grammar would silently re-open the phantom-edge path
//! [src: .claude/plans/r1-resolver-completion/tier-01-call-shape-gate.md].

use ariadne_core::Lang;
use ariadne_parser::{
    CallKind, ParserRegistry, SyntacticFacts, extract_syntactic_facts, parse_file,
};

/// Parse inline source and extract its merged facts.
fn facts_of(lang: Lang, src: &str) -> SyntacticFacts {
    let registry = ParserRegistry::new();
    let bytes = src.as_bytes();
    let parsed = parse_file(lang, &registry, bytes, None, &[]).expect("parse ok");
    assert!(
        !parsed.host.1.root_node().has_error(),
        "inline {lang:?} source produced a tree-sitter parse error",
    );
    extract_syntactic_facts(&parsed, bytes).expect("facts extraction")
}

/// Shape captured for the call to `callee`. Panics (with the captured set) if
/// no such call site was extracted, so a dropped capture fails loudly.
fn kind_of(facts: &SyntacticFacts, callee: &str) -> CallKind {
    facts
        .calls
        .iter()
        .find(|c| c.callee == callee)
        .unwrap_or_else(|| panic!("no call to `{callee}`; calls = {:?}", facts.calls))
        .kind
}

#[test]
fn rust_call_shapes() {
    let facts = facts_of(
        Lang::Rust,
        "fn caller() { free_call(); receiver.method_call(); Type::path_call(); }",
    );
    assert_eq!(kind_of(&facts, "free_call"), CallKind::Free);
    assert_eq!(kind_of(&facts, "method_call"), CallKind::Method);
    assert_eq!(kind_of(&facts, "path_call"), CallKind::Path);
}

#[test]
fn cpp_call_shapes() {
    let facts = facts_of(
        Lang::Cpp,
        "void caller() { free_call(); obj.method_call(); ns::path_call(); }",
    );
    assert_eq!(kind_of(&facts, "free_call"), CallKind::Free);
    assert_eq!(kind_of(&facts, "method_call"), CallKind::Method);
    assert_eq!(kind_of(&facts, "path_call"), CallKind::Path);
}

#[test]
fn c_call_shapes() {
    let facts = facts_of(Lang::C, "void caller() { free_call(); obj.method_call(); }");
    assert_eq!(kind_of(&facts, "free_call"), CallKind::Free);
    assert_eq!(kind_of(&facts, "method_call"), CallKind::Method);
}

#[test]
fn typescript_call_shapes() {
    let facts = facts_of(
        Lang::TypeScript,
        "function caller() { freeCall(); obj.methodCall(); }",
    );
    assert_eq!(kind_of(&facts, "freeCall"), CallKind::Free);
    assert_eq!(kind_of(&facts, "methodCall"), CallKind::Method);
}

#[test]
fn javascript_call_shapes() {
    let facts = facts_of(
        Lang::JavaScript,
        "function caller() { freeCall(); obj.methodCall(); }",
    );
    assert_eq!(kind_of(&facts, "freeCall"), CallKind::Free);
    assert_eq!(kind_of(&facts, "methodCall"), CallKind::Method);
}

#[test]
fn python_call_shapes() {
    let facts = facts_of(
        Lang::Python,
        "def caller():\n    free_call()\n    obj.method_call()\n",
    );
    assert_eq!(kind_of(&facts, "free_call"), CallKind::Free);
    assert_eq!(kind_of(&facts, "method_call"), CallKind::Method);
}

#[test]
fn go_call_shapes() {
    let facts = facts_of(
        Lang::Go,
        "package p\nfunc caller() {\n\tfreeCall()\n\tobj.methodCall()\n}\n",
    );
    assert_eq!(kind_of(&facts, "freeCall"), CallKind::Free);
    assert_eq!(kind_of(&facts, "methodCall"), CallKind::Method);
}

#[test]
fn csharp_call_shapes() {
    let facts = facts_of(
        Lang::CSharp,
        "class C { void M() { FreeCall(); obj.MethodCall(); } }",
    );
    assert_eq!(kind_of(&facts, "FreeCall"), CallKind::Free);
    assert_eq!(kind_of(&facts, "MethodCall"), CallKind::Method);
}

/// Java's `method_invocation` has no callee sub-grammar split, so the query
/// distinguishes shape with the `!object` negated-field predicate: an
/// object-less invocation is `Free`, one with a receiver is `Method`.
#[test]
fn java_call_shapes() {
    let facts = facts_of(
        Lang::Java,
        "class C { void m() { freeCall(); obj.methodCall(); } }",
    );
    assert_eq!(kind_of(&facts, "freeCall"), CallKind::Free);
    assert_eq!(kind_of(&facts, "methodCall"), CallKind::Method);
}

// Kotlin's `@call.free` relabel is mechanical: the tree-sitter-kotlin-ng
// `call_expression` shape the query assumes does not match real call nodes, so
// the capture is inert (the `facts_kotlin` golden snapshot already shows
// `calls: []`). There is no captured shape to assert; the relabel's correctness
// is pinned by that snapshot staying empty and the query still compiling.
