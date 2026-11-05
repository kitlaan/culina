//! Ingredient amount aggregation (design.md, Quantities and units).
//!
//! An ingredient used in several steps carries one amount list per use —
//! each list is equivalents ("2 1/2 cups", "600 g"), and the uses add up.
//! `summarize` turns the uses into a total plus the breakdown behind it.
//!
//! The total is a display fact, never invented precision:
//!
//! - A bucket (mass, volume, length, or one countable unit) totals only
//!   when every use carries it. A use with no mass amount means a mass
//!   total would undercount, so it is left out.
//! - The total prefers the units the author wrote. Volume written in cups
//!   and tablespoons totals in cups and tablespoons. Only when those cannot
//!   state it exactly does volume fall through to the spoons below the
//!   smallest unit written (5/6 cup is not a measure; it is 3/4 cup + 1
//!   tbsp + 1 tsp).
//! - A total that no measure can state is dropped; the breakdown still
//!   carries it.

use crate::fields::{Field, Quantity, Value};
use crate::units::{Dimension, Unit};

/// Relative slack for unit ratios (a cup is 15.99997 tablespoons in the
/// lexicon's liters) and for matching a fraction.
const EPS: f64 = 1e-3;

/// Units that never take a plural `s`.
const ABBREVIATIONS: &[&str] = &[
    "g", "kg", "mg", "ml", "l", "tsp", "tbsp", "oz", "lb", "in", "ft", "cm", "mm", "m", "min",
    "sec", "h", "hr", "F", "C",
];

/// One quantity field of a use: the parsed value, and the text as written.
#[derive(Debug, Clone)]
pub struct Amount {
    pub quantity: Quantity,
    pub raw: String,
}

