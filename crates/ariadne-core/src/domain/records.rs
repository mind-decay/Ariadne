//! On-disk record types persisted via the [`Storage`](super::ports::Storage)
//! port. All fields are owned so the records survive past any borrow of the
//! storage backend.

use serde::{Deserialize, Serialize};

use super::types::{FileId, IdEncode, Lang, Span, SymbolId, Visibility};

/// File-level record. `blake3` is the content hash; `mtime_ns` is nanoseconds
/// since the UNIX epoch (signed to admit pre-1970 fixtures).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileRecord {
    /// Project-root-relative path as recorded by the watcher.
    pub path: String,
    /// Detected language tag.
    pub lang: Lang,
    /// File size in bytes at the recorded revision.
    pub size: u64,
    /// blake3 content hash.
    pub blake3: [u8; 32],
    /// Modification time, nanoseconds since the UNIX epoch.
    pub mtime_ns: i128,
}

/// Symbol record. `kind` is a free-form string until tier-05 (SCIP ingest)
/// canonicalizes the taxonomy.
///
/// `visibility` and `attributes` are appended after the v1/v2 fields so the
/// v3 postcard layout extends the v2 byte prefix unchanged; the redb v2->v3
/// migration step decodes the historical 4-field record and re-encodes it
/// with the new fields defaulted [src: post-v1-roadmap plan.md RD10].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolRecord {
    /// Canonical symbol name as emitted by the ingest pipeline.
    pub canonical_name: String,
    /// Free-form kind tag (e.g. "function", "struct", "method").
    pub kind: String,
    /// File the defining occurrence lives in.
    pub defining_file: FileId,
    /// Span of the defining occurrence.
    pub defining_span: Span,
    /// Coarse visibility lattice; `Unknown` when the producing pipeline
    /// observed no modifier or the format predates the field.
    pub visibility: Visibility,
    /// Attribute / annotation / decorator identifiers attached to the
    /// declaration (e.g. Rust `#[test]`, Java `@Override`, TS decorators).
    /// Empty when none observed.
    pub attributes: Vec<String>,
}

/// Per-file Git-history churn record. Persisted in the `CHURN` table by the
/// `ariadne-git` driven adapter (tier-11); consumed by the tier-13 hotspot
/// metrics. `author_keys` stores the distinct-author *set* (not a bare count)
/// so tier-11a can merge incremental walks by set union without a second
/// record migration [src: post-v1-roadmap plan.md RD7 + tier-11 step 7].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileChurn {
    /// Repository-root-relative path the churn was observed for.
    pub path: String,
    /// Number of commits in the walked window that touched `path`.
    pub commits: u32,
    /// Distinct author identity keys (8-byte digests of author identity)
    /// observed touching `path`. Set semantics: deduplicated, order-stable.
    pub author_keys: Vec<[u8; 8]>,
    /// Latest committer time touching `path`, nanoseconds since the UNIX
    /// epoch (signed to match [`FileRecord::mtime_ns`]).
    pub last_changed_ns: i128,
}

impl FileChurn {
    /// Distinct-author count, i.e. the cardinality of [`FileChurn::author_keys`].
    #[must_use]
    pub fn authors(&self) -> usize {
        self.author_keys.len()
    }
}

/// Unordered file-pair co-change record: how many commits in the walked
/// window changed both `a` and `b` together. `a < b` lexicographically so the
/// pair is canonical. Persisted in the `CO_CHANGE` table by `ariadne-git`
/// (tier-11) [src: post-v1-roadmap plan.md RD7 + tier-11 step 7].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CoChangePair {
    /// Lexicographically-smaller path of the pair.
    pub a: String,
    /// Lexicographically-larger path of the pair.
    pub b: String,
    /// Number of commits that changed both `a` and `b`.
    pub count: u32,
}

