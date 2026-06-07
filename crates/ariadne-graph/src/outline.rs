//! Pure code-skeleton (outline) assembler — a deterministic projection of a
//! file's bytes + ordered symbol spans into folded source plus a symbol index.
//!
//! No IO, no graph query, no model: [`assemble`] is a total function over its
//! [`OutlineRequest`]. Signatures and leading doc comments are sliced
//! byte-faithfully from the source; bodies are folded to a marker carrying the
//! exact elided-line count; nesting is derived from span containment; doc
//! comments from the language's leading-comment syntax. Sibling to `docgen` /
//! `api_surface`; both driving adapters reuse it without a driving→driving edge
//! [src: .claude/plans/context-efficient-read/plan.md D3/D4/D5].

use ariadne_core::{Lang, Visibility};

/// One symbol's identity, byte span, and visibility, as fed by a driving
/// adapter from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineSymbol {
    /// Declared identifier name.
    pub name: String,
    /// Free-form kind tag (e.g. `function`, `struct`, `impl`).
    pub kind: String,
    /// Byte offset of the symbol's first byte in the source.
    pub byte_start: u32,
    /// Byte offset one past the symbol's last byte.
    pub byte_end: u32,
    /// Coarse visibility consulted by the private filter.
    pub visibility: Visibility,
}

/// Options controlling the projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutlineOptions {
    /// Keep symbols whose visibility is not `Public` (and their bodies).
    pub include_private: bool,
    /// Cap on rendered top-level symbols; `0` means unbounded. The cap is
    /// noted in the skeleton tail, never silently truncated.
    pub max_symbols: usize,
}

/// Input to [`assemble`]: source bytes, the file's symbol spans, its language,
/// and the projection options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineRequest {
    /// The file's bytes; signatures and doc comments are sliced from these.
    pub source: Vec<u8>,
    /// The file's symbols, in any order (normalized internally by `byte_start`).
    pub symbols: Vec<OutlineSymbol>,
    /// The defining file's language (selects the doc-comment syntax).
    pub lang: Lang,
    /// Projection options.
    pub options: OutlineOptions,
}

/// One entry in the rendered outline's symbol index — advertises the source a
/// consumer can expand on demand via `read_symbol`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlineEntry {
    /// Declared identifier name.
    pub name: String,
    /// Kind tag carried from the input symbol.
    pub kind: String,
    /// 1-based first source line of the symbol.
    pub line_start: u32,
    /// 1-based last source line of the symbol.
    pub line_end: u32,
    /// Source lines spanned by the (folded or kept) body.
    pub body_lines: u32,
    /// Whether the symbol has a body beyond its signature line.
    pub has_body: bool,
}

/// Folded-source projection of a file plus its symbol index. `kept_lines +
/// elided_lines` accounts for every source line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outline {
    /// The rendered folded source.
    pub skeleton: String,
    /// Retained symbols in source order.
    pub symbols: Vec<OutlineEntry>,
    /// Source lines kept verbatim.
    pub kept_lines: u32,
    /// Source lines folded away (bodies + hidden symbols + elided gaps).
    pub elided_lines: u32,
}

/// Inter-symbol gaps with at most this many non-blank lines survive verbatim
/// (imports / attributes); larger gaps collapse to an elide marker.
const GAP_KEEP_LINES: usize = 8;
/// Leaf bodies spanning at most this many source lines stay inline; longer
/// ones fold to a marker.
const INLINE_LINES: u32 = 2;

/// Leading-comment syntax of a language, used to scan doc comments above a
/// declaration.
#[derive(Clone, Copy)]
enum CommentSyntax {
    /// `//` line comments and `/* */` block comments (C-family + dialects).
    CFamily,
    /// `#` line comments.
    Hash,
    /// No recognised comment syntax — capture nothing.
    None,
}

/// Geometry of one symbol in line space.
struct NodeGeo {
    first: usize,
    sig_line: usize,
    sig_off: usize,
    last: usize,
    doc: usize,
}

/// Immutable rendering context shared by the recursive walk.
struct Ctx<'a> {
    nodes: &'a [NodeGeo],
    children: &'a [Vec<usize>],
    retained: &'a [bool],
    capped: &'a [bool],
}

