//! Tier-03 step 10: C# syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_csharp_sample() {
    let facts = common::facts_for(Lang::CSharp, "csharp/Sample.cs");
    insta::assert_debug_snapshot!(facts);
}
