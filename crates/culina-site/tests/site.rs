//! culina-site integration: a temp collection renders into a site —
//! config read, lexicon overlay applied, pages and index written.

use std::path::PathBuf;
use std::process::{Command, Stdio};

const CURRY: &str = include_str!("../../culina-core/tests/fixtures/red-curry.culina");

fn tmp_collection(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("culina-site-test-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_culina-site"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn culina-site");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn renders_a_collection_with_config_and_overlay() {
    let dir = tmp_collection("full");
    std::fs::write(
        dir.join("culina.toml"),
        "[site]\ntitle = \"Test Kitchen\"\ndescription = \"rendered by CI\"\n\n[lexicon]\nfile = \"lex.toml\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("lex.toml"),
        "[units.splash]\ndim = \"volume\"\nfactor = 0.005\n",
    )
    .unwrap();
    std::fs::write(dir.join("red-curry.culina"), CURRY).unwrap();
    std::fs::write(
        dir.join("wine.culina"),
        ">> title: Mulled wine\n>> servings: 2\n>> unit: verre = 90 ml\n\n:warm @{wine|4 verre} with @{water|1 splash}.\n",
    )
    .unwrap();

    let out = dir.join("site");
    let (code, stdout, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "stdout: {stdout}, stderr: {stderr}");

    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(index.contains("Test Kitchen"), "{index}");
    assert!(index.contains("red-curry.html"), "{index}");
    assert!(
        index.contains("Mulled wine"),
        "sorted into the index: {index}"
    );

    let curry = std::fs::read_to_string(out.join("red-curry.html")).unwrap();
    assert!(curry.contains("culina-title"), "{curry}");
    assert!(curry.contains("Serves 4"), "{curry}");
    assert!(
        curry.contains("culina.css"),
        "stylesheet referenced: {curry}"
    );

    // The document registry (verre) and the collection overlay (splash)
    // both resolved — the wine page carries the amounts as written.
    let wine = std::fs::read_to_string(out.join("wine.html")).unwrap();
    assert!(wine.contains("4 verre"), "{wine}");
    assert!(wine.contains("1 splash"), "{wine}");

    assert!(out.join("culina.css").exists(), "default CSS written");
}

#[test]
fn broken_recipe_fails_the_build() {
    let dir = tmp_collection("broken");
    std::fs::write(
        dir.join("bad.culina"),
        ">> title: Bad\n>> servings: 1\n\n:knead _ghost\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1), "a bad recipe fails the site build");
    assert!(stderr.contains("never created"), "{stderr}");
}

#[test]
fn custom_css_is_used_instead_of_the_default() {
    let dir = tmp_collection("css");
    std::fs::write(dir.join("culina.toml"), "[site]\ntitle = \"CSS test\"\n").unwrap();
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n\n:mix => _dough\n",
    )
    .unwrap();
    std::fs::write(dir.join("culina.css"), "body { color: hotpink; }\n").unwrap();
    let out = dir.join("site");
    let (code, _, _) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    let css = std::fs::read_to_string(out.join("culina.css")).unwrap();
    assert!(css.contains("hotpink"), "collection CSS wins: {css}");
}

#[test]
fn dotfile_config_drives_the_index() {
    let dir = tmp_collection("dotfile");
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\ntitle = \"Test Kitchen\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n\n:mix => _dough\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "stderr: {stderr}");
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(index.contains("Test Kitchen"), "{index}");
}

#[test]
fn no_config_keeps_the_default_title() {
    let dir = tmp_collection("defaults");
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n\n:mix => _dough\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "no policy file is not an error: {stderr}");
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(index.contains("Recipes"), "default title: {index}");
}

#[test]
fn both_config_names_fail_the_build() {
    let dir = tmp_collection("conflict");
    std::fs::write(dir.join("culina.toml"), "[site]\ntitle = \"Old\"\n").unwrap();
    std::fs::write(dir.join(".culina.toml"), "[site]\ntitle = \"New\"\n").unwrap();
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n\n:mix => _dough\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1), "two policy files conflict: {stderr}");
    assert!(stderr.contains(".culina.toml"), "{stderr}");
    assert!(stderr.contains("remove one"), "{stderr}");
}

#[test]
fn static_dir_is_copied_verbatim() {
    let dir = tmp_collection("static");
    std::fs::create_dir_all(dir.join("static/photos")).unwrap();
    std::fs::write(dir.join("static/photos/top.jpg"), b"jpg").unwrap();
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n>> image: static/photos/top.jpg\n\n:mix => _dough\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, _) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0));
    assert!(out.join("static/photos/top.jpg").exists(), "static copied");
    let page = std::fs::read_to_string(out.join("a.html")).unwrap();
    assert!(
        page.contains("static/photos/top.jpg"),
        "photo referenced verbatim: {page}"
    );
}

// --- The CfE grid layout (design.md, Collection config) ---

#[test]
fn grid_on_puts_the_grid_in_each_page_and_writes_its_sheet() {
    let dir = tmp_collection("grid");
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\ngrid = true\nlayout = \"grid-slant\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("red-curry.culina"), CURRY).unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");

    let page = std::fs::read_to_string(out.join("red-curry.html")).unwrap();
    assert!(
        page.contains("<html lang=\"en\" data-layout=\"grid-slant\">"),
        "{page}"
    );
    assert!(
        page.contains("<link rel=\"stylesheet\" href=\"culina-grid.css\">"),
        "{page}"
    );
    // The card keeps its list and method; the grid follows the method.
    let grid = page
        .find("<section class=\"culina-grid-view\">")
        .expect("grid section");
    let method = page.find("<div class=\"culina-method\"").expect("method");
    let ld = page
        .find("<script type=\"application/ld+json\"")
        .expect("JSON-LD");
    assert!(method < grid && grid < ld, "{page}");
    assert!(page.contains("<ul class=\"culina-ingredients\""), "{page}");
    assert!(out.join("culina-grid.css").exists());
}

#[test]
fn grid_off_leaves_pages_as_cards() {
    let dir = tmp_collection("nogrid");
    std::fs::write(dir.join("red-curry.culina"), CURRY).unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let page = std::fs::read_to_string(out.join("red-curry.html")).unwrap();
    assert!(!page.contains("culina-grid"), "{page}");
    assert!(!page.contains("data-layout"), "{page}");
    assert!(!out.join("culina-grid.css").exists());
}

#[test]
fn a_grid_layout_without_the_grid_fails_loudly() {
    for (toml, says) in [
        ("[site]\nlayout = \"grid\"\n", "add grid = true"),
        (
            "[site]\ngrid = true\nlayout = \"tiles\"\n",
            "use one of card, grid, grid-slant",
        ),
    ] {
        let dir = tmp_collection("badlayout");
        std::fs::write(dir.join(".culina.toml"), toml).unwrap();
        std::fs::write(dir.join("red-curry.culina"), CURRY).unwrap();
        let out = dir.join("site");
        let (code, _, stderr) = run(&[
            "--in",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ]);
        assert_eq!(code, Some(1), "{toml}");
        assert!(stderr.contains(says), "{stderr}");
    }
}
