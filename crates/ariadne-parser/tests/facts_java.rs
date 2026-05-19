//! Tier-03 step 10: Java syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_java_sample() {
    let facts = common::facts_for(Lang::Java, "java/Sample.java");
    insta::assert_debug_snapshot!(facts);
}
