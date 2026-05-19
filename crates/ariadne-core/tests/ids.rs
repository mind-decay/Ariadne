//! Round-trip and ordering invariants for the stable id types
//! [src: .claude/plans/ariadne-core/tier-01-workspace.md step 7].

use std::cmp::Ordering;

use ariadne_core::{EdgeId, FileId, IdEncode, Span, SymbolId};
use proptest::prelude::*;

proptest! {
    #[test]
    fn file_id_round_trip(value in 1u32..=u32::MAX) {
        let id = FileId::new(value).expect("non-zero by construction");
        prop_assert_eq!(FileId::from_bytes(id.to_bytes()), Some(id));
    }

    #[test]
    fn symbol_id_round_trip(value in 1u64..=u64::MAX) {
        let id = SymbolId::new(value).expect("non-zero by construction");
        prop_assert_eq!(SymbolId::from_bytes(id.to_bytes()), Some(id));
    }

    #[test]
    fn edge_id_round_trip(value in 1u64..=u64::MAX) {
        let id = EdgeId::new(value).expect("non-zero by construction");
        prop_assert_eq!(EdgeId::from_bytes(id.to_bytes()), Some(id));
    }

    #[test]
    fn span_ordering_is_total_and_transitive(
        a_file in 1u32..1_000_000,
        a_start in 0u32..100_000,
        a_len in 0u32..1_000,
        b_file in 1u32..1_000_000,
        b_start in 0u32..100_000,
        b_len in 0u32..1_000,
        c_file in 1u32..1_000_000,
        c_start in 0u32..100_000,
        c_len in 0u32..1_000,
    ) {
        let a = span(a_file, a_start, a_len);
        let b = span(b_file, b_start, b_len);
        let c = span(c_file, c_start, c_len);

        // Totality: every pair yields a concrete Ordering.
        let _: Ordering = a.cmp(&b);
        let _: Ordering = b.cmp(&c);
        let _: Ordering = a.cmp(&c);

        // Transitivity: a <= b && b <= c => a <= c.
        if a <= b && b <= c {
            prop_assert!(a <= c);
        }
        // And the antisymmetric mirror.
        if a >= b && b >= c {
            prop_assert!(a >= c);
        }
    }
}

#[test]
fn file_id_rejects_zero() {
    assert!(FileId::new(0).is_none());
    assert!(FileId::from_bytes([0; 8]).is_none());
    // High bytes must be zero — populating them invalidates the encoding.
    assert!(FileId::from_bytes([0, 0, 0, 1, 0, 0, 0, 1]).is_none());
}

#[test]
fn symbol_id_rejects_zero() {
    assert!(SymbolId::new(0).is_none());
    assert!(SymbolId::from_bytes([0; 8]).is_none());
}

fn span(file: u32, byte_start: u32, len: u32) -> Span {
    Span {
        file: FileId::new(file).expect("non-zero by construction"),
        byte_start,
        byte_end: byte_start.saturating_add(len),
    }
}
