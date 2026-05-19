//! Per-language ingest golden for TypeScript.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_typescript_summary() {
    let bytes = synth_bytes(
        "scip-typescript",
        "src/index.ts",
        "TypeScript",
        &[
            SymBp {
                raw: "scip-typescript npm demo 1.0 src/index/Foo#",
                occurrences: 6,
                relationships: 1,
            },
            SymBp {
                raw: "scip-typescript npm demo 1.0 src/index/run().",
                occurrences: 4,
                relationships: 1,
            },
            SymBp {
                raw: "scip-typescript npm demo 1.0 src/index/Foo#bar().",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::TypeScript, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
