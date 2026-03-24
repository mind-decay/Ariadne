//! Integration tests for Phase 4 — Symbol Graph (D-077, D-081).
//!
//! Tests symbol extraction for TypeScript/JS, Rust, and Go using existing fixtures,
//! plus unit-level tests for backward compatibility, warnings, and determinism.

use ariadne_graph::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
use ariadne_graph::parser::SymbolExtractor;

// ──────────────────────────────────────────────────────────────────────────────
// Helper: parse source with a tree-sitter language and extract symbols
// ──────────────────────────────────────────────────────────────────────────────

fn parse_and_extract(
    source: &str,
    lang: tree_sitter::Language,
    extractor: &dyn SymbolExtractor,
) -> Vec<SymbolDef> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang).unwrap();
    let tree = parser.parse(source.as_bytes(), None).unwrap();
    let mut symbols = extractor.extract_symbols(&tree, source.as_bytes());
    symbols.sort();
    symbols
}

// ──────────────────────────────────────────────────────────────────────────────
// TypeScript / JavaScript symbol extraction
// ──────────────────────────────────────────────────────────────────────────────

mod typescript {
    use super::*;

    fn ts_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT)
    }

    fn ts_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::typescript_symbol_extractor())
    }

    #[test]
    fn exported_functions() {
        let source = r#"
export function formatDate(date: Date): string {
  return date.toISOString().split('T')[0];
}

export function formatName(first: string, last: string): string {
  return `${first} ${last}`.trim();
}
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());
        assert_eq!(symbols.len(), 2);

        let format_date = symbols.iter().find(|s| s.name == "formatDate").unwrap();
        assert_eq!(format_date.kind, SymbolKind::Function);
        assert_eq!(format_date.visibility, Visibility::Public);
        assert!(format_date.signature.is_some());

        let format_name = symbols.iter().find(|s| s.name == "formatName").unwrap();
        assert_eq!(format_name.kind, SymbolKind::Function);
        assert_eq!(format_name.visibility, Visibility::Public);
    }

    #[test]
    fn interface_and_function() {
        let source = r#"
export interface LoginParams {
  username: string;
  password: string;
}

export function login(params: LoginParams): boolean {
  return true;
}
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let interface = symbols.iter().find(|s| s.name == "LoginParams").unwrap();
        assert_eq!(interface.kind, SymbolKind::Interface);
        assert_eq!(interface.visibility, Visibility::Public);

        let func = symbols.iter().find(|s| s.name == "login").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
    }

    #[test]
    fn class_with_methods() {
        let source = r#"
export class UserService {
  authenticate(username: string): boolean {
    return true;
  }
  getName(): string {
    return "test";
  }
}
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.visibility, Visibility::Public);

        let methods: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Method).collect();
        assert_eq!(methods.len(), 2);
        for m in &methods {
            assert_eq!(m.parent.as_deref(), Some("UserService"));
        }
    }

    #[test]
    fn named_arrow_function() {
        let source = r#"
export const greet = (name: string) => `Hello ${name}`;
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let func = symbols.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
        assert_eq!(func.visibility, Visibility::Public);
    }

    #[test]
    fn upper_case_const() {
        let source = r#"
export const MAX_RETRIES = 3;
const internal_value = 42;
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let max_retries = symbols.iter().find(|s| s.name == "MAX_RETRIES").unwrap();
        assert_eq!(max_retries.kind, SymbolKind::Const);
        assert_eq!(max_retries.visibility, Visibility::Public);

        let internal = symbols.iter().find(|s| s.name == "internal_value").unwrap();
        assert_eq!(internal.kind, SymbolKind::Variable);
        assert_eq!(internal.visibility, Visibility::Private);
    }

    #[test]
    fn type_alias() {
        let source = r#"
export type UserId = string;
type InternalId = number;
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let user_id = symbols.iter().find(|s| s.name == "UserId").unwrap();
        assert_eq!(user_id.kind, SymbolKind::Type);
        assert_eq!(user_id.visibility, Visibility::Public);

        let internal = symbols.iter().find(|s| s.name == "InternalId").unwrap();
        assert_eq!(internal.kind, SymbolKind::Type);
        assert_eq!(internal.visibility, Visibility::Private);
    }

    #[test]
    fn enum_declaration() {
        let source = r#"
export enum Color { Red, Green, Blue }
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        let color = symbols.iter().find(|s| s.name == "Color").unwrap();
        assert_eq!(color.kind, SymbolKind::Enum);
        assert_eq!(color.visibility, Visibility::Public);
    }

    #[test]
    fn private_items_detected() {
        let source = r#"
function helper() {}
const localVar = 1;
"#;
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());

        for sym in &symbols {
            assert_eq!(sym.visibility, Visibility::Private);
        }
    }

    #[test]
    fn empty_source() {
        let extractor = ts_extractor();
        let symbols = parse_and_extract("", ts_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }

    #[test]
    fn line_span_correct() {
        let source = "export function foo() {\n  return 1;\n}\n";
        let extractor = ts_extractor();
        let symbols = parse_and_extract(source, ts_lang(), extractor.as_ref());
        let foo = symbols.iter().find(|s| s.name == "foo").unwrap();
        assert_eq!(foo.span.start, 1);
        assert!(foo.span.end >= 3);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Rust symbol extraction
// ──────────────────────────────────────────────────────────────────────────────

mod rust_lang {
    use super::*;

    fn rust_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_rust::LANGUAGE)
    }

    fn rust_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::rust_symbol_extractor())
    }

    #[test]
    fn pub_functions_and_structs() {
        let source = r#"
pub struct LoginParams {
    pub username: String,
    pub password: String,
}

pub fn login(params: &LoginParams) -> bool {
    true
}
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let s = symbols.iter().find(|s| s.name == "LoginParams").unwrap();
        assert_eq!(s.kind, SymbolKind::Struct);
        assert_eq!(s.visibility, Visibility::Public);

        let f = symbols.iter().find(|s| s.name == "login").unwrap();
        assert_eq!(f.kind, SymbolKind::Function);
        assert_eq!(f.visibility, Visibility::Public);
    }

    #[test]
    fn impl_methods() {
        let source = r#"
pub struct Foo;

impl Foo {
    pub fn bar(&self) {}
    fn baz(&self) {}
}
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let bar = symbols.iter().find(|s| s.name == "bar").unwrap();
        assert_eq!(bar.kind, SymbolKind::Method);
        assert_eq!(bar.parent.as_deref(), Some("Foo"));
        assert_eq!(bar.visibility, Visibility::Public);

        let baz = symbols.iter().find(|s| s.name == "baz").unwrap();
        assert_eq!(baz.kind, SymbolKind::Method);
        assert_eq!(baz.visibility, Visibility::Private);
    }

    #[test]
    fn trait_item() {
        let source = r#"
pub trait Authenticator {
    fn authenticate(&self) -> bool;
}
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let t = symbols.iter().find(|s| s.name == "Authenticator").unwrap();
        assert_eq!(t.kind, SymbolKind::Trait);
        assert_eq!(t.visibility, Visibility::Public);
    }

    #[test]
    fn enum_item() {
        let source = r#"
pub enum Color { Red, Green, Blue }
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let e = symbols.iter().find(|s| s.name == "Color").unwrap();
        assert_eq!(e.kind, SymbolKind::Enum);
        assert_eq!(e.visibility, Visibility::Public);
    }

    #[test]
    fn const_and_static() {
        let source = r#"
pub const MAX_SIZE: usize = 1024;
static COUNTER: u32 = 0;
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let max = symbols.iter().find(|s| s.name == "MAX_SIZE").unwrap();
        assert_eq!(max.kind, SymbolKind::Const);
        assert_eq!(max.visibility, Visibility::Public);

        let counter = symbols.iter().find(|s| s.name == "COUNTER").unwrap();
        assert_eq!(counter.kind, SymbolKind::Const);
        assert_eq!(counter.visibility, Visibility::Private);
    }

    #[test]
    fn pub_crate_is_internal() {
        let source = r#"
pub(crate) fn internal_fn() {}
pub(super) fn super_fn() {}
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        for sym in &symbols {
            assert_eq!(sym.visibility, Visibility::Internal);
        }
    }

    #[test]
    fn type_alias() {
        let source = r#"
pub type Result<T> = std::result::Result<T, Error>;
"#;
        let extractor = rust_extractor();
        let symbols = parse_and_extract(source, rust_lang(), extractor.as_ref());

        let t = symbols.iter().find(|s| s.name == "Result").unwrap();
        assert_eq!(t.kind, SymbolKind::Type);
        assert_eq!(t.visibility, Visibility::Public);
    }

    #[test]
    fn empty_source() {
        let extractor = rust_extractor();
        let symbols = parse_and_extract("", rust_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Go symbol extraction
// ──────────────────────────────────────────────────────────────────────────────

mod go {
    use super::*;

    fn go_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_go::LANGUAGE)
    }

    fn go_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::go_symbol_extractor())
    }

    #[test]
    fn function_and_struct() {
        let source = r#"
package handler

type Handler struct {
	svc interface{}
}

func New(svc interface{}) *Handler {
	return &Handler{svc: svc}
}
"#;
        let extractor = go_extractor();
        let symbols = parse_and_extract(source, go_lang(), extractor.as_ref());

        let handler = symbols.iter().find(|s| s.name == "Handler").unwrap();
        assert_eq!(handler.kind, SymbolKind::Struct);
        assert_eq!(handler.visibility, Visibility::Public);

        let new_fn = symbols.iter().find(|s| s.name == "New").unwrap();
        assert_eq!(new_fn.kind, SymbolKind::Function);
        assert_eq!(new_fn.visibility, Visibility::Public);
    }

    #[test]
    fn method_with_receiver() {
        let source = r#"
package handler

type Handler struct {}

func (h *Handler) HandleLogin(username, password string) bool {
	return true
}
"#;
        let extractor = go_extractor();
        let symbols = parse_and_extract(source, go_lang(), extractor.as_ref());

        let method = symbols.iter().find(|s| s.name == "HandleLogin").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.visibility, Visibility::Public);
        assert_eq!(method.parent.as_deref(), Some("Handler"));
    }

    #[test]
    fn interface_type() {
        let source = r#"
package main

type Logger interface {
	Log(msg string)
}
"#;
        let extractor = go_extractor();
        let symbols = parse_and_extract(source, go_lang(), extractor.as_ref());

        let iface = symbols.iter().find(|s| s.name == "Logger").unwrap();
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert_eq!(iface.visibility, Visibility::Public);
    }

    #[test]
    fn const_declaration() {
        let source = r#"
package main

const MaxRetries = 3
const maxInternal = 5
"#;
        let extractor = go_extractor();
        let symbols = parse_and_extract(source, go_lang(), extractor.as_ref());

        let max = symbols.iter().find(|s| s.name == "MaxRetries").unwrap();
        assert_eq!(max.kind, SymbolKind::Const);
        assert_eq!(max.visibility, Visibility::Public);

        let internal = symbols.iter().find(|s| s.name == "maxInternal").unwrap();
        assert_eq!(internal.kind, SymbolKind::Const);
        assert_eq!(internal.visibility, Visibility::Private);
    }

    #[test]
    fn private_function() {
        let source = r#"
package main

func privateHelper() {}
func PublicHelper() {}
"#;
        let extractor = go_extractor();
        let symbols = parse_and_extract(source, go_lang(), extractor.as_ref());

        let priv_fn = symbols.iter().find(|s| s.name == "privateHelper").unwrap();
        assert_eq!(priv_fn.visibility, Visibility::Private);

        let pub_fn = symbols.iter().find(|s| s.name == "PublicHelper").unwrap();
        assert_eq!(pub_fn.visibility, Visibility::Public);
    }

    #[test]
    fn empty_source() {
        let extractor = go_extractor();
        let symbols = parse_and_extract("package main", go_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Backward compatibility: deserialize graph.json without symbols field
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn backward_compat_graph_json_without_symbols() {
    let json = r#"{
        "version": 1,
        "project_root": ".",
        "node_count": 1,
        "edge_count": 0,
        "nodes": {
            "src/a.ts": {
                "type": "source",
                "layer": "util",
                "arch_depth": 0,
                "lines": 10,
                "hash": "abc123",
                "exports": ["foo"],
                "cluster": "src"
            }
        },
        "edges": []
    }"#;

    let output: ariadne_graph::serial::GraphOutput = serde_json::from_str(json).unwrap();
    let node = output.nodes.get("src/a.ts").unwrap();
    assert!(node.symbols.is_empty(), "symbols should default to empty vec");
}

// ──────────────────────────────────────────────────────────────────────────────
// Determinism: sort order is stable
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn symbol_sort_order_deterministic() {
    let mut symbols = [
        SymbolDef {
            name: "Zebra".to_string(),
            kind: SymbolKind::Class,
            visibility: Visibility::Public,
            span: LineSpan { start: 1, end: 5 },
            signature: None,
            parent: None,
        },
        SymbolDef {
            name: "Alpha".to_string(),
            kind: SymbolKind::Function,
            visibility: Visibility::Private,
            span: LineSpan { start: 10, end: 15 },
            signature: None,
            parent: None,
        },
        SymbolDef {
            name: "Alpha".to_string(),
            kind: SymbolKind::Const,
            visibility: Visibility::Public,
            span: LineSpan { start: 20, end: 20 },
            signature: None,
            parent: None,
        },
    ];
    symbols.sort();

    // Sorted by (name, kind, visibility, span, ...)
    assert_eq!(symbols[0].name, "Alpha");
    assert_eq!(symbols[1].name, "Alpha");
    assert_eq!(symbols[2].name, "Zebra");
}

// ──────────────────────────────────────────────────────────────────────────────
// Signature truncation (D-081)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn signature_truncated_to_200_chars() {
    let long_params = "a: string, ".repeat(25); // ~275 chars
    let source = format!("export function longFunc({}) {{}}", long_params);
    let extractor = ariadne_graph::parser::typescript_symbol_extractor();
    let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
    let symbols = parse_and_extract(&source, lang, &extractor);

    let func = symbols.iter().find(|s| s.name == "longFunc").unwrap();
    if let Some(ref sig) = func.signature {
        assert!(
            sig.len() <= 203,
            "signature should be truncated: len={}",
            sig.len()
        ); // 200 + "..."
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Non-ASCII signature truncation (C1 regression test)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn signature_truncation_non_ascii_does_not_panic() {
    // Build a long TypeScript function with non-ASCII characters that exceeds
    // 200 characters. Each CJK character is 3 bytes in UTF-8, so the old
    // byte-offset truncation would panic on a multi-byte char boundary.
    let unicode_params: String = (0..200).map(|_| '\u{4e16}').collect(); // 世 repeated 200 times
    let source = format!(
        "export function unicodeFunc(p: string /* {} */) {{}}",
        unicode_params
    );
    let extractor = ariadne_graph::parser::typescript_symbol_extractor();
    let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
    // This should NOT panic (previously it would due to byte-offset slicing)
    let symbols = parse_and_extract(&source, lang, &extractor);

    let func = symbols.iter().find(|s| s.name == "unicodeFunc").unwrap();
    assert!(func.signature.is_some(), "signature should be present");
    if let Some(ref sig) = func.signature {
        assert!(sig.ends_with("..."), "long signature should be truncated");
        // Verify the truncated string is valid UTF-8 (it is since it's a String)
        assert!(sig.chars().count() <= 203); // 200 chars + "..."
    }
}

#[test]
fn rust_signature_truncation_non_ascii_does_not_panic() {
    // Rust function with CJK characters in a comment that makes the line long
    let unicode_comment: String = (0..70).map(|_| '\u{4e16}').collect();
    let source = format!(
        "pub fn unicode_fn() -> bool {{ true }} // {}",
        unicode_comment
    );
    let extractor = ariadne_graph::parser::rust_symbol_extractor();
    let lang = tree_sitter::Language::from(tree_sitter_rust::LANGUAGE);
    let symbols = parse_and_extract(&source, lang, &extractor);

    let func = symbols.iter().find(|s| s.name == "unicode_fn").unwrap();
    assert!(func.signature.is_some());
}

#[test]
fn go_signature_truncation_non_ascii_does_not_panic() {
    let unicode_comment: String = (0..70).map(|_| '\u{4e16}').collect();
    let source = format!(
        "package main\n\nfunc UnicodeFunc() bool {{ return true }} // {}",
        unicode_comment
    );
    let extractor = ariadne_graph::parser::go_symbol_extractor();
    let lang = tree_sitter::Language::from(tree_sitter_go::LANGUAGE);
    let symbols = parse_and_extract(&source, lang, &extractor);

    let func = symbols.iter().find(|s| s.name == "UnicodeFunc").unwrap();
    assert!(func.signature.is_some());
}

// ──────────────────────────────────────────────────────────────────────────────
// TypeScript class member visibility (W1 fix)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn ts_class_member_visibility() {
    let source = r#"
export class MyService {
    public publicMethod(): void {}
    private privateMethod(): void {}
    protected protectedMethod(): void {}
    defaultMethod(): void {}
}
"#;
    let extractor = ariadne_graph::parser::typescript_symbol_extractor();
    let lang = tree_sitter::Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
    let symbols = parse_and_extract(source, lang, &extractor);

    let public_m = symbols.iter().find(|s| s.name == "publicMethod").unwrap();
    assert_eq!(public_m.visibility, Visibility::Public);

    let private_m = symbols.iter().find(|s| s.name == "privateMethod").unwrap();
    assert_eq!(private_m.visibility, Visibility::Private);

    let protected_m = symbols.iter().find(|s| s.name == "protectedMethod").unwrap();
    assert_eq!(protected_m.visibility, Visibility::Internal); // TS protected → Internal

    let default_m = symbols.iter().find(|s| s.name == "defaultMethod").unwrap();
    assert_eq!(default_m.visibility, Visibility::Public); // TS default is public
}

// ──────────────────────────────────────────────────────────────────────────────
// SymbolDef serialization round-trip
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn symbol_def_serde_round_trip() {
    let sym = SymbolDef {
        name: "foo".to_string(),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        span: LineSpan { start: 1, end: 10 },
        signature: Some("fn foo() -> bool".to_string()),
        parent: Some("MyStruct".to_string()),
    };

    let json = serde_json::to_string(&sym).unwrap();
    let deserialized: SymbolDef = serde_json::from_str(&json).unwrap();
    assert_eq!(sym, deserialized);
}

#[test]
fn symbol_def_serde_skips_none_fields() {
    let sym = SymbolDef {
        name: "bar".to_string(),
        kind: SymbolKind::Const,
        visibility: Visibility::Private,
        span: LineSpan { start: 5, end: 5 },
        signature: None,
        parent: None,
    };

    let json = serde_json::to_string(&sym).unwrap();
    assert!(!json.contains("signature"), "None signature should be skipped");
    assert!(!json.contains("parent"), "None parent should be skipped");
}

// ──────────────────────────────────────────────────────────────────────────────
// Python symbol extraction (Phase 4b)
// ──────────────────────────────────────────────────────────────────────────────

mod python {
    use super::*;

    fn py_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_python::LANGUAGE)
    }

    fn py_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::python_symbol_extractor())
    }

    #[test]
    fn function_and_class() {
        let source = r#"
def greet(name):
    return f"Hello {name}"

class UserService:
    def authenticate(self, username, password):
        return True
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        let func = symbols.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
        assert_eq!(func.visibility, Visibility::Public);

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.visibility, Visibility::Public);

        let method = symbols.iter().find(|s| s.name == "authenticate").unwrap();
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.parent.as_deref(), Some("UserService"));
    }

    #[test]
    fn private_symbols() {
        let source = r#"
def _helper():
    pass

def __internal():
    pass

class _Private:
    pass
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        for sym in &symbols {
            assert_eq!(sym.visibility, Visibility::Private,
                "symbol '{}' should be Private", sym.name);
        }
    }

    #[test]
    fn upper_case_constant() {
        let source = r#"
MAX_RETRIES = 3
DB_URL = "localhost"
normal_var = 42
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        let max = symbols.iter().find(|s| s.name == "MAX_RETRIES").unwrap();
        assert_eq!(max.kind, SymbolKind::Const);

        let db = symbols.iter().find(|s| s.name == "DB_URL").unwrap();
        assert_eq!(db.kind, SymbolKind::Const);

        // normal_var is not UPPER_CASE, so should not be extracted
        assert!(symbols.iter().find(|s| s.name == "normal_var").is_none());
    }

    #[test]
    fn decorated_function() {
        let source = r#"
@app.route("/login")
def login():
    return "ok"
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        let func = symbols.iter().find(|s| s.name == "login").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
        assert_eq!(func.visibility, Visibility::Public);
    }

    #[test]
    fn decorated_class() {
        let source = r#"
@dataclass
class Config:
    host: str
    port: int
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "Config").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
    }

    #[test]
    fn nested_class_methods() {
        let source = r#"
class Outer:
    def method_a(self):
        pass

    def method_b(self):
        pass
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());

        let methods: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Method).collect();
        assert_eq!(methods.len(), 2);
        for m in &methods {
            assert_eq!(m.parent.as_deref(), Some("Outer"));
        }
    }

    #[test]
    fn empty_source() {
        let extractor = py_extractor();
        let symbols = parse_and_extract("", py_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }

    #[test]
    fn signature_present() {
        let source = r#"
def complex_func(a, b, c=None):
    pass
"#;
        let extractor = py_extractor();
        let symbols = parse_and_extract(source, py_lang(), extractor.as_ref());
        let func = symbols.iter().find(|s| s.name == "complex_func").unwrap();
        assert!(func.signature.is_some());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// C# symbol extraction (Phase 4b)
// ──────────────────────────────────────────────────────────────────────────────

mod csharp {
    use super::*;

    fn cs_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_c_sharp::LANGUAGE)
    }

    fn cs_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::csharp_symbol_extractor())
    }

    #[test]
    fn class_and_method() {
        let source = r#"
namespace MyApp {
    public class UserService {
        public bool Authenticate(string username, string password) {
            return true;
        }
        private void Helper() {}
    }
}
"#;
        let extractor = cs_extractor();
        let symbols = parse_and_extract(source, cs_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.visibility, Visibility::Public);

        let auth = symbols.iter().find(|s| s.name == "Authenticate").unwrap();
        assert_eq!(auth.kind, SymbolKind::Method);
        assert_eq!(auth.visibility, Visibility::Public);
        assert_eq!(auth.parent.as_deref(), Some("UserService"));

        let helper = symbols.iter().find(|s| s.name == "Helper").unwrap();
        assert_eq!(helper.kind, SymbolKind::Method);
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn interface_and_struct() {
        let source = r#"
public interface IRepository {
    void Save();
}

public struct Point {
    public int X;
    public int Y;
}
"#;
        let extractor = cs_extractor();
        let symbols = parse_and_extract(source, cs_lang(), extractor.as_ref());

        let iface = symbols.iter().find(|s| s.name == "IRepository").unwrap();
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert_eq!(iface.visibility, Visibility::Public);

        let st = symbols.iter().find(|s| s.name == "Point").unwrap();
        assert_eq!(st.kind, SymbolKind::Struct);
        assert_eq!(st.visibility, Visibility::Public);
    }

    #[test]
    fn const_field() {
        let source = r#"
public class Config {
    public const int MaxRetries = 3;
    private const string ApiUrl = "http://example.com";
}
"#;
        let extractor = cs_extractor();
        let symbols = parse_and_extract(source, cs_lang(), extractor.as_ref());

        let consts: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Const).collect();
        assert_eq!(consts.len(), 2);

        let max = consts.iter().find(|s| s.name == "MaxRetries").unwrap();
        assert_eq!(max.visibility, Visibility::Public);

        let url = consts.iter().find(|s| s.name == "ApiUrl").unwrap();
        assert_eq!(url.visibility, Visibility::Private);
    }

    #[test]
    fn internal_visibility() {
        let source = r#"
internal class InternalService {
    void PackageMethod() {}
}
"#;
        let extractor = cs_extractor();
        let symbols = parse_and_extract(source, cs_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "InternalService").unwrap();
        assert_eq!(class.visibility, Visibility::Internal);
    }

    #[test]
    fn empty_source() {
        let extractor = cs_extractor();
        let symbols = parse_and_extract("", cs_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Java symbol extraction (Phase 4b)
// ──────────────────────────────────────────────────────────────────────────────

mod java {
    use super::*;

    fn java_lang() -> tree_sitter::Language {
        tree_sitter::Language::from(tree_sitter_java::LANGUAGE)
    }

    fn java_extractor() -> Box<dyn SymbolExtractor> {
        Box::new(ariadne_graph::parser::java_symbol_extractor())
    }

    #[test]
    fn class_and_method() {
        let source = r#"
public class UserService {
    public boolean authenticate(String username, String password) {
        return true;
    }
    private void helper() {}
}
"#;
        let extractor = java_extractor();
        let symbols = parse_and_extract(source, java_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "UserService").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
        assert_eq!(class.visibility, Visibility::Public);

        let auth = symbols.iter().find(|s| s.name == "authenticate").unwrap();
        assert_eq!(auth.kind, SymbolKind::Method);
        assert_eq!(auth.visibility, Visibility::Public);
        assert_eq!(auth.parent.as_deref(), Some("UserService"));

        let helper = symbols.iter().find(|s| s.name == "helper").unwrap();
        assert_eq!(helper.kind, SymbolKind::Method);
        assert_eq!(helper.visibility, Visibility::Private);
    }

    #[test]
    fn interface_declaration() {
        let source = r#"
public interface Repository {
    void save();
    void delete(int id);
}
"#;
        let extractor = java_extractor();
        let symbols = parse_and_extract(source, java_lang(), extractor.as_ref());

        let iface = symbols.iter().find(|s| s.name == "Repository").unwrap();
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert_eq!(iface.visibility, Visibility::Public);
    }

    #[test]
    fn static_final_const() {
        let source = r#"
public class Config {
    public static final int MAX_RETRIES = 3;
    private static final String API_URL = "http://example.com";
    public int normalField = 0;
}
"#;
        let extractor = java_extractor();
        let symbols = parse_and_extract(source, java_lang(), extractor.as_ref());

        let consts: Vec<_> = symbols.iter().filter(|s| s.kind == SymbolKind::Const).collect();
        assert_eq!(consts.len(), 2);

        let max = consts.iter().find(|s| s.name == "MAX_RETRIES").unwrap();
        assert_eq!(max.visibility, Visibility::Public);

        // normalField should not appear (not static final)
        assert!(symbols.iter().find(|s| s.name == "normalField").is_none());
    }

    #[test]
    fn package_private_is_internal() {
        let source = r#"
class PackageClass {
    void packageMethod() {}
}
"#;
        let extractor = java_extractor();
        let symbols = parse_and_extract(source, java_lang(), extractor.as_ref());

        let class = symbols.iter().find(|s| s.name == "PackageClass").unwrap();
        assert_eq!(class.visibility, Visibility::Internal);
    }

    #[test]
    fn enum_declaration() {
        let source = r#"
public enum Status {
    ACTIVE, INACTIVE, PENDING
}
"#;
        let extractor = java_extractor();
        let symbols = parse_and_extract(source, java_lang(), extractor.as_ref());

        let e = symbols.iter().find(|s| s.name == "Status").unwrap();
        assert_eq!(e.kind, SymbolKind::Enum);
        assert_eq!(e.visibility, Visibility::Public);
    }

    #[test]
    fn empty_source() {
        let extractor = java_extractor();
        let symbols = parse_and_extract("", java_lang(), extractor.as_ref());
        assert!(symbols.is_empty());
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// SymbolIndex unit tests (Phase 4b)
// ──────────────────────────────────────────────────────────────────────────────

mod symbol_index {
    use ariadne_graph::model::edge::{Edge, EdgeType};
    use ariadne_graph::model::node::{ArchLayer, FileType, Node};
    use ariadne_graph::model::symbol::{LineSpan, SymbolDef, SymbolKind, Visibility};
    use ariadne_graph::model::symbol_index::SymbolIndex;
    use ariadne_graph::model::types::{CanonicalPath, ClusterId, ContentHash, Symbol};
    use std::collections::BTreeMap;

    fn make_node(symbols: Vec<SymbolDef>) -> Node {
        Node {
            file_type: FileType::Source,
            layer: ArchLayer::Util,
            fsd_layer: None,
            arch_depth: 0,
            lines: 50,
            hash: ContentHash::new("abc123".to_string()),
            exports: vec![],
            cluster: ClusterId::new("src"),
            symbols,
        }
    }

    fn make_symbol(name: &str, kind: SymbolKind) -> SymbolDef {
        SymbolDef {
            name: name.to_string(),
            kind,
            visibility: Visibility::Public,
            span: LineSpan { start: 1, end: 10 },
            signature: None,
            parent: None,
        }
    }

    #[test]
    fn build_and_lookup_by_file() {
        let mut nodes = BTreeMap::new();
        let path = CanonicalPath::new("src/a.ts");
        nodes.insert(
            path.clone(),
            make_node(vec![
                make_symbol("Foo", SymbolKind::Class),
                make_symbol("bar", SymbolKind::Function),
            ]),
        );

        let index = SymbolIndex::build(&nodes, &[]);
        let symbols = index.symbols_for_file(&path).unwrap();
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn lookup_by_name() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            make_node(vec![make_symbol("Foo", SymbolKind::Class)]),
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            make_node(vec![make_symbol("Foo", SymbolKind::Function)]),
        );

        let index = SymbolIndex::build(&nodes, &[]);
        let results = index.search("Foo", None);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn search_case_insensitive() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            make_node(vec![make_symbol("UserService", SymbolKind::Class)]),
        );

        let index = SymbolIndex::build(&nodes, &[]);

        let results = index.search("userservice", None);
        assert_eq!(results.len(), 1);

        let results = index.search("USERSERVICE", None);
        assert_eq!(results.len(), 1);

        let results = index.search("user", None);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_with_kind_filter() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            make_node(vec![
                make_symbol("process", SymbolKind::Function),
                make_symbol("process", SymbolKind::Class),
            ]),
        );

        let index = SymbolIndex::build(&nodes, &[]);

        let results = index.search("process", Some(SymbolKind::Function));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, SymbolKind::Function);
    }

    #[test]
    fn empty_index() {
        let nodes = BTreeMap::new();
        let index = SymbolIndex::build(&nodes, &[]);

        assert!(index.symbols_for_file(&CanonicalPath::new("src/a.ts")).is_none());
        assert!(index.search("anything", None).is_empty());
        assert!(index.usages_of(&CanonicalPath::new("src/a.ts"), "foo").is_none());
    }

    #[test]
    fn usages_from_edges() {
        let mut nodes = BTreeMap::new();
        let a = CanonicalPath::new("src/a.ts");
        let b = CanonicalPath::new("src/b.ts");

        nodes.insert(
            a.clone(),
            make_node(vec![make_symbol("helper", SymbolKind::Function)]),
        );
        nodes.insert(b.clone(), make_node(vec![]));

        let edges = vec![Edge {
            from: b.clone(),
            to: a.clone(),
            edge_type: EdgeType::Imports,
            symbols: vec![Symbol::new("helper".to_string())],
        }];

        let index = SymbolIndex::build(&nodes, &edges);

        let usages = index.usages_of(&a, "helper").unwrap();
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].file, b);
    }

    #[test]
    fn search_early_termination() {
        let mut nodes = BTreeMap::new();
        // Create 150 symbols all matching the query
        for i in 0..150 {
            let path = CanonicalPath::new(format!("src/file{}.ts", i));
            nodes.insert(
                path,
                make_node(vec![make_symbol(
                    &format!("item_{}", i),
                    SymbolKind::Function,
                )]),
            );
        }

        let index = SymbolIndex::build(&nodes, &[]);
        let results = index.search("item", None);
        assert_eq!(results.len(), 100, "should cap at 100 results");
    }

    #[test]
    fn duplicate_names_across_files() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            CanonicalPath::new("src/a.ts"),
            make_node(vec![make_symbol("Config", SymbolKind::Class)]),
        );
        nodes.insert(
            CanonicalPath::new("src/b.ts"),
            make_node(vec![make_symbol("Config", SymbolKind::Interface)]),
        );
        nodes.insert(
            CanonicalPath::new("src/c.ts"),
            make_node(vec![make_symbol("Config", SymbolKind::Type)]),
        );

        let index = SymbolIndex::build(&nodes, &[]);
        let results = index.search("Config", None);
        assert_eq!(results.len(), 3);
    }
}
