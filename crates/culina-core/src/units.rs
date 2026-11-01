//! Unit dictionary — the seed of the units lexicon channel (design.md,
//! Lexicons). Phase 7 swaps this static table for loaded per-language data;
//! the resolution order and payload shapes stay the same.

/// What a unit measures. Drives classification: time quantities on actions
/// are durations, temperatures on tools are state magnitudes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Mass,
    Volume,
    Length,
    Time,
    Temperature,
    /// A countable measure — cloves, cans, handfuls. Present in the
    /// dictionary so `6 cloves` parses as one amount, but scaling treats
    /// these like counts.
    Count,
    /// An escaped unknown unit (`1%glug`): scales arithmetically, never
    /// converts.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub name: String,
    pub dim: Dimension,
}

/// The deliberately tiny English core. Everything else is prose until the
/// ecosystem grows it.
pub fn core_units() -> &'static [(&'static str, Dimension)] {
    &[
        // mass
        ("kg", Dimension::Mass),
        ("g", Dimension::Mass),
        ("mg", Dimension::Mass),
        ("oz", Dimension::Mass),
        ("lb", Dimension::Mass),
        // volume — longest match sorts tbsp before tsp
        ("tbsp", Dimension::Volume),
        ("tsp", Dimension::Volume),
        ("ml", Dimension::Volume),
        ("cup", Dimension::Volume),
        ("cups", Dimension::Volume),
        ("l", Dimension::Volume),
        // length
        ("in", Dimension::Length),
        ("cm", Dimension::Length),
        ("ft", Dimension::Length),
        // time
        ("sec", Dimension::Time),
        ("min", Dimension::Time),
        ("hour", Dimension::Time),
        ("hours", Dimension::Time),
        ("h", Dimension::Time),
        ("day", Dimension::Time),
        ("days", Dimension::Time),
        // temperature
        ("F", Dimension::Temperature),
        ("C", Dimension::Temperature),
        // countable
        ("cloves", Dimension::Count),
        ("clove", Dimension::Count),
        ("stalk", Dimension::Count),
        ("cans", Dimension::Count),
        ("can", Dimension::Count),
        ("bunch", Dimension::Count),
        ("handful", Dimension::Count),
        ("batch", Dimension::Count),
        ("slices", Dimension::Count),
        ("slice", Dimension::Count),
    ]
}
