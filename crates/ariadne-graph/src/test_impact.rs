//! Static test-impact reachability (Block A, A1).
//!
//! Answers "which tests does *this change* reach" by the standard call-graph
//! test-impact technique: resolve the changeset's line hunks to the changed-
//! symbol seed set (the same [`crate::span_lines`] machinery [`crate::diff_blast`]
//! uses), reverse-reach each seed over the call/ref edges, and keep the test
//! subset of the seeds ∪ reached set [src:
//! <https://martinfowler.com/articles/rise-test-impact-analysis.html>; block-a
//! plan.md D1]. No coverage ingest — purely the static graph (D1).
//!
//! Test classification ([`classify_test_symbols`]) is a pure function over each
//! symbol's `attributes` + file path + language, never a parser fact (D2): the
//! per-`Lang` table below combines attribute markers (Rust `#[test]`, JVM/C#
//! `@Test`/`[Fact]`) with path conventions (`*_test.go`, `*.test.*`/`*.spec.*`,
//! `test_*.py`, `*Test.java`) — inputs already on every record [src:
//! crates/ariadne-parser/src/adapters/treesitter/facts.rs:99-106;
//! crates/ariadne-daemon/src/domain/catalog.rs:48-53].
//!
//! Pure and deterministic: no clock, no RNG; every output collection is sorted
//! (the `BTreeSet` seed/reach sets and the `intersection` iterate in `SymbolId`
//! order, `unresolved` by path), so re-runs are byte-identical.

use std::collections::BTreeSet;

use ariadne_core::{Lang, LineHunk, SymbolId};

use crate::build::{EdgeKindSet, GraphIndex};
use crate::span_lines::{FileSymbolSpans, changed_symbols};

/// One symbol's classification input for [`classify_test_symbols`]: the id plus
/// the three fields the per-`Lang` table reads — its declaration `attributes`,
/// its defining file `path`, and free-form `kind`/`name`. Borrowed so a caller
/// projects it straight off a catalog's per-symbol metadata with no allocation.
#[derive(Debug, Clone, Copy)]
pub struct TestRootInput<'a> {
    /// The symbol being classified.
    pub id: SymbolId,
    /// Language of the defining file.
    pub lang: Lang,
    /// Project-root-relative defining file path.
    pub path: &'a str,
    /// Free-form kind tag (`function`, `method`, …).
    pub kind: &'a str,
    /// Canonical symbol name.
    pub name: &'a str,
    /// Attribute / annotation / decorator identifiers on the declaration.
    pub attributes: &'a [String],
}

/// Result of [`GraphIndex::affected_tests`].
///
/// `tests` is the test subset of the changed seeds ∪ their reverse-reachable
/// dependents — the tests a change can affect. `seeds` lists the changed
/// symbols the hunks resolved to. `unresolved` are changed paths that owned no
/// seed symbol (new, binary, or deleted files). All three are sorted, so the
/// report is byte-identical across runs on the same inputs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AffectedTestsReport {
    /// Affected test symbols, sorted by `SymbolId`.
    pub tests: Vec<SymbolId>,
    /// Changed-symbol seeds, sorted by `SymbolId`.
    pub seeds: Vec<SymbolId>,
    /// Changed paths that resolved to no symbol seed, sorted.
    pub unresolved: Vec<String>,
}

/// Classify each input symbol as a test root or not, returning the set of test
/// roots (sorted, by `BTreeSet` iteration). A symbol is a test root when its
/// language's convention matches — an attribute marker and/or a path/name
/// convention (see the module docs for the per-`Lang` table). Pure: a function
/// of the inputs alone, so the warm projection and the cold path agree.
#[must_use]
pub fn classify_test_symbols<'a, I>(roots: I) -> BTreeSet<SymbolId>
where
    I: IntoIterator<Item = TestRootInput<'a>>,
{
    roots
        .into_iter()
        .filter(is_test_root)
        .map(|r| r.id)
        .collect()
}

