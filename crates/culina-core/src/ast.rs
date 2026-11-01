//! AST for `.culina` documents: blocks, steps, and inline segments.
//!
//! Field classification (quantity vs note) happens at parse time and rides
//! along as `FieldKind`; scaling and DAG consumers read the kinds, the
//! printer reads the raw strings.

use crate::fields::{Field, Quantity};

/// A parsed document: metadata and blocks in file order.
#[derive(Debug)]
pub struct Document {
    pub blocks: Vec<Block>,
}

#[derive(Debug)]
pub enum Block {
    /// One or more `>> key: value` lines that were consecutive in the file.
    /// Blank lines between meta lines collapse — metadata is one block.
    Meta(Vec<MetaLine>),
    Section {
        /// Number of leading `#` characters.
        level: usize,
        title: String,
    },
    Step(Step),
}

#[derive(Debug)]
pub struct MetaLine {
    pub key: String,
    pub value: String,
}

#[derive(Debug)]
pub struct Step {
    pub action: Action,
    /// Segments of the head line after the action.
    pub head: Vec<Seg>,
    pub continuations: Vec<Continuation>,
}

/// The `:verb` or `:{verb|fields}` that opens a step.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub verb: String,
    /// Classified `|` fields, in order.
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Continuation {
    /// Original leading whitespace, preserved verbatim.
    pub indent: String,
    pub segs: Vec<Seg>,
}

#[derive(Debug, PartialEq)]
pub enum Seg {
    /// Verbatim text between tags.
    Prose(String),
    Tag(Tag),
    /// A standalone `+` joining two thread references.
    Plus,
    /// The production sigil — parsed from `=>` or the `→` input sugar.
    Arrow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub sigil: Sigil,
    /// `?` on the name — only meaningful for `@`, parsed anywhere.
    pub optional: bool,
    pub name: String,
    /// Classified `|` fields, in order.
    pub fields: Vec<Field>,
    /// Parallel complete tag groups: `@{A|1}{B|1}`.
    pub alternatives: Vec<AltGroup>,
}

impl Tag {
    /// The amount list — the leading quantity fields. Empty means implicit 1
    /// (design.md: a missing amount on `@` is 1 of the thing).
    pub fn amounts(&self) -> Vec<&Quantity> {
        self.fields.iter().filter_map(|f| f.quantity()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sigil {
    At,
    Hash,
    Underscore,
}

impl Sigil {
    pub fn as_char(self) -> char {
        match self {
            Sigil::At => '@',
            Sigil::Hash => '#',
            Sigil::Underscore => '_',
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AltGroup {
    pub name: String,
    /// Classified `|` fields, in order.
    pub fields: Vec<Field>,
}

impl Document {
    /// All metadata lines in file order, across meta blocks.
    pub fn meta_lines(&self) -> impl Iterator<Item = &MetaLine> {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Meta(lines) => Some(lines.iter()),
                _ => None,
            })
            .flatten()
    }

    pub fn step_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| matches!(b, Block::Step(_)))
            .count()
    }

    pub fn section_count(&self) -> usize {
        self.blocks
            .iter()
            .filter(|b| matches!(b, Block::Section { .. }))
            .count()
    }

    pub fn continuation_count(&self) -> usize {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Step(s) => Some(s.continuations.len()),
                _ => None,
            })
            .sum()
    }
}
