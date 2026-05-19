//! prost-generated SCIP protobuf types.
//!
//! The `include!` pulls in the file emitted by `build.rs` (see crate root).
//! Generated identifiers follow the proto file at the SHA pinned in
//! `proto/SCIP_COMMIT` [src: https://github.com/sourcegraph/scip/blob/main/scip.proto].
//!
//! `unsafe_code = "forbid"` (workspace lint) does not apply to generated
//! prost output because prost emits safe Rust. The module re-exports the
//! generated types so the rest of the crate never references `OUT_DIR`.

#![allow(
    missing_docs,
    clippy::pedantic,
    clippy::nursery,
    clippy::doc_overindented_list_items,
    clippy::doc_markdown,
    clippy::doc_lazy_continuation,
    rustdoc::invalid_rust_codeblocks,
    rustdoc::bare_urls,
    rustdoc::broken_intra_doc_links
)]

include!(concat!(env!("OUT_DIR"), "/scip.rs"));
