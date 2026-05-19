//! Tier-03 step 9: proptest random edit sequence vs full reparse.
//!
//! For each randomly generated edit applied to a JS fixture, an incremental
//! parse (with the prior tree + the [`InputEdit`] delta) must produce a
//! tree whose S-expression matches a from-scratch parse of the same
//! content [src: .claude/plans/ariadne-core/tier-03-parser.md exit criterion 5].

use ariadne_core::Lang;
use ariadne_parser::{ParserRegistry, TreeSitterParser};
use proptest::collection::vec;
use proptest::prelude::*;
use tree_sitter::{InputEdit, Point};

const INITIAL: &str =
    "function alpha(x, y) {\n  const z = x + y;\n  return z;\n}\n\nconst k = alpha(1, 2);\n";

#[derive(Debug, Clone)]
struct EditSpec {
    pos: usize,
    old_len: usize,
    insert: String,
}

fn arb_insert() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z_ ]{0,8}").unwrap()
}

fn arb_edit_seq() -> impl Strategy<Value = Vec<EditSpec>> {
    vec(
        (0u16..1024, 0u8..4, arb_insert()).prop_map(|(pos, old_len, insert)| EditSpec {
            pos: pos as usize,
            old_len: old_len as usize,
            insert,
        }),
        1..6,
    )
}

fn point_for(content: &[u8], byte: usize) -> Point {
    let prefix = &content[..byte.min(content.len())];
    let line_start = prefix
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |i| i + 1);
    let row = prefix
        .iter()
        .fold(0usize, |a, b| a + usize::from(*b == b'\n'));
    let column = byte.min(content.len()) - line_start;
    Point { row, column }
}

fn apply_edit(content: &[u8], spec: &EditSpec) -> (Vec<u8>, InputEdit) {
    let len = content.len();
    let start = spec.pos.min(len);
    let mut end = start + spec.old_len;
    if end > len {
        end = len;
    }
    let insert = spec.insert.as_bytes();
    let mut new_content = Vec::with_capacity(len - (end - start) + insert.len());
    new_content.extend_from_slice(&content[..start]);
    new_content.extend_from_slice(insert);
    new_content.extend_from_slice(&content[end..]);

    let start_position = point_for(content, start);
    let old_end_position = point_for(content, end);
    let new_end = start + insert.len();
    let new_end_position = point_for(&new_content, new_end);

    let edit = InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: new_end,
        start_position,
        old_end_position,
        new_end_position,
    };
    (new_content, edit)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn incremental_matches_full_reparse(specs in arb_edit_seq()) {
        let registry = ParserRegistry::new();
        let mut parser = TreeSitterParser::for_lang(Lang::JavaScript, &registry).unwrap();
        let mut content = INITIAL.as_bytes().to_vec();
        let mut tree = parser.parse_file(&content, None, &[]).unwrap();
        for spec in specs {
            let (new_content, input_edit) = apply_edit(&content, &spec);
            let inc_tree = parser
                .parse_file(&new_content, Some(&tree), &[input_edit])
                .expect("incremental parse");
            let mut full_parser = TreeSitterParser::for_lang(Lang::JavaScript, &registry).unwrap();
            let full_tree = full_parser
                .parse_file(&new_content, None, &[])
                .expect("full parse");
            prop_assert_eq!(
                inc_tree.root_node().to_sexp(),
                full_tree.root_node().to_sexp(),
            );
            content = new_content;
            tree = inc_tree;
        }
    }
}
