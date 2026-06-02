//! Per-query handlers, one module per concern. Each function mirrors the
//! matching v1 MCP tool handler so daemon-served results are byte-identical
//! to the cold path, substituting the warm `WarmCatalog` + `WarmSnapshot`
//! for the MCP `Catalog` + redb snapshot
//! [src: .claude/plans/post-v1-roadmap/tier-07-daemon-warm-graph.md step 5].

pub(crate) mod analytics;
pub(crate) mod docs;
pub(crate) mod health;
pub(crate) mod impact;
pub(crate) mod meta;
pub(crate) mod navigate;
pub(crate) mod refactor;
