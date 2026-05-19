//! Salsa input entities (tier-04 step 3) + per-path durability policy
//! (tier-04 step 5).
//!
//! Salsa requires every input field to implement `salsa::Update`. Two
//! adaptations from the tier-04 plan letter are forced by that bound and by
//! the architecture invariant (ariadne-salsa may not depend on ariadne-parser
//! or ariadne-scip — [src: tests/architecture.rs lines 30-33]):
//!
//! 1. `content` and `raw_proto` use `Vec<u8>` instead of `Arc<[u8]>`; Salsa
//!    interns inputs internally so the Arc-flattening loses no de-dup.
//!    `Arc<[u8]>` is unsized and not covered by salsa's blanket `Update`
//!    impls [src: salsa-rs/salsa src/update.rs blanket impl list].
//! 2. `Lang` carries an `Other(&'static str)` variant whose `'static str` is
//!    not `Update`. We store the language tag as a `String` here and
//!    convert via [`ariadne_core::Lang::tag`] / [`ariadne_core::Lang::from_tag`]
//!    at the boundary.

use std::path::PathBuf;

use salsa::Durability;

/// Per-file byte content. Tier-04 step 3 input #1.
#[salsa::input]
pub struct FileContentInput {
    /// Project-root-relative path. The salsa setter chain is the only way
    /// to mutate this; mutations bump the salsa revision.
    pub path: String,
    /// File bytes. `Vec<u8>` per the `Update` bound discussed in the module
    /// header.
    pub content: Vec<u8>,
    /// blake3 content hash, kept as a 32-byte fixed array so updates can
    /// short-circuit on hash equality if a future tier wants that.
    pub hash: [u8; 32],
}

/// Per-file metadata: lang tag + size + mtime. Tier-04 step 3 input #2.
#[salsa::input]
pub struct FileMetadataInput {
    /// Stable language tag (`ariadne_core::Lang::tag`). String chosen for the
    /// `Update` bound discussed in the module header.
    pub lang_tag: String,
    /// File size in bytes.
    pub size: u64,
    /// Modification time, nanoseconds since the UNIX epoch.
    pub mtime_ns: u64,
}

/// SCIP document blob for a file. Tier-05 fills `raw_proto`; tier-04 ships
/// the input shape so the salsa query graph is in place.
#[salsa::input]
pub struct ScipDocInput {
    /// Project-root-relative path of the file the SCIP doc describes.
    pub path: String,
    /// Raw SCIP protobuf blob, if the language has a SCIP indexer wired.
    pub raw_proto: Option<Vec<u8>>,
}

/// Workspace-level configuration. Tier-04 step 3 input #4.
#[salsa::input]
pub struct ProjectConfigInput {
    /// Project root.
    pub root: PathBuf,
    /// Stable language tags enabled for this project.
    pub enabled_lang_tags: Vec<String>,
    /// Path globs ignored by the watcher.
    pub ignore: Vec<String>,
}

/// Per-path durability policy. Tier-04 step 5: stdlib roots are `HIGH`,
/// vendor/dep trees are `MEDIUM`, project source is `LOW`. Policy lives
/// here so it can be unit-tested in isolation.
#[must_use]
pub fn durability_for(path: &str) -> Durability {
    // High-durability roots: stdlib + Rust toolchain dirs.
    const HIGH_ROOTS: &[&str] = &[
        "/usr/lib/rustlib/",
        "/usr/local/lib/rustlib/",
        "~/.rustup/toolchains/",
        "/usr/lib/python3",
        "/usr/local/lib/python3",
    ];
    // Medium-durability roots: vendored / dep / cache trees.
    const MEDIUM_FRAGMENTS: &[&str] = &[
        "/node_modules/",
        "/vendor/",
        "/target/",
        "/.venv/",
        "/.cargo/registry/",
    ];
    for root in HIGH_ROOTS {
        if path.starts_with(root) {
            return Durability::HIGH;
        }
    }
    for frag in MEDIUM_FRAGMENTS {
        if path.contains(frag) {
            return Durability::MEDIUM;
        }
    }
    Durability::LOW
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durability_high_for_stdlib() {
        assert_eq!(
            durability_for("/usr/lib/rustlib/src/rust/library/std/src/lib.rs"),
            Durability::HIGH,
        );
    }

    #[test]
    fn durability_medium_for_vendored_node_modules() {
        assert_eq!(
            durability_for("/repo/node_modules/foo/index.js"),
            Durability::MEDIUM,
        );
    }

    #[test]
    fn durability_medium_for_target_dir() {
        assert_eq!(
            durability_for("/repo/target/debug/build/foo"),
            Durability::MEDIUM,
        );
    }

    #[test]
    fn durability_low_for_project_source() {
        assert_eq!(durability_for("/repo/src/lib.rs"), Durability::LOW);
    }
}
