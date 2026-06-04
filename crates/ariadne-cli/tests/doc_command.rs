//! `ariadne doc` integration test (tier-06).
//!
//! Indexes a tiny two-crate Rust fixture, runs the real `ariadne doc`
//! subcommand, and asserts it writes the `.md` + sidecar `.svg` pair, rewrites
//! the Markdown image link to the chosen `--svg` basename, is byte-identical on
//! re-run (determinism), and honours `--exclude` by dropping a source row from
//! the rendered output [src: tier-06-cli-doc-command.md `<steps>` 1].

use std::path::Path;
use std::process::Command;

/// Workspace `ariadne` build under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ariadne")
}

/// Source for `crates/alpha/src/lib.rs` — a first-party `Source` module so the
/// crate appears in the overview's Architecture table by default.
const ALPHA_RS: &str = "pub fn helper(value: i32) -> i32 {\n    value + 1\n}\n";
/// Source for `crates/beta/src/lib.rs`; its cross-file call to `helper` gives
/// the crate graph a real inter-crate edge.
const BETA_RS: &str = "pub fn run() -> i32 {\n    helper(20)\n}\n";

/// Run `<bin> <args...>`; fail unless it exits successfully.
fn run_ok(args: &[&str]) {
    let output = Command::new(bin())
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn `ariadne {}`: {e}", args.join(" ")));
    assert!(
        output.status.success(),
        "`ariadne {}` exited with {}: {}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
}

/// Run `ariadne doc <root> --out <out> --svg <svg> [--exclude <e>]...`.
fn run_doc(root: &Path, out: &Path, svg: &Path, excludes: &[&str]) {
    let mut cmd = Command::new(bin());
    cmd.arg("doc")
        .arg(root)
        .arg("--out")
        .arg(out)
        .arg("--svg")
        .arg(svg);
    for ex in excludes {
        cmd.arg("--exclude").arg(ex);
    }
    let output = cmd.output().expect("spawn `ariadne doc`");
    assert!(
        output.status.success(),
        "`ariadne doc` exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim(),
    );
}

#[test]
fn doc_writes_pair_is_deterministic_and_honours_exclude() {
    let project = tempfile::tempdir().expect("create fixture tempdir");
    let root = project.path();
    let alpha_dir = root.join("crates/alpha/src");
    let beta_dir = root.join("crates/beta/src");
    std::fs::create_dir_all(&alpha_dir).expect("mkdir alpha");
    std::fs::create_dir_all(&beta_dir).expect("mkdir beta");
    std::fs::write(alpha_dir.join("lib.rs"), ALPHA_RS).expect("write alpha lib.rs");
    std::fs::write(beta_dir.join("lib.rs"), BETA_RS).expect("write beta lib.rs");

    run_ok(&["init", root.to_str().expect("utf8 root")]);
    run_ok(&["index", root.to_str().expect("utf8 root")]);

    let out = root.join("o.md");
    let svg = root.join("o.svg");

    // First run writes both files.
    run_doc(root, &out, &svg, &[]);
    assert!(out.exists(), "doc did not write the markdown file");
    assert!(svg.exists(), "doc did not write the sidecar svg");
    let md1 = std::fs::read(&out).expect("read markdown");
    let svg1 = std::fs::read(&svg).expect("read svg");

    let md1_text = String::from_utf8(md1.clone()).expect("markdown is utf8");
    let svg1_text = String::from_utf8(svg1.clone()).expect("svg is utf8");
    assert!(
        svg1_text.contains("<svg"),
        "sidecar is not an SVG document:\n{svg1_text}",
    );
    // The Markdown image link is rewritten to the chosen `--svg` basename so
    // the committed pair is self-contained.
    assert!(
        md1_text.contains("![architecture](o.svg)"),
        "markdown missing the rewritten relative sidecar link:\n{md1_text}",
    );
    // Both source crates appear as Architecture rows by default.
    assert!(
        md1_text.contains("`alpha`"),
        "expected the alpha crate row in the overview:\n{md1_text}",
    );
    assert!(
        md1_text.contains("`beta`"),
        "expected the beta crate row in the overview:\n{md1_text}",
    );

    // Determinism: a second run to the same paths is byte-identical.
    run_doc(root, &out, &svg, &[]);
    assert_eq!(
        md1,
        std::fs::read(&out).expect("re-read markdown"),
        "markdown not byte-identical on re-run",
    );
    assert_eq!(
        svg1,
        std::fs::read(&svg).expect("re-read svg"),
        "svg not byte-identical on re-run",
    );

    // `--exclude alpha` is a substring exclude over `Source` paths: it drops
    // `crates/alpha/...` from the reported output while keeping beta.
    let out_ex = root.join("ex.md");
    let svg_ex = root.join("ex.svg");
    run_doc(root, &out_ex, &svg_ex, &["alpha"]);
    let md_ex = std::fs::read_to_string(&out_ex).expect("read excluded markdown");
    assert!(
        !md_ex.contains("`alpha`"),
        "--exclude alpha did not drop the alpha row:\n{md_ex}",
    );
    assert!(
        md_ex.contains("`beta`"),
        "--exclude alpha wrongly dropped the beta row:\n{md_ex}",
    );
}
