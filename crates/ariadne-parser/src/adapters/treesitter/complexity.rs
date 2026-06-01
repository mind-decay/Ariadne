//! `McCabe` cyclomatic complexity from the tree-sitter CST.
//!
//! [`attach_complexity`] walks a parsed layer once, counts decision points,
//! attributes each to the innermost [`Decl`] whose span contains it (so a
//! nested captured function owns its own decisions — tier-12 D3), and sets
//! `complexity = decisions + 1` on every function-like decl (Function /
//! Method / Component — tier-12 D4); all other decls carry `0`.
//!
//! Strict `McCabe` (tier-12 D2 / plan RD8): `&&` and `||` each count as a
//! decision. The control-flow node set is per-`Lang`; the boolean operator is
//! a `binary_expression` `operator` field in every grammar except Python,
//! which uses a dedicated `boolean_operator` node. Every node kind below was
//! verified against the bundled grammar's `node-types.json` this session
//! [src: tree-sitter-{rust,python,javascript,typescript,go,java,kotlin-ng,
//! c-sharp,c,cpp} node-types.json].

use ariadne_core::Lang;
use tree_sitter::Node;

use super::Tree;
use super::facts::{Decl, DeclKind, innermost_containing_decl};

/// Count `McCabe` decisions per decl in one CST walk and write
/// `complexity = decisions + 1` on function-like decls (`0` on the rest).
///
/// `tree` is one parse layer; its node offsets are file-absolute (injected
/// layers parse over the full file), matching the decls' `def_byte_range`.
pub(super) fn attach_complexity(lang: Lang, decls: &mut [Decl], tree: &Tree) {
    if decls.is_empty() {
        return;
    }
    let mut counts = vec![0u32; decls.len()];
    let mut cursor = tree.walk();
    // Pre-order traversal of every node, including the anonymous operator
    // tokens, visiting each exactly once.
    'walk: loop {
        let node = cursor.node();
        if is_decision_node(lang, node) {
            let r = node.byte_range();
            #[allow(clippy::cast_possible_truncation)]
            let range = (r.start as u32, r.end as u32);
            if let Some(i) = innermost_containing_decl(range, decls) {
                counts[i] += 1;
            }
        }
        if cursor.goto_first_child() {
            continue 'walk;
        }
        loop {
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
    for (i, decl) in decls.iter_mut().enumerate() {
        decl.complexity = if is_function_like(&decl.kind) {
            counts[i] + 1
        } else {
            0
        };
    }
}

/// Only function-like decls carry a `McCabe` value; everything else is `0`
/// (tier-12 D4). A non-component arrow bound to a `Variable` reads `0` — the
/// known limitation recorded in ADR-0020.
fn is_function_like(kind: &DeclKind) -> bool {
    matches!(
        kind,
        DeclKind::Function | DeclKind::Method | DeclKind::Component
    )
}

/// Whether `node` is a `McCabe` decision point for `lang`.
fn is_decision_node(lang: Lang, node: Node<'_>) -> bool {
    let kind = node.kind();
    let control = match lang {
        Lang::Rust => matches!(
            kind,
            "if_expression"
                | "while_expression"
                | "loop_expression"
                | "for_expression"
                | "match_arm"
                | "try_expression"
        ),
        Lang::Python => matches!(
            kind,
            "if_statement"
                | "elif_clause"
                | "while_statement"
                | "for_statement"
                | "except_clause"
                | "conditional_expression"
                | "case_clause"
                | "if_clause"
        ),
        Lang::JavaScript | Lang::TypeScript | Lang::Tsx => matches!(
            kind,
            "if_statement"
                | "while_statement"
                | "do_statement"
                | "for_statement"
                | "for_in_statement"
                | "switch_case"
                | "ternary_expression"
                | "catch_clause"
        ),
        Lang::Go => matches!(
            kind,
            "if_statement"
                | "for_statement"
                | "expression_case"
                | "type_case"
                | "communication_case"
        ),
        Lang::Java => matches!(
            kind,
            "if_statement"
                | "while_statement"
                | "do_statement"
                | "for_statement"
                | "enhanced_for_statement"
                | "switch_label"
                | "switch_rule"
                | "ternary_expression"
                | "catch_clause"
        ),
        Lang::Kotlin => matches!(
            kind,
            "if_expression"
                | "while_statement"
                | "do_while_statement"
                | "for_statement"
                | "when_entry"
                | "catch_block"
        ),
        Lang::CSharp => matches!(
            kind,
            "if_statement"
                | "while_statement"
                | "do_statement"
                | "for_statement"
                | "foreach_statement"
                | "switch_section"
                | "switch_expression_arm"
                | "conditional_expression"
                | "catch_clause"
        ),
        Lang::C => matches!(
            kind,
            "if_statement"
                | "while_statement"
                | "do_statement"
                | "for_statement"
                | "case_statement"
                | "conditional_expression"
        ),
        Lang::Cpp => matches!(
            kind,
            "if_statement"
                | "while_statement"
                | "do_statement"
                | "for_statement"
                | "for_range_loop"
                | "case_statement"
                | "conditional_expression"
                | "catch_clause"
        ),
        _ => false,
    };
    control || is_boolean_operator(lang, node)
}

/// Whether `node` is a short-circuit `&&` / `||` (strict `McCabe`). Python uses
/// a dedicated `boolean_operator` node; every other grammar exposes the
/// operator as the `operator` field of a `binary_expression` (so a bare `&&`
/// token in, e.g., a Rust reference pattern is never miscounted).
fn is_boolean_operator(lang: Lang, node: Node<'_>) -> bool {
    match lang {
        Lang::Python => node.kind() == "boolean_operator",
        _ => {
            node.kind() == "binary_expression"
                && node
                    .child_by_field_name("operator")
                    .is_some_and(|op| matches!(op.kind(), "&&" | "||"))
        }
    }
}
