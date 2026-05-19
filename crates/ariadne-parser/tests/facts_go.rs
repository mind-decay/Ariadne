//! Tier-03 step 10: Go syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_go_sample() {
    let facts = common::facts_for(Lang::Go, "go/sample.go");
    insta::assert_debug_snapshot!(facts);
}
