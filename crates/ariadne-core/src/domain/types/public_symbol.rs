//! Public-surface symbol value object, shared across adapters.
//!
//! Produced by the `ariadne-parser` public-surface extractor and consumed by
//! the `ariadne-graph` API-surface diff (block A, A2). It lives in the
//! dependency-free domain interior so the parser produces it and the graph
//! consumes it without a cross-adapter dependency
//! [src: .claude/plans/intelligence-platform/block-a/plan.md D3/D6].

use serde::{Deserialize, Serialize};

use super::Visibility;

/// One public declaration's identity for API-surface comparison.
///
/// `signature` is the whitespace-normalized text of the declaration *header* —
/// the source slice from the declaration's start up to (excluding) its
/// body-open delimiter — so two refs' surfaces compare header-for-header
/// without the body's churn [src: block-a plan.md D3]. All fields are owned so
/// the value survives past any borrow of the parsed source, mirroring the
/// sibling on-disk records [src: crates/ariadne-core/src/domain/records.rs:37-59].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PublicSymbol {
    /// Declared identifier name.
    pub name: String,
    /// Free-form kind tag (e.g. `"function"`, `"struct"`), mirroring
    /// [`crate::SymbolRecord::kind`].
    pub kind: String,
    /// Coarse visibility lattice point; [`Visibility::Public`] for every
    /// symbol on the public surface.
    pub visibility: Visibility,
    /// Whitespace-normalized declaration-header text.
    pub signature: String,
}
