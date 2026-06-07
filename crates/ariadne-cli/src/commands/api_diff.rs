//! `ariadne api-diff <base>..<head>` — the public-API semver verdict (block A,
//! A2).
//!
//! A2 runs entirely in the querying process (D6 / ADR-0027): unlike the warm
//! tools there is no daemon leg. The command splits the `<base>..<head>` spec
//! and calls the same `ariadne_mcp::tools::api_surface_diff::handle` the MCP
//! `api_surface_diff` tool calls — git diff → base/head blob reads → parser
//! surface extraction → pure classify — so the CLI and MCP answers are parity by
//! construction. Exit 0 (informational): the verdict travels in the JSON payload
//! for the Block-B PR-risk bot [src:
//! .claude/plans/intelligence-platform/block-a/plan.md step 5;
//! docs/adr/0027-mcp-parser-dependency.md].

use std::path::Path;

use anyhow::{Context, Result, bail};
use ariadne_mcp::tools;

/// Run the `api-diff <base>..<head>` subcommand: split the spec, run the
/// in-process composition, and print the pretty JSON report.
///
/// # Errors
/// Fails when the spec is not `<base>..<head>`, or when the git diff, a
/// base/head blob read, or a public-surface re-parse errors.
pub fn run(root: &Path, spec: &str) -> Result<()> {
    let (base, head) = parse_spec(spec)?;
    let out = tools::api_surface_diff::handle(root, base, head)
        .with_context(|| format!("classify api surface diff {base}..{head}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&out).context("serialize api-diff output")?
    );
    Ok(())
}

/// Split a `<base>..<head>` spec into its two revspecs. Both sides are required:
/// the verdict is defined between two named refs, so there is no working-tree
/// default (unlike `affected-tests`).
fn parse_spec(spec: &str) -> Result<(&str, &str)> {
    let Some((base, head)) = spec.trim().split_once("..") else {
        bail!("api-diff spec must be `<base>..<head>`, got `{spec}`");
    };
    let (base, head) = (base.trim(), head.trim());
    if base.is_empty() || head.is_empty() {
        bail!("api-diff spec must name both refs as `<base>..<head>`, got `{spec}`");
    }
    Ok((base, head))
}

#[cfg(test)]
mod tests {
    use super::parse_spec;

    #[test]
    fn parses_a_two_ref_range() {
        assert_eq!(parse_spec("HEAD~1..HEAD").unwrap(), ("HEAD~1", "HEAD"));
    }

    #[test]
    fn rejects_a_missing_range_separator() {
        assert!(parse_spec("HEAD").is_err());
    }

    #[test]
    fn rejects_an_empty_side() {
        assert!(parse_spec("..HEAD").is_err());
        assert!(parse_spec("HEAD..").is_err());
    }
}
