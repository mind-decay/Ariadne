//! `ariadne mem` — report salsa per-table memory against the budget.

use std::path::Path;

use ariadne_salsa::{AriadneDb, TABLE_BUDGET_BYTES};

use crate::domain::index_path;

/// Print [`AriadneDb::memory_report`] per table and flag any table over the
/// 256 MiB budget. Returns `false` when a table is over budget so the caller
/// can exit non-zero [src: tier-10 step 9, plan risk R1].
#[must_use]
pub fn run(root: &Path) -> bool {
    let db_path = index_path(root);
    if db_path.exists() {
        println!("index: {}", db_path.display());
    } else {
        println!("index: (none — run `ariadne index`)");
    }

    let db = AriadneDb::new();
    let report = db.memory_report();
    println!("salsa table              estimated_bytes");
    for (name, bytes) in &report.tables {
        println!("  {name:<22} {bytes}");
    }
    println!("  {:<22} {}", "total", report.total_bytes());

    let over: Vec<_> = report.over_budget().collect();
    if over.is_empty() {
        println!("ok: no table exceeds the {TABLE_BUDGET_BYTES}-byte per-table budget");
        true
    } else {
        for (name, bytes) in over {
            eprintln!("error: table {name} is {bytes} bytes (over {TABLE_BUDGET_BYTES})");
        }
        false
    }
}
