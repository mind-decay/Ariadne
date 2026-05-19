//! Per-language ingest golden for Python.
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_python_summary() {
    let bytes = synth_bytes(
        "scip-python",
        "demo/__init__.py",
        "Python",
        &[
            SymBp {
                raw: "scip-python pip demo 1.0 demo/Module/Demo#",
                occurrences: 4,
                relationships: 2,
            },
            SymBp {
                raw: "scip-python pip demo 1.0 demo/Module/run().",
                occurrences: 4,
                relationships: 0,
            },
            SymBp {
                raw: "scip-python pip demo 1.0 demo/Module/Demo#greet().",
                occurrences: 1,
                relationships: 1,
            },
        ],
    );
    let summary = summarize(Lang::Python, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
