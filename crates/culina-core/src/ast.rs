//! Block-level AST for `.culina` documents.
//!
//! Inline tag parsing (sigils, `|` fields) arrives with the next phase; a
//! step's physical lines stay raw text here.

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
    /// One step: the `:` head line plus any indented continuation lines.
    /// Continuations keep their original leading whitespace.
    Step {
        head: String,
        continuations: Vec<String>,
    },
}

#[derive(Debug)]
pub struct MetaLine {
    pub key: String,
    pub value: String,
}

impl Document {
    /// All metadata lines in file order, across meta blocks.
    pub fn meta_lines(&self) -> impl Iterator<Item = &MetaLine> {
        self.blocks.iter().filter_map(|b| match b {
            Block::Meta(lines) => Some(lines.iter()),
            _ => None,
        }).flatten()
    }

    pub fn step_count(&self) -> usize {
        self.blocks.iter().filter(|b| matches!(b, Block::Step { .. })).count()
    }

    pub fn section_count(&self) -> usize {
        self.blocks.iter().filter(|b| matches!(b, Block::Section { .. })).count()
    }

    pub fn continuation_count(&self) -> usize {
        self.blocks
            .iter()
            .filter_map(|b| match b {
                Block::Step { continuations, .. } => Some(continuations.len()),
                _ => None,
            })
            .sum()
    }
}
