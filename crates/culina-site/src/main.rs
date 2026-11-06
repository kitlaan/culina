//! culina-site — render a collection of `.culina` recipes into a static
//! site: flat HTML pages, an index, and CSS.
//!
//! The recipe pages are culina-core's semantic fragments with stable
//! `culina-*` classes; this tool only assembles pages around them. The
//! collection owns policy through a `.culina.toml` or `culina.toml` at the
//! input root (`[site]` title, description, source, source_link,
//! source_link_text, repository, footer; `[lexicon] file` overlay) and
//! theming through a `culina.css` — present means used, absent means the
//! built-in default. With `[site] grid = true`,
//! each page also carries its Cooking For Engineers grid, which
//! `data-layout` on `<html>` shows.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use culina_core::grid::{grid, html_grid};
use culina_core::render::{card, html_fragment};
use culina_core::{error, fmt};

#[derive(serde::Deserialize, Default)]
struct Config {
    #[serde(default)]
    site: Site,
    #[serde(default)]
    lexicon: Lexicon,
}

#[derive(serde::Deserialize, Default)]
struct Site {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<String>,
    /// Emit each recipe's CfE grid into its page (hidden until a layout
    /// shows it) and write culina-grid.css.
    #[serde(default)]
    grid: bool,
    /// The layout a page opens in: `card` (the default), `grid`, or
    /// `grid-slant` — the card plus its grid. Written as `data-layout` on `<html>`; a collection's
    /// own script may change it.
    #[serde(default)]
    layout: Option<String>,
    /// Put the recipe's own `.culina` text on its page, in a closed
    /// `<details>` after the card: hosts serve the extension as a
    /// download, and the page stays one artifact.
    #[serde(default)]
    source: bool,
    /// Link each recipe page to its file in the repository. `{path}` is
    /// the recipe's path from the collection root; the rest — host,
    /// owner, branch, subdirectory — is the collection's to write.
    #[serde(default)]
    source_link: Option<String>,
    /// The words of that link, with `{path}` in them substituted; the
    /// default is the path itself, a quiet name beside the repo name.
    #[serde(default)]
    source_link_text: Option<String>,
    /// The repository link in the footer of every page: the forge's
    /// mark plus the `owner/repo` tail of this URL.
    #[serde(default)]
    repository: Option<String>,
    /// Free text in the same footer, escaped.
    #[serde(default)]
    footer: Option<String>,
}

const LAYOUTS: [&str; 3] = ["card", "grid", "grid-slant"];

#[derive(serde::Deserialize, Default)]
struct Lexicon {
    #[serde(default)]
    file: Option<String>,
}

