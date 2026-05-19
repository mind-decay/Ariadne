//! Tier-03 step 3 + 10: TypeScript syntactic-fact golden snapshot.

mod common;

use ariadne_core::Lang;

#[test]
fn facts_typescript_sample() {
    let facts = common::facts_for(Lang::TypeScript, "typescript/sample.ts");
    insta::assert_debug_snapshot!(facts);
}
