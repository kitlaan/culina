//! culina-site — render a collection of `.culina` recipes into a static
//! site: flat HTML pages, an index, and CSS.
//!
//! The recipe pages are culina-core's semantic fragments with stable
//! `culina-*` classes; this tool only assembles pages around them. The
//! collection owns policy through a `.culina.toml` or `culina.toml` at the
//! input root (`[site]` title, description; `[lexicon] file` overlay) and
//! theming through a `culina.css` — present means used, absent means the
//! built-in default. With `[site] grid = true`, each page also carries its
//! Cooking For Engineers grid, which `data-layout` on `<html>` shows.

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
                let page = page(&title, &body, &config.site);
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

/// All `.culina` files under `dir`, skipping the output directory.
fn collect(dir: &Path, skip: &Path, acc: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name == ".git" || path.canonicalize().ok() == skip.canonicalize().ok() {
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

fn page(title: &str, fragment: &str, site: &Site) -> String {
    let amp = char::from_u32(0x26).unwrap();
    let lt = format!("{amp}lt;");
    let layout = match &site.layout {
        Some(l) if site.grid => format!(" data-layout=\"{l}\""),
        _ => String::new(),
    };
    let grid_css = if site.grid {
        "<link rel=\"stylesheet\" href=\"culina-grid.css\">\n"
    } else {
        ""
    };
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"{layout}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"culina.css\">\n{grid_css}</head>\n<body>\n{}\n</body>\n</html>\n",
        title.replace('&', &format!("{amp}amp;"))
            .replace('<', &lt),
        fragment
    )
}

fn index_page(site: &Site, recipes: &[(String, Option<String>, String)]) -> String {
    let amp = char::from_u32(0x26).unwrap();
    let esc = |s: &str| {
        s.replace('&', &format!("{amp}amp;"))
            .replace('<', &format!("{amp}lt;"))
    };
    let title = site.title.as_deref().unwrap_or("Recipes");
    let mut html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"culina.css\">\n</head>\n<body>\n<header class=\"culina-site-header\">\n<h1>{}</h1>\n",
        esc(title),
        esc(title)
    );
    if let Some(d) = &site.description {
        html.push_str(&format!("<p>{}</p>\n", esc(d)));
    }
    html.push_str("</header>\n<ul class=\"culina-recipe-index\">\n");
    for (title, summary, href) in recipes {
        html.push_str(&format!(
            "<li class=\"culina-recipe-index-item\"><a href=\"{}\">{}</a>",
            esc(href),
            esc(title)
        ));
        if let Some(s) = summary {
            html.push_str(&format!(
                " <span class=\"culina-recipe-index-summary\">{}</span>",
                esc(s)
            ));
        }
        html.push_str("</li>\n");
    }
    html.push_str("</ul>\n</body>\n</html>\n");
    html
}

fn print_help() {
    println!(
        "culina-site {} — render a collection of .culina recipes into a static site

Usage: culina-site [--in DIR] [--out DIR]

  --in    the collection root (default: current directory)
  --out   output directory (default: site)

The collection root may carry a policy file (.culina.toml or
culina.toml — [site] title, description, grid = true, layout =
\"card\" | \"grid\" | \"grid-slant\"; [lexicon] file = \"overlay.toml\"),
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
