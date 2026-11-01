//! The field grammar: split, classify, partition.
//!
//! Split happens in the inline scanner; classify and partition live here.
//!
//! A field is a quantity only if the quantity grammar consumes it fully —
//! approx prefix, number (decimal, fraction, mixed, range), optional unit by
//! longest match, nothing left over. Otherwise it is a note. This
//! full-consumption rule is what makes parse-or-note deterministic.
//!
//! The partition rule: quantities come first; the first note ends the amount
//! list; a quantity after a note is a syntax error, not a demotion.

use crate::error::{ParseError, Result};
use crate::units::{core_units, Dimension, Unit};

/// One `|` field of a tag or action: the raw text (printed verbatim) plus
/// its classification.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub raw: String,
    pub kind: FieldKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FieldKind {
    Quantity(Quantity),
    Note,
}

impl Field {
    pub fn quantity(&self) -> Option<&Quantity> {
        match &self.kind {
            FieldKind::Quantity(q) => Some(q),
            FieldKind::Note => None,
        }
    }

    pub fn is_quantity(&self) -> bool {
        matches!(self.kind, FieldKind::Quantity(_))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    pub approx: bool,
    pub value: Value,
    /// `None` means a plain count (`@{egg|1}`).
    pub unit: Option<Unit>,
}

impl Quantity {
    pub fn dim(&self) -> Option<Dimension> {
        self.unit.as_ref().map(|u| u.dim)
    }

