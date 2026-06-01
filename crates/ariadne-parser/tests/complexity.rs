//! Tier-12: `McCabe` cyclomatic-complexity goldens.
//!
//! Each language fixture is hand-counted as `decisions + 1` for every
//! function-like decl (Function / Method / Component) and `0` for everything
//! else. Decision points follow the per-`Lang` predicate in
//! `adapters/treesitter/complexity.rs` (control-flow nodes + switch/match arms
//! + ternary + catch + `&&`/`||`, strict `McCabe` — plan RD8 / tier-12 D2).
//!
//! The suite covers, across the ten languages, a branchy case, a
//! nested-function case (Rust/Python/JS/TS, the only langs that capture a
//! function-like decl inside another), and a boolean-operator case
//! [src: .claude/plans/post-v1-roadmap/tier-12-cyclomatic-complexity.md
//! `exit_criteria`].

use std::collections::HashMap;

use ariadne_core::Lang;
use ariadne_parser::{ParserRegistry, extract_syntactic_facts, parse_file};

/// Parse `src` as `lang` and return a `name -> complexity` map over every
/// captured decl. Panics on a parse error so a malformed fixture fails loudly
/// rather than silently under-counting.
fn complexities(lang: Lang, src: &str) -> HashMap<String, u32> {
    let registry = ParserRegistry::new();
    let parsed = parse_file(lang, &registry, src.as_bytes(), None, &[]).expect("parse ok");
    assert!(
        !parsed.host.1.root_node().has_error(),
        "fixture for {lang:?} produced a tree-sitter parse error",
    );
    let facts = extract_syntactic_facts(&parsed, src.as_bytes()).expect("facts");
    facts
        .decls
        .iter()
        .map(|d| (d.name.clone(), d.complexity))
        .collect()
}

/// Complexity of the single decl named `name` (panics when absent).
fn cx(map: &HashMap<String, u32>, name: &str) -> u32 {
    *map.get(name)
        .unwrap_or_else(|| panic!("decl {name} not captured"))
}

#[test]
fn rust_complexity() {
    let src = r"
struct Point { x: i32 }

fn branchy(a: i32, b: i32) -> i32 {
    if a > 0 && b > 0 {
        if a > b {
            return a;
        }
    }
    b
}

fn boolean(a: bool, b: bool, c: bool) -> bool {
    a && b || c
}

fn outer(x: i32) -> i32 {
    fn inner(y: i32) -> i32 {
        if y > 0 { 1 } else { 0 }
    }
    if x > 0 { inner(x) } else { 0 }
}
";
    let m = complexities(Lang::Rust, src);
    // Non-function decl carries 0.
    assert_eq!(cx(&m, "Point"), 0, "struct is not function-like");
    // if(1) + &&(1) + if(1) = 3 decisions -> 4.
    assert_eq!(cx(&m, "branchy"), 4);
    // (a && b) || c: ||(1) + &&(1) = 2 -> 3.
    assert_eq!(cx(&m, "boolean"), 3);
    // outer keeps only its own if(1) -> 2; inner's if is attributed to inner.
    assert_eq!(
        cx(&m, "outer"),
        2,
        "nested fn's decision must not inflate parent"
    );
    assert_eq!(cx(&m, "inner"), 2);
}

#[test]
fn python_complexity() {
    let src = r"
def branchy(a, b):
    if a > 0 and b > 0:
        if a > b:
            return a
    for i in range(b):
        pass
    return b

def boolean(a, b, c):
    return a and b or c

def outer(x):
    def inner(y):
        if y > 0:
            return 1
        return 0
    if x > 0:
        return inner(x)
    return 0
";
    let m = complexities(Lang::Python, src);
    // if(1) + and(1) + if(1) + for(1) = 4 -> 5.
    assert_eq!(cx(&m, "branchy"), 5);
    // (a and b) or c: or(1) + and(1) = 2 -> 3.
    assert_eq!(cx(&m, "boolean"), 3);
    assert_eq!(
        cx(&m, "outer"),
        2,
        "nested def's decision must not inflate parent"
    );
    assert_eq!(cx(&m, "inner"), 2);
}

#[test]
fn javascript_complexity() {
    let src = r"
function branchy(a, b) {
    if (a > 0 && b > 0) {
        if (a > b) {
            return a;
        }
    }
    for (let i = 0; i < b; i++) {}
    return b;
}

function boolean(a, b, c) {
    return a && b || c;
}

function outer(x) {
    function inner(y) {
        if (y > 0) { return 1; }
        return 0;
    }
    return x > 0 ? inner(x) : 0;
}
";
    let m = complexities(Lang::JavaScript, src);
    // if(1) + &&(1) + if(1) + for(1) = 4 -> 5.
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "boolean"), 3);
    // outer: ternary(1) -> 2; inner's if attributed to inner.
    assert_eq!(cx(&m, "outer"), 2);
    assert_eq!(cx(&m, "inner"), 2);
}

