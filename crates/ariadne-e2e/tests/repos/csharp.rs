//! End-to-end suite: C# fixture (`dotnet/runtime`) against the full stack.
//!
//! `#[ignore]` — shallow-clones a real OSS repo (network + multiple GB).
//! Run explicitly: `cargo nextest run -p ariadne-e2e --run-ignored all`
//! [src: .claude/plans/ariadne-core/tier-10-cli-e2e.md step 10].

use ariadne_e2e::domain::verify_fixture_index;
use tempfile::tempdir;

#[test]
#[ignore = "clones a real OSS repo; run via --run-ignored"]
fn csharp_fixture_indexes_within_slo() {
    let dir = tempdir().expect("create tempdir");
    verify_fixture_index("csharp", dir.path());
}
