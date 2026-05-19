//! Tier-03 step 10: JavaScript syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_javascript_sample() {
    let facts = common::facts_for(Lang::JavaScript, "javascript/sample.js");
    insta::assert_debug_snapshot!(facts);
}
