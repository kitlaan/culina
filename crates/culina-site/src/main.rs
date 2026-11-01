//! culina-site — render a collection of `.culina` recipes into a static
//! site: flat HTML pages, an index, and CSS.
//!
//! The recipe pages are culina-core's semantic fragments with stable
//! `culina-*` classes; this tool only assembles pages around them. The
//! collection owns policy through `culina.toml` (`[site]` title, description;
//! `[lexicon] file` overlay) and theming through a `culina.css` at the input
//! root — present means used, absent means the built-in default.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
}

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
    let path = input.join("culina.toml");
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
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
                let page = page(&title, &html_fragment(&model));
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

fn page(title: &str, fragment: &str) -> String {
    let amp = char::from_u32(0x26).unwrap();
    let lt = format!("{amp}lt;");
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<link rel=\"stylesheet\" href=\"culina.css\">\n</head>\n<body>\n{}\n</body>\n</html>\n",
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

The collection root may carry a culina.toml ([site] title, description;
[lexicon] file = \"overlay.toml\"), a culina.css (used instead of the
built-in default), and a static/ directory copied verbatim.",
        env!("CARGO_PKG_VERSION")
    );
}

/// The built-in stylesheet: the classic recipe card (paper, double rule,
/// dotted leaders). Collections override by dropping a `culina.css` at the
/// input root; the `css/` directory ships three alternative stylesheets.
const DEFAULT_CSS: &str = include_str!("../css/recipe-card.css");
