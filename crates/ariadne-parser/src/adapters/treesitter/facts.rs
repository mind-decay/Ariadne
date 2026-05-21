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
use tree_sitter::{Language, Node, Query, QueryCursor, StreamingIterator};

use super::registry::ParserRegistry;
use super::{ParsedFile, Tree};
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
    /// A JSX/TSX component — a `function` or `const`/`let` declaration whose
    /// body renders JSX (covers `function Foo()` and the idiomatic arrow form
    /// `const Foo = () => <jsx/>`). Assigned by a post-filter in
    /// [`FactExtractor::extract`], not a query tag, so the `"component"` arm of
    /// [`DeclKind::from_tag`] is reserved for any future query that captures
    /// `@def.component` directly.
    Component,
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
            "component" => Self::Component,
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

/// A JSX/TSX render site — one child-component element (`<Child/>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderSite {
    /// Rendered component's tag-name identifier text.
    pub component: String,
    /// Byte range of the tag-name identifier.
    pub byte_range: (u32, u32),
}

/// A hook / reactive-primitive call site (`useState(…)`, `createSignal(…)`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HookSite {
    /// Hook callee identifier text.
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
    /// JSX/TSX render sites in source order.
    pub renders: Vec<RenderSite>,
    /// Hook / reactive-primitive call sites in source order.
    pub hooks: Vec<HookSite>,
}

const QUERY_TYPESCRIPT: &str = include_str!("queries/typescript.scm");
const QUERY_TSX: &str = include_str!("queries/tsx.scm");
const QUERY_JAVASCRIPT: &str = include_str!("queries/javascript.scm");
const QUERY_PYTHON: &str = include_str!("queries/python.scm");
const QUERY_RUST: &str = include_str!("queries/rust.scm");
const QUERY_GO: &str = include_str!("queries/go.scm");
const QUERY_JAVA: &str = include_str!("queries/java.scm");
const QUERY_KOTLIN: &str = include_str!("queries/kotlin.scm");
const QUERY_CSHARP: &str = include_str!("queries/csharp.scm");
const QUERY_C: &str = include_str!("queries/c.scm");
const QUERY_CPP: &str = include_str!("queries/cpp.scm");
const QUERY_VUE: &str = include_str!("queries/vue.scm");
const QUERY_SVELTE: &str = include_str!("queries/svelte.scm");
const QUERY_ASTRO: &str = include_str!("queries/astro.scm");

fn query_source(lang: Lang) -> Option<&'static str> {
    Some(match lang {
        Lang::TypeScript => QUERY_TYPESCRIPT,
        Lang::Tsx => QUERY_TSX,
        Lang::JavaScript => QUERY_JAVASCRIPT,
        Lang::Python => QUERY_PYTHON,
        Lang::Rust => QUERY_RUST,
        Lang::Go => QUERY_GO,
        Lang::Java => QUERY_JAVA,
        Lang::Kotlin => QUERY_KOTLIN,
        Lang::CSharp => QUERY_CSHARP,
        Lang::C => QUERY_C,
        Lang::Cpp => QUERY_CPP,
        // Vue's host layer is the HTML grammar; `vue.scm` captures child-
        // component render sites. The `<script>` block's decls/calls/hooks
        // come from the injected JS/TS layer's own query.
        Lang::Vue => QUERY_VUE,
        // Svelte / Astro host layers capture child-component render sites the
        // same way; their `<script>` / frontmatter decls/calls/hooks come
        // from the injected JS/TS layer's own query (tier-04 step 6).
        Lang::Svelte => QUERY_SVELTE,
        Lang::Astro => QUERY_ASTRO,
        _ => return None,
    })
}

/// Compiled per-language fact query plus a reusable [`QueryCursor`].
///
/// `Query::new` compiles a bundled `.scm` source against a grammar — a
/// non-trivial cost the cold index previously paid once per file. A
/// `FactExtractor` compiles the query once and reuses one cursor across
/// every [`FactExtractor::extract`] call, so a parse worker that caches one
/// extractor per [`Lang`] never recompiles
/// [src: <https://docs.rs/tree-sitter/0.26.8/tree_sitter/struct.QueryCursor.html>].
pub struct FactExtractor {
    lang: Lang,
    query: Query,
    cursor: QueryCursor,
}

impl FactExtractor {
    /// Compile the fact query for `lang`, resolving its grammar from
    /// `registry`. The parse-worker entry point: build one per [`Lang`] and
    /// keep it for the worker's lifetime.
    ///
    /// # Errors
    /// [`ParserError::UnsupportedLang`] when no query or grammar is bundled
    /// for `lang`; [`ParserError::QueryCompile`] when the query source fails
    /// to compile against the grammar (indicates a node-type drift).
    pub fn for_lang(lang: Lang, registry: &ParserRegistry) -> Result<Self, ParserError> {
        let language = registry
            .language(lang)
            .ok_or(ParserError::UnsupportedLang(lang))?;
        Self::compile(lang, language)
    }

    /// Compile against an already-resolved `tree_sitter::Language`. Crate-
    /// internal so the grammar type never crosses the adapter boundary
    /// [src: docs/folder-layout.md rule 4].
    pub(crate) fn compile(lang: Lang, language: &Language) -> Result<Self, ParserError> {
        let query_src = query_source(lang).ok_or(ParserError::UnsupportedLang(lang))?;
        let query = Query::new(language, query_src)
            .map_err(|source| ParserError::QueryCompile { lang, source })?;
        Ok(Self {
            lang,
            query,
            cursor: QueryCursor::new(),
        })
    }

    /// [`Lang`] this extractor compiled its query for.
    #[must_use]
    pub fn lang(&self) -> Lang {
        self.lang
    }

