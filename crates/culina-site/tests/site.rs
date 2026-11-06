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
    assert!(!page.contains("culina-source"), "source is opt-in: {page}");
    assert!(
        !page.contains("culina-site-footer"),
        "footer fields unset, no footer: {page}"
    );
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

// --- Page chrome: source and footer (design.md, Collection config) ---

#[test]
fn source_on_embeds_the_recipe_text_escaped() {
    let dir = tmp_collection("source");
    std::fs::write(dir.join(".culina.toml"), "[site]\nsource = true\n").unwrap();
    std::fs::write(
        dir.join("a.culina"),
        ">> title: A\n>> servings: 1\n\n:mix @{butter|50 g} & @{salt|1 pinch} => _dough\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let page = std::fs::read_to_string(out.join("a.html")).unwrap();
    assert!(
        page.contains(
            "<details class=\"culina-source\">\n<summary>View source</summary>\n<pre class=\"culina-source-text\">"
        ),
        "{page}"
    );
    // The text is escaped, otherwise byte-exact. The entity is built at
    // runtime — a literal one in this source would arrive decoded.
    let amp = char::from_u32(0x26).unwrap();
    assert!(
        page.contains(&format!(
            ":mix @{{butter|50 g}} {amp}amp; @{{salt|1 pinch}} => _dough"
        )),
        "{page}"
    );
    // Page chrome, not card content: the block follows the article.
    let article = page.find("</article>").expect("the card");
    let details = page
        .find("<details class=\"culina-source\">")
        .expect("source block");
    assert!(article < details, "{page}");
    // The index lists recipes; it carries no source of its own.
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!index.contains("culina-source"), "{index}");
}

#[test]
fn source_link_rides_in_the_footer_with_the_recipes_path() {
    let dir = tmp_collection("sourcelink");
    std::fs::create_dir_all(dir.join("recipes/soups")).unwrap();
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\nsource = true\nsource_link = \"https://github.com/tlin/culina/blob/main/{path}\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("recipes/soups/wine.culina"),
        ">> title: Mulled wine\n>> servings: 2\n\n:warm @{wine|4 verre} with @{water|1 splash}.\n",
    )
    .unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    let page = std::fs::read_to_string(out.join("wine.html")).unwrap();
    // {path} becomes the recipe's path from the collection root, and
    // the words are that path by default — a name beside the repo name.
    assert!(
        page.contains(
            "<span class=\"culina-source-link\"><a href=\"https://github.com/tlin/culina/blob/main/recipes/soups/wine.culina\">recipes/soups/wine.culina</a></span>"
        ),
        "{page}"
    );
    // The link rides in the footer, clear of the source block.
    let details = page.find("</details>").expect("source block closes");
    let footer = page
        .find("<footer class=\"culina-site-footer\">")
        .expect("footer");
    let link = page
        .find("<span class=\"culina-source-link\">")
        .expect("repo link");
    assert!(details < footer && footer < link, "{page}");
    // The index has no one recipe to point at, so its footer has none.
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!index.contains("culina-source-link"), "{index}");
}

