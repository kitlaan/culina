//! Loud parse errors. Line numbers now; miette spans with the validation
//! phase.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("line {line}: indented line continues nothing — expected a ':' step first")]
    ContinuationOutsideStep { line: usize },

    #[error("line {line}: a block must start with ':', '# ', '>>', or be an indented continuation — found {found:?}")]
    StrayLine { line: usize, found: String },

    #[error("line {line}: section header needs a space and a title after the hashes")]
    SectionMissingSpace { line: usize },

    #[error("line {line}: metadata needs 'key: value' after '>>'")]
    BadMetaLine { line: usize },

    #[error("line {line}: metadata key is empty")]
    EmptyMetaKey { line: usize },

    #[error("line {line}: step needs a verb after ':'")]
    EmptyAction { line: usize },

    #[error("line {line}: '{sigil}' tag needs a name")]
    EmptyTagName { line: usize, sigil: char },

    #[error("line {line}: bare '{sigil}' tag cannot carry fields — write it braced: {sigil}{{name|amount}}")]
    BareTagWithFields { line: usize, sigil: char },

    #[error("line {line}: '{{' does not start a tag here — braces belong to tags")]
    StrayBrace { line: usize },

    #[error("line {line}: '{{' without a matching '}}'")]
    UnterminatedBrace { line: usize },
}

pub type Result<T> = std::result::Result<T, ParseError>;
