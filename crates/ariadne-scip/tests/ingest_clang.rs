//! Per-language ingest golden for C/C++ (scip-clang). The Lang tag is
//! `Lang::Other("clang")` because `ariadne-core::Lang` has no C/C++
//! variant — see `src/indexer/scip_clang.rs` for the rationale.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_clang_summary() {
    let bytes = synth_bytes(
        "scip-clang",
        "src/demo.cpp",
        "Cpp",
        &[
            SymBp {
                raw: "scip-clang cmake demo 1.0 demo/run().",
                occurrences: 6,
                relationships: 1,
            },
            SymBp {
                raw: "scip-clang cmake demo 1.0 demo/Demo#",
                occurrences: 4,
                relationships: 2,
            },
            SymBp {
                raw: "scip-clang cmake demo 1.0 demo/Demo#run().",
                occurrences: 1,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::Other("clang"), &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
