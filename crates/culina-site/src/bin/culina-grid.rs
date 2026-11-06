//! culina-grid — print one recipe as a Cooking For Engineers grid.
//!
//! The fragment is unstyled, like the card's; `--page` wraps it in a whole
//! page with the built-in grid stylesheet, ready to open or print.

use std::process::ExitCode;

use culina_core::fmt::prepare;
use culina_core::grid::{grid, html_grid_with, GridOptions};
use culina_core::lexicon::Lexicon;

const GRID_CSS: &str = include_str!("../../css/grid.css");

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut page = false;
    let mut opts = GridOptions::default();
    let mut lexicon: Option<String> = None;
    let mut file: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "-V" | "--version" => {
                println!("culina-grid {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--page" => page = true,
            "--slant" => opts.slant = true,
            "--lexicon" => {
                i += 1;
                match args.get(i) {
                    Some(p) => lexicon = Some(p.clone()),
                    None => {
                        eprintln!("culina-grid: --lexicon needs a path");
                        return ExitCode::FAILURE;
                    }
                }
            }
            a if a.starts_with('-') => {
                eprintln!("culina-grid: unknown flag '{a}'");
                return ExitCode::FAILURE;
            }
            a => {
                if file.replace(a.to_string()).is_some() {
                    eprintln!("culina-grid: one recipe at a time");
                    return ExitCode::FAILURE;
                }
            }
        }
        i += 1;
    }
    let Some(path) = file else {
        print_help();
        return ExitCode::FAILURE;
    };

    let collection = match &lexicon {
        Some(p) => match Lexicon::from_path(std::path::Path::new(p)) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("culina-grid: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("culina-grid: {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let doc = match prepare(&src, collection.as_ref()) {
        Ok((doc, _, warnings)) => {
            for w in warnings {
                eprintln!("culina-grid: {path}: warning: {w}");
            }
            doc
        }
        Err(e) => {
            eprintln!("culina-grid: {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let model = grid(&doc);
    let fragment = html_grid_with(&model, &opts);
    if page {
        let title = escape(&model.title);
        print!(
            "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<style>\n{GRID_CSS}</style>\n</head>\n<body>\n<h1 class=\"culina-grid-title\">{title}</h1>\n{fragment}</body>\n</html>\n"
        );
    } else {
        print!("{fragment}");
    }
    ExitCode::SUCCESS
}

// Built from code points: tool parameters arrive with entities decoded
// (AGENTS.md, "Things that bit us").
fn escape(s: &str) -> String {
    let amp = char::from_u32(0x26).unwrap();
    s.replace(amp, &format!("{amp}amp;"))
        .replace('<', &format!("{amp}lt;"))
        .replace('>', &format!("{amp}gt;"))
}

fn print_help() {
    println!(
        "culina-grid {} — print a .culina recipe as a Cooking For Engineers grid

Usage: culina-grid [--page] [--slant] [--lexicon PATH] FILE

  --page      a whole HTML page with the built-in grid stylesheet
  --slant     lean the columns into a forward slash (culina-grid-slant)
  --lexicon   a collection overlay on the tool core (TOML)
Without --page: an unstyled <table class=\"culina-grid\"> fragment.",
        env!("CARGO_PKG_VERSION")
    );
}
