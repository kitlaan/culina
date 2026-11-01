//! Loud parse errors, plus the miette renderer that draws them against the
//! source.

use miette::{Diagnostic, GraphicalReportHandler, LabeledSpan, NamedSource, SourceCode};
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

    #[error("line {line}: quantity after note — '{field}' must move before the notes, or silent demotion drops the amount from scaling")]
    QuantityAfterNote { line: usize, field: String },

    #[error("line {line}: thread '{name}' was never created — a typo silently forks the graph")]
    ThreadNeverCreated { line: usize, name: String },

    #[error("line {line}: thread '{name}' is no longer live — it ended at line {at_line}, becoming {became}")]
    ThreadEnded {
        line: usize,
        name: String,
        at_line: usize,
        became: String,
    },

    #[error("line {line}: thread '{name}' is already live — rename to rebind it, or fork with distinct group names")]
    ThreadAlreadyLive { line: usize, name: String },

    #[error("line {line}: step has two production clauses — one '=>' per step, at the end")]
    SecondProduction { line: usize },

    #[error("line {line}: production clause names nothing — did you forget the '_'?")]
    EmptyProduction { line: usize },

    #[error("line {line}: '_{{{name}|…}}' — piece selection is deferred; consumption references are bare, forks name their groups")]
    MultiplicityOnConsumption { line: usize, name: String },

    #[error("line {line}: '{name}' multiplicity must be one whole count, found '{found}'")]
    BadMultiplicity {
        line: usize,
        name: String,
        found: String,
    },

    #[error("line {line}: '?' is for @ names only")]
    OptionalOutsideAt { line: usize },

    #[error("bad lexicon: {detail}")]
    BadLexicon { detail: String },

    #[error("line {line}: bad unit registration '{value}' — write it as 'name = amount unit', e.g. 'verre = 90 ml'")]
    BadUnitRegistration { line: usize, value: String },
}

impl ParseError {
    /// The 1-based line the error points at.
    pub fn line(&self) -> usize {
        match self {
            ParseError::ContinuationOutsideStep { line }
            | ParseError::StrayLine { line, .. }
            | ParseError::SectionMissingSpace { line }
            | ParseError::BadMetaLine { line }
            | ParseError::EmptyMetaKey { line }
            | ParseError::EmptyAction { line }
            | ParseError::EmptyTagName { line, .. }
            | ParseError::BareTagWithFields { line, .. }
            | ParseError::StrayBrace { line }
            | ParseError::UnterminatedBrace { line }
            | ParseError::QuantityAfterNote { line, .. }
            | ParseError::ThreadNeverCreated { line, .. }
            | ParseError::ThreadEnded { line, .. }
            | ParseError::ThreadAlreadyLive { line, .. }
            | ParseError::SecondProduction { line }
            | ParseError::EmptyProduction { line }
            | ParseError::MultiplicityOnConsumption { line, .. }
            | ParseError::BadMultiplicity { line, .. }
            | ParseError::OptionalOutsideAt { line }
            | ParseError::BadUnitRegistration { line, .. } => *line,
            ParseError::BadLexicon { .. } => 0,
        }
    }
}

/// Render an error against its source with miette's graphical handler —
/// the error's line gets the arrow. Span granularity is the whole line
/// (canonical steps are one line each); token spans arrive with the
/// formatter phase.
pub fn render(source: &str, name: &str, err: &ParseError) -> String {
    let (offset, len) = line_span(source, err.line());
    let handler = GraphicalReportHandler::new();
    let mut out = String::new();
    let rendered = RenderedErr {
        msg: err.to_string(),
        src: NamedSource::new(name, source.to_string()),
        label: LabeledSpan::new(Some("here".to_string()), offset, len.max(1)),
    };
    handler
        .render_report(&mut out, &rendered)
        .expect("miette render to a String cannot fail");
    out
}

#[derive(Debug)]
struct RenderedErr {
    msg: String,
    src: NamedSource<String>,
    label: LabeledSpan,
}

impl std::fmt::Display for RenderedErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for RenderedErr {}

impl Diagnostic for RenderedErr {
    fn code(&self) -> Option<Box<dyn std::fmt::Display + '_>> {
        Some(Box::new("culina"))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(self.label.clone())))
    }
}

fn line_span(source: &str, line_no: usize) -> (usize, usize) {
    let mut offset = 0;
    for (idx, line) in source.split('\n').enumerate() {
        if idx + 1 == line_no {
            return (offset, line.trim_end_matches('\r').len());
        }
        offset += line.len() + 1;
    }
    (0, 0)
}

pub type Result<T> = std::result::Result<T, ParseError>;
