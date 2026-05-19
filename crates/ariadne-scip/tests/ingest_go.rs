//! Per-language ingest golden for Go (lsif-go → scip convert pipeline).
//! The scheme on the synthesized symbols reflects what `scip convert`
//! emits after consuming an `lsif-go` dump.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_go_summary() {
    let bytes = synth_bytes(
        "scip-go",
        "demo.go",
        "Go",
        &[
            SymBp {
                raw: "scip-go gomod demo 1.0 demo/Run().",
                occurrences: 4,
                relationships: 1,
            },
            SymBp {
                raw: "scip-go gomod demo 1.0 demo/Demo#",
                occurrences: 3,
                relationships: 2,
            },
            SymBp {
                raw: "scip-go gomod demo 1.0 demo/Demo#Field.",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::Go, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