/// Mutable skeleton builder: accumulates rendered lines and the line tallies.
struct Builder<'a> {
    bytes: &'a [u8],
    bounds: &'a [(usize, usize)],
    out: Vec<String>,
    kept: u32,
    elided: u32,
}

/// Assemble a folded-source [`Outline`] from `req`. Pure and deterministic: the
/// same request always renders byte-identical output.
#[must_use]
pub fn assemble(req: &OutlineRequest) -> Outline {
    let bytes = &req.source;
    let bounds = line_bounds(bytes);
    let total = bounds.len();
    let syntax = lang_comment(req.lang);

    // Normalize symbol order by (byte_start, byte_end).
    let mut order: Vec<usize> = (0..req.symbols.len()).collect();
    order.sort_by_key(|&i| (req.symbols[i].byte_start, req.symbols[i].byte_end));
    let syms: Vec<OutlineSymbol> = order.iter().map(|&i| req.symbols[i].clone()).collect();
    let n = syms.len();

    // Parent / children by nearest-enclosing span containment (R2).
    let parents: Vec<Option<usize>> = (0..n).map(|i| parent_of(&syms, i)).collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, p) in parents.iter().enumerate() {
        if let Some(pi) = *p {
            children[pi].push(i);
        }
    }
    for c in &mut children {
        c.sort_by_key(|&i| syms[i].byte_start);
    }

    // Geometry per symbol.
    let nodes: Vec<NodeGeo> = (0..n)
        .map(|i| {
            let bs = syms[i].byte_start as usize;
            let be = syms[i].byte_end as usize;
            let first = line_index(bytes, bs);
            let sig_off = signature_end(bytes, bs, be);
            NodeGeo {
                first,
                sig_line: line_index(bytes, sig_off),
                sig_off,
                last: line_index(bytes, be.saturating_sub(1)),
                doc: doc_above_line(bytes, &bounds, first, syntax),
            }
        })
        .collect();

    // Retained set: Public, or (transitively) a container of a retained symbol.
    let retained: Vec<bool> = (0..n)
        .map(|i| is_retained(&syms, &children, i, req.options.include_private))
        .collect();

    // Top-level symbols in source order; cap the retained ones (never silent).
    let top: Vec<usize> = (0..n).filter(|&i| parents[i].is_none()).collect();
    let (capped, over) = cap_top_level(&top, &children, &retained, n, req.options.max_symbols);

    let ctx = Ctx {
        nodes: &nodes,
        children: &children,
        retained: &retained,
        capped: &capped,
    };
    let mut b = Builder {
        bytes,
        bounds: &bounds,
        out: Vec::new(),
        kept: 0,
        elided: 0,
    };
    let mut cursor = 0usize;
    for &i in &top {
        render_node(&mut b, &ctx, i, &mut cursor);
    }
    b.gap(cursor, total);
    if over > 0 {
        let word = if over == 1 { "symbol" } else { "symbols" };
        b.out.push(format!(
            "// … {over} more {word} elided (max_symbols={})",
            req.options.max_symbols
        ));
    }

    let symbols = (0..n)
        .filter(|&i| retained[i] && !capped[i])
        .map(|i| {
            let g = &nodes[i];
            let body = u32c(g.last.saturating_sub(g.sig_line));
            OutlineEntry {
                name: syms[i].name.clone(),
                kind: syms[i].kind.clone(),
                line_start: u32c(g.first + 1),
                line_end: u32c(g.last + 1),
                body_lines: body,
                has_body: body > 0,
            }
        })
        .collect();

    Outline {
        skeleton: b.out.join("\n"),
        symbols,
        kept_lines: b.kept,
        elided_lines: b.elided,
    }
}

