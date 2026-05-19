//! Per-language ingest golden for C# (scip-dotnet).
//!
//! Plan ref: `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 15.

mod common;

use ariadne_core::Lang;

use crate::common::{SymBp, summarize, synth_bytes};

#[test]
fn ingest_csharp_summary() {
    let bytes = synth_bytes(
        "scip-dotnet",
        "Demo/Demo.cs",
        "CSharp",
        &[
            SymBp {
                raw: "scip-dotnet nuget Demo 1.0 Demo/Service#",
                occurrences: 5,
                relationships: 2,
            },
            SymBp {
                raw: "scip-dotnet nuget Demo 1.0 Demo/Service#Run().",
                occurrences: 3,
                relationships: 1,
            },
            SymBp {
                raw: "scip-dotnet nuget Demo 1.0 Demo/Service#Field.",
                occurrences: 2,
                relationships: 0,
            },
        ],
    );
    let summary = summarize(Lang::CSharp, &bytes).expect("synth bytes must parse");
    insta::assert_snapshot!(summary);
}
