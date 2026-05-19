//! Determinism + canonicalization proptest for `normalize_scip_symbol`.
//!
//! Strategy: generate 1024 random *legal* SCIP symbol strings, parse each
//! 10 times → identical `SymbolId`; then build an "escaped-identifier"
//! variant (every simple-identifier name re-wrapped in backticks) and
//! confirm both forms hash equal. Plan ref:
//! `.claude/plans/ariadne-core/tier-05-scip-ingest.md` step 14.

use ariadne_scip::normalize_scip_symbol;
use proptest::prelude::*;

const ITERATIONS_PER_CASE: usize = 10;

const IDENT_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_+-$";

fn ident_string() -> impl Strategy<Value = String> {
    proptest::collection::vec(0usize..IDENT_CHARS.len(), 1..16).prop_map(|indices| {
        indices
            .into_iter()
            .map(|i| IDENT_CHARS[i] as char)
            .collect()
    })
}

#[derive(Debug, Clone)]
enum DescriptorKind {
    Namespace,
    Type,
    Term,
    Method,
    TypeParameter,
    Parameter,
    Meta,
    Macro,
}

fn descriptor_kind() -> impl Strategy<Value = DescriptorKind> {
    prop_oneof![
        Just(DescriptorKind::Namespace),
        Just(DescriptorKind::Type),
        Just(DescriptorKind::Term),
        Just(DescriptorKind::Method),
        Just(DescriptorKind::TypeParameter),
        Just(DescriptorKind::Parameter),
        Just(DescriptorKind::Meta),
        Just(DescriptorKind::Macro),
    ]
}

fn encode_descriptor(
    name: &str,
    disambig: Option<&str>,
    kind: &DescriptorKind,
    escaped: bool,
) -> String {
    let n: String = if escaped {
        format!("`{name}`")
    } else {
        name.to_owned()
    };
    match kind {
        DescriptorKind::Namespace => format!("{n}/"),
        DescriptorKind::Type => format!("{n}#"),
        DescriptorKind::Term => format!("{n}."),
        DescriptorKind::Meta => format!("{n}:"),
        DescriptorKind::Macro => format!("{n}!"),
        DescriptorKind::Method => {
            let d = disambig.unwrap_or("");
            format!("{n}({d}).")
        }
        DescriptorKind::TypeParameter => format!("[{n}]"),
        DescriptorKind::Parameter => format!("({n})"),
    }
}

#[derive(Debug, Clone)]
struct DescPart {
    name: String,
    disambig: Option<String>,
    kind: DescriptorKind,
}

fn descriptor_part() -> impl Strategy<Value = DescPart> {
    (
        ident_string(),
        prop::option::of(ident_string()),
        descriptor_kind(),
    )
        .prop_map(|(name, disambig, kind)| DescPart {
            name,
            disambig,
            kind,
        })
}

fn scheme_string() -> impl Strategy<Value = String> {
    // Prefix every scheme with `s-` so the proptest can never accidentally
    // produce the reserved `local` token (which the parser rightly
    // rejects).
    ident_string().prop_map(|tail| format!("s-{tail}"))
}

fn symbol_parts() -> impl Strategy<Value = (String, String, String, String, Vec<DescPart>)> {
    (
        scheme_string(),
        ident_string(),
        ident_string(),
        ident_string(),
        proptest::collection::vec(descriptor_part(), 1..5),
    )
}

fn render(parts: &(String, String, String, String, Vec<DescPart>), escaped: bool) -> String {
    let (scheme, manager, package, version, descs) = parts;
    let mut out = String::new();
    out.push_str(scheme);
    out.push(' ');
    out.push_str(manager);
    out.push(' ');
    out.push_str(package);
    out.push(' ');
    out.push_str(version);
    out.push(' ');
    for d in descs {
        out.push_str(&encode_descriptor(
            &d.name,
            d.disambig.as_deref(),
            &d.kind,
            escaped,
        ));
    }
    out
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 1024,
        ..ProptestConfig::default()
    })]

    /// Determinism: parsing the same string ten times yields the same id.
    #[test]
    fn normalize_is_deterministic(parts in symbol_parts()) {
        let raw = render(&parts, false);
        let first = normalize_scip_symbol(&raw)
            .expect("generated symbols must parse")
            .id();
        for _ in 0..ITERATIONS_PER_CASE {
            let next = normalize_scip_symbol(&raw).unwrap().id();
            prop_assert_eq!(next, first);
        }
    }

    /// Equivalent forms hash equal: simple-identifier `foo` and the
    /// escaped form `` `foo` `` carry the same canonical name.
    #[test]
    fn plain_and_escaped_forms_match(parts in symbol_parts()) {
        let raw_plain = render(&parts, false);
        let raw_escaped = render(&parts, true);
        let plain = normalize_scip_symbol(&raw_plain)
            .expect("plain form must parse")
            .id();
        let escaped = normalize_scip_symbol(&raw_escaped)
            .expect("escaped form must parse")
            .id();
        prop_assert_eq!(plain, escaped);
    }
}