#[test]
fn source_link_text_sets_the_words() {
    for (text, says) in [
        ("source_link_text = \"Edit {path}\"\n", "Edit a.culina"),
        (
            "source_link_text = \"Improve this recipe\"\n",
            "Improve this recipe",
        ),
    ] {
        let dir = tmp_collection("sourcetext");
        std::fs::write(
            dir.join(".culina.toml"),
            format!(
                "[site]\nsource_link = \"https://gitlab.com/tlin/culina/-/blob/main/{{path}}\"\n{text}"
            ),
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
        assert_eq!(code, Some(0), "{text}: {stderr}");
        let page = std::fs::read_to_string(out.join("a.html")).unwrap();
        assert!(
            page.contains(&format!(
                "<span class=\"culina-source-link\"><a href=\"https://gitlab.com/tlin/culina/-/blob/main/a.culina\">{says}</a></span>"
            )),
            "{text}: {page}"
        );
        // No source = true: the link still brings the footer with it.
        assert!(
            !page.contains("<details"),
            "no embedded text without source = true: {page}"
        );
        let footer = page
            .find("<footer class=\"culina-site-footer\">")
            .expect("the link rides in a footer");
        let link = page
            .find("<span class=\"culina-source-link\">")
            .expect("repo link");
        let closes = page.find("</footer>").expect("footer closes");
        assert!(footer < link && link < closes, "{text}: {page}");
    }
}

#[test]
fn a_source_link_without_the_placeholder_fails_loudly() {
    let dir = tmp_collection("badlink");
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\nsource_link = \"https://github.com/tlin/culina/blob/main\"\n",
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
    assert_eq!(
        code,
        Some(1),
        "a template without {{path}} fails the build: {stderr}"
    );
    assert!(stderr.contains("has no {path}"), "{stderr}");
}

#[test]
fn repository_and_footer_fill_a_footer_on_every_page() {
    let dir = tmp_collection("footer");
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\nrepository = \"https://github.com/tlin/recipebox\"\nfooter = \"Cooked & photographed by Ted\"\nsource_link = \"https://github.com/tlin/recipebox/blob/main/{path}\"\n",
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
    assert_eq!(code, Some(0), "{stderr}");
    let amp = char::from_u32(0x26).unwrap();
    for name in ["index.html", "a.html"] {
        let page = std::fs::read_to_string(out.join(name)).unwrap();
        assert!(
            page.contains("<footer class=\"culina-site-footer\">"),
            "{name}: {page}"
        );
        assert!(
            page.contains("<a class=\"culina-repo\" href=\"https://github.com/tlin/recipebox\">"),
            "{name}: {page}"
        );
        // The GitHub mark rides the page, not a stand-in, at an intrinsic
        // 16px so a collection's own sheet can skip the icon rules.
        assert!(page.contains("M12 .297c-6.63"), "{name}: {page}");
        assert!(page.contains("width='16' height='16'"), "{name}: {page}");
        // The owner/repo tail is the link text; the footer text escapes.
        assert!(
            page.contains("<span class=\"culina-repo-text\">tlin/recipebox</span>"),
            "{name}: {page}"
        );
        assert!(
            page.contains(&format!("Cooked {amp}amp; photographed by Ted")),
            "{name}: {page}"
        );
    }
    // On a recipe page the recipe link sits beside the repository link,
    // and the footer text follows; the index carries no recipe link.
    let page = std::fs::read_to_string(out.join("a.html")).unwrap();
    let repo = page
        .find("<a class=\"culina-repo\"")
        .expect("repository link");
    let link = page
        .find("<span class=\"culina-source-link\">")
        .expect("recipe link");
    let text = page
        .find("<span class=\"culina-footer-text\">")
        .expect("footer text");
    assert!(repo < link && link < text, "{page}");
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!index.contains("culina-source-link"), "{index}");
}

#[test]
fn an_unknown_host_gets_the_plain_link_icon_and_the_whole_url() {
    let dir = tmp_collection("plainicon");
    std::fs::write(
        dir.join(".culina.toml"),
        "[site]\nrepository = \"https://git.example.org\"\n",
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
    assert_eq!(code, Some(0), "{stderr}");
    let page = std::fs::read_to_string(out.join("a.html")).unwrap();
    assert!(page.contains("culina-repo-icon"), "{page}");
    assert!(!page.contains("M12 .297c-6.63"), "no GitHub mark: {page}");
    assert!(!page.contains("m23.6004 9.5927"), "no GitLab mark: {page}");
    // No owner/repo tail to show, so the whole URL is the text.
    assert!(
        page.contains("<span class=\"culina-repo-text\">https://git.example.org</span>"),
        "{page}"
    );
}

// --- Discovery (README, culina-site) ---

#[test]
fn recipes_are_found_at_any_depth_but_not_in_hidden_dirs() {
    let dir = tmp_collection("depth");
    std::fs::create_dir_all(dir.join("recipes/soups")).unwrap();
    std::fs::create_dir_all(dir.join(".culina-src/fixtures")).unwrap();
    std::fs::write(dir.join("recipes/soups/red-curry.culina"), CURRY).unwrap();
    std::fs::write(dir.join(".culina-src/fixtures/stray.culina"), CURRY).unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(out.join("red-curry.html").exists(), "nested recipe found");
    assert!(!out.join("stray.html").exists(), "hidden dir skipped");
}

#[test]
fn two_recipes_with_one_page_name_fail_loudly() {
    let dir = tmp_collection("collide");
    std::fs::create_dir_all(dir.join("a")).unwrap();
    std::fs::create_dir_all(dir.join("b")).unwrap();
    std::fs::write(dir.join("a/curry.culina"), CURRY).unwrap();
    std::fs::write(dir.join("b/curry.culina"), CURRY).unwrap();
    let out = dir.join("site");
    let (code, _, stderr) = run(&[
        "--in",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("both become curry.html — rename one"),
        "{stderr}"
    );
}
