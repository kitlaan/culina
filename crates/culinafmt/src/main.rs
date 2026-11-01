//! culinafmt — canonicalize `.culina` recipe files.
//!
//! gofmt surface: rewrite files in place by default; `-l` lists files whose
//! form differs; `-d` prints unified diffs. With no files: stdin to stdout,
//! no stamping — the `>> modified:` stamp is a file fact.

use std::io::Read;
use std::process::ExitCode;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Write,
    List,
    Diff,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = Mode::Write;
    let mut files: Vec<String> = Vec::new();
    let mut lexicon_path: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].clone();
        match arg.as_str() {
            "--help" => {
                print_help();
                return ExitCode::SUCCESS;
            }
            "--version" => {
                println!("culinafmt {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--lexicon" => {
                i += 1;
                match args.get(i) {
                    Some(p) => lexicon_path = Some(p.clone()),
                    None => {
                        eprintln!("culinafmt: --lexicon needs a path");
                        return ExitCode::FAILURE;
                    }
                }
            }
            _ if arg.starts_with('-') => {
                for c in arg.chars().skip(1) {
                    match c {
                        'l' => mode = Mode::List,
                        'd' => mode = Mode::Diff,
                        'h' => {
                            print_help();
                            return ExitCode::SUCCESS;
                        }
                        'V' => {
                            println!("culinafmt {}", env!("CARGO_PKG_VERSION"));
                            return ExitCode::SUCCESS;
                        }
                        other => {
                            eprintln!("culinafmt: unknown flag '-{other}'");
                            return ExitCode::FAILURE;
                        }
                    }
                }
            }
            _ => files.push(arg),
        }
        i += 1;
    }

    let collection = match &lexicon_path {
        Some(p) => match culina_core::lexicon::Lexicon::from_path(std::path::Path::new(p)) {
            Ok(l) => Some(l),
            Err(e) => {
                eprintln!("culinafmt: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    if files.is_empty() {
        return run_stdin(collection.as_ref());
    }

    let today = culina_core::fmt::today_utc();
    let mut failed = false;
    for path in &files {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("culinafmt: {path}: {e}");
                failed = true;
                continue;
            }
        };
        match culina_core::fmt::canonicalize_full(&text, Some(&today), collection.as_ref()) {
            Ok((out, warnings)) => {
                for w in warnings {
                    eprintln!("culinafmt: {path}: warning: {w}");
                }
                match mode {
                    Mode::Write => {
                        if out != text {
                            if let Err(e) = std::fs::write(path, &out) {
                                eprintln!("culinafmt: {path}: {e}");
                                failed = true;
                            }
                        }
                    }
                    Mode::List => {
                        if out != text {
                            println!("{path}");
                        }
                    }
                    Mode::Diff => {
                        if out != text {
                            print!("{}", unified_diff(&text, &out, path));
                        }
                    }
                }
            }
            Err(e) => {
                eprint!("{}", culina_core::error::render(&text, path, &e));
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_stdin(collection: Option<&culina_core::lexicon::Lexicon>) -> ExitCode {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("culinafmt: failed to read stdin");
        return ExitCode::FAILURE;
    }
    match culina_core::fmt::canonicalize_full(&input, None, collection) {
        Ok((out, warnings)) => {
            for w in warnings {
                eprintln!("culinafmt: warning: {w}");
            }
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprint!("{}", culina_core::error::render(&input, "<stdin>", &e));
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        "culinafmt {} — canonicalize .culina recipes

Usage: culinafmt [--lexicon PATH] [-l|-d] [file ...]

With files: rewrite each in place when its form differs.
  -l            list files whose form differs; do not write
  -d            print unified diffs; do not write
  --lexicon     a collection overlay on the tool core (TOML)
With no files: read stdin, print canonical text to stdout (no stamping).
",
        env!("CARGO_PKG_VERSION")
    );
}

/// A small unified diff — LCS over lines, three lines of context. Recipes
/// are small; no diff crate needed.
fn unified_diff(old: &str, new: &str, name: &str) -> String {
    #[derive(PartialEq, Clone, Copy)]
    enum Op {
        Keep,
        Del,
        Add,
    }

    let a = drop_last_empty(old.split('\n').collect::<Vec<_>>());
    let b = drop_last_empty(new.split('\n').collect::<Vec<_>>());
    let (n, m) = (a.len(), b.len());

    let mut out = format!("--- {name}\n+++ {name}\n");

    // The LCS table is O(n·m) memory; recipes are small, but formatters get
    // pointed at big things. Above the threshold, fall back to one
    // whole-file hunk — correct, not minimal.
    const MAX_LCS_CELLS: usize = 4_000_000;
    if n.saturating_mul(m) > MAX_LCS_CELLS {
        let _ = &mut out;
        out.push_str(&format!("@@ -1,{n} +1,{m} @@\n"));
        for line in &a {
            out.push('-');
            out.push_str(line.trim_end_matches('\r'));
            out.push('\n');
        }
        for line in &b {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        return out;
    }

    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }

    let mut ops: Vec<(Op, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] {
            ops.push((Op::Keep, a[i]));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            ops.push((Op::Del, a[i]));
            i += 1;
        } else {
            ops.push((Op::Add, b[j]));
            j += 1;
        }
    }
    while i < n {
        ops.push((Op::Del, a[i]));
        i += 1;
    }
    while j < m {
        ops.push((Op::Add, b[j]));
        j += 1;
    }

    const CONTEXT: usize = 3;
    let mut include = vec![false; ops.len()];
    for (idx, (op, _)) in ops.iter().enumerate() {
        if *op != Op::Keep {
            let hi = (idx + CONTEXT).min(ops.len() - 1);
            for slot in &mut include[idx.saturating_sub(CONTEXT)..=hi] {
                *slot = true;
            }
        }
    }

    let mut idx = 0;
    while idx < ops.len() {
        if !include[idx] {
            idx += 1;
            continue;
        }
        let start = idx;
        while idx < ops.len() && include[idx] {
            idx += 1;
        }
        let end = idx;

        let old_start = 1 + ops[..start].iter().filter(|(op, _)| *op != Op::Add).count();
        let new_start = 1 + ops[..start].iter().filter(|(op, _)| *op != Op::Del).count();
        let old_count = ops[start..end]
            .iter()
            .filter(|(op, _)| *op != Op::Add)
            .count();
        let new_count = ops[start..end]
            .iter()
            .filter(|(op, _)| *op != Op::Del)
            .count();
        out.push_str(&format!(
            "@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        ));
        for (op, line) in &ops[start..end] {
            let prefix = match op {
                Op::Keep => ' ',
                Op::Del => '-',
                Op::Add => '+',
            };
            out.push(prefix);
            // Trim the \r for display only — comparison above stays on raw
            // lines so CRLF inputs diff honestly.
            out.push_str(line.trim_end_matches('\r'));
            out.push('\n');
        }
    }
    out
}

fn drop_last_empty(mut v: Vec<&str>) -> Vec<&str> {
    if v.last() == Some(&"") {
        v.pop();
    }
    v
}
