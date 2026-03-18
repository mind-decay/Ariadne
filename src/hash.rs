use crate::model::ContentHash;

/// Compute xxHash64 of the given bytes, returning a ContentHash (lowercase hex, 16 chars).
pub fn hash_content(bytes: &[u8]) -> ContentHash {
    let hash = xxhash_rust::xxh64::xxh64(bytes, 0);
    ContentHash::new(format!("{:016x}", hash))
}
