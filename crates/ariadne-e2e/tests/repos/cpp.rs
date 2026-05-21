//! End-to-end suite: C++ fixture (`nlohmann/json`) against the full stack.
//!
//! `#[ignore]` — shallow-clones a real OSS repo (network + tens of MB).
//! Run explicitly: `cargo nextest run -p ariadne-e2e --run-ignored all`
//! [src: .claude/plans/ariadne-core/tier-12-parallel-cold-index.md step 7].

use ariadne_e2e::domain::verify_fixture_index;
use tempfile::tempdir;

#[test]
#[ignore = "clones a real OSS repo; run via --run-ignored"]
fn cpp_fixture_indexes_within_slo() {
    let dir = tempdir().expect("create tempdir");
    verify_fixture_index("cpp", dir.path());
}
