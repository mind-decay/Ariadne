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

use ariadne_core::{Lang, Visibility};
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
    /// the private `DeclKind::from_tag` tag dispatch is reserved for any future
    /// query that captures `@def.component` directly.
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
    /// Visibility extracted from a `@visibility` capture (or the Go
    /// exported-by-leading-case rule). `Visibility::Unknown` when the
    /// grammar exposes no visibility node for this kind of decl.
    pub visibility: Visibility,
    /// Attribute / annotation / decorator identifiers attached to the
    /// declaration — e.g. `["test"]` for `#[test] fn …`. Empty when none
    /// captured.
    pub attributes: Vec<String>,
    /// `McCabe` cyclomatic complexity (`decisions + 1`) for function-like
    /// decls; `0` for every other kind. Built `0` and filled by
    /// [`attach_complexity`](super::complexity::attach_complexity) in
    /// [`FactExtractor::extract`] [src: tier-12 D1/D2/D4].
    pub complexity: u32,
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

impl SyntacticFacts {
    /// Append one parse layer's facts onto the running merge.
    ///
    /// Every layer of a [`ParsedFile`] parses over the full file bytes
    /// (injected layers via `set_included_ranges`), so all spans share one
    /// file-absolute coordinate space and a plain concatenation is sound.
    /// Call [`SyntacticFacts::finalize`] once after the last layer to restore
    /// source order.
    ///
    /// This is the single per-layer merge point: callers that fold layers
    /// themselves — [`extract_syntactic_facts`] and the cold-index parse
    /// worker — both route through here, so a new `SyntacticFacts` field is
    /// handled in this method and [`SyntacticFacts::finalize`], never at a
    /// call site.
    pub fn absorb_layer(&mut self, layer: SyntacticFacts) {
        self.decls.extend(layer.decls);
        self.imports.extend(layer.imports);
        self.calls.extend(layer.calls);
        self.renders.extend(layer.renders);
        self.hooks.extend(layer.hooks);
    }

    /// Sort every fact vector into file-absolute source order and drop exact
    /// duplicates. Spans are already file-absolute, so a plain byte-offset
    /// sort merges the absorbed layers; the dedup guards against a fact
    /// captured by two overlapping layer queries. Run once after the final
    /// [`SyntacticFacts::absorb_layer`].
    pub fn finalize(&mut self) {
        self.decls.sort_by_key(|d| d.def_byte_range.0);
        self.imports.sort_by_key(|i| i.byte_range.0);
        self.calls.sort_by_key(|c| c.byte_range.0);
        self.renders.sort_by_key(|r| r.byte_range.0);
        self.hooks.sort_by_key(|h| h.byte_range.0);
        self.decls.dedup();
        self.imports.dedup();
        self.calls.dedup();
        self.renders.dedup();
        self.hooks.dedup();
    }
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
        let lang = self.lang;
        // `@visibility` / `@attribute` captures live in their own top-level
        // query patterns (siblings of, or wrapping parents around, a decl),
        // so collect them globally and attach to decls in a post-pass below.
        let mut visibility_marks: Vec<VisibilityMark> = Vec::new();
        let mut attribute_marks: Vec<AttributeMark> = Vec::new();

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
                } else if name == "visibility" {
                    visibility_marks.push(VisibilityMark {
                        range: byte_range(capture.node),
                        text: text_of(capture.node, source),
                    });
                } else if name == "attribute" {
                    let attr = attribute_name(capture.node, source);
                    if !attr.is_empty() {
                        attribute_marks.push(AttributeMark {
                            range: byte_range(capture.node),
                            name: attr,
                        });
                    }
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
                    visibility: Visibility::Unknown,
                    attributes: Vec::new(),
                    complexity: 0,
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

        attach_visibility(lang, &mut facts.decls, &visibility_marks);
        attach_attributes(&mut facts.decls, &attribute_marks);
        // One CST walk attributes `McCabe` decisions to the innermost decl by
        // span, then sets `decisions + 1` on function-like decls (tier-12 D3).
        super::complexity::attach_complexity(lang, &mut facts.decls, tree);

        facts.decls.sort_by_key(|d| d.def_byte_range.0);
        facts.imports.sort_by_key(|i| i.byte_range.0);
        facts.calls.sort_by_key(|c| c.byte_range.0);
        facts.renders.sort_by_key(|r| r.byte_range.0);
        facts.hooks.sort_by_key(|h| h.byte_range.0);
        facts
    }
}

