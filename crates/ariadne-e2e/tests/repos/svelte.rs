//! End-to-end suite: Svelte fixture (`sveltejs/realworld`) — component graph.
//!
//! `#[ignore]` — shallow-clones a real OSS repo (network + hundreds of MB).
//! Run explicitly: `cargo nextest run -p ariadne-e2e --run-ignored all`
//! [src: .claude/plans/js-framework-support/tier-09-component-graph-e2e.md step 5].

use ariadne_e2e::domain::verify_framework_fixture;
use tempfile::tempdir;

#[test]
#[ignore = "clones a real OSS repo; run via --run-ignored"]
fn svelte_fixture_has_component_graph() {
    let dir = tempdir().expect("create tempdir");
    verify_framework_fixture("svelte", dir.path());
}
