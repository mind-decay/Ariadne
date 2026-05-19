//! Tier-03 step 10: Kotlin syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_kotlin_sample() {
    let facts = common::facts_for(Lang::Kotlin, "kotlin/sample.kt");
    insta::assert_debug_snapshot!(facts);
}