/// Whether one symbol is a test root under its language's convention.
fn is_test_root(r: &TestRootInput<'_>) -> bool {
    match r.lang {
        // Rust: the `#[test]` attribute (also `#[tokio::test]` etc. — the last
        // path segment) [src: https://doc.rust-lang.org/reference/attributes/testing.html].
        Lang::Rust => attr_marks(r.attributes, &["test"]),
        // Go: a `*_test.go` file whose function name starts `Test`/`Benchmark`/
        // `Fuzz` [src: https://pkg.go.dev/testing].
        Lang::Go => go_test_file(r.path) && name_starts_any(r.name, &["Test", "Benchmark", "Fuzz"]),
        // JVM: JUnit/Kotlin `@Test`, or a `*Test.{java,kt}` file / `src/test/`
        // tree [src: https://junit.org/junit5/docs/current/user-guide/].
        Lang::Java | Lang::Kotlin => {
            attr_marks(r.attributes, &["Test", "Fact"]) || jvm_test_path(r.path)
        }
        // C#: xUnit `[Fact]`/`[Theory]`, NUnit/MSTest `[Test]`, or a `*Test(s).cs`
        // file [src: https://learn.microsoft.com/dotnet/core/testing/].
        Lang::CSharp => {
            attr_marks(r.attributes, &["Test", "Fact", "Theory"]) || dotnet_test_path(r.path)
        }
        // Python (pytest) and C (Unity/CMocka): a `test_*` / `*_test` file, or a
        // callable named `test*`
        // [src: https://docs.pytest.org/en/stable/explanation/goodpractices.html;
        //  https://github.com/ThrowTheSwitch/Unity].
        Lang::Python | Lang::C => {
            snake_test_file(r.path) || (callable(r.kind) && r.name.starts_with("test"))
        }
        // C++: same file convention, or a callable named `test*`/`Test*`/`TEST*`
        // (GoogleTest macros expand to such names) [src: https://google.github.io/googletest/].
        Lang::Cpp => {
            snake_test_file(r.path)
                || (callable(r.kind) && name_starts_any(r.name, &["test", "Test", "TEST"]))
        }
        // JS/TS + the SFC frameworks: a `*.test.*` / `*.spec.*` file (Jest/Vitest
        // convention) [src: https://jestjs.io/docs/configuration#testmatch-arraystring].
        Lang::TypeScript
        | Lang::Tsx
        | Lang::JavaScript
        | Lang::Vue
        | Lang::Svelte
        | Lang::Astro => js_test_file(r.path),
        // `Lang` is `#[non_exhaustive]`; an unclassified language is not a test.
        _ => false,
    }
}

/// Whether `kind` is a callable symbol (the name-convention gate).
fn callable(kind: &str) -> bool {
    matches!(kind, "function" | "method")
}

/// Whether any attribute's last `::`/`.`-segment matches a marker (case-insensitive),
/// so `#[test]`, `#[tokio::test]`, and `@org.junit.jupiter.api.Test` all hit.
fn attr_marks(attrs: &[String], markers: &[&str]) -> bool {
    attrs.iter().any(|a| {
        let seg = a.rsplit([':', '.']).next().unwrap_or(a);
        markers.iter().any(|m| seg.eq_ignore_ascii_case(m))
    })
}

/// Whether `name` starts with any of `prefixes`.
fn name_starts_any(name: &str, prefixes: &[&str]) -> bool {
    prefixes.iter().any(|p| name.starts_with(p))
}

/// The file-name component of a `/`-separated path.
fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The stem (everything before the last `.`) of a file name.
fn stem(file: &str) -> &str {
    file.rsplit_once('.').map_or(file, |(s, _)| s)
}

/// `*_test.go`.
fn go_test_file(path: &str) -> bool {
    path.ends_with("_test.go")
}

/// `*.test.*` or `*.spec.*`.
fn js_test_file(path: &str) -> bool {
    let f = file_name(path);
    f.contains(".test.") || f.contains(".spec.")
}

/// A `test_*` or `*_test` file — the `snake_case` test-file convention shared by
/// Python (`test_*.py` / `*_test.py`), C, and C++. The caller's `Lang` already
/// pins the extension, so only the stem pattern is checked here.
fn snake_test_file(path: &str) -> bool {
    let f = file_name(path);
    f.starts_with("test_") || stem(f).ends_with("_test")
}

/// A `*Test.{java,kt}` file or a symbol under a `src/test/` tree.
fn jvm_test_path(path: &str) -> bool {
    path.contains("src/test/") || stem(file_name(path)).ends_with("Test")
}

/// A `*Test.cs` / `*Tests.cs` file.
fn dotnet_test_path(path: &str) -> bool {
    let s = stem(file_name(path));
    s.ends_with("Test") || s.ends_with("Tests")
}

