//! tier-08 incrementality: applying a random sequence of edits / creates /
//! deletes through the `LiveEngine` yields a warm catalog identical to one
//! built fresh from the committed storage (divergence 0). This mirrors the
//! v1 plan `<verification>` divergence=0 invariant at the warm-graph layer:
//! `apply_changeset` must be a faithful mirror of `WriteTxn::apply` +
//! `WarmCatalog::build` [src: .claude/plans/post-v1-roadmap/tier-08-daemon-watcher-live.md
//! step 6; plan.md RD6/RD12].

use ariadne_core::Invalidation;
use ariadne_daemon::{CatalogDump, LiveEngine};
use proptest::prelude::*;

/// One slot of a small fixed corpus. Each op targets a slot so a sequence
/// exercises create → edit → delete → re-create churn on stable paths.
#[derive(Debug, Clone)]
enum Op {
    /// Write `variant` content into slot `slot` and apply a `Modified` event.
    Set { slot: usize, variant: u8 },
    /// Remove slot `slot` from disk and apply a `Removed` event.
    Del { slot: usize },
}

const SLOTS: usize = 3;

/// Content variants for a slot. They differ in symbol set and call edges so a
/// transition between variants drives symbol churn and edge re-resolution. A
/// callee defined in another slot exercises cross-file edge resolution.
fn content_for(slot: usize, variant: u8) -> String {
    match variant % 4 {
        0 => format!("fn a{slot}() {{}}\n"),
        1 => format!("fn a{slot}() {{}}\nfn b{slot}() {{ a{slot}(); }}\n"),
        2 => format!("fn b{slot}() {{ a{slot}(); shared(); }}\n"),
        _ => format!("fn shared() {{}}\nfn a{slot}() {{ shared(); }}\n"),
    }
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..SLOTS, any::<u8>()).prop_map(|(slot, variant)| Op::Set { slot, variant }),
        (0..SLOTS).prop_map(|slot| Op::Del { slot }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn warm_apply_equals_fresh_rebuild(ops in prop::collection::vec(op_strategy(), 1..24)) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join(".ariadne")).expect("create .ariadne");

        let mut engine = LiveEngine::start(&root).expect("start engine on empty index");

        for op in &ops {
            match op {
                Op::Set { slot, variant } => {
                    let path = root.join(format!("f{slot}.rs"));
                    std::fs::write(&path, content_for(*slot, *variant)).expect("write slot");
                    engine
                        .apply(&Invalidation::Modified { path })
                        .expect("apply set");
                }
                Op::Del { slot } => {
                    let path = root.join(format!("f{slot}.rs"));
                    let _ = std::fs::remove_file(&path);
                    engine
                        .apply(&Invalidation::Removed { path })
                        .expect("apply del");
                }
            }
        }

        let fresh = CatalogDump::from_storage(&root).expect("fresh dump");
        prop_assert_eq!(engine.dump(), fresh);
    }
}
