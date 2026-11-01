//! CLI behavior: in-place writes, -l listing, -d diffs, stdin mode, and
//! rendered validation errors. Exercises the real binary.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const CROISSANT: &str = include_str!("../../culina-core/tests/fixtures/croissant.culina");
const NONCANONICAL: &str = ">> title: T\n>> servings: 1\n\n\n\n:mix => _dough\n\n\n";

fn tmp(tag: &str, content: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "culinafmt-test-{}-{tag}.culina",
        std::process::id()
    ));
    std::fs::write(&path, content).expect("write temp file");
    path
}

/// Run culinafmt with args and optional stdin; returns (exit code, stdout, stderr).
fn run(args: &[&str], stdin: Option<&str>) -> (Option<i32>, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_culinafmt"));
    cmd.args(args);
    if stdin.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn culinafmt");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .expect("stdin piped")
            .write_all(input.as_bytes())
            .expect("write stdin");
    }
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn canonical_fixture_is_untouched_in_place() {
    let path = tmp("canonical", CROISSANT);
    let (code, out, err) = run(&[path.to_str().unwrap()], None);
    assert_eq!(code, Some(0), "stdout: {out}, stderr: {err}");
    // No stamp added, no churn — the fixture is already canonical.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), CROISSANT);
}

#[test]
fn list_then_write_then_list_clean() {
    let path = tmp("list-write", NONCANONICAL);

    // -l reports it and does not write.
    let (code, out, _) = run(&["-l", path.to_str().unwrap()], None);
    assert_eq!(code, Some(0));
    assert!(
        out.contains(path.to_str().unwrap()),
        "lists the path: {out}"
    );
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        NONCANONICAL,
        "-l must not write"
    );

    // Default mode writes in place, with a stamp (the file has metadata).
    let (code, _, _) = run(&[path.to_str().unwrap()], None);
    assert_eq!(code, Some(0));
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains(">> modified: "), "stamped: {written}");
    assert!(!written.contains("\n\n\n"), "blanks collapsed: {written}");

    // Now canonical: -l is silent.
    let (code, out, _) = run(&["-l", path.to_str().unwrap()], None);
    assert_eq!(code, Some(0));
    assert!(out.is_empty(), "clean file lists nothing: {out}");
}

#[test]
fn diff_mode_prints_unified_diff_and_does_not_write() {
    let path = tmp("diff", NONCANONICAL);
    let (code, out, _) = run(&["-d", path.to_str().unwrap()], None);
    assert_eq!(code, Some(0));
    assert!(out.contains("--- "), "diff header: {out}");
    assert!(out.contains("@@"), "hunk header: {out}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        NONCANONICAL,
        "-d must not write"
    );
}

#[test]
fn stdin_to_stdout_canonicalizes_without_stamping() {
    let (code, out, _) = run(&[], Some(">> title: T\n\n\n:mix => _dough\n"));
    assert_eq!(code, Some(0));
    assert_eq!(out, ">> title: T\n\n:mix => _dough\n");
    assert!(!out.contains("modified"), "stdin never stamps: {out}");
}

#[test]
fn validation_error_renders_and_fails() {
    let bad = ">> title: T\n>> servings: 1\n\n:knead _ghost\n";
    let (code, out, err) = run(&[], Some(bad));
    assert_eq!(code, Some(1));
    assert!(out.is_empty());
    assert!(err.contains("never created"), "rendered: {err}");
    assert!(err.contains("'ghost'"), "names the thread: {err}");
}

#[test]
fn unknown_meta_key_warns_but_formats() {
    let path = tmp("warn", ">> title: T\n>> servngs: 6\n\n:mix => _dough\n");
    let (code, _, err) = run(&[path.to_str().unwrap()], None);
    assert_eq!(code, Some(0), "warnings never fail: {err}");
    assert!(err.contains("warning"), "{err}");
    assert!(err.contains("servngs"), "{err}");
    assert!(err.contains("line 2"), "{err}");
    // The opaque value survives formatting.
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains(">> servngs: 6"));
}

#[test]
fn version_flag() {
    let (code, out, _) = run(&["--version"], None);
    assert_eq!(code, Some(0));
    assert!(out.starts_with("culinafmt 0."), "{out}");
}

#[test]
fn lexicon_flag_applies_a_collection_overlay() {
    // The embedded core works as a valid (no-op) overlay — same shape,
    // same parse.
    let core =
        std::fs::read_to_string("../../crates/culina-core/lexicons/lexicon.en.toml").unwrap();
    let overlay = tmp("overlay", &core);
    // Scruffy (extra blanks) so the run has something to fix and stamp.
    let recipe = tmp(
        "overlay-recipe",
        ">> title: T\n>> servings: 1\n\n\n\n:mix => _dough\n",
    );
    let (code, _, err) = run(
        &[
            "--lexicon",
            overlay.to_str().unwrap(),
            recipe.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(code, Some(0), "overlay path must not fail: {err}");
    assert!(std::fs::read_to_string(&recipe)
        .unwrap()
        .contains(">> modified: "));
}

#[test]
fn lexicon_flag_missing_file_fails_loudly() {
    let (code, _, err) = run(&["--lexicon", "/nonexistent/lex.toml"], None);
    assert_eq!(code, Some(1));
    assert!(err.contains("lexicon"), "{err}");
}

#[test]
fn lexicon_flag_needs_a_path() {
    let (code, _, err) = run(&["--lexicon"], None);
    assert_eq!(code, Some(1));
    assert!(err.contains("needs a path"), "{err}");
}
