//! Metadata validation — the key registry as a conformance check.
//!
//! design.md (Metadata) declares the registry normative: every key has a
//! form and a cardinality, literals are machine-validated, and unknown keys
//! warn and pass through with the value opaque. This module is that table.
//! The `unit` key's full validation happens at registration
//! (`fmt::resolve_lexicon`); here only its `name = value` shape is checked.

use crate::ast::{Document, MetaLine};
use crate::error::ParseError;
use crate::fields::{parse_quantity, Value};
use crate::lexicon::Lexicon;
use crate::units::Dimension;

/// The two-axes type system: form + cardinality (`true` = one value).
enum Form {
    /// Free text, may contain commas.
    Prose,
    /// Short comma-free tokens; comma lists.
    Token,
    /// Path or URL; commas are invalid.
    Reference,
    /// Machine-validated; the string is the human phrase for errors.
    Literal(&'static str),
    /// `name = value` entries.
    Definition,
}

fn known(key: &str) -> Option<(Form, bool)> {
    use Form::*;
    Some(match key {
        "title" => (Prose, true),
        "servings" => (Literal("a number or range, e.g. 4 or 4-6"), true),
        "author" => (Token, false),
        "date" => (
            Literal("a date, e.g. 1961, circa 1961, early 1970s — note"),
            true,
        ),
        "source" => (Prose, false),
        "summary" => (Prose, true),
        "keywords" => (Token, false),
        "cuisine" => (Token, false),
        "course" => (Token, false),
        "diet" => (Token, false),
        "dish" => (Token, false),
        "aka" => (Prose, false),
        "image" => (Reference, false),
        "note" => (Prose, false),
        "units" => (Literal("metric, imperial, or us"), true),
        "lang" => (Literal("a BCP 47 tag, e.g. en or fr-FR"), true),
        "time" => (Literal("a duration, e.g. 45 min"), true),
        "modified" => (Literal("an ISO date, e.g. 2026-11-01"), true),
        "unit" => (Definition, false),
        "makes" => (Token, false),
        "yield-scales" => (Literal("count or size"), true),
        _ => return None,
    })
}

/// Validate every metadata line. Errors are loud (canonicalize fails on
/// the first); warnings ride along — unknown keys keep their value opaque
/// per the spec's warn-and-pass-through rule.
pub fn validate(doc: &Document, lex: &Lexicon) -> (Vec<ParseError>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut seen: Vec<&str> = Vec::new();

    for m in doc.meta_lines() {
        match known(&m.key) {
            None => warnings.push(format!(
                "line {}: unknown metadata key '{}' — kept as-is",
                m.line_no, m.key
            )),
            Some((form, single)) => {
                if single && seen.contains(&m.key.as_str()) {
                    errors.push(ParseError::MetaCardinality {
                        line: m.line_no,
                        key: m.key.clone(),
                    });
                    continue;
                }
                seen.push(m.key.as_str());
                validate_value(m, form, lex, &mut errors);
            }
        }
    }
    (errors, warnings)
}

fn validate_value(m: &MetaLine, form: Form, lex: &Lexicon, errors: &mut Vec<ParseError>) {
    let shape = |found: &str| ParseError::MetaShape {
        line: m.line_no,
        key: m.key.clone(),
        found: found.to_string(),
    };

    if m.value.trim().is_empty() {
        errors.push(shape("<empty>"));
        return;
    }
    match form {
        Form::Prose => {}
        Form::Token => {
            if m.value.split(',').any(|t| t.trim().is_empty()) {
                errors.push(shape(&m.value));
            }
        }
        Form::Reference => {
            // A comma is not path/URL shape — "a.jpg, b.jpg" is the lazy
            // mistake; repetition is the only way to add one.
            if m.value.contains(',') {
                errors.push(shape(&m.value));
            }
        }
        Form::Literal(expected) => {
            let ok = match m.key.as_str() {
                "servings" => parse_servings(lex, &m.value),
                "date" => parse_date(&m.value, lex),
                "time" => parse_duration(lex, &m.value),
                "units" => matches!(m.value.trim(), "metric" | "imperial" | "us"),
                "lang" => is_bcp47(m.value.trim()),
                "modified" => is_iso_date(m.value.trim()),
                "yield-scales" => matches!(m.value.trim(), "count" | "size"),
                _ => false,
            };
            if !ok {
                errors.push(ParseError::MetaLiteral {
                    line: m.line_no,
                    key: m.key.clone(),
                    expected: expected.to_string(),
                    found: m.value.clone(),
                });
            }
        }
        Form::Definition => {
            // Full validation happens at registration; the shape must at
            // least carry '='.
            if !m.value.contains('=') {
                errors.push(shape(&m.value));
            }
        }
    }
}

/// Servings: a number or range, no unit ("4", "4-6").
fn parse_servings(lex: &Lexicon, v: &str) -> bool {
    match parse_quantity(lex, v) {
        Some(q) => {
            q.unit.is_none()
                && match q.value {
                    Value::Single(n) => n > 0.0,
                    Value::Range(a, b) => a > 0.0 && b > 0.0,
                }
        }
        None => false,
    }
}

/// A duration quantity: "45 min", "1 hour".
fn parse_duration(lex: &Lexicon, v: &str) -> bool {
    parse_quantity(lex, v).is_some_and(|q| q.dim() == Some(Dimension::Time))
}

/// Date grammar (design.md, Version provenance): optional qualifier word
/// from the dates channel, a date token (year, decade, year-month, ISO),
/// then an empty remainder or a note behind a dash.
fn parse_date(value: &str, lex: &Lexicon) -> bool {
    let mut rest = value.trim();
    for w in &lex.dates.words {
        if let Some(r) = rest.strip_prefix(w.as_str()) {
            if r.starts_with(char::is_whitespace) {
                rest = r.trim_start();
                break;
            }
        }
    }
    let (len, valid) = scan_date_token(rest);
    if !valid {
        return false;
    }
    let note = rest[len..].trim_start();
    note.is_empty() || note.starts_with('—') || note.starts_with('-')
}

/// A leading date token: `1961` | `1970s` | `1961-06` | `1961-06-14`.
/// Months 1–12, days 1–31. Returns (consumed length, valid).
fn scan_date_token(s: &str) -> (usize, bool) {
    let b = s.as_bytes();
    if b.len() < 4 || !b[..4].iter().all(u8::is_ascii_digit) {
        return (0, false);
    }
    if b.len() >= 5 && b[4] == b's' {
        return (5, true); // decade
    }
    if b.get(4) != Some(&b'-') {
        return (4, true); // bare year
    }
    let month_ok =
        b.len() >= 7 && b[5..7].iter().all(u8::is_ascii_digit) && (1..=12).contains(&month(b));
    if !month_ok {
        return (0, false);
    }
    // No fourth dash: a year-month (1961-06). Any tail is the note check's
    // business, not a scan failure.
    if b.get(7) != Some(&b'-') {
        return (7, true);
    }
    let day_ok = b.len() >= 10 && b[8..10].iter().all(u8::is_ascii_digit) && b[8..10] != *b"00";
    if !day_ok {
        return (0, false);
    }
    (10, true)
}

fn month(b: &[u8]) -> u32 {
    (b[5] - b'0') as u32 * 10 + (b[6] - b'0') as u32
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[0..4].iter().all(u8::is_ascii_digit)
        && b[4] == b'-'
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[7] == b'-'
        && b[8..10].iter().all(u8::is_ascii_digit)
        && (1..=12).contains(&((b[5] - b'0') as u32 * 10 + (b[6] - b'0') as u32))
        && b[8..10] != *b"00"
}

/// Loose BCP 47: 2–8 letters, then 1–8 alphanumeric subtags.
fn is_bcp47(s: &str) -> bool {
    let mut parts = s.split('-');
    let primary = parts.next().unwrap_or_default();
    (2..=8).contains(&primary.len())
        && primary.chars().all(char::is_alphabetic)
        && parts
            .all(|p| !p.is_empty() && p.len() <= 8 && p.chars().all(|c| c.is_ascii_alphanumeric()))
}
