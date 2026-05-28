//! Daemon IO adapters. One external technology per location: `ipc` owns the
//! `interprocess` local-socket transport and the filesystem/process glue that
//! drives the daemon lifecycle; `codec` owns the postcard wire framing
//! [src: docs/folder-layout.md rule 4].

pub(crate) mod codec;
pub mod ipc;
