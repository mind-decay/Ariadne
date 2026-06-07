//! Per-subcommand implementations. `main.rs` parses the clap tree and
//! dispatches one of these; each module owns exactly one subcommand
//! (src: .claude/plans/ariadne-core/tier-10-cli-e2e.md `<files>`).

pub mod affected_tests;
pub mod api_diff;
pub mod daemon;
pub mod digest;
pub mod doc;
pub mod fitness;
pub mod index;
pub mod init;
pub mod mem;
pub mod outline;
pub mod query;
pub mod serve;
pub mod setup;
pub mod status;
pub mod watch;
