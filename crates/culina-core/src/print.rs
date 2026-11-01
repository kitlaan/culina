//! Canonical printer. Block level only in this phase: raw step text, `# `
//! sections, `>> key: value` metadata.
//!
//! Canonical separation: a blank line surrounds section headers and metadata
//! blocks; consecutive steps run adjacent — the dense wall is the point.

use crate::ast::{Block, Document, MetaLine};

pub fn print(doc: &Document) -> String {
    let mut out = String::new();
    for (i, block) in doc.blocks.iter().enumerate() {
        // Every block ends its text with '\n', so one more makes the blank
        // separator line — except between two steps, which stay adjacent.
        if i > 0 && !adjacent_steps(&doc.blocks[i - 1], block) {
            out.push('\n');
        }
        match block {
            Block::Meta(lines) => {
                for MetaLine { key, value } in lines {
                    out.push_str(">> ");
                    out.push_str(key);
                    out.push_str(": ");
                    out.push_str(value);
                    out.push('\n');
                }
            }
            Block::Section { level, title } => {
                for _ in 0..*level {
                    out.push('#');
                }
                out.push(' ');
                out.push_str(title);
                out.push('\n');
            }
            Block::Step { head, continuations } => {
                out.push_str(head);
                out.push('\n');
                for c in continuations {
                    out.push_str(c);
                    out.push('\n');
                }
            }
        }
    }
    out
}

fn adjacent_steps(prev: &Block, next: &Block) -> bool {
    matches!(prev, Block::Step { .. }) && matches!(next, Block::Step { .. })
}
