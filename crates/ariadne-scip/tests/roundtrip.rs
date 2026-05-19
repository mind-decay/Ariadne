//! Round-trip a `rust-analyzer scip`-produced index: decode → re-encode
//! → decode → structural equality. Guards two contracts:
//!
//! 1. The vendored `proto/scip.proto` matches the format rust-analyzer
//!    actually emits at the SHA pinned in `proto/SCIP_COMMIT` for every
//!    field we model [src:
//!    <https://github.com/sourcegraph/scip/blob/main/scip.proto>].
//! 2. `prost-build`-generated types preserve every field on the wire —
//!    no `oneof` or `optional` shape drift between prost minor versions
//!    [src: <https://docs.rs/prost/0.14.3/prost/trait.Message.html#tymethod.encode>].
//!
//! ## Why structural, not byte-for-byte
//!
//! prost 0.14 does not retain unknown fields on decode [src:
//! <https://github.com/tokio-rs/prost/issues/2> — preserving unknowns is
//! a documented gap]. rust-analyzer's bundled SCIP proto may carry
//! fields the vendored copy at `proto/SCIP_COMMIT` does not yet model
//! (or vice versa), so a literal `bytes == re_encoded` check fails the
//! instant either side is off by even one optional field. Structural
//! `decode == decode(encode(decode))` exercises the same wire-coverage
//! invariant for every field the proto file does declare, which is what
//! tier-04+ consumers actually rely on. Tier plan deviation recorded in
//! `tier-05-scip-ingest.md`.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 3.

use std::path::PathBuf;

use ariadne_scip::proto;
use prost::Message;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample.scip")
}

#[test]
fn rust_analyzer_scip_roundtrips_structurally() {
    let bytes = std::fs::read(fixture_path())
        .expect("tests/fixtures/sample.scip is committed alongside the test");
    assert!(
        !bytes.is_empty(),
        "fixture must be non-empty for the round-trip to be meaningful",
    );

    let decoded = proto::Index::decode(bytes.as_slice())
        .expect("prost must decode the fixture against the vendored proto");

    // The fixture is a real rust-analyzer SCIP index, so a few invariants
    // are easy to assert directly; they fail loud if the proto definitions
    // ever drift away from rust-analyzer's emitter.
    let metadata = decoded
        .metadata
        .as_ref()
        .expect("rust-analyzer always populates Metadata");
    let tool_info = metadata
        .tool_info
        .as_ref()
        .expect("rust-analyzer always populates ToolInfo");
    assert_eq!(tool_info.name, "rust-analyzer");
    assert!(
        !decoded.documents.is_empty(),
        "fixture crate has a non-empty src/lib.rs so Documents must be present",
    );

    let mut re_encoded = Vec::with_capacity(bytes.len());
    decoded
        .encode(&mut re_encoded)
        .expect("prost encode must succeed for a decoded Index");

    let re_decoded = proto::Index::decode(re_encoded.as_slice())
        .expect("prost decode of its own output must succeed");

    assert_eq!(
        decoded, re_decoded,
        "every field declared by the vendored proto must round-trip without loss",
    );
}
