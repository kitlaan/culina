//! Unit types. The dictionary itself lives in the lexicon (design.md,
//! Lexicons); this module holds the shapes the field grammar and lexicon
//! share.

/// What a unit measures. Drives classification: time quantities on actions
/// are durations, temperatures on tools are state magnitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Mass,
    Volume,
    Length,
    Time,
    Temperature,
    /// A countable measure — cloves, cans, handfuls.
    Count,
    /// An escaped unknown unit (`1%glug`): scales arithmetically, never
    /// converts.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// The canonical unit name — the lexicon key ("hour"), not the matched
    /// spelling.
    pub name: String,
    /// The spelling as written ("hours") — what output dials choose from.
    pub spelling: String,
    pub dim: Dimension,
    /// To the dimension base (grams, liters, meters, seconds); `None` for
    /// countables and temperatures.
    pub factor: Option<f64>,
}
