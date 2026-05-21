//! Round-trip invariants for `Lang` string tags and `EdgeKind` byte tags
//! [src: .claude/plans/js-framework-support/tier-01-domain.md steps 1, 7].

use ariadne_core::{EdgeKey, EdgeKind, Lang, SymbolId};

#[test]
fn lang_framework_tags_round_trip() {
    // Each JS-framework variant added in tier-01 must survive `tag` → `from_tag`.
    for (lang, tag) in [
        (Lang::Tsx, "tsx"),
        (Lang::Vue, "vue"),
        (Lang::Svelte, "svelte"),
        (Lang::Astro, "astro"),
    ] {
        assert_eq!(lang.tag(), tag, "{lang:?} encodes to its tag");
        assert_eq!(
            Lang::from_tag(tag),
            Some(lang),
            "{tag:?} decodes to {lang:?}"
        );
    }
}

#[test]
fn edge_kind_component_variants_round_trip() {
    // `EdgeKind`'s on-wire form is the single-byte tag consumed by
    // `EdgeKey`'s fixed-width key encoding [src: records.rs `to_byte`/`from_byte`].
    for kind in [EdgeKind::Renders, EdgeKind::UsesHook] {
        assert_eq!(
            EdgeKind::from_byte(kind.to_byte()),
            Some(kind),
            "{kind:?} round-trips through its byte tag",
        );
    }
}

#[test]
fn edge_key_carries_component_edge_kinds() {
    // The component-graph edge kinds must also survive the composite
    // `EdgeKey` key encoding so they persist through the `EDGES` table.
    let src = SymbolId::new(7).expect("non-zero");
    let dst = SymbolId::new(42).expect("non-zero");
    for kind in [EdgeKind::Renders, EdgeKind::UsesHook] {
        let key = EdgeKey { src, kind, dst };
        assert_eq!(EdgeKey::from_bytes(&key.to_bytes()), Some(key));
    }
}
