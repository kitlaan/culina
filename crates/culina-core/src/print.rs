//! Canonical printer — blocks and inline segments.
//!
//! Canonical separation: a blank line surrounds section headers and metadata
//! blocks; consecutive steps run adjacent — the dense wall is the point.
//! Inline canonicalization: `→` input sugar prints as `=>`; single-word,
//! field-less tags print bare (design.md, Tags and sigils).

use crate::ast::{Block, Document, MetaLine, Seg, Step, Tag};

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
            Block::Step(step) => print_step(&mut out, step),
        }
    }
    out
}

fn adjacent_steps(prev: &Block, next: &Block) -> bool {
    matches!(prev, Block::Step(_)) && matches!(next, Block::Step(_))
}

fn print_step(out: &mut String, step: &Step) {
    out.push(':');
    if step.action.fields.is_empty() {
        out.push_str(&step.action.verb);
    } else {
        out.push('{');
        out.push_str(&step.action.verb);
        for f in &step.action.fields {
            out.push('|');
            out.push_str(&f.raw);
        }
        out.push('}');
    }
    for seg in &step.head {
        print_seg(out, seg);
    }
    out.push('\n');
    for cont in &step.continuations {
        out.push_str(&cont.indent);
        for seg in &cont.segs {
            print_seg(out, seg);
        }
        out.push('\n');
    }
}

fn print_seg(out: &mut String, seg: &Seg) {
    match seg {
        Seg::Prose(s) => out.push_str(s),
        Seg::Plus => out.push('+'),
        Seg::Arrow => out.push_str("=>"),
        Seg::Tag(t) => print_tag(out, t),
    }
}

/// Braces when the tag is multiword, carries fields, or has alternatives —
/// bare otherwise.
fn print_tag(out: &mut String, t: &Tag) {
    let braced = t.name.chars().any(char::is_whitespace)
        || !t.fields.is_empty()
        || !t.alternatives.is_empty();
    out.push(t.sigil.as_char());
    if braced {
        out.push('{');
        if t.optional {
            out.push('?');
        }
        out.push_str(&t.name);
        if !t.fields.is_empty() {
            for f in &t.fields {
                out.push('|');
                out.push_str(&f.raw);
            }
        }
        out.push('}');
        for alt in &t.alternatives {
            out.push('{');
            out.push_str(&alt.name);
            if !alt.fields.is_empty() {
                for f in &alt.fields {
                    out.push('|');
                    out.push_str(&f.raw);
                }
            }
            out.push('}');
        }
    } else {
        if t.optional {
            out.push('?');
        }
        out.push_str(&t.name);
    }
}
