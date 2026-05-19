//! Compiles `proto/scip.proto` into `OUT_DIR/scip.rs` via prost-build.
//!
//! The proto is vendored from sourcegraph/scip at the SHA pinned in
//! `proto/SCIP_COMMIT`
//! [src: <https://github.com/sourcegraph/scip/blob/main/scip.proto>].
//! `protoc` itself is supplied by `protoc-bin-vendored`, keeping the build
//! self-contained on systems without a system protoc
//! [src: <https://crates.io/crates/protoc-bin-vendored>]. We feed the
//! vendored path to `Config::protoc_executable` rather than setting
//! `PROTOC`, so the workspace `unsafe_code = "forbid"` lint stays clean
//! [src: <https://docs.rs/prost-build/0.14.3/prost_build/struct.Config.html#method.protoc_executable>].
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 2.

fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("protoc-bin-vendored must ship a protoc binary for this target");

    println!("cargo:rerun-if-changed=proto/scip.proto");
    println!("cargo:rerun-if-changed=proto/SCIP_COMMIT");

    // Disable comment emission on generated types: prost forwards the
    // proto comments verbatim as Rust rustdoc, but those comments include
    // ASCII grammar diagrams and bare braces that rustdoc misreads as
    // doctest code blocks (`error[E0762]: numeric character escape …`
    // observed at SHA 99236e35). `cargo test --doc` then aborts. The `.`
    // path acts as a wildcard across the whole codebase
    // [src: <https://docs.rs/prost-build/0.14.3/prost_build/struct.Config.html#method.disable_comments>].
    prost_build::Config::new()
        .protoc_executable(protoc)
        .disable_comments(["."])
        .compile_protos(&["proto/scip.proto"], &["proto/"])
        .expect("scip.proto must compile via prost-build");
}
