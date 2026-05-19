//! Tier-03 step 10: Python syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_python_sample() {
    let facts = common::facts_for(Lang::Python, "python/sample.py");
    insta::assert_debug_snapshot!(facts);
}
