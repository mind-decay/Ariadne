//! Stable domain value objects. Split across focused submodules to stay
//! under the project's 200-line authoring cap.

pub mod ids;
pub mod lang;
pub mod span;

pub use ids::{EdgeId, FileId, IdEncode, SymbolId};
pub use lang::Lang;
pub use span::Span;
