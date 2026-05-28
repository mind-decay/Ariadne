//! Architecture invariant test.
//!
//! Asserts that the workspace dependency graph honors the hexagonal boundary
//! fixed by ADR-0001:
//!
//! 1. `ariadne-core` has zero in-workspace dependencies.
//! 2. `ariadne-graph` and `ariadne-salsa` depend only on `ariadne-core` and
//!    `ariadne-storage` (read-only port).
//! 3. Driving adapters (`ariadne-mcp`, `ariadne-watcher`, `ariadne-daemon`)
//!    are not depended on by any crate except the composition root
//!    `ariadne-cli`; the composition root itself is depended on by nothing
//!    (ADR-0007).
//! 4. Driven adapters (`ariadne-storage`, `ariadne-parser`, `ariadne-scip`)
//!    depend only on `ariadne-core`; never on each other.
//!
//! This test is committed in a deliberately failing state during tier-00.
//! Tier-01 introduces the Cargo workspace, the `cargo_metadata` dependency,
//! and the crates above — at which point this test becomes the gate.
//!
//! Sources:
//! - [src: docs/adr/0001-architecture-style.md]
//! - [src: docs/folder-layout.md]
//! - [src: .claude/plans/ariadne-core/tier-00-foundations.md step 1]

use std::collections::{BTreeMap, BTreeSet};

use cargo_metadata::MetadataCommand;

const DOMAIN_INTERIOR: &str = "ariadne-core";

/// Crates that may depend on `ariadne-core` + (read-only) `ariadne-storage`.
const USE_CASE_CRATES: &[&str] = &["ariadne-graph", "ariadne-salsa"];

/// Allowed in-workspace deps for use-case crates.
const USE_CASE_ALLOWED_DEPS: &[&str] = &["ariadne-core", "ariadne-storage"];

/// Driven adapters: may depend only on `ariadne-core`.
const DRIVEN_ADAPTERS: &[&str] = &["ariadne-storage", "ariadne-parser", "ariadne-scip"];

/// Composition root: wires every adapter together; nothing may depend on it,
/// and it alone may depend on a driving adapter [src: docs/adr/0007-cli-composition-root.md].
const COMPOSITION_ROOT: &str = "ariadne-cli";

/// Pure driving adapters: nothing may depend on them except the composition
/// root [src: docs/adr/0007-cli-composition-root.md]. `ariadne-daemon` is the
/// long-running daemon host introduced by post-v1 tier-06 (RD5); it is also a
/// driving adapter and is wired only by the composition root
/// [src: docs/adr/0015-daemon-mode-ipc.md].
const DRIVING_ADAPTERS: &[&str] = &["ariadne-mcp", "ariadne-watcher", "ariadne-daemon"];

#[test]
fn architecture_invariants_hold() {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("cargo metadata must succeed inside the workspace");

    let workspace_members: BTreeSet<String> = metadata
        .workspace_packages()
        .into_iter()
        .map(|pkg| pkg.name.to_string())
        .collect();

    let deps_by_crate: BTreeMap<String, BTreeSet<String>> = metadata
        .workspace_packages()
        .into_iter()
        .map(|pkg| {
            let in_workspace: BTreeSet<String> = pkg
                .dependencies
                .iter()
                .map(|d| d.name.clone())
                .filter(|n| workspace_members.contains(n))
                .collect();
            (pkg.name.to_string(), in_workspace)
        })
        .collect();

    // (1) ariadne-core is hermetic.
    let core_deps = deps_by_crate
        .get(DOMAIN_INTERIOR)
        .expect("ariadne-core must be a workspace member");
    assert!(
        core_deps.is_empty(),
        "ariadne-core must have zero in-workspace dependencies, found: {core_deps:?}",
    );

    // (2) Use-case crates may depend only on core (+ storage).
    let allowed: BTreeSet<&str> = USE_CASE_ALLOWED_DEPS.iter().copied().collect();
    for name in USE_CASE_CRATES {
        let Some(deps) = deps_by_crate.get(*name) else {
            continue;
        };
        for dep in deps {
            assert!(
                allowed.contains(dep.as_str()),
                "{name} may depend only on {USE_CASE_ALLOWED_DEPS:?}; found {dep}",
            );
        }
    }

    // (3) Driven adapters may depend only on ariadne-core.
    for name in DRIVEN_ADAPTERS {
        let Some(deps) = deps_by_crate.get(*name) else {
            continue;
        };
        for dep in deps {
            assert_eq!(
                dep, DOMAIN_INTERIOR,
                "driven adapter {name} may depend only on {DOMAIN_INTERIOR}; found {dep}",
            );
        }
    }

    // (4) Driving-adapter containment (ADR-0007):
    //   - nothing may depend on the composition root `ariadne-cli`;
    //   - nothing may depend on a pure driving adapter except the
    //     composition root, which exists precisely to wire them.
    let driving: BTreeSet<&str> = DRIVING_ADAPTERS.iter().copied().collect();
    for (name, deps) in &deps_by_crate {
        for dep in deps {
            assert_ne!(
                dep, COMPOSITION_ROOT,
                "{name} must not depend on the composition root {COMPOSITION_ROOT}",
            );
            if driving.contains(dep.as_str()) {
                assert_eq!(
                    name, COMPOSITION_ROOT,
                    "{name} must not depend on driving adapter {dep}; only the \
                     composition root {COMPOSITION_ROOT} may (ADR-0007)",
                );
            }
        }
    }
}
