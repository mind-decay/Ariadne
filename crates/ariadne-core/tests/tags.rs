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
fn lang_extension_table_resolves() {
    // The canonical extension→`Lang` table (I1 audit follow-up). Both
    // `ariadne_cli::lang_for_path` and the `ariadne-scip` React ingest test
    // resolve through this single map, so a future remap is caught here.
    for (ext, lang) in [
        ("rs", Lang::Rust),
        ("ts", Lang::TypeScript),
        ("mts", Lang::TypeScript),
        ("cts", Lang::TypeScript),
        ("tsx", Lang::Tsx),
        ("js", Lang::JavaScript),
        ("jsx", Lang::JavaScript),
        ("mjs", Lang::JavaScript),
        ("cjs", Lang::JavaScript),
        ("vue", Lang::Vue),
        ("svelte", Lang::Svelte),
        ("astro", Lang::Astro),
        ("py", Lang::Python),
        ("pyi", Lang::Python),
        ("go", Lang::Go),
        ("java", Lang::Java),
        ("kt", Lang::Kotlin),
        ("kts", Lang::Kotlin),
        ("cs", Lang::CSharp),
        ("c", Lang::C),
        ("h", Lang::C),
        ("cpp", Lang::Cpp),
        ("hpp", Lang::Cpp),
    ] {
        assert_eq!(Lang::from_extension(ext), Some(lang), "{ext:?} → {lang:?}");
    }
    assert_eq!(Lang::from_extension("md"), None, "unknown extension → None");
    assert_eq!(
        Lang::from_extension("TSX"),
        None,
        "extension match is case-sensitive, mirroring Path::extension",
    );
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
