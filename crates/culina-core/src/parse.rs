//! Block-grammar parser.
//!
//! Rules (design.md, Document structure):
//! - a block starting `# ` is a section header
//! - a block starting `:` is one step
//! - indented physical lines continue the current step
//! - a blank line separates blocks
//!
//! Lenient input, one output: blocks self-delimit, so missing blank lines
//! parse fine; the printer always emits exactly one blank between blocks.

use crate::ast::{Block, Document, MetaLine};
use crate::error::{ParseError, Result};

struct StepAcc {
    head: String,
    continuations: Vec<String>,
}

pub fn parse(input: &str) -> Result<Document> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut open: Option<StepAcc> = None;

    // The printer always ends with one newline; strip it before splitting so
    // round-trips are byte-stable.
    let body = input.strip_suffix('\n').unwrap_or(input);
    for (idx, raw) in body.split('\n').enumerate() {
        let line_no = idx + 1;
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() {
            flush_step(&mut blocks, &mut open);
            continue;
        }

        // Any leading whitespace marks a continuation, whatever follows it.
        if line.starts_with([' ', '\t']) {
            match open.as_mut() {
                Some(acc) => acc.continuations.push(line.to_string()),
                None => return Err(ParseError::ContinuationOutsideStep { line: line_no }),
            }
            continue;
        }

        flush_step(&mut blocks, &mut open);

        if let Some(rest) = line.strip_prefix(">>") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            let (key, value) = match rest.split_once(':') {
                Some((k, v)) => (k.trim().to_string(), v.trim().to_string()),
                None => return Err(ParseError::BadMetaLine { line: line_no }),
            };
            if key.is_empty() {
                return Err(ParseError::EmptyMetaKey { line: line_no });
            }
            match blocks.last_mut() {
                Some(Block::Meta(lines)) => lines.push(MetaLine { key, value }),
                _ => blocks.push(Block::Meta(vec![MetaLine { key, value }])),
            }
        } else if line.starts_with('#') {
            let hashes = line.len() - line.trim_start_matches('#').len();
            let title = match line[hashes..].strip_prefix(' ') {
                Some(t) if !t.is_empty() => t.to_string(),
                _ => return Err(ParseError::SectionMissingSpace { line: line_no }),
            };
            blocks.push(Block::Section { level: hashes, title });
        } else if line.starts_with(':') {
            open = Some(StepAcc { head: line.to_string(), continuations: Vec::new() });
        } else {
            return Err(ParseError::StrayLine {
                line: line_no,
                found: first_word(line),
            });
        }
    }
    flush_step(&mut blocks, &mut open);

    Ok(Document { blocks })
}

fn flush_step(blocks: &mut Vec<Block>, open: &mut Option<StepAcc>) {
    if let Some(acc) = open.take() {
        blocks.push(Block::Step { head: acc.head, continuations: acc.continuations });
    }
}

fn first_word(line: &str) -> String {
    line.split_whitespace().next().unwrap_or_default().to_string()
}
