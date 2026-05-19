//! End-to-end test harness. Tier-10 fills in OSS-repo runners + perf
//! gates; tier-01 just scaffolds the crate.

#![deny(missing_docs)]

pub mod domain;
pub mod errors;

pub use errors::E2eError;