/// Visibility modifier captured by an `@visibility` query rule. The text is
/// the modifier's verbatim source — `pub`, `pub(crate)`, `public`, `export`,
/// the Python dunder name, …
#[derive(Debug, Clone)]
struct VisibilityMark {
    range: (u32, u32),
    text: String,
}

/// Attribute / annotation / decorator captured by an `@attribute` rule.
/// `name` is the bare identifier head extracted from the captured node.
#[derive(Debug, Clone)]
struct AttributeMark {
    range: (u32, u32),
    name: String,
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
        merged.absorb_layer(extractor.extract(tree, source));
    }
    merged.finalize();
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

/// Attach visibility marks onto decls + apply the Go fallback rule.
///
/// Each visibility node binds to exactly one decl:
/// 1. If the mark is contained in any decl, the *smallest* such decl wins
///    (innermost) — handles Rust `visibility_modifier` nested inside a
///    `function_item` even when an outer `mod_item` also contains it.
/// 2. Otherwise, if any decl is contained in the mark, the *largest* such
///    decl wins — handles TS `export_statement` wrapping a top-level
///    `function_declaration`; the export keyword's range covers the
///    function's range.
///
/// Go has no visibility keyword and no annotation system; the language spec
/// defines exported identifiers as those whose first character is an
/// upper-case Unicode letter
/// [src: <https://go.dev/ref/spec#Exported_identifiers>]. Decls that receive
/// no mark fall back to the case rule for Go and to `Visibility::Unknown`
/// otherwise.
fn attach_visibility(lang: Lang, decls: &mut [Decl], marks: &[VisibilityMark]) {
    let mut by_decl: Vec<Visibility> = vec![Visibility::Unknown; decls.len()];
    let mut received: Vec<bool> = vec![false; decls.len()];
    for mark in marks {
        let Some(target) = best_decl_for(mark.range, decls) else {
            continue;
        };
        let v = classify_visibility_text(lang, &mark.text);
        by_decl[target] = visibility_max(by_decl[target], v);
        received[target] = true;
    }
    for (i, decl) in decls.iter_mut().enumerate() {
        decl.visibility = if received[i] {
            by_decl[i]
        } else if lang == Lang::Go {
            if decl.name.chars().next().is_some_and(char::is_uppercase) {
                Visibility::Public
            } else {
                Visibility::Private
            }
        } else {
            Visibility::Unknown
        };
    }
}

/// Attach attribute marks to decls.
///
/// 1. If the mark is contained in a decl (Java `@Override` inside a
///    `modifiers` block), the innermost containing decl wins.
/// 2. Otherwise, the mark attaches to the next decl by start byte (Rust
///    `#[test]` preceding a `fn`, TS `@decorator` preceding a class
///    member).
fn attach_attributes(decls: &mut [Decl], marks: &[AttributeMark]) {
    let mut order: Vec<usize> = (0..decls.len()).collect();
    order.sort_by_key(|&i| decls[i].def_byte_range.0);
    for mark in marks {
        let target = innermost_containing_decl(mark.range, decls).or_else(|| {
            order
                .iter()
                .copied()
                .find(|&i| decls[i].def_byte_range.0 >= mark.range.1)
        });
        if let Some(idx) = target {
            decls[idx].attributes.push(mark.name.clone());
        }
    }
}

fn best_decl_for(mark_range: (u32, u32), decls: &[Decl]) -> Option<usize> {
    innermost_containing_decl(mark_range, decls)
        .or_else(|| largest_contained_decl(mark_range, decls))
}

pub(super) fn innermost_containing_decl(mark: (u32, u32), decls: &[Decl]) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_size: u64 = u64::MAX;
    for (i, decl) in decls.iter().enumerate() {
        if range_contains(decl.def_byte_range, mark) {
            let size = u64::from(decl.def_byte_range.1.saturating_sub(decl.def_byte_range.0));
            if size < best_size {
                best = Some(i);
                best_size = size;
            }
        }
    }
    best
}

