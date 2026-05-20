//! `ariadne` CLI entrypoint. Tier-10 wires subcommands; tier-04 adds the
//! `debug mem` stub the plan's verification step asks for \[src:
//! .claude/plans/ariadne-core/tier-04-salsa.md `<verification>` line 3].
//!
//! The `anyhow` dependency is declared at the crate level (per ADR-0001 /
//! folder-layout rule 5: `anyhow` is only permitted inside `ariadne-cli`
//! and `ariadne-e2e`) so later tiers do not need to amend the manifest.

mod domain;
mod errors;

use std::process::ExitCode;

use ariadne_salsa::{AriadneDb, TABLE_BUDGET_BYTES};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    let second = args.next();
    match (first.as_deref(), second.as_deref()) {
        (Some("debug"), Some("mem")) => debug_mem(),
        (Some("watch"), _) => watch_stub(),
        _ => {
            let pkg_version = env!("CARGO_PKG_VERSION");
            println!("ariadne {pkg_version} — stub binary; tier-10 wires real commands");
            ExitCode::SUCCESS
        }
    }
}

fn watch_stub() -> ExitCode {
    // tier-06 ships only the stub; tier-10 wires the full pipeline
    // (ariadne_watcher::NotifyWatcher::start + AriadneDbSink).
    println!("ariadne watch — tier-06 stub; full pipeline arrives in tier-10");
    ExitCode::SUCCESS
}

fn debug_mem() -> ExitCode {
    let db = AriadneDb::new();
    let report = db.memory_report();
    println!("salsa table\testimated_bytes");
    for (name, bytes) in &report.tables {
        println!("{name}\t{bytes}");
    }
    println!("total\t{}", report.total_bytes());

    let over: Vec<_> = report.over_budget().collect();
    if over.is_empty() {
        println!("ok: no table exceeds the {TABLE_BUDGET_BYTES} byte per-table budget");
        ExitCode::SUCCESS
    } else {
        for (name, bytes) in over {
            eprintln!("error: table {name} is {bytes} bytes (over {TABLE_BUDGET_BYTES})");
        }
        ExitCode::FAILURE
    }
}
