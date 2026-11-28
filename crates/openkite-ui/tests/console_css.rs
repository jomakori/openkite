//! console.css is generated: the three reference sheets are the edit surface and
//! `build_console_css.py` concatenates them in document order. Hand-editing the
//! generated file is drift, and drift used to ship: the glass layer landed in
//! console.css alone, so one generator run deleted it.
//!
//! These tests run the generator, so CI fails on the next hand-edit instead of
//! discovering it weeks later.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("build_console_css.py")
}

fn generate(args: &[&str]) -> Output {
    Command::new("python3")
        .arg(script())
        .args(args)
        .output()
        .expect("run build_console_css.py with python3")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Stub sheets plus a committed stylesheet, so the generator's verdicts can be
/// exercised without copying the real 167 KB of reference CSS.
fn fixture(name: &str, committed: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&root);
    let sheets = root.join("sheets");
    std::fs::create_dir_all(&sheets).expect("create fixture dir");
    std::fs::write(sheets.join("openkite-console.css"), ":root{--a:1}\n").expect("write sheet");
    std::fs::write(sheets.join("openkite-workloads.css"), "\n").expect("write sheet");
    std::fs::write(sheets.join("openkite-charts.css"), "\n").expect("write sheet");
    let out = root.join("console.css");
    std::fs::write(&out, committed).expect("write committed stylesheet");
    (sheets, out, root)
}

/// The committed stylesheet reproduces exactly from the reference sheets.
#[test]
fn the_committed_stylesheet_reproduces_from_the_reference_sheets() {
    let output = generate(&["--check"]);
    assert!(
        output.status.success(),
        "console.css is out of sync with docs/reference/opendesign-console/*.css.\n\
         Edit the reference sheets, then run:\n  \
         python3 crates/openkite-ui/assets/build_console_css.py\n\n{}",
        stderr(&output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("in sync"), "{stdout}");
}

/// A hand-edit of the generated file is drift, and the check says so.
#[test]
fn a_hand_edited_stylesheet_fails_the_check() {
    let (sheets, out, _root) =
        fixture("console_css_hand_edit", ":root{--a:1}\n.stale{color:red}\n");
    let output = generate(&[
        "--check",
        "--ref",
        sheets.to_str().expect("utf-8 path"),
        "--out",
        out.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("out of sync"),
        "{}",
        stderr(&output)
    );
}

/// Regenerating must never cost a custom property that only the shipped file
/// defines - that is how the glass tokens disappeared.
#[test]
fn regeneration_refuses_to_drop_a_token_only_the_output_defines() {
    let (sheets, out, _root) =
        fixture("console_css_token_drop", ":root{--a:1;--glass-alpha:.56}\n");
    let args = [
        "--check",
        "--ref",
        sheets.to_str().expect("utf-8 path"),
        "--out",
        out.to_str().expect("utf-8 path"),
    ];
    let output = generate(&args);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("--glass-alpha"),
        "{}",
        stderr(&output)
    );

    // The escape hatch: an intentional removal is declared, not silent.
    let mut allowed = args.to_vec();
    allowed.push("--allow-drop=--glass-alpha");
    let output = generate(&allowed);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(!stderr(&output).contains("refusing"), "{}", stderr(&output));
}