    /// Run the compiled query over `tree` and collect [`SyntacticFacts`].
    /// Reuses the owned [`QueryCursor`], so no cursor scratch buffers are
    /// allocated per call.
    #[must_use]
    pub fn extract(&mut self, tree: &Tree, source: &[u8]) -> SyntacticFacts {
        let names = self.query.capture_names();
        let mut matches = self.cursor.matches(&self.query, tree.root_node(), source);
        let mut facts = SyntacticFacts::default();
        // Byte ranges of every JSX tag-name (host *and* component); drives the
        // component post-filter below.
        let mut jsx_spans: Vec<(u32, u32)> = Vec::new();

        while let Some(m) = matches.next() {
            let mut def_node: Option<(Node<'_>, DeclKind)> = None;
            let mut name_node: Option<Node<'_>> = None;
            let mut import_path_node: Option<Node<'_>> = None;
            let mut call_callee_node: Option<Node<'_>> = None;
            let mut render_node: Option<Node<'_>> = None;
            let mut hook_node: Option<Node<'_>> = None;

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
                } else if name == "render.component" {
                    render_node = Some(capture.node);
                } else if name == "hook.callee" {
                    hook_node = Some(capture.node);
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
            } else if let Some(tag) = render_node {
                let component = text_of(tag, source);
                let range = byte_range(tag);
                // Every JSX tag marks JSX presence for the component filter.
                jsx_spans.push(range);
                // Only capitalised tag names are child components; lower-case
                // names are host elements (`div`, `span`) — see queries/tsx.scm.
                if component.chars().next().is_some_and(char::is_uppercase) {
                    facts.renders.push(RenderSite {
                        component,
                        byte_range: range,
                    });
                }
            } else if let Some(callee) = hook_node {
                facts.hooks.push(HookSite {
                    callee: text_of(callee, source),
                    byte_range: byte_range(callee),
                });
            }
        }

        // A `function` or `const`/`let` declaration whose body encloses any
        // JSX is a component — this covers `function Foo()` and the idiomatic
        // arrow form `const Foo = () => <jsx/>` (captured as `@def.variable`).
        // A tree-sitter pattern cannot express "returns JSX at any depth", so
        // the classification is a post-filter here (see queries/tsx.scm).
        for decl in &mut facts.decls {
            if matches!(decl.kind, DeclKind::Function | DeclKind::Variable)
                && jsx_spans.iter().any(|&(start, end)| {
                    start >= decl.def_byte_range.0 && end <= decl.def_byte_range.1
                })
            {
                decl.kind = DeclKind::Component;
            }
        }

        facts.decls.sort_by_key(|d| d.def_byte_range.0);
        facts.imports.sort_by_key(|i| i.byte_range.0);
        facts.calls.sort_by_key(|c| c.byte_range.0);
        facts.renders.sort_by_key(|r| r.byte_range.0);
        facts.hooks.sort_by_key(|h| h.byte_range.0);
        facts
    }
}

impl std::fmt::Debug for FactExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactExtractor")
            .field("lang", &self.lang)
            .finish_non_exhaustive()
    }
}

/// Extract and merge [`SyntacticFacts`] from every layer of a [`ParsedFile`].
///
/// Runs the per-layer fact query — the host grammar's query plus each
/// injected layer's — and concatenates the results. Every span is already
/// file-absolute (injected layers parse over the full file via
/// `set_included_ranges`), so the merged facts share one coordinate space.
/// A single-grammar file degenerates to the host layer alone.
///
/// Builds a one-shot [`FactExtractor`] per layer — kept so callers with no
/// per-worker cache (the parser test suites) stay unchanged. The cold-index
/// pipeline caches a [`FactExtractor`] per [`Lang`] and calls
/// [`FactExtractor::extract`] directly instead.
///
/// # Errors
/// [`ParserError::UnsupportedLang`] when no query is bundled for a layer's
/// lang; [`ParserError::QueryCompile`] when a query fails to compile against
/// its grammar (indicates a node-type drift in the grammar crate).
pub fn extract_syntactic_facts(
    parsed: &ParsedFile,
    source: &[u8],
) -> Result<SyntacticFacts, ParserError> {
    let mut merged = SyntacticFacts::default();
    for (lang, tree) in std::iter::once(&parsed.host).chain(parsed.injected.iter()) {
        let mut extractor = FactExtractor::compile(*lang, &tree.language())?;
        let layer = extractor.extract(tree, source);
        merged.decls.extend(layer.decls);
        merged.imports.extend(layer.imports);
        merged.calls.extend(layer.calls);
        merged.renders.extend(layer.renders);
        merged.hooks.extend(layer.hooks);
    }
    // Spans are already file-absolute, so a plain byte-offset sort merges
    // the layers into source order; the exact-duplicate dedup guards against
    // a fact captured by two overlapping layer queries.
    merged.decls.sort_by_key(|d| d.def_byte_range.0);
    merged.imports.sort_by_key(|i| i.byte_range.0);
    merged.calls.sort_by_key(|c| c.byte_range.0);
    merged.renders.sort_by_key(|r| r.byte_range.0);
    merged.hooks.sort_by_key(|h| h.byte_range.0);
    merged.decls.dedup();
    merged.imports.dedup();
    merged.calls.dedup();
    merged.renders.dedup();
    merged.hooks.dedup();
    Ok(merged)
}

fn byte_range(node: Node<'_>) -> (u32, u32) {
    let r = node.byte_range();
    #[allow(clippy::cast_possible_truncation)]
    (r.start as u32, r.end as u32)
}

fn text_of(node: Node<'_>, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or_default().to_owned()
}