impl GraphIndex {
    /// Compute the test symbols a changeset reaches.
    ///
    /// `symbol_lines` carries the indexed symbol spans per file; `hunks` are the
    /// changeset's new-side changed line ranges and `changed_paths` its full
    /// changed-path list (both from the `ariadne-git` adapter). Each hunk
    /// resolves to the seed symbols whose defining span covers it; each seed is
    /// reverse-reached at `depth`/`kinds` (calls collapse onto the `CALLS` flag),
    /// and the result is the test subset (`test_roots ∩ (seeds ∪ reached)`).
    /// `test_roots` is the precomputed set from [`classify_test_symbols`]. A
    /// changed path owning no seed is reported in
    /// [`AffectedTestsReport::unresolved`].
    // Each input is a distinct, plan-mandated facet of the query — the indexed
    // spans, the changeset's hunks + changed-path list, the precomputed test
    // roots, and the v1 blast-radius depth/kind filter — mirroring the shape
    // `diff_blast` is allowed (tier-14).
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn affected_tests(
        &self,
        symbol_lines: &[FileSymbolSpans],
        hunks: &[LineHunk],
        changed_paths: &[String],
        test_roots: &BTreeSet<SymbolId>,
        depth: u8,
        kinds: EdgeKindSet,
    ) -> AffectedTestsReport {
        // Seed set: symbols whose indexed span covers a changed line.
        let seed_set = changed_symbols(symbol_lines, hunks);

        // Reachable closure: the seeds plus their reverse-reachable dependents
        // (the callers a change ripples out to). A seed absent from the graph
        // contributes nothing rather than dropping the seed.
        let mut reached: BTreeSet<SymbolId> = seed_set.clone();
        for &symbol in &seed_set {
            let radius = self.blast_radius(symbol, depth, kinds).unwrap_or_default();
            reached.extend(radius.must_touch);
            reached.extend(radius.may_touch);
        }

        // The answer is the test subset of the reachable closure. `intersection`
        // over two `BTreeSet`s iterates in `SymbolId` order, so `tests` is sorted.
        let tests: Vec<SymbolId> = reached.intersection(test_roots).copied().collect();
        let seeds: Vec<SymbolId> = seed_set.iter().copied().collect();

        // A changed path is resolved when it owns at least one seed; everything
        // else (new / binary / deleted, or a change in no symbol's span) is an
        // unresolved-impact entry.
        let resolved_paths: BTreeSet<&str> = symbol_lines
            .iter()
            .filter(|file| file.symbols.iter().any(|(id, _, _)| seed_set.contains(id)))
            .map(|file| file.path.as_str())
            .collect();
        let mut unresolved: Vec<String> = changed_paths
            .iter()
            .filter(|path| !resolved_paths.contains(path.as_str()))
            .cloned()
            .collect();
        unresolved.sort();
        unresolved.dedup();

        AffectedTestsReport {
            tests,
            seeds,
            unresolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EdgeKind;

    fn sid(n: u64) -> SymbolId {
        SymbolId::new(n).expect("nonzero symbol id")
    }

    /// The classifier marks a known test symbol in every supported language —
    /// one representative case per `Lang` grammar, covering both the attribute-
    /// marker and the path/name-convention arms (block-a plan.md BR2).
    #[test]
    fn classifies_a_test_symbol_in_every_language() {
        let test = vec!["test".to_owned()];
        let jtest = vec!["Test".to_owned()];
        let fact = vec!["Fact".to_owned()];
        let none: Vec<String> = Vec::new();
        let check = |lang, path: &str, kind: &str, name: &str, attrs: &[String]| {
            let r = TestRootInput {
                id: sid(1),
                lang,
                path,
                kind,
                name,
                attributes: attrs,
            };
            assert!(
                !classify_test_symbols([r]).is_empty(),
                "{lang:?} test symbol `{name}` ({path}) was not classified as a test",
            );
        };
        // One representative case per `Lang` grammar — attribute-marker and
        // path/name-convention arms both covered (block-a plan.md BR2).
        check(Lang::Rust, "src/lib.rs", "function", "checks_add", &test);
        check(Lang::Go, "pkg/math_test.go", "function", "TestAdd", &none);
        check(Lang::Java, "src/FooTest.java", "method", "ok", &jtest);
        check(Lang::Kotlin, "src/FooTest.kt", "method", "ok", &jtest);
        check(Lang::CSharp, "tests/CalcTests.cs", "method", "Adds", &fact);
        check(
            Lang::Python,
            "tests/test_math.py",
            "function",
            "test_add",
            &none,
        );
        check(Lang::C, "src/math_test.c", "function", "test_add", &none);
        check(Lang::Cpp, "src/vec_test.cpp", "function", "TestAdd", &none);
        check(
            Lang::TypeScript,
            "src/calc.test.ts",
            "function",
            "adds",
            &none,
        );
        check(
            Lang::Tsx,
            "src/Widget.spec.tsx",
            "function",
            "renders",
            &none,
        );
        check(
            Lang::JavaScript,
            "src/util.test.js",
            "function",
            "adds",
            &none,
        );
        check(
            Lang::Vue,
            "src/Widget.spec.vue",
            "function",
            "renders",
            &none,
        );
        check(Lang::Svelte, "src/App.test.svelte", "function", "x", &none);
        check(Lang::Astro, "src/Page.spec.astro", "function", "x", &none);
    }

    /// A non-test symbol (no marker, no convention) classifies as not-a-test —
    /// the negative half of the contract.
    #[test]
    fn does_not_classify_plain_symbols() {
        let none: Vec<String> = Vec::new();
        let plain = classify_test_symbols([
            TestRootInput {
                id: sid(1),
                lang: Lang::Rust,
                path: "src/lib.rs",
                kind: "function",
                name: "add",
                attributes: &none,
            },
            TestRootInput {
                id: sid(2),
                lang: Lang::Python,
                path: "src/app.py",
                kind: "function",
                name: "compute",
                attributes: &none,
            },
            TestRootInput {
                id: sid(3),
                lang: Lang::TypeScript,
                path: "src/calc.ts",
                kind: "function",
                name: "add",
                attributes: &none,
            },
        ]);
        assert!(plain.is_empty(), "plain symbols must not classify as tests");
    }

    /// Synthetic graph: a test `T` (sid 2) and a non-test caller `U` (sid 3) both
    /// call the changed symbol `S` (sid 1). `affected_tests` over a hunk inside
    /// `S` returns exactly `{T}` — the test ancestor — and excludes the non-test
    /// ancestor `U`.
    #[test]
    fn affected_tests_returns_only_the_test_ancestor() {
        let mut g = GraphIndex::new();
        // T → S and U → S (callers of S).
        g.add_edge(sid(2), sid(1), EdgeKind::Calls);
        g.add_edge(sid(3), sid(1), EdgeKind::Calls);

        // One file: S on line 1, T on line 3, U on line 5.
        let spans = vec![FileSymbolSpans {
            path: "src/lib.rs".to_owned(),
            line_starts: vec![0, 11, 21, 31, 41],
            symbols: vec![(sid(1), 0, 10), (sid(2), 21, 30), (sid(3), 41, 50)],
        }];
        let hunks = vec![LineHunk {
            path: "src/lib.rs".to_owned(),
            start_line: 1,
            end_line: 1,
        }];
        let changed_paths = vec!["src/lib.rs".to_owned()];

        // `T` is a test (Rust `#[test]`); `S` and `U` are not.
        let test_attr = vec!["test".to_owned()];
        let none: Vec<String> = Vec::new();
        let test_roots = classify_test_symbols([
            TestRootInput {
                id: sid(1),
                lang: Lang::Rust,
                path: "src/lib.rs",
                kind: "function",
                name: "subject",
                attributes: &none,
            },
            TestRootInput {
                id: sid(2),
                lang: Lang::Rust,
                path: "src/lib.rs",
                kind: "function",
                name: "checks",
                attributes: &test_attr,
            },
            TestRootInput {
                id: sid(3),
                lang: Lang::Rust,
                path: "src/lib.rs",
                kind: "function",
                name: "caller",
                attributes: &none,
            },
        ]);
        assert_eq!(test_roots, BTreeSet::from([sid(2)]));

        let report = g.affected_tests(
            &spans,
            &hunks,
            &changed_paths,
            &test_roots,
            5,
            EdgeKindSet::ALL,
        );

        assert_eq!(report.seeds, vec![sid(1)], "the hunk resolves to S");
        assert_eq!(
            report.tests,
            vec![sid(2)],
            "only the test ancestor T is affected; the non-test caller U is excluded",
        );
        assert!(
            report.unresolved.is_empty(),
            "the changed file owns a seed, so nothing is unresolved",
        );

        // Determinism: a re-run is identical.
        let again = g.affected_tests(
            &spans,
            &hunks,
            &changed_paths,
            &test_roots,
            5,
            EdgeKindSet::ALL,
        );
        assert_eq!(report, again, "affected_tests is deterministic across runs");
    }
}
