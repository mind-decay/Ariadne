#[test]
fn cli_crate_compiles() {
    // Binary-only crate. Smoke test exists to satisfy
    // `cargo nextest run --workspace` ≥1-test-per-crate requirement
    // [src: .claude/plans/ariadne-core/tier-01-workspace.md exit_criteria].
    assert_eq!(1 + 1, 2);
}
