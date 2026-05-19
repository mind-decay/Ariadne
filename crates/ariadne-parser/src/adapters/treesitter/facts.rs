//! Query-driven syntactic-fact extraction.
//!
//! For each v1 language, [`extract_syntactic_facts`] runs the per-lang
//! `.scm` query bundled at compile time against a parsed tree and returns
//! a typed [`SyntacticFacts`] record. The query schema is documented in
//! the query files themselves (`queries/<lang>.scm`).
//!
//! Capture-name convention:
//! - `@def.<kind>` on the declaration node; matched with `@name` on the
//!   declared identifier. Kind tag = the suffix after the `.`.
//! - `@import` on the whole statement; `@import.path` on the module-path
//!   node.
//! - `@call.callee` on the callee identifier of a call/invocation.
//!
//! tree-sitter API contract: iteration over `QueryMatches` requires
//! `StreamingIterator` in scope.
//! (src: <https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html>)

use ariadne_core::Lang;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

use super::Tree;
use crate::errors::ParserError;

/// Declaration kind tag — kept loose this tier; tier-05 canonicalizes from
/// SCIP. The string is the literal suffix found in `@def.<kind>` captures.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeclKind {
    /// `@def.function`
    Function,
    /// `@def.method`
    Method,
    /// `@def.class`
    Class,
    /// `@def.struct`
    Struct,
    /// `@def.enum`
    Enum,
    /// `@def.interface`
    Interface,
    /// `@def.trait`
    Trait,
    /// `@def.type`
    TypeAlias,
    /// `@def.record`
    Record,
    /// `@def.object`
    Object,
    /// `@def.module`
    Module,
    /// `@def.variable`
    Variable,
    /// Any other `@def.<tag>` suffix.
    Other(String),
}

impl DeclKind {
    fn from_tag(tag: &str) -> Self {
        match tag {
            "function" => Self::Function,
            "method" => Self::Method,
            "class" => Self::Class,
            "struct" => Self::Struct,
            "enum" => Self::Enum,
            "interface" => Self::Interface,
            "trait" => Self::Trait,
            "type" => Self::TypeAlias,
            "record" => Self::Record,
            "object" => Self::Object,
            "module" => Self::Module,
            "variable" => Self::Variable,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// A single declaration captured from the syntax tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Decl {
    /// Declaration kind.
    pub kind: DeclKind,
    /// Identifier text as it appeared in source.
    pub name: String,
    /// Byte range of the declaration's name node.
    pub name_byte_range: (u32, u32),
    /// Byte range of the whole declaration node.
    pub def_byte_range: (u32, u32),
}

/// An import statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
    /// Raw module path text (unparsed; per-lang quoting preserved).
    pub path: String,
    /// Byte range of the path node.
    pub byte_range: (u32, u32),
}

/// A call/invocation site.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallSite {
    /// Callee identifier text.
    pub callee: String,
    /// Byte range of the callee identifier.
    pub byte_range: (u32, u32),
}

/// Aggregate output of [`extract_syntactic_facts`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct SyntacticFacts {
    /// Declarations in source order.
    pub decls: Vec<Decl>,
    /// Imports in source order.
    pub imports: Vec<Import>,
    /// Call sites in source order.
    pub calls: Vec<CallSite>,
}

const QUERY_TYPESCRIPT: &str = include_str!("queries/typescript.scm");
const QUERY_JAVASCRIPT: &str = include_str!("queries/javascript.scm");
const QUERY_PYTHON: &str = include_str!("queries/python.scm");
const QUERY_RUST: &str = include_str!("queries/rust.scm");
const QUERY_GO: &str = include_str!("queries/go.scm");
const QUERY_JAVA: &str = include_str!("queries/java.scm");
const QUERY_KOTLIN: &str = include_str!("queries/kotlin.scm");
const QUERY_CSHARP: &str = include_str!("queries/csharp.scm");

fn query_source(lang: Lang) -> Option<&'static str> {
    Some(match lang {
        Lang::TypeScript => QUERY_TYPESCRIPT,
        Lang::JavaScript => QUERY_JAVASCRIPT,
        Lang::Python => QUERY_PYTHON,
        Lang::Rust => QUERY_RUST,
        Lang::Go => QUERY_GO,
        Lang::Java => QUERY_JAVA,
        Lang::Kotlin => QUERY_KOTLIN,
        Lang::CSharp => QUERY_CSHARP,
        _ => return None,
    })
}

/// Run the per-lang fact query over `tree` and collect [`SyntacticFacts`].
///
/// # Errors
/// [`ParserError::UnsupportedLang`] when no query is bundled for `lang`;
/// [`ParserError::QueryCompile`] when the query source fails to compile
/// against the lang's grammar (indicates a node-type drift in the grammar
/// crate).
pub fn extract_syntactic_facts(
    tree: &Tree,
    lang: Lang,
    source: &[u8],
) -> Result<SyntacticFacts, ParserError> {
    let query_src = query_source(lang).ok_or(ParserError::UnsupportedLang(lang))?;
    let language = tree.language();
    let query = Query::new(&language, query_src)
        .map_err(|source| ParserError::QueryCompile { lang, source })?;

    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source);
    let names = query.capture_names();
    let mut facts = SyntacticFacts::default();

    while let Some(m) = matches.next() {
        let mut def_node: Option<(Node<'_>, DeclKind)> = None;
        let mut name_node: Option<Node<'_>> = None;
        let mut import_path_node: Option<Node<'_>> = None;
        let mut call_callee_node: Option<Node<'_>> = None;

        for capture in m.captures {
            let name = names[capture.index as usize];
            if let Some(rest) = name.strip_prefix("def.") {
                def_node = Some((capture.node, DeclKind::from_tag(rest)));
            } else if name == "name" {
                name_node = Some(capture.node);
            } else if name == "import.path" {
                import_path_node = Some(capture.node);
            } else if name == "call.callee" {
                call_callee_node = Some(capture.node);
            }
        }

        if let (Some((def, kind)), Some(name)) = (def_node, name_node) {
            facts.decls.push(Decl {
                kind,
                name: text_of(name, source),
                name_byte_range: byte_range(name),
                def_byte_range: byte_range(def),
            });
        } else if let Some(path) = import_path_node {
            facts.imports.push(Import {
                path: text_of(path, source),
                byte_range: byte_range(path),
            });
        } else if let Some(callee) = call_callee_node {
            facts.calls.push(CallSite {
                callee: text_of(callee, source),
                byte_range: byte_range(callee),
            });
        }
    }

    facts.decls.sort_by_key(|d| d.def_byte_range.0);
    facts.imports.sort_by_key(|i| i.byte_range.0);
    facts.calls.sort_by_key(|c| c.byte_range.0);
    Ok(facts)
}

fn byte_range(node: Node<'_>) -> (u32, u32) {
    let r = node.byte_range();
    #[allow(clippy::cast_possible_truncation)]
    (r.start as u32, r.end as u32)
}

fn text_of(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}
