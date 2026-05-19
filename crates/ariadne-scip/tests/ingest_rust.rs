//! Per-language ingest golden for Rust. Synthesizes a minimal `scip-rust`
//! Index, decodes via the public `parse` free function, and asserts the
//! summary insta snapshot.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_rust_summary() {
    let bytes = synth_bytes(
        "scip-rust",
        "src/lib.rs",
        "Rust",
        &[
            SymBp {
                raw: "scip-rust cargo demo 1.0 lib/main/",
                occurrences: 5,
                relationships: 1,
            },
            SymBp {
                raw: "scip-rust cargo demo 1.0 lib/run().",
                occurrences: 3,
                relationships: 2,
            },
            SymBp {
                raw: "scip-rust cargo demo 1.0 lib/Doc#",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::Rust, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