/// Transient symbol-churn join input (tier-11b): one contiguous range of
/// *new-side* lines changed by a single commit in `path`, 1-based and
/// inclusive. Emitted by the `ariadne-git` adapter (per modified blob, via
/// `gix` `blob-diff`) and consumed by the `ariadne-graph` symbol-churn
/// attribution use-case. Never persisted — it is the symbol-agnostic wire
/// between the git adapter and the symbol join (ADR-0019)
/// [src: post-v1-roadmap plan.md RD7 + tier-11b].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LineHunk {
    /// Repository-root-relative path the changed lines belong to.
    pub path: String,
    /// First changed line (1-based, inclusive).
    pub start_line: u32,
    /// Last changed line (1-based, inclusive).
    pub end_line: u32,
}

/// Per-symbol Git-history churn (tier-11b): how many commits in the attributed
/// window changed lines covered by the symbol's defining span. Persisted in the
/// `SYMBOL_CHURN` table keyed by [`SymbolId`]; produced by the `ariadne-graph`
/// attribution use-case from [`LineHunk`]s + symbol spans. A symbol with no
/// attributed commit is absent from the table (read as zero), so only churned
/// symbols are stored [src: post-v1-roadmap plan.md RD7 + tier-11b].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolChurn {
    /// The symbol the churn is attributed to.
    pub symbol: SymbolId,
    /// Number of commits in the window that touched the symbol's span.
    pub commits: u32,
}

/// Edge kind tag. Definition / reference / import are the syntactic core;
/// `Renders` and `UsesHook` carry the component graph (ADR-0012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum EdgeKind {
    /// Definition site → defined symbol.
    Defines = 0,
    /// Reference site → referenced symbol.
    References = 1,
    /// Import site → imported module/symbol.
    Imports = 2,
    /// Component → child component it renders (ADR-0012).
    Renders = 3,
    /// Component → hook / reactive primitive it uses (ADR-0012).
    UsesHook = 4,
}

impl EdgeKind {
    /// Single-byte tag used by [`EdgeKey`]'s fixed-width key encoding.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Inverse of [`EdgeKind::to_byte`]. Returns `None` for unknown tags.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Defines),
            1 => Some(Self::References),
            2 => Some(Self::Imports),
            3 => Some(Self::Renders),
            4 => Some(Self::UsesHook),
            _ => None,
        }
    }
}

/// Composite key of the `EDGES` table: `(src, kind, dst)`. The lex-ordered
/// big-endian byte form is the storage primary key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EdgeKey {
    /// Source symbol.
    pub src: SymbolId,
    /// Edge kind tag.
    pub kind: EdgeKind,
    /// Destination symbol.
    pub dst: SymbolId,
}

impl EdgeKey {
    /// 17-byte fixed-width key: `[src(8) | kind(1) | dst(8)]`. Big-endian
    /// fixed-width keys preserve lex order under redb's default `&[u8]`
    /// comparator.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 17] {
        let mut out = [0u8; 17];
        out[..8].copy_from_slice(&self.src.to_bytes());
        out[8] = self.kind.to_byte();
        out[9..].copy_from_slice(&self.dst.to_bytes());
        out
    }

    /// Inverse of [`EdgeKey::to_bytes`]. Returns `None` if any subfield
    /// fails to decode (zero ids, unknown edge kind).
    #[must_use]
    pub fn from_bytes(bytes: &[u8; 17]) -> Option<Self> {
        let mut src = [0u8; 8];
        src.copy_from_slice(&bytes[..8]);
        let mut dst = [0u8; 8];
        dst.copy_from_slice(&bytes[9..]);
        Some(Self {
            src: SymbolId::from_bytes(src)?,
            kind: EdgeKind::from_byte(bytes[8])?,
            dst: SymbolId::from_bytes(dst)?,
        })
    }
}

/// Edge body. The body lives behind `EdgeKey` in the `EDGES` table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeRecord {
    /// Span the edge was observed at.
    pub source_span: Span,
    /// Language the evidence came from.
    pub evidence_lang: Lang,
    /// Coupling weight; reserved for tier-07 analytics.
    pub weight: u32,
}