impl Amount {
    /// `None` when the field is a note.
    pub fn from_field(f: &Field) -> Option<Amount> {
        f.quantity().map(|q| Amount {
            quantity: q.clone(),
            raw: f.raw.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// The summed amounts, bucket by bucket (`2 5/6 cups, 680 g`); `None`
    /// for a single use or when nothing sums cleanly.
    pub total: Option<String>,
    /// Each use, equivalents in parentheses, joined by `+`
    /// (`2 1/2 cups (600 g) + 1/3 cup (80 g)`).
    pub breakdown: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Key {
    Dim(Dimension),
    Unit(String),
    Plain,
}

fn key(q: &Quantity) -> Option<Key> {
    match &q.unit {
        None => Some(Key::Plain),
        Some(u) => match u.dim {
            Dimension::Mass | Dimension::Volume | Dimension::Length if u.factor.is_some() => {
                Some(Key::Dim(u.dim))
            }
            Dimension::Count | Dimension::Other => Some(Key::Unit(u.name.clone())),
            _ => None,
        },
    }
}

fn bounds(q: &Quantity) -> (f64, f64) {
    let scale = q.unit.as_ref().and_then(|u| u.factor).unwrap_or(1.0);
    match q.value {
        Value::Single(v) => (v * scale, v * scale),
        Value::Range(a, b) => (a * scale, b * scale),
    }
}

/// Summarize the measured uses of one ingredient. Uses with no amount are
/// the caller's to drop; they carry nothing to add.
pub fn summarize(uses: &[Vec<Amount>]) -> Summary {
    Summary {
        total: if uses.len() >= 2 { total(uses) } else { None },
        breakdown: breakdown(uses),
    }
}

fn breakdown(uses: &[Vec<Amount>]) -> String {
    uses.iter()
        .filter(|u| !u.is_empty())
        .map(|u| {
            let mut s = u[0].raw.clone();
            if u.len() > 1 {
                let rest: Vec<&str> = u[1..].iter().map(|a| a.raw.as_str()).collect();
                s.push_str(&format!(" ({})", rest.join(", ")));
            }
            s
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn total(uses: &[Vec<Amount>]) -> Option<String> {
    let mut keys: Vec<Key> = Vec::new();
    for a in uses.first()? {
        if let Some(k) = key(&a.quantity) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    let parts: Vec<String> = keys.iter().filter_map(|k| bucket_total(k, uses)).collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// One rung of a unit ladder: a unit the author wrote, with the spellings
/// seen for it (and whether each was written for a plural amount).
struct Rung {
    name: String,
    factor: f64,
    spellings: Vec<(String, bool)>,
}

fn bucket_total(k: &Key, uses: &[Vec<Amount>]) -> Option<String> {
    let mut lo = 0.0;
    let mut hi = 0.0;
    let mut approx = false;
    for u in uses {
        let a = u.iter().find(|a| key(&a.quantity).as_ref() == Some(k))?;
        let (l, h) = bounds(&a.quantity);
        lo += l;
        hi += h;
        approx |= a.quantity.approx;
    }

    let mut ladder: Vec<Rung> = Vec::new();
    let mut fractions = false;
    for a in uses.iter().flatten() {
        if key(&a.quantity).as_ref() != Some(k) {
            continue;
        }
        fractions |= a.raw.contains('/');
        let plural = bounds(&a.quantity).1 > 1.0 + EPS;
        match &a.quantity.unit {
            Some(u) => add_rung(&mut ladder, u, plural),
            None => {
                if ladder.is_empty() {
                    ladder.push(Rung {
                        name: String::new(),
                        factor: 1.0,
                        spellings: Vec::new(),
                    });
                }
            }
        }
    }
    ladder.sort_by(|a, b| b.factor.total_cmp(&a.factor));

    let text = if fractions {
        let pair = match decompose_pair(lo, hi, &ladder) {
            Some(p) => p,
            None => {
                // The written units cannot say it; volume may borrow the
                // spoons below its smallest written unit.
                if *k != Key::Dim(Dimension::Volume) {
                    return None;
                }
                extend_with_spoons(&mut ladder);
                decompose_pair(lo, hi, &ladder)?
            }
        };
        range_text(lo, hi, &pair, &ladder)?
    } else {
        decimal_text(lo, hi, &ladder)?
    };
    Some(if approx { format!("~{text}") } else { text })
}

/// The US kitchen ladder, mirroring the English lexicon's volume units.
const KITCHEN_LADDER: [(&str, f64); 3] = [
    ("cup", 0.236_588_4),
    ("tbsp", 1.478_68e-2),
    ("tsp", 4.928_92e-3),
];

/// Add the cup/tbsp/tsp rungs smaller than the smallest written unit.
fn extend_with_spoons(ladder: &mut Vec<Rung>) {
    let smallest = ladder.iter().map(|r| r.factor).fold(f64::MAX, f64::min);
    for (name, factor) in KITCHEN_LADDER {
        if factor < smallest * (1.0 - EPS) && !ladder.iter().any(|r| r.name == name) {
            ladder.push(Rung {
                name: name.to_string(),
                factor,
                spellings: Vec::new(),
            });
        }
    }
    ladder.sort_by(|a, b| b.factor.total_cmp(&a.factor));
}

fn add_rung(ladder: &mut Vec<Rung>, u: &Unit, plural: bool) {
    let factor = u.factor.unwrap_or(1.0);
    match ladder.iter_mut().find(|r| r.name == u.name) {
        Some(r) => {
            if !r
                .spellings
                .iter()
                .any(|(s, p)| *s == u.spelling && *p == plural)
            {
                r.spellings.push((u.spelling.clone(), plural));
            }
        }
        None => ladder.push(Rung {
            name: u.name.clone(),
            factor,
            spellings: vec![(u.spelling.clone(), plural)],
        }),
    }
}

fn pluralize(name: &str) -> String {
    if name.is_empty() || ABBREVIATIONS.contains(&name) {
        name.to_string()
    } else if name.ends_with(['s', 'x']) || name.ends_with("ch") || name.ends_with("sh") {
        format!("{name}es")
    } else {
        format!("{name}s")
    }
}

/// The unit word for `value`, preferring a spelling the author wrote for
/// the same plurality so "1/2 cup" and "2 cups" both read right.
fn spell(r: &Rung, value: f64) -> String {
    let plural = value > 1.0 + EPS;
    if let Some((s, _)) = r.spellings.iter().find(|(_, p)| *p == plural) {
        return s.clone();
    }
    if plural {
        pluralize(&r.name)
    } else {
        r.name.clone()
    }
}

struct Part {
    number: String,
    /// The amount in this rung's unit; picks the plural.
    value: f64,
    rung: usize,
}

/// Fraction-mode totals. Cup fractions are real measures, so a cup takes a
/// quarter, third, half, two thirds, or three quarters; spoons take whole
/// units, and only the smallest unit takes a fraction of itself.
fn decompose_pair(lo: f64, hi: f64, ladder: &[Rung]) -> Option<(Vec<Part>, Vec<Part>)> {
    if ladder.is_empty() {
        return None;
    }
    Some((decompose(lo, ladder, 0)?, decompose(hi, ladder, 0)?))
}

fn decompose(rem: f64, ladder: &[Rung], idx: usize) -> Option<Vec<Part>> {
    let rung = &ladder[idx];
    let q = rem / rung.factor;
    if idx + 1 == ladder.len() {
        if q < EPS {
            return Some(Vec::new());
        }
        return Some(vec![Part {
            number: mixed(q, denominators(&rung.name))?,
            value: q,
            rung: idx,
        }]);
    }
    let whole = (q + EPS).floor();
    let r1 = (rem - whole * rung.factor).max(0.0);
    if rung.name == "cup" {
        const STEPS: [(f64, &str); 5] = [
            (3.0 / 4.0, "3/4"),
            (2.0 / 3.0, "2/3"),
            (1.0 / 2.0, "1/2"),
            (1.0 / 3.0, "1/3"),
            (1.0 / 4.0, "1/4"),
        ];
        // Every cup fraction that leaves a statable remainder, scored so
        // the plainest wording wins: fewest parts, then fewest fractions
        // in the spoons, then the larger spoon (2 tbsp over 6 tsp).
        let mut best: Option<((usize, usize, usize), Vec<Part>)> = None;
        for (f, label) in STEPS {
            let take = f * rung.factor;
            if take > r1 + EPS * rung.factor {
                continue;
            }
            let r2 = (r1 - take).max(0.0);
            // The spoons carry less than the smallest cup step; "1/3 cup
            // + 8 tbsp" is half a cup said badly.
            if r2 >= 0.25 * rung.factor * (1.0 - EPS) {
                continue;
            }
            if let Some(rest) = tail(r2, ladder, idx + 1) {
                let number = if whole > 0.0 {
                    format!("{whole} {label}")
                } else {
                    label.to_string()
                };
                let parts = prepend(number, whole + f, idx, rest);
                let score = (
                    parts.len(),
                    parts[1..].iter().filter(|p| p.number.contains('/')).count(),
                    parts.last().map_or(0, |p| p.rung),
                );
                if best.as_ref().is_none_or(|(s, _)| score < *s) {
                    best = Some((score, parts));
                }
            }
        }
        if let Some((_, parts)) = best {
            return Some(parts);
        }
    }
    let rest = tail(r1, ladder, idx + 1)?;
    if whole > 0.0 {
        Some(prepend(format!("{whole}"), whole, idx, rest))
    } else {
        Some(rest)
    }
}

fn tail(rem: f64, ladder: &[Rung], idx: usize) -> Option<Vec<Part>> {
    if rem < EPS * ladder[idx].factor {
        Some(Vec::new())
    } else {
        decompose(rem, ladder, idx)
    }
}

fn prepend(number: String, value: f64, rung: usize, mut rest: Vec<Part>) -> Vec<Part> {
    rest.insert(
        0,
        Part {
            number,
            value,
            rung,
        },
    );
    rest
}

/// The fractions a unit is measured in. A cup comes in halves, thirds, and
/// quarters; 5/6 of a cup is not a measure. Everything else also takes
/// eighths.
fn denominators(unit: &str) -> &'static [u32] {
    if unit == "cup" {
        &[2, 3, 4]
    } else {
        &[2, 3, 4, 8]
    }
}

/// A mixed number over the given denominators.
fn mixed(q: f64, dens: &[u32]) -> Option<String> {
    let whole = (q + EPS).floor();
    let frac = (q - whole).max(0.0);
    if frac < EPS {
        return Some(format!("{whole}"));
    }
    for &d in dens {
        for n in 1..d {
            if (frac - n as f64 / d as f64).abs() < EPS {
                return Some(if whole > 0.0 {
                    format!("{whole} {n}/{d}")
                } else {
                    format!("{n}/{d}")
                });
            }
        }
    }
    None
}

fn parts_text(parts: &[Part], ladder: &[Rung]) -> String {
    parts
        .iter()
        .map(|p| {
            let unit = spell(&ladder[p.rung], p.value);
            if unit.is_empty() {
                p.number.clone()
            } else {
                format!("{} {}", p.number, unit)
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn range_text(lo: f64, hi: f64, pair: &(Vec<Part>, Vec<Part>), ladder: &[Rung]) -> Option<String> {
    let (a, b) = pair;
    if a.is_empty() || b.is_empty() {
        return None;
    }
    if (hi - lo).abs() <= EPS * hi.max(1e-9) {
        return Some(parts_text(b, ladder));
    }
    // One rung either side: "2-3 tbsp", the unit once.
    if a.len() == 1 && b.len() == 1 && a[0].rung == b[0].rung {
        let unit = spell(&ladder[b[0].rung], b[0].value);
        return Some(if unit.is_empty() {
            format!("{}-{}", a[0].number, b[0].number)
        } else {
            format!("{}-{} {}", a[0].number, b[0].number, unit)
        });
    }
    Some(format!(
        "{} to {}",
        parts_text(a, ladder),
        parts_text(b, ladder)
    ))
}

/// Decimal-mode totals (the author wrote no fractions): one unit — the
/// largest the total reaches whole, else the smallest written — and a
/// short decimal.
fn decimal_text(lo: f64, hi: f64, ladder: &[Rung]) -> Option<String> {
    for (i, r) in ladder.iter().enumerate() {
        let last = i + 1 == ladder.len();
        if !last && lo / r.factor < 1.0 - EPS {
            continue;
        }
        let a = decimal(lo / r.factor);
        let b = decimal(hi / r.factor);
        if let (Some(a), Some(b)) = (a, b) {
            let value = hi / r.factor;
            let unit = spell(r, value);
            let number = if (hi - lo).abs() <= EPS * hi.max(1e-9) {
                b
            } else {
                format!("{a}-{b}")
            };
            return Some(if unit.is_empty() {
                number
            } else {
                format!("{number} {unit}")
            });
        }
    }
    None
}

/// Up to two decimals, trailing zeros trimmed; `None` when two decimals
/// would be a visible lie.
fn decimal(v: f64) -> Option<String> {
    let r = (v * 100.0).round() / 100.0;
    if (v - r).abs() > EPS * v.abs().max(1.0) {
        return None;
    }
    let s = format!("{r:.2}");
    Some(s.trim_end_matches('0').trim_end_matches('.').to_string())
}
