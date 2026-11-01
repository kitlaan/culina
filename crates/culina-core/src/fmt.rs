//! culinafmt's core: resolve the lexicon, parse, validate, print, stamp.

use crate::ast::{Block, Document, MetaLine};
use crate::error::Result;
use crate::lexicon::Lexicon;
use crate::{dag, parse, print};

/// Canonicalize a document: resolve the lexicon (document registry →
/// collection → core, per `>> lang:`), parse, validate, print.
///
/// Stamping rule (design.md, Provenance and dates): when the formatted text
/// differs from the input, `>> modified: <today>` is stamped or refreshed —
/// an already-canonical file keeps its old stamp, so re-running never dirties
/// a file. `today: None` skips stamping entirely (stdin mode — the stamp is
/// a file fact, and stdout is not a file).
pub fn canonicalize(input: &str, today: Option<&str>) -> Result<String> {
    canonicalize_with(input, today, None)
}

/// Canonicalize with a collection overlay on the tool core, discarding
/// warnings (unknown metadata keys).
pub fn canonicalize_with(
    input: &str,
    today: Option<&str>,
    collection: Option<&Lexicon>,
) -> Result<String> {
    canonicalize_full(input, today, collection).map(|(text, _)| text)
}

/// Canonicalize, returning warnings alongside the text — unknown metadata
/// keys warn and pass through (design.md, Key registry), and callers like
/// culinafmt surface them on stderr.
pub fn canonicalize_full(
    input: &str,
    today: Option<&str>,
    collection: Option<&Lexicon>,
) -> Result<(String, Vec<String>)> {
    let (doc, _lex, warnings) = prepare(input, collection)?;
    let printed = print::print(&doc);
    let out = match today {
        Some(today) if printed != input => {
            let mut doc = doc;
            stamp_modified(&mut doc, today);
            print::print(&doc)
        }
        _ => printed,
    };
    Ok((out, warnings))
}

/// Parse and validate a recipe against the full lexicon resolution order,
/// returning the document for consumers that render rather than reformat
/// (culina-site, future cook-mode). Errors are loud; warnings ride along.
pub fn prepare(
    input: &str,
    collection: Option<&Lexicon>,
) -> Result<(Document, Lexicon, Vec<String>)> {
    let lex = resolve_lexicon(input, collection)?;
    let doc = parse::parse_with(input, &lex)?;
    dag::validate(&doc)?;
    let (errors, warnings) = crate::metadata::validate(&doc, &lex);
    if let Some(e) = errors.into_iter().next() {
        return Err(e);
    }
    Ok((doc, lex, warnings))
}

/// The resolution order (design.md, Lexicons): document registry →
/// collection config → tool core selected by `>> lang:`. The registry and
/// the language live in metadata, which is pre-scanned before the full
/// parse so the field grammar holds the final lexicon in hand. Public so
/// embedders can run the same pipeline as `parse` + consumers.
pub fn resolve_lexicon(input: &str, collection: Option<&Lexicon>) -> Result<Lexicon> {
    let lang = prescan(input, "lang").first().map(|(_, v)| v.clone());
    let mut lex = Lexicon::core_for(lang.as_deref().unwrap_or("en"));
    if let Some(col) = collection {
        lex = lex.overlay(col);
    }
    for (line_no, value) in prescan(input, "unit") {
        lex.register_unit(&value, line_no)?;
    }
    Ok(lex)
}

/// Metadata lines for one key, with their line numbers — in file order.
fn prescan(input: &str, key: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (idx, line) in input.split('\n').enumerate() {
        if let Some(rest) = line.trim_end_matches('\r').strip_prefix(">>") {
            if let Some((k, v)) = rest.trim_start().split_once(':') {
                if k.trim() == key {
                    out.push((idx + 1, v.trim().to_string()));
                }
            }
        }
    }
    out
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
        // Synthetic line — the stamp is added after validation, so no
        // error ever points at it.
        lines.push(MetaLine {
            line_no: 0,
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