    pub fn is_time(&self) -> bool {
        self.dim() == Some(Dimension::Time)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Single(f64),
    Range(f64, f64),
}

/// Approx prefixes — the seed of the approx-prefix lexicon channel.
const APPROX_WORDS: [&str; 4] = ["about", "around", "roughly", "approximately"];

/// Classify one raw field. Never fails: an unparseable field is a note.
pub fn classify(raw: &str) -> FieldKind {
    match parse_quantity(raw) {
        Some(q) => FieldKind::Quantity(q),
        None => FieldKind::Note,
    }
}

/// Quantities must precede notes. A quantity after a note fails loudly —
/// silent demotion would drop the amount from scaling.
pub fn check_partition(line_no: usize, fields: &[Field]) -> Result<()> {
    let mut seen_note = false;
    for f in fields {
        match &f.kind {
            FieldKind::Note => seen_note = true,
            FieldKind::Quantity(_) if seen_note => {
                return Err(ParseError::QuantityAfterNote {
                    line: line_no,
                    field: f.raw.clone(),
                });
            }
            FieldKind::Quantity(_) => {}
        }
    }
    Ok(())
}

pub fn parse_quantity(raw: &str) -> Option<Quantity> {
    let chars: Vec<char> = raw.trim().chars().collect();
    let mut i = 0;

    let mut approx = false;
    if let Some(len) = approx_prefix_len(&chars) {
        approx = true;
        i = len;
    }

    let (value, next) = parse_number(&chars, i)?;
    i = next;
    let (unit, next) = parse_unit(&chars, i)?;
    i = next;

    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    if i != chars.len() {
        return None; // leftover text — the whole field is a note
    }
    Some(Quantity {
        approx,
        value,
        unit,
    })
}

fn approx_prefix_len(chars: &[char]) -> Option<usize> {
    let mut i = if chars.first() == Some(&'~') {
        1
    } else {
        let word: &str = &chars.iter().collect::<String>();
        APPROX_WORDS
            .iter()
            .find(|w| word.starts_with(**w))?
            .chars()
            .count()
    };
    while chars.get(i).is_some_and(|c| c.is_whitespace()) {
        i += 1;
    }
    if i >= chars.len() {
        return None; // prefix with nothing after it — not a quantity
    }
    Some(i)
}

fn parse_number(chars: &[char], start: usize) -> Option<(Value, usize)> {
    let (first, i) = parse_single(chars, start)?;

    // Range: 1-2 (spaces around the dash are lenient input)
    let k = skip_ws(chars, i);
    if chars.get(k) == Some(&'-') {
        let k2 = skip_ws(chars, k + 1);
        if let Some((second, end)) = parse_single(chars, k2) {
            return Some((Value::Range(first, second), end));
        }
    }

    // Mixed number: 1 1/3
    let k = skip_ws(chars, i);
    if k > i {
        if let Some((frac, end)) = parse_fraction(chars, k) {
            return Some((Value::Single(first + frac), end));
        }
    }

    Some((Value::Single(first), i))
}

fn parse_single(chars: &[char], start: usize) -> Option<(f64, usize)> {
    if let Some((v, end)) = parse_fraction(chars, start) {
        return Some((v, end));
    }
    parse_decimal(chars, start)
}

fn parse_fraction(chars: &[char], start: usize) -> Option<(f64, usize)> {
    let (num, i) = parse_digits(chars, start)?;
    if chars.get(i) != Some(&'/') {
        return None;
    }
    let (den, j) = parse_digits(chars, i + 1)?;
    if den == 0.0 {
        return None;
    }
    Some((num / den, j))
}

fn parse_decimal(chars: &[char], start: usize) -> Option<(f64, usize)> {
    let (int, i) = parse_digits(chars, start)?;
    if chars.get(i) == Some(&'.') {
        if let Some((frac, j)) = parse_digits(chars, i + 1) {
            let scale = 10f64.powi((j - (i + 1)) as i32);
            return Some((int + frac / scale, j));
        }
    }
    Some((int, i))
}

fn parse_digits(chars: &[char], start: usize) -> Option<(f64, usize)> {
    let mut i = start;
    let mut value: f64 = 0.0;
    while chars.get(i).is_some_and(|c| c.is_ascii_digit()) {
        value = value * 10.0 + chars[i].to_digit(10).unwrap() as f64;
        i += 1;
    }
    if i == start {
        return None;
    }
    Some((value, i))
}

/// Units are optional (a count has none). Always succeeds; the caller's
/// full-consumption check catches leftovers.
fn parse_unit(chars: &[char], start: usize) -> Option<(Option<Unit>, usize)> {
    let j = skip_ws(chars, start);

    // % escape: force an unknown unit (`1%glug`).
    if chars.get(j) == Some(&'%') {
        let mut k = j + 1;
        let word_start = k;
        while k < chars.len() && !chars[k].is_whitespace() {
            k += 1;
        }
        if k == word_start {
            return None;
        }
        let name: String = chars[word_start..k].iter().collect();
        return Some((
            Some(Unit {
                name,
                dim: Dimension::Other,
            }),
            k,
        ));
    }

    // Longest match from the dictionary, glued or spaced; the character
    // after the unit must end the field or be whitespace.
    let mut best: Option<(usize, &'static str, Dimension)> = None;
    for (name, dim) in core_units() {
        if starts_with_at(chars, j, name) {
            let end = j + name.chars().count();
            if end == chars.len() || chars[end].is_whitespace() {
                if best.map_or(true, |(len, _, _)| name.chars().count() > len) {
                    best = Some((name.chars().count(), name, *dim));
                }
            }
        }
    }
    match best {
        Some((len, name, dim)) => Some((
            Some(Unit {
                name: name.to_string(),
                dim,
            }),
            j + len,
        )),
        None => Some((None, j)),
    }
}

fn starts_with_at(chars: &[char], at: usize, prefix: &str) -> bool {
    let mut prefix_chars = prefix.chars();
    for &c in &chars[at..] {
        match prefix_chars.next() {
            Some(p) if p == c => {}
            _ => return false,
        }
    }
    // The prefix must be fully consumed — zip alone would truncate-match
    // "hour" against the "hours" entry and overflow the slice.
    prefix_chars.next().is_none()
}

fn skip_ws(chars: &[char], at: usize) -> usize {
    let mut i = at;
    while chars.get(i).is_some_and(|c| c.is_whitespace()) {
        i += 1;
    }
    i
}
