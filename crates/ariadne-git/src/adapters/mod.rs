//! Driven-adapter implementations. One location per external tech; `gix` is
//! the only backend (`adapters/gix/`, split into `mod.rs` for the full walk +
//! `incremental.rs` for the tier-11a watermarked walk to stay inside the
//! authoring cap) [src: docs/folder-layout.md rule 4].

pub mod gix;
