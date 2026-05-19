use ariadne_core::Parser;
use ariadne_parser::TreeSitterParser;

#[test]
fn tree_sitter_parser_implements_parser_port() {
    fn assert_parser<T: Parser>() {}
    assert_parser::<TreeSitterParser>();
}
