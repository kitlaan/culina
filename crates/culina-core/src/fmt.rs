//! culinafmt's core: parse, validate, print, stamp.

use crate::ast::{Block, Document, MetaLine};
use crate::error::Result;
use crate::{dag, parse, print};

/// Canonicalize a document: parse, validate, print.
///
/// Stamping rule (design.md, Provenance and dates): when the formatted text
/// differs from the input, `>> modified: <today>` is stamped or refreshed —
/// an already-canonical file keeps its old stamp, so re-running never dirties
/// a file. `today: None` skips stamping entirely (stdin mode — the stamp is
/// a file fact, and stdout is not a file).
pub fn canonicalize(input: &str, today: Option<&str>) -> Result<String> {
    let doc = parse::parse(input)?;
    dag::validate(&doc)?;
    let printed = print::print(&doc);
    match today {
        Some(today) if printed != input => {
            let mut doc = doc;
            stamp_modified(&mut doc, today);
            Ok(print::print(&doc))
        }
        _ => Ok(printed),
    }
}

/// Refresh an existing `modified` line in place, or append one to the first
/// metadata block. culinafmt maintains the stamp; it never creates metadata
/// for a file without any.
fn stamp_modified(doc: &mut Document, today: &str) {
    for block in &mut doc.blocks {
        if let Block::Meta(lines) = block {
            for m in lines.iter_mut() {
                if m.key == "modified" {
                    m.value = today.to_string();
                    return;
                }
            }
        }
    }
    if let Some(Block::Meta(lines)) = doc.blocks.first_mut() {
        lines.push(MetaLine {
            key: "modified".into(),
            value: today.into(),
        });
    }
}

/// Today's UTC date, `YYYY-MM-DD`. UTC for determinism — it is a stamp, not
/// an appointment.
pub fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's civil-from-days: days since the Unix epoch to a
/// Gregorian date. Hand-rolled to keep the dependency list at near-zero.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn epoch_day() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn leap_day() {
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }

    #[test]
    fn this_session() {
        assert_eq!(civil_from_days(20_758), (2026, 11, 1));
    }
}
