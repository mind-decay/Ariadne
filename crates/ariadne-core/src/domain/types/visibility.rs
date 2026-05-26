//! Coarse visibility lattice attached to a [`crate::SymbolRecord`].
//!
//! `Visibility` spans the ~10 language-specific visibility models Ariadne
//! ingests (Rust `pub`, JS/TS `export`, Java/Kotlin/C# `public`/`private`/…,
//! Python `__name__`, Go exported-by-identifier-case) at four points:
//!
//! - `Public` — visible to any consumer of the defining unit (Rust `pub`,
//!   `export …`, Java `public`, Go capitalised identifier).
//! - `Restricted` — narrower than `Public` but wider than `Private`
//!   (Rust `pub(crate)` / `pub(super)`, Java/Kotlin/C# `protected`,
//!   `internal`).
//! - `Private` — visible only inside the defining module / type
//!   (Java/C# `private`, SCIP `local …` symbol).
//! - `Unknown` — the producing pipeline did not observe a modifier
//!   (default for grammars without a visibility node, fallback for SCIP
//!   indexers that emit no scope signal). Default-private grammars
//!   (Rust `mod inner`, Python `_x`) land here too — the language
//!   default is not re-inferred from absence; tier-05 owns that policy.
//!
//! tier-05 dead-code classification consumes `Visibility` as part of the
//! per-language entry-point root set [src: post-v1-roadmap plan.md RD4,
//! RD10].

use serde::{Deserialize, Serialize};

/// Coarse visibility tag for a defined symbol.
///
/// The variant discriminants are wire-stable u8 tags
/// ([`Visibility::to_byte`] / [`Visibility::from_byte`]) and intentionally
/// do **not** follow the lattice order. `PartialOrd`/`Ord` are therefore
/// not derived — comparing visibility for "more / less visible" must go
/// through [`Visibility::rank`] (`Public` > `Restricted` > `Private` >
/// `Unknown`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[non_exhaustive]
pub enum Visibility {
    /// Visible to any consumer of the defining unit.
    Public = 0,
    /// Narrower than `Public` but wider than `Private`.
    Restricted = 1,
    /// Visible only inside the defining module / type
    /// (e.g. Java/C# `private`, SCIP `local …` symbol).
    Private = 2,
    /// No visibility modifier observed by the producing pipeline.
    ///
    /// Default-private grammars (Rust, Python, …) emit `Unknown` when the
    /// surface syntax exposes no modifier token — the language-level
    /// default is *not* re-inferred at parse time. tier-05 dead-code
    /// classification owns the per-language interpretation of `Unknown`.
    #[default]
    Unknown = 3,
}

impl Visibility {
    /// Lattice rank: `Public` (3) > `Restricted` (2) > `Private` (1) >
    /// `Unknown` (0). Used to choose the strongest observed visibility
    /// when several captures land on the same decl.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::Public => 3,
            Self::Restricted => 2,
            Self::Private => 1,
            Self::Unknown => 0,
        }
    }

    /// Single-byte tag — used by salsa-side mirrors that cannot depend on
    /// `salsa::Update` for [`Visibility`] directly (ariadne-core is
    /// dependency-free per the architecture invariant). The byte value is
    /// the wire discriminant, not the lattice rank — use [`Self::rank`]
    /// for ordering.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        self as u8
    }

    /// Inverse of [`Visibility::to_byte`]; returns `None` for unknown tags.
    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Public),
            1 => Some(Self::Restricted),
            2 => Some(Self::Private),
            3 => Some(Self::Unknown),
            _ => None,
        }
    }
}
