//! Per-language root classifier — tier-05 RD4.
//!
//! Each fixture seeds one entry-point root and one genuinely dead
//! non-root in the same graph (zero incoming edges on both). The root
//! classifier (`roots::is_root`) reads the tier-04 `SymbolRecord`
//! metadata — visibility, attributes, kind, name — and the file-level
//! `Lang`; `dead_code` then excludes the root set before the fan-in=0
//! filter so the dead-code report names only the non-root symbol.

use std::collections::BTreeSet;

use ariadne_core::{Lang, SymbolId, Visibility};
use ariadne_graph::{DeadCodeConfig, EdgeKind, GraphIndex, roots::is_root};

fn sid(n: u64) -> SymbolId {
    SymbolId::new(n).expect("non-zero")
}

struct Sym {
    id: SymbolId,
    lang: Lang,
    visibility: Visibility,
    attributes: Vec<&'static str>,
    kind: &'static str,
    name: &'static str,
}

fn run(symbols: &[Sym]) -> (BTreeSet<SymbolId>, Vec<SymbolId>) {
    let mut g = GraphIndex::new();
    for s in symbols {
        g.add_symbol(s.id);
    }
    // Drop a single referenced symbol that bumps fan-in for symbols not
    // under test; the root + the dead candidate both stay at fan-in 0.
    let mut roots = BTreeSet::new();
    for s in symbols {
        let attrs: Vec<String> = s.attributes.iter().map(|a| (*a).to_owned()).collect();
        if is_root(s.lang, s.visibility, &attrs, s.kind, s.name) {
            roots.insert(s.id);
        }
    }
    let cfg = DeadCodeConfig {
        roots: roots.clone(),
        ..Default::default()
    };
    let dead: Vec<SymbolId> = g
        .dead_code(&cfg)
        .symbols
        .into_iter()
        .map(|d| d.id)
        .collect();
    (roots, dead)
}

#[test]
fn rust_main_is_root_orphan_is_dead() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::Rust,
            visibility: Visibility::Unknown,
            attributes: vec![],
            kind: "function",
            name: "main",
        },
        Sym {
            id: sid(2),
            lang: Lang::Rust,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "orphan_helper",
        },
    ]);
    assert!(roots.contains(&sid(1)), "Rust fn main must be a root");
    assert!(
        !roots.contains(&sid(2)),
        "private helper must not be a root"
    );
    assert_eq!(dead, vec![sid(2)]);
}

#[test]
fn rust_public_and_test_are_roots() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::Rust,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "function",
            name: "lib_api",
        },
        Sym {
            id: sid(2),
            lang: Lang::Rust,
            visibility: Visibility::Private,
            attributes: vec!["test"],
            kind: "function",
            name: "smoke",
        },
        Sym {
            id: sid(3),
            lang: Lang::Rust,
            visibility: Visibility::Private,
            attributes: vec!["no_mangle"],
            kind: "function",
            name: "ffi_export",
        },
        Sym {
            id: sid(4),
            lang: Lang::Rust,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "dead_helper",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(roots.contains(&sid(3)));
    assert!(!roots.contains(&sid(4)));
    assert_eq!(dead, vec![sid(4)]);
}

#[test]
fn go_exported_and_test_prefix_are_roots() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::Go,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "function",
            name: "Serve",
        },
        Sym {
            id: sid(2),
            lang: Lang::Go,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "TestServe",
        },
        Sym {
            id: sid(3),
            lang: Lang::Go,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "BenchmarkServe",
        },
        Sym {
            id: sid(4),
            lang: Lang::Go,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "main",
        },
        Sym {
            id: sid(5),
            lang: Lang::Go,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "unreferencedHelper",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(roots.contains(&sid(3)));
    assert!(roots.contains(&sid(4)));
    assert!(!roots.contains(&sid(5)));
    assert_eq!(dead, vec![sid(5)]);
}

