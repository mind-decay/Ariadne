//! Stable domain value objects.
//!
//! These IDs are the on-disk and on-wire identity of every entity Ariadne
//! tracks. Their byte encodings are part of the storage contract consumed
//! by tier-02 (redb codec) [src: .claude/plans/ariadne-core/tier-01-workspace.md step 6].

use std::num::{NonZeroU32, NonZeroU64};

/// 8-byte fixed-width id encoding used as the redb key/value codec contract.
pub trait IdEncode: Sized {
    /// Big-endian byte encoding. Lexicographic order on bytes matches numeric
    /// order on the underlying integer — required for ordered redb scans.
    fn to_bytes(&self) -> [u8; 8];
    /// Inverse of [`IdEncode::to_bytes`]. Returns `None` for the zero value
    /// or for encodings outside the type's domain (e.g. non-zero high bytes
    /// in [`FileId`]).
    fn from_bytes(bytes: [u8; 8]) -> Option<Self>;
}

/// Interned file path identity. The interner lives in tier-02 storage; this
/// crate fixes only the on-disk shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(NonZeroU32);

impl FileId {
    /// Wraps a non-zero `u32`. Returns `None` for zero.
    #[must_use]
    pub fn new(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    /// Underlying numeric value (always non-zero).
    #[must_use]
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl IdEncode for FileId {
    fn to_bytes(&self) -> [u8; 8] {
        let mut out = [0u8; 8];
        out[4..].copy_from_slice(&self.0.get().to_be_bytes());
        out
    }

    fn from_bytes(bytes: [u8; 8]) -> Option<Self> {
        if bytes[..4].iter().any(|&b| b != 0) {
            return None;
        }
        let raw = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        NonZeroU32::new(raw).map(Self)
    }
}

/// Symbol identity. Wider than `FileId` because symbols outnumber files
/// at ~10:1 in indexed repos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(NonZeroU64);

impl SymbolId {
    /// Wraps a non-zero `u64`. Returns `None` for zero.
    #[must_use]
    pub fn new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }

    /// Underlying numeric value (always non-zero).
    #[must_use]
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl IdEncode for SymbolId {
    fn to_bytes(&self) -> [u8; 8] {
        self.0.get().to_be_bytes()
    }

    fn from_bytes(bytes: [u8; 8]) -> Option<Self> {
        NonZeroU64::new(u64::from_be_bytes(bytes)).map(Self)
    }
}

/// Edge identity (def→ref, ref→def, contains, calls, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(NonZeroU64);

impl EdgeId {
    /// Wraps a non-zero `u64`. Returns `None` for zero.
    #[must_use]
    pub fn new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }

    /// Underlying numeric value (always non-zero).
    #[must_use]
    pub fn get(self) -> u64 {
        self.0.get()
    }
}

impl IdEncode for EdgeId {
    fn to_bytes(&self) -> [u8; 8] {
        self.0.get().to_be_bytes()
    }

    fn from_bytes(bytes: [u8; 8]) -> Option<Self> {
        NonZeroU64::new(u64::from_be_bytes(bytes)).map(Self)
    }
}

/// Language tag attached to files and symbols.
///
/// `Other(&'static str)` lets adapters carry a syntactic-only language that
/// the semantic pipeline (tier-05) does not understand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Lang {
    /// TypeScript.
    TypeScript,
    /// JavaScript.
    JavaScript,
    /// Python.
    Python,
    /// Rust.
    Rust,
    /// Go.
    Go,
    /// Java.
    Java,
    /// Kotlin.
    Kotlin,
    /// C#.
    CSharp,
    /// Any other tree-sitter grammar; carries its `tree-sitter-<lang>` name.
    Other(&'static str),
}

/// Half-open byte range inside a single file: `[byte_start, byte_end)`.
///
/// `Ord` is derived field-wise: `file`, then `byte_start`, then `byte_end`.
/// This total order is what query results sort on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Span {
    /// File the span belongs to.
    pub file: FileId,
    /// Inclusive start byte offset.
    pub byte_start: u32,
    /// Exclusive end byte offset.
    pub byte_end: u32,
}
