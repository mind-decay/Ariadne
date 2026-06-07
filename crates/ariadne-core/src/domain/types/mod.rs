//! Stable domain value objects. Split across focused submodules to stay
//! under the project's 200-line authoring cap.

pub mod ids;
pub mod lang;
pub mod public_symbol;
pub mod span;
pub mod visibility;

pub use ids::{EdgeId, FileId, IdEncode, SymbolId};
pub use lang::Lang;
pub use public_symbol::PublicSymbol;
pub use span::Span;
pub use visibility::Visibility;
