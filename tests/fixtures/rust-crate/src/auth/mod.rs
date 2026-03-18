pub mod login;

use crate::utils::format;

pub fn auth_version() -> &'static str {
    let _ = format::format_date("2026-03-18");
    "1.0.0"
}
