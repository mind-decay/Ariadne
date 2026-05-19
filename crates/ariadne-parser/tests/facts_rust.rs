//! Tier-03 step 10: Rust syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_rust_sample() {
    let facts = common::facts_for(Lang::Rust, "rust/sample.rs");
    insta::assert_debug_snapshot!(facts);
}