#[test]
fn typescript_complexity() {
    let src = r"
function branchy(a: number, b: number): number {
    if (a > 0 && b > 0) {
        while (a > b) {
            return a;
        }
    }
    return b;
}

function boolean(a: boolean, b: boolean, c: boolean): boolean {
    return a && b || c;
}

function outer(x: number): number {
    function inner(y: number): number {
        if (y > 0) { return 1; }
        return 0;
    }
    return x > 0 ? inner(x) : 0;
}
";
    let m = complexities(Lang::TypeScript, src);
    // if(1) + &&(1) + while(1) = 3 -> 4.
    assert_eq!(cx(&m, "branchy"), 4);
    assert_eq!(cx(&m, "boolean"), 3);
    assert_eq!(cx(&m, "outer"), 2);
    assert_eq!(cx(&m, "inner"), 2);
}

#[test]
fn go_complexity() {
    let src = r"
package sample

func branchy(a int, b int) int {
	if a > 0 && b > 0 {
		if a > b {
			return a
		}
	}
	for i := 0; i < b; i++ {
	}
	return b
}

func boolean(a bool, b bool, c bool) bool {
	return a && b || c
}

func selector(x int) int {
	switch x {
	case 1:
		return 1
	case 2:
		return 2
	}
	return 0
}
";
    let m = complexities(Lang::Go, src);
    // if(1) + &&(1) + if(1) + for(1) = 4 -> 5.
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "boolean"), 3);
    // two expression_case arms (default excluded) = 2 -> 3.
    assert_eq!(cx(&m, "selector"), 3);
}

#[test]
fn java_complexity() {
    let src = r"
class Sample {
    int branchy(int a, int b) {
        if (a > 0 && b > 0) {
            if (a > b) {
                return a;
            }
        }
        for (int i = 0; i < b; i++) {}
        return b;
    }

    boolean bool3(boolean a, boolean b, boolean c) {
        return a && b || c;
    }

    int guarded(int x) {
        try {
            return x;
        } catch (RuntimeException e) {
            return 0;
        }
    }
}
";
    let m = complexities(Lang::Java, src);
    assert_eq!(cx(&m, "Sample"), 0, "class is not function-like");
    // if(1) + &&(1) + if(1) + for(1) = 4 -> 5.
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "bool3"), 3);
    // catch_clause(1) = 1 -> 2.
    assert_eq!(cx(&m, "guarded"), 2);
}

#[test]
fn csharp_complexity() {
    let src = r"
class Sample
{
    int Branchy(int a, int b)
    {
        if (a > 0 && b > 0)
        {
            if (a > b)
            {
                return a;
            }
        }
        for (int i = 0; i < b; i++) { }
        return b;
    }

    bool Bool3(bool a, bool b, bool c)
    {
        return a && b || c;
    }

    int Guarded(int x)
    {
        try { return x; }
        catch (System.Exception) { return 0; }
    }
}
";
    let m = complexities(Lang::CSharp, src);
    assert_eq!(cx(&m, "Branchy"), 5);
    assert_eq!(cx(&m, "Bool3"), 3);
    assert_eq!(cx(&m, "Guarded"), 2);
}

#[test]
fn kotlin_complexity() {
    let src = r#"
class Sample {
    fun branchy(a: Int, b: Int): Int {
        if (a > 0 && b > 0) {
            if (a > b) {
                return a
            }
        }
        for (i in 0 until b) {
        }
        return b
    }

    fun bool3(a: Boolean, b: Boolean, c: Boolean): Boolean {
        return a && b || c
    }

    fun whenly(x: Int) {
        when (x) {
            1 -> println("one")
            2 -> println("two")
        }
    }
}
"#;
    let m = complexities(Lang::Kotlin, src);
    // if(1) + &&(1) + if(1) + for(1) = 4 -> 5.
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "bool3"), 3);
    // two when_entry arms (no else) = 2 -> 3.
    assert_eq!(cx(&m, "whenly"), 3);
}

#[test]
fn c_complexity() {
    let src = r"
int branchy(int a, int b) {
    if (a > 0 && b > 0) {
        if (a > b) {
            return a;
        }
    }
    for (int i = 0; i < b; i++) {}
    return b;
}

int boolean(int a, int b, int c) {
    return (a && b) || c;
}

int selector(int x) {
    switch (x) {
        case 1: return 1;
        case 2: return 2;
    }
    return 0;
}
";
    let m = complexities(Lang::C, src);
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "boolean"), 3);
    // two case_statement arms = 2 -> 3.
    assert_eq!(cx(&m, "selector"), 3);
}

#[test]
fn cpp_complexity() {
    let src = r"
int branchy(int a, int b) {
    if (a > 0 && b > 0) {
        if (a > b) {
            return a;
        }
    }
    for (int i = 0; i < b; i++) {}
    return b;
}

bool boolean(bool a, bool b, bool c) {
    return (a && b) || c;
}

int guarded(int x) {
    try {
        return x;
    } catch (...) {
        return 0;
    }
}
";
    let m = complexities(Lang::Cpp, src);
    assert_eq!(cx(&m, "branchy"), 5);
    assert_eq!(cx(&m, "boolean"), 3);
    // catch_clause(1) = 1 -> 2.
    assert_eq!(cx(&m, "guarded"), 2);
}
