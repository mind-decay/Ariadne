//! End-to-end suite: C fixture (`torvalds/linux`) against the full stack.
//!
//! `#[ignore]` — shallow-clones the Linux kernel (network + ~1.4 GB checkout).
//! Run explicitly: `cargo nextest run -p ariadne-e2e --run-ignored all`
//! [src: .claude/plans/ariadne-core/tier-12-parallel-cold-index.md step 7].

use ariadne_e2e::domain::verify_fixture_index;
use tempfile::tempdir;

#[test]
#[ignore = "clones the Linux kernel; run via --run-ignored"]
fn c_fixture_indexes_within_slo() {
    let dir = tempdir().expect("create tempdir");
    verify_fixture_index("c", dir.path());
}
