//! Half-open byte range inside a single file.

use serde::{Deserialize, Serialize};

use super::ids::FileId;

/// Half-open byte range inside a single file: `[byte_start, byte_end)`.
///
/// `Ord` is derived field-wise: `file`, then `byte_start`, then `byte_end`.
/// This total order is what query results sort on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Span {
    /// File the span belongs to.
    pub file: FileId,
    /// Inclusive start byte offset.
    pub byte_start: u32,
    /// Exclusive end byte offset.
    pub byte_end: u32,
}
