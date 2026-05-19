//! Per-language ingest golden for Java.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_java_summary() {
    let bytes = synth_bytes(
        "scip-java",
        "src/main/java/Demo.java",
        "Java",
        &[
            SymBp {
                raw: "scip-java maven demo 1.0 com/example/Demo#",
                occurrences: 7,
                relationships: 2,
            },
            SymBp {
                raw: "scip-java maven demo 1.0 com/example/Demo#run().",
                occurrences: 3,
                relationships: 1,
            },
            SymBp {
                raw: "scip-java maven demo 1.0 com/example/Demo#field.",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::Java, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