#[test]
fn python_dunder_main_and_decorated_are_roots() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::Python,
            visibility: Visibility::Unknown,
            attributes: vec![],
            kind: "function",
            name: "__main__",
        },
        Sym {
            id: sid(2),
            lang: Lang::Python,
            visibility: Visibility::Unknown,
            attributes: vec!["pytest.fixture"],
            kind: "function",
            name: "fixture_db",
        },
        Sym {
            id: sid(3),
            lang: Lang::Python,
            visibility: Visibility::Unknown,
            attributes: vec![],
            kind: "function",
            name: "unused",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(!roots.contains(&sid(3)));
    assert_eq!(dead, vec![sid(3)]);
}

#[test]
fn ts_exported_is_root_internal_is_dead() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::TypeScript,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "function",
            name: "createServer",
        },
        Sym {
            id: sid(2),
            lang: Lang::Tsx,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "component",
            name: "App",
        },
        Sym {
            id: sid(3),
            lang: Lang::TypeScript,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "internalUnused",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(!roots.contains(&sid(3)));
    assert_eq!(dead, vec![sid(3)]);
}

#[test]
fn java_public_main_and_test_annotation_are_roots() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::Java,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "method",
            name: "main",
        },
        Sym {
            id: sid(2),
            lang: Lang::Java,
            visibility: Visibility::Private,
            attributes: vec!["Test"],
            kind: "method",
            name: "shouldServe",
        },
        Sym {
            id: sid(3),
            lang: Lang::CSharp,
            visibility: Visibility::Private,
            attributes: vec!["Fact"],
            kind: "method",
            name: "ShouldServe",
        },
        Sym {
            id: sid(4),
            lang: Lang::Java,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "method",
            name: "unreachable",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(roots.contains(&sid(3)));
    assert!(!roots.contains(&sid(4)));
    assert_eq!(dead, vec![sid(4)]);
}

#[test]
fn c_main_and_public_extern_are_roots() {
    let (roots, dead) = run(&[
        Sym {
            id: sid(1),
            lang: Lang::C,
            visibility: Visibility::Unknown,
            attributes: vec![],
            kind: "function",
            name: "main",
        },
        Sym {
            id: sid(2),
            lang: Lang::Cpp,
            visibility: Visibility::Public,
            attributes: vec![],
            kind: "function",
            name: "exported_api",
        },
        Sym {
            id: sid(3),
            lang: Lang::C,
            visibility: Visibility::Private,
            attributes: vec![],
            kind: "function",
            name: "static_helper",
        },
    ]);
    assert!(roots.contains(&sid(1)));
    assert!(roots.contains(&sid(2)));
    assert!(!roots.contains(&sid(3)));
    assert_eq!(dead, vec![sid(3)]);
}

#[test]
fn cycle_among_non_roots_with_orphan_dead() {
    // No edges: fan-in=0 for everyone. The dead-code filter still names
    // only the non-root symbols.
    let mut g = GraphIndex::new();
    for n in 1u64..=4 {
        g.add_symbol(sid(n));
    }
    // Add one edge so sid(2) has fan-in 1 — confirms the classifier is
    // independent of, and composes with, the fan-in test.
    g.add_edge(sid(1), sid(2), EdgeKind::Calls);
    let mut roots = BTreeSet::new();
    // sid(1) is a Rust fn main → root.
    if is_root(Lang::Rust, Visibility::Unknown, &[], "function", "main") {
        roots.insert(sid(1));
    }
    let cfg = DeadCodeConfig {
        roots,
        ..Default::default()
    };
    let dead: Vec<SymbolId> = g
        .dead_code(&cfg)
        .symbols
        .into_iter()
        .map(|d| d.id)
        .collect();
    // sid(1) excluded as root; sid(2) has fan-in 1 → not dead;
    // sid(3) + sid(4) have fan-in 0 → dead.
    assert_eq!(dead, vec![sid(3), sid(4)]);
}