fn largest_contained_decl(mark: (u32, u32), decls: &[Decl]) -> Option<usize> {
    let mut best: Option<usize> = None;
    let mut best_size: u64 = 0;
    for (i, decl) in decls.iter().enumerate() {
        if range_contains(mark, decl.def_byte_range) {
            let size = u64::from(decl.def_byte_range.1.saturating_sub(decl.def_byte_range.0));
            if best.is_none() || size > best_size {
                best = Some(i);
                best_size = size;
            }
        }
    }
    best
}

fn range_contains(outer: (u32, u32), inner: (u32, u32)) -> bool {
    outer.0 <= inner.0 && outer.1 >= inner.1
}

fn classify_visibility_text(lang: Lang, text: &str) -> Visibility {
    match lang {
        Lang::Rust => rust_visibility(text),
        Lang::Java | Lang::Kotlin => jvm_visibility(text),
        Lang::CSharp => csharp_visibility(text),
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => ts_visibility(text),
        Lang::C | Lang::Cpp => c_visibility(text),
        _ => Visibility::Unknown,
    }
}

fn visibility_max(a: Visibility, b: Visibility) -> Visibility {
    // Ordering on the lattice: Public > Restricted > Private > Unknown.
    if a.rank() >= b.rank() { a } else { b }
}

fn rust_visibility(text: &str) -> Visibility {
    // `visibility_modifier` text is the verbatim token, e.g. `pub` or
    // `pub(crate)` [src: https://github.com/tree-sitter/tree-sitter-rust].
    let trimmed = text.trim();
    if trimmed == "pub" {
        Visibility::Public
    } else if trimmed.starts_with("pub(") {
        Visibility::Restricted
    } else {
        Visibility::Private
    }
}

fn jvm_visibility(text: &str) -> Visibility {
    // Java/Kotlin `modifiers` blocks list keywords + annotations on one
    // node; pick the strongest visibility keyword anywhere in the text.
    if word_present(text, "public") {
        Visibility::Public
    } else if word_present(text, "protected") || word_present(text, "internal") {
        Visibility::Restricted
    } else if word_present(text, "private") {
        Visibility::Private
    } else {
        Visibility::Unknown
    }
}

fn csharp_visibility(text: &str) -> Visibility {
    if word_present(text, "public") {
        Visibility::Public
    } else if word_present(text, "protected") || word_present(text, "internal") {
        Visibility::Restricted
    } else if word_present(text, "private") {
        Visibility::Private
    } else {
        Visibility::Unknown
    }
}

fn word_present(haystack: &str, needle: &str) -> bool {
    haystack
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|w| w == needle)
}

fn ts_visibility(text: &str) -> Visibility {
    let trimmed = text.trim_start();
    if trimmed == "public" || trimmed.starts_with("export") {
        Visibility::Public
    } else if trimmed == "protected" {
        Visibility::Restricted
    } else if trimmed == "private" {
        Visibility::Private
    } else {
        Visibility::Unknown
    }
}

fn c_visibility(text: &str) -> Visibility {
    match text.trim() {
        "public:" | "public" => Visibility::Public,
        "protected:" | "protected" => Visibility::Restricted,
        "static" | "private:" | "private" => Visibility::Private,
        _ => Visibility::Unknown,
    }
}

/// Read the identifier portion of an attribute / annotation / decorator
/// node. Tree-sitter attribute nodes preserve the surrounding syntax
/// (`#[…]`, `@…`, `[Attr]`); the bare identifier the lattice cares about
/// is recovered by stripping the surrounding punctuation, then taking the
/// leading dotted-name path. Empty when no readable identifier remains.
fn attribute_name(node: Node<'_>, source: &[u8]) -> String {
    let raw = node.utf8_text(source).unwrap_or_default();
    let trimmed = raw.trim();
    let stripped = trimmed
        .trim_start_matches('#')
        .trim_start_matches('!')
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_start_matches('@')
        .trim();
    let head: String = stripped
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.' || *c == ':')
        .collect();
    head
}
