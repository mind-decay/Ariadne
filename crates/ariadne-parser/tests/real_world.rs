//! Tier-03 manual verification: parse a real-world file (this crate's own
//! `cache.rs`) and assert facts cover the top-level items.

use ariadne_core::Lang;
use ariadne_parser::{DeclKind, ParserRegistry, extract_syntactic_facts, parse_file};

#[test]
fn parses_self_rust_source_and_yields_decls() {
    let source = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/adapters/treesitter/cache.rs"
    ))
    .expect("cache.rs readable");
    let registry = ParserRegistry::new();
    let parsed = parse_file(Lang::Rust, &registry, &source, None, &[]).unwrap();
    assert!(!parsed.host.1.root_node().has_error());
    let facts = extract_syntactic_facts(&parsed, &source).unwrap();
    let names: Vec<&str> = facts.decls.iter().map(|d| d.name.as_str()).collect();
    for expected in [
        "codec",
        "ParseCache",
        "capture",
        "encode",
        "decode",
        "rehydrate",
    ] {
        assert!(
            names.contains(&expected),
            "missing decl {expected}; got {names:?}",
        );
    }
    assert!(facts.decls.iter().any(|d| d.kind == DeclKind::Struct));
    assert!(facts.decls.iter().any(|d| d.kind == DeclKind::Function));
}