/// Render one symbol and advance `cursor` past its last line. Hidden (filtered
/// or capped) symbols elide their doc + span; containers recurse; leaves fold a
/// long body to a marker or keep a short one verbatim.
fn render_node(b: &mut Builder<'_>, ctx: &Ctx<'_>, i: usize, cursor: &mut usize) {
    let g = &ctx.nodes[i];
    let doc = g.doc.max(*cursor);
    b.gap(*cursor, doc);

    if !ctx.retained[i] || ctx.capped[i] {
        b.elide(doc, g.last + 1);
        *cursor = g.last + 1;
        return;
    }

    b.keep(doc, g.first); // leading doc comments, byte-faithful.

    if ctx.children[i].iter().any(|&c| ctx.retained[c]) {
        // Container: header through the body-opening line, recurse, closer.
        b.keep(g.first, g.sig_line + 1);
        let mut inner = g.sig_line + 1;
        for &c in &ctx.children[i] {
            render_node(b, ctx, c, &mut inner);
        }
        b.gap(inner, g.last);
        b.keep(g.last, g.last + 1);
    } else {
        let body = u32c(g.last.saturating_sub(g.sig_line));
        if body > INLINE_LINES {
            b.keep(g.first, g.sig_line); // multi-line signature prefix.
            let (ls, _) = b.bounds[g.sig_line];
            let sig = String::from_utf8_lossy(&b.bytes[ls..g.sig_off]).into_owned();
            b.out.push(format!("{sig}{{ … {body} lines }}"));
            b.kept = b.kept.saturating_add(1);
            b.elided = b.elided.saturating_add(body);
        } else {
            b.keep(g.first, g.last + 1); // short const/type kept verbatim.
        }
    }
    *cursor = g.last + 1;
}

impl Builder<'_> {
    /// Whole-line text (without its trailing newline) at line index `i`.
    fn line(&self, i: usize) -> String {
        let (s, e) = self.bounds[i];
        String::from_utf8_lossy(&self.bytes[s..e]).into_owned()
    }

    /// Emit lines `[lo, hi)` verbatim, counting them as kept.
    fn keep(&mut self, lo: usize, hi: usize) {
        for i in lo..hi {
            let l = self.line(i);
            self.out.push(l);
        }
        self.kept = self.kept.saturating_add(u32c(hi.saturating_sub(lo)));
    }

    /// Count lines `[lo, hi)` as elided without emitting them (hidden symbols).
    fn elide(&mut self, lo: usize, hi: usize) {
        if hi > lo {
            self.elided = self.elided.saturating_add(u32c(hi - lo));
        }
    }

    /// Inter-symbol gap: keep verbatim when small, else collapse to a marker.
    fn gap(&mut self, lo: usize, hi: usize) {
        if hi <= lo {
            return;
        }
        let nonblank = (lo..hi)
            .filter(|&i| !self.line(i).trim().is_empty())
            .count();
        if nonblank <= GAP_KEEP_LINES {
            self.keep(lo, hi);
        } else {
            self.out.push(format!("// … {} lines elided", hi - lo));
            self.elided = self.elided.saturating_add(u32c(hi - lo));
        }
    }
}

/// Mark the retained top-level symbols past `max_symbols` (and all their
/// descendants) as capped; returns the capped flags and the overflow count.
/// `0` means unbounded. Descendants of a capped container are capped too so the
/// index stays in step with the skeleton, which elides the container whole
/// (INFO-3).
fn cap_top_level(
    top: &[usize],
    children: &[Vec<usize>],
    retained: &[bool],
    n: usize,
    max_symbols: usize,
) -> (Vec<bool>, usize) {
    let mut capped = vec![false; n];
    let mut over = 0usize;
    if max_symbols == 0 {
        return (capped, over);
    }
    let mut kept = 0usize;
    for &i in top {
        if retained[i] {
            if kept >= max_symbols {
                capped[i] = true;
                over += 1;
            } else {
                kept += 1;
            }
        }
    }
    if over > 0 {
        let mut stack: Vec<usize> = (0..n).filter(|&i| capped[i]).collect();
        while let Some(i) = stack.pop() {
            for &c in &children[i] {
                if !capped[c] {
                    capped[c] = true;
                    stack.push(c);
                }
            }
        }
    }
    (capped, over)
}

/// A symbol is retained when private symbols are included, when it is `Public`,
/// or when it (transitively) contains a retained symbol.
fn is_retained(syms: &[OutlineSymbol], children: &[Vec<usize>], i: usize, inc_priv: bool) -> bool {
    inc_priv
        || syms[i].visibility == Visibility::Public
        || children[i]
            .iter()
            .any(|&c| is_retained(syms, children, c, inc_priv))
}

