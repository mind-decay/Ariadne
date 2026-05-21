//! Tier-04 step 8: Svelte SFC incremental-parse equivalence proptest.
//!
//! 100 random `InputEdit` sequences are applied to a `.svelte` fixture. After
//! every edit an incremental `parse_file` (the prior [`ParsedFile`] plus the
//! edit delta) must produce a `ParsedFile` equal to a from-scratch reparse —
//! every layer's root S-expression equal, host and injected alike. A
//! divergence fails loud: the injection engine must not break incremental
//! correctness on a second host grammar (plan.md R-Inject; tier-04 exit
//! criterion 5).
//!
//! Mirrors `tests/incremental_vue.rs` (tier-03); the only changes are the
//! host [`Lang`] and the fixture.

use ariadne_core::Lang;
use ariadne_parser::{ParserRegistry, parse_file};
use proptest::collection::vec;
use proptest::prelude::*;
use tree_sitter::{InputEdit, Point};

const SAMPLE: &str = include_str!("../fixtures/svelte/sample.svelte");

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
    fn incremental_svelte_matches_full_reparse(specs in arb_edit_seq()) {
        let registry = ParserRegistry::new();
        let mut content = SAMPLE.as_bytes().to_vec();
        let mut parsed = parse_file(Lang::Svelte, &registry, &content, None, &[])
            .expect("cold parse");
        for spec in specs {
            let (new_content, input_edit) = apply_edit(&content, &spec);
            let inc = parse_file(Lang::Svelte, &registry, &new_content, Some(&parsed), &[input_edit])
                .expect("incremental parse");
            let full = parse_file(Lang::Svelte, &registry, &new_content, None, &[])
                .expect("full parse");

            prop_assert_eq!(
                inc.host.1.root_node().to_sexp(),
                full.host.1.root_node().to_sexp(),
                "host layer diverged from full reparse",
            );
            prop_assert_eq!(
                inc.injected.len(),
                full.injected.len(),
                "injected-layer count diverged from full reparse",
            );
            for (a, b) in inc.injected.iter().zip(full.injected.iter()) {
                prop_assert_eq!(a.0, b.0, "injected-layer lang diverged");
                prop_assert_eq!(
                    a.1.root_node().to_sexp(),
                    b.1.root_node().to_sexp(),
                    "injected layer diverged from full reparse",
                );
            }
            content = new_content;
            parsed = inc;
        }
    }
}