fn main() -> ExitCode {
    let mut input = PathBuf::from(".");
    let mut out = PathBuf::from("site");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" => {
                println!("culina-site {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--in" => {
                i += 1;
                match args.get(i) {
                    Some(p) => input = PathBuf::from(p),
                    None => return flag_error("--in needs a directory"),
                }
            }
            "--out" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out = PathBuf::from(p),
                    None => return flag_error("--out needs a directory"),
                }
            }
            other => return flag_error(format!("unknown flag '{other}'")),
        }
        i += 1;
    }

    let config = match read_config(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("culina-site: {e}");
            return ExitCode::FAILURE;
        }
    };
    let collection = match &config.lexicon.file {
        Some(rel) => match culina_core::lexicon::Lexicon::from_path(&input.join(rel)) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("culina-site: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    if let Err(e) = run(&input, &out, &config, collection.as_ref()) {
        eprintln!("culina-site: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn flag_error(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("culina-site: {msg}");
    ExitCode::FAILURE
}

fn read_config(input: &Path) -> Result<Config, String> {
    // Two policy files is a conflict, not a precedence question —
    // design.md § Collection config owns the rule.
    let dotfile = input.join(".culina.toml");
    let visible = input.join("culina.toml");
    let path = match (dotfile.exists(), visible.exists()) {
        (true, true) => {
            return Err(format!(
                "both {} and {} exist; the policy must live in one file — remove one",
                dotfile.display(),
                visible.display()
            ));
        }
        (false, false) => return Ok(Config::default()),
        (true, false) => dotfile,
        (false, true) => visible,
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let config: Config = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(l) = &config.site.layout {
        if !LAYOUTS.contains(&l.as_str()) {
            return Err(format!(
                "{}: [site] layout = \"{l}\" is not a layout — use one of {}",
                path.display(),
                LAYOUTS.join(", ")
            ));
        }
        if l != "card" && !config.site.grid {
            return Err(format!(
                "{}: [site] layout = \"{l}\" needs the grid — add grid = true",
                path.display()
            ));
        }
    }
    if let Some(t) = &config.site.source_link {
        if !t.contains("{path}") {
            return Err(format!(
                "{}: [site] source_link has no {{path}} — the link needs the recipe's path, e.g. \"https://github.com/owner/repo/blob/main/{{path}}\"",
                path.display()
            ));
        }
    }
    Ok(config)
}

fn run(
    input: &Path,
    out: &Path,
    config: &Config,
    collection: Option<&culina_core::lexicon::Lexicon>,
) -> Result<(), String> {
    let mut recipes: Vec<PathBuf> = Vec::new();
    collect(input, out, &mut recipes).map_err(|e| e.to_string())?;
    recipes.sort();

    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    copy_css(input, out).map_err(|e| e.to_string())?;
    if config.site.grid {
        std::fs::write(out.join("culina-grid.css"), GRID_CSS).map_err(|e| e.to_string())?;
    }
    let static_dir = input.join("static");
    if static_dir.is_dir() {
        copy_dir(&static_dir, &out.join("static")).map_err(|e| e.to_string())?;
    }

    // Pages are flat, named by file stem, so two recipes in different
    // directories can claim one page; a silent overwrite would drop one.
    let mut pages: std::collections::HashMap<String, &PathBuf> = std::collections::HashMap::new();
    for path in &recipes {
        let s = slug(
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("recipe"),
        );
        if let Some(first) = pages.insert(s.clone(), path) {
            return Err(format!(
                "{} and {} both become {s}.html — rename one",
                first.display(),
                path.display()
            ));
        }
    }

    let mut index: Vec<(String, Option<String>, String)> = Vec::new();
    let mut failed = false;
    for path in &recipes {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        match fmt::prepare(&text, collection) {
            Ok((doc, _, warnings)) => {
                for w in warnings {
                    eprintln!("culina-site: {}: warning: {w}", path.display());
                }
                let model = card(&doc);
                let slug = slug(
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("recipe"),
                );
                let title = if model.title.is_empty() {
                    slug.clone()
                } else {
                    model.title.clone()
                };
                let mut body = html_fragment(&model);
                if config.site.grid {
                    body = with_grid(&body, &html_grid(&grid(&doc)));
                }
                // The one fact the generator knows about the repo: the
                // recipe's location from the collection root.
                let rel = path.strip_prefix(input).unwrap_or(path);
                let source_link = config.site.source_link.as_deref().map(|template| {
                    source_link_html(
                        template,
                        config.site.source_link_text.as_deref(),
                        &rel.to_string_lossy(),
                    )
                });
                let source = config.site.source.then_some(text.as_str());
                let page = page(&title, &body, &config.site, source, source_link.as_deref());
                std::fs::write(out.join(format!("{slug}.html")), page)
                    .map_err(|e| format!("{}: {e}", out.display()))?;
                index.push((title, model.summary.clone(), format!("{slug}.html")));
            }
            Err(e) => {
                eprint!("{}", error::render(&text, &path.display().to_string(), &e));
                failed = true;
            }
        }
    }
    if failed {
        return Err("one or more recipes failed to parse".into());
    }
    index.sort_by(|a, b| a.0.cmp(&b.0));
    let html = index_page(&config.site, &index);
    std::fs::write(out.join("index.html"), html).map_err(|e| e.to_string())?;
    Ok(())
}

/// All `.culina` files under `dir`, at any depth, skipping the output
/// directory and hidden directories (`.git`, a tool checkout such as
/// `.culina-src`): a dot means "not part of the collection".
fn collect(dir: &Path, skip: &Path, acc: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.starts_with('.') || path.canonicalize().ok() == skip.canonicalize().ok() {
                continue;
            }
            collect(&path, skip, acc)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("culina") {
            acc.push(path);
        }
    }
    Ok(())
}

fn copy_css(input: &Path, out: &Path) -> std::io::Result<()> {
    let own = input.join("culina.css");
    let text = if own.exists() {
        std::fs::read_to_string(&own)?
    } else {
        DEFAULT_CSS.to_string()
    };
    std::fs::write(out.join("culina.css"), text)
}

fn copy_dir(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let path = entry?.path();
        let target = dst.join(path.file_name().expect("dir entries have names"));
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn slug(stem: &str) -> String {
    let mut slug = String::new();
    let mut prev_dash = false;
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

/// Put the grid right after the method, ahead of the notes and photos: the
/// card reads as before, and the grid follows as its one-glance summary.
/// The fragment's order after the method is notes, gallery, JSON-LD.
fn with_grid(card: &str, grid: &str) -> String {
    // The sheet stacks the heading over the grid inside the centring view.
    let section = format!(
        "<section class=\"culina-grid-view\">\n<div class=\"culina-grid-sheet\">\n<h3 class=\"culina-section culina-grid-heading\">At a glance</h3>\n{grid}</div>\n</section>\n"
    );
    let at = [
        "<aside class=\"culina-notes\"",
        "<div class=\"culina-gallery\"",
        "<script type=\"application/ld+json\"",
        "</article>",
    ]
    .iter()
    .find_map(|m| card.find(m))
    .unwrap_or(card.len());
    format!("{}{section}{}", &card[..at], &card[at..])
}

fn page(
    title: &str,
    fragment: &str,
    site: &Site,
    source: Option<&str>,
    source_link: Option<&str>,
) -> String {
    let layout = match &site.layout {
        Some(l) if site.grid => format!(" data-layout=\"{l}\""),
        _ => String::new(),
    };
    let grid_css = if site.grid {
        "<link rel=\"stylesheet\" href=\"culina-grid.css\">\n"
    } else {
        ""
    };
    let mut body = fragment.to_string();
    if let Some(text) = source {
        body.push('\n');
        body.push_str(&source_details(text));
    }
    let footer = footer_html(site, source_link);
    if !footer.is_empty() {
        body.push('\n');
        body.push_str(&footer);
    }
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"{layout}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"culina.css\">\n{grid_css}</head>\n<body>\n{}\n</body>\n</html>\n",
        escape(title),
        body
    )
}

fn index_page(site: &Site, recipes: &[(String, Option<String>, String)]) -> String {
    let title = site.title.as_deref().unwrap_or("Recipes");
    let mut html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"culina.css\">\n</head>\n<body>\n<header class=\"culina-site-header\">\n<h1>{}</h1>\n",
        escape(title),
        escape(title)
    );
    if let Some(d) = &site.description {
        html.push_str(&format!("<p>{}</p>\n", escape(d)));
    }
    html.push_str("</header>\n<ul class=\"culina-recipe-index\">\n");
    for (title, summary, href) in recipes {
        html.push_str(&format!(
            "<li class=\"culina-recipe-index-item\"><a href=\"{}\">{}</a>",
            escape(href),
            escape(title)
        ));
        if let Some(s) = summary {
            html.push_str(&format!(
                " <span class=\"culina-recipe-index-summary\">{}</span>",
                escape(s)
            ));
        }
        html.push_str("</li>\n");
    }
    html.push_str("</ul>\n");
    let footer = footer_html(site, None);
    if !footer.is_empty() {
        html.push('\n');
        html.push_str(&footer);
    }
    html.push_str("</body>\n</html>\n");
    html
}

// --- Page chrome: source block and footer (design.md, Collection config) ---

/// HTML-escape text content: `&` first, then `<`, so the second pass
/// cannot re-escape what the first wrote. Entities are built at runtime;
/// a literal one in this source would arrive decoded.
fn escape(text: &str) -> String {
    let amp = char::from_u32(0x26).unwrap();
    text.replace(amp, &format!("{amp}amp;"))
        .replace('<', &format!("{amp}lt;"))
}

/// HTML-escape a double-quoted attribute value: `&` first, then `"`.
fn escape_attr(text: &str) -> String {
    let amp = char::from_u32(0x26).unwrap();
    text.replace(amp, &format!("{amp}amp;"))
        .replace('"', &format!("{amp}quot;"))
}

/// The host of a repository URL, lowercased. The scheme is optional, so
/// `https://github.com/x` and `github.com/x` pick the same mark.
fn repo_host(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// The forge mark for a repository URL. GitHub and GitLab are known
/// hosts; anything else gets the plain link icon — a wrong mark is worse
/// than a plain one. Every mark rides `currentColor`, so the card
/// sheet's link colour carries it.
fn repo_icon(url: &str) -> &'static str {
    match repo_host(url).as_str() {
        "github.com" => GITHUB_MARK,
        "gitlab.com" => GITLAB_MARK,
        _ => LINK_MARK,
    }
}

/// The visible text: the `owner/repo` tail of the URL when the path has
/// one, else the whole URL.
fn repo_text(url: &str) -> String {
    let rest = url
        .trim_end_matches('/')
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let mut segments = rest.split('/');
    let _host = segments.next().unwrap_or_default();
    match (segments.next(), segments.next()) {
        (Some(owner), Some(repo)) => format!("{owner}/{repo}"),
        _ => url.to_string(),
    }
}

/// The repo link for one recipe: the URL from the collection's
/// `source_link` template, the words from `source_link_text` — the
/// recipe's path by default, a quiet name beside the repository name.
fn source_link_html(template: &str, text: Option<&str>, rel_path: &str) -> String {
    let url = template.replace("{path}", rel_path);
    let words = match text {
        Some(t) => t.replace("{path}", rel_path),
        None => rel_path.to_string(),
    };
    format!(
        "<span class=\"culina-source-link\"><a href=\"{}\">{}</a></span>",
        escape_attr(&url),
        escape(&words)
    )
}

/// The recipe's own text, closed until opened: escaped, otherwise
/// byte-exact.
fn source_details(text: &str) -> String {
    format!(
        "<details class=\"culina-source\">\n<summary>View source</summary>\n<pre class=\"culina-source-text\">{}</pre>\n</details>\n",
        escape(text)
    )
}

/// The footer every page shares: the repository as an icon-and-text
/// link, the page's own recipe link beside it on a recipe page, and
/// free text last. Nothing set means no footer — a collection that
/// wants no chrome gets no chrome. The index passes no recipe link:
/// it has no one recipe to point at.
fn footer_html(site: &Site, recipe_link: Option<&str>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(url) = &site.repository {
        parts.push(format!(
            "<a class=\"culina-repo\" href=\"{}\">{}<span class=\"culina-repo-text\">{}</span></a>\n",
            escape_attr(url),
            repo_icon(url),
            escape(&repo_text(url))
        ));
    }
    if let Some(link) = recipe_link {
        parts.push(format!("{link}\n"));
    }
    if let Some(text) = &site.footer {
        parts.push(format!(
            "<span class=\"culina-footer-text\">{}</span>\n",
            escape(text)
        ));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(
        "<footer class=\"culina-site-footer\">\n{}</footer>\n",
        parts.join("")
    )
}

fn print_help() {
    println!(
        "culina-site {} — render a collection of .culina recipes into a static site

Usage: culina-site [--in DIR] [--out DIR]

  --in    the collection root (default: current directory)
  --out   output directory (default: site)

The collection root may carry a policy file (.culina.toml or
culina.toml — [site] title, description, grid = true, layout =
\"card\" | \"grid\" | \"grid-slant\", source = true, source_link = \"url
with {{path}}\", source_link_text = \"link text\", repository = \"url\",
footer = \"text\"; [lexicon] file = \"overlay.toml\"),
a culina.css (used instead of the built-in default), and a static/
directory copied verbatim. With grid on, a font file at
static/fonts/AtkinsonHyperlegibleNext.woff2 is used by the grid.",
        env!("CARGO_PKG_VERSION")
    );
}

/// The built-in stylesheet: the classic recipe card (paper, double rule,
/// dotted leaders). Collections override by dropping a `culina.css` at the
/// input root; the `css/` directory ships three alternative stylesheets.
const DEFAULT_CSS: &str = include_str!("../css/recipe-card.css");

/// The grid stylesheet, written as culina-grid.css when the grid is on. It
/// sits beside culina.css, so a collection's own card sheet keeps the grid.
const GRID_CSS: &str = include_str!("../css/grid.css");

// The forge marks, single-quoted so the strings stay readable in Rust.
// GitHub's and GitLab's come from Simple Icons (CC0); the neutral link is
// Feather's (MIT). All ride currentColor, so each card sheet's link
// colour carries them. The 16px attributes are the no-sheet size: a
// collection's own culina.css does not know .culina-repo-icon, and a bare
// inline SVG defaults to 300x150 — the sheets' rules still override it.

const GITHUB_MARK: &str = "<svg class='culina-repo-icon' width='16' height='16' viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12'/></svg>";

const GITLAB_MARK: &str = "<svg class='culina-repo-icon' width='16' height='16' viewBox='0 0 24 24' aria-hidden='true'><path fill='currentColor' d='m23.6004 9.5927-.0337-.0862L20.3.9814a.851.851 0 0 0-.3362-.405.8748.8748 0 0 0-.9997.0539.8748.8748 0 0 0-.29.4399l-2.2055 6.748H7.5375l-2.2057-6.748a.8573.8573 0 0 0-.29-.4412.8748.8748 0 0 0-.9997-.0537.8585.8585 0 0 0-.3362.4049L.4332 9.5015l-.0325.0862a6.0657 6.0657 0 0 0 2.0119 7.0105l.0113.0087.03.0213 4.976 3.7264 2.462 1.8633 1.4995 1.1321a1.0085 1.0085 0 0 0 1.2197 0l1.4995-1.1321 2.4619-1.8633 5.006-3.7489.0125-.01a6.0682 6.0682 0 0 0 2.0094-7.003z'/></svg>";

const LINK_MARK: &str = "<svg class='culina-repo-icon' width='16' height='16' viewBox='0 0 24 24' aria-hidden='true'><path fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' d='M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71'/><path fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round' d='M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71'/></svg>";