/// Nearest-enclosing parent of symbol `i`: the strictly-containing symbol with
/// the smallest span. Ties on the minimal span fall back to top-level (R2).
fn parent_of(syms: &[OutlineSymbol], i: usize) -> Option<usize> {
    let s = &syms[i];
    let (mut best, mut best_span, mut tie) = (None, u64::MAX, false);
    for (j, p) in syms.iter().enumerate() {
        if j == i {
            continue;
        }
        let contains = p.byte_start <= s.byte_start && s.byte_end <= p.byte_end;
        let equal = p.byte_start == s.byte_start && p.byte_end == s.byte_end;
        if contains && !equal {
            let span = u64::from(p.byte_end - p.byte_start);
            if span < best_span {
                best_span = span;
                best = Some(j);
                tie = false;
            } else if span == best_span {
                tie = true;
            }
        }
    }
    if tie { None } else { best }
}

/// Map a language to its leading-comment syntax (R1).
fn lang_comment(lang: Lang) -> CommentSyntax {
    match lang {
        Lang::Python => CommentSyntax::Hash,
        Lang::Other(_) => CommentSyntax::None,
        _ => CommentSyntax::CFamily,
    }
}

/// Whether a trimmed line is a comment under `syntax`.
fn is_comment_line(syntax: CommentSyntax, trimmed: &str) -> bool {
    match syntax {
        CommentSyntax::CFamily => {
            trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.ends_with("*/")
        }
        CommentSyntax::Hash => trimmed.starts_with('#'),
        CommentSyntax::None => false,
    }
}

/// First line index of the contiguous comment block directly above `first`
/// (stopping at a blank or non-comment line); `first` when none.
fn doc_above_line(
    bytes: &[u8],
    bounds: &[(usize, usize)],
    first: usize,
    syntax: CommentSyntax,
) -> usize {
    let mut top = first;
    while top > 0 {
        let (s, e) = bounds[top - 1];
        let line = String::from_utf8_lossy(&bytes[s..e]);
        let t = line.trim();
        if t.is_empty() || !is_comment_line(syntax, t) {
            break;
        }
        top -= 1;
    }
    top
}

/// End offset of a declaration's signature: the first `{` or `;` scanning from
/// `start` (bounded by `end`), or a trailing block-opening `:` (Python). The
/// scan spans lines so multi-line signatures (generics / where-clauses) reach
/// their body opener (R3).
fn signature_end(bytes: &[u8], start: usize, end: usize) -> usize {
    let cap = end.min(bytes.len());
    let mut lo = start.min(cap);
    loop {
        let le = line_end(bytes, lo).min(cap);
        if let Some(off) = bytes[lo..le].iter().position(|&b| b == b'{' || b == b';') {
            return lo + off;
        }
        if le > lo && bytes[le - 1] == b':' {
            return le - 1;
        }
        if le >= cap {
            return le;
        }
        lo = le + 1;
        if lo >= cap {
            return cap;
        }
    }
}

/// Byte bounds `(start, end-excluding-newline)` of every source line; a
/// trailing newline yields no phantom empty line.
fn line_bounds(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut s = 0;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            v.push((s, i));
            s = i + 1;
        }
    }
    if s < bytes.len() {
        v.push((s, bytes.len()));
    }
    v
}

/// End offset of the line containing `pos` — the next `\n` (exclusive) or the
/// file length.
fn line_end(bytes: &[u8], pos: usize) -> usize {
    if pos >= bytes.len() {
        return bytes.len();
    }
    match bytes[pos..].iter().position(|&b| b == b'\n') {
        Some(p) => pos + p,
        None => bytes.len(),
    }
}

/// 0-based line index of byte `off` — the count of `\n` before it.
#[allow(clippy::naive_bytecount)]
fn line_index(bytes: &[u8], off: usize) -> usize {
    let off = off.min(bytes.len());
    bytes[..off].iter().filter(|&&b| b == b'\n').count()
}

/// Saturating `usize` → `u32` for line tallies.
fn u32c(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
