//! `ariadne` CLI entrypoint. Tier-10 wires subcommands; tier-01 ships a
//! stub that prints the build identity so `cargo run` succeeds.
//!
//! The `anyhow` dependency is declared at the crate level (per ADR-0001 /
//! folder-layout rule 5: `anyhow` is only permitted inside `ariadne-cli`
//! and `ariadne-e2e`) so later tiers do not need to amend the manifest.

mod domain;
mod errors;

fn main() {
    println!(
        "ariadne {} — stub binary; tier-10 wires real commands",
        env!("CARGO_PKG_VERSION"),
    );
}
