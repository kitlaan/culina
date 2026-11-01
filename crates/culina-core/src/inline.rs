//! Inline sigil scanner: turns a step's physical lines into segments —
//! prose, tags, `+` joins, `=>` productions.
//!
//! Phase 3 keeps `|` fields raw; quantity-vs-note classification is the
//! field-grammar phase. Input sugar accepted here: `→` parses as an arrow
//! and prints as `=>`.

use crate::ast::{Action, AltGroup, Continuation, Seg, Sigil, Step, Tag};
use crate::error::{ParseError, Result};
use crate::fields::{check_partition, classify, Field};

/// Characters that end a bare tag name or a bare verb. Everything else up to
/// whitespace is name material — including non-ASCII, so Thai thread names
/// work bare if they are single words.
const BARE_ENDS: &str = "|}{=+.,;:!?()[]";

/// A step in flight while the block parser walks lines.
pub struct StepAcc {
    pub line_no: usize,
    pub head: String,
    pub continuations: Vec<(usize, String)>,
}

pub fn parse_step(acc: &StepAcc) -> Result<Step> {
    let (action, rest) = parse_action(acc.line_no, &acc.head)?;
    let head = scan(acc.line_no, rest)?;
    let continuations = acc
        .continuations
        .iter()
        .map(|(line_no, line)| {
            let trimmed = line.trim_start_matches([' ', '\t']);
            let indent = &line[..line.len() - trimmed.len()];
            let segs = scan(*line_no, trimmed)?;
            Ok(Continuation {
                indent: indent.to_string(),
                segs,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Step {
        action,
        head,
        continuations,
    })
}

fn parse_action(line_no: usize, head: &str) -> Result<(Action, &str)> {
    // The block parser only routes lines that start with ':'.
    let rest = &head[1..];
    if let Some(open) = rest.strip_prefix('{') {
        let end = open
            .find('}')
            .ok_or(ParseError::UnterminatedBrace { line: line_no })?;
        let body = &open[..end];
        let (verb, fields) = split_name_fields(line_no, body)?;
        if verb.is_empty() {
            return Err(ParseError::EmptyAction { line: line_no });
        }
        Ok((Action { verb, fields }, &open[end + 1..]))
    } else {
        let mut end = 0;
        for c in rest.chars() {
            if c.is_whitespace() || BARE_ENDS.contains(c) {
                break;
            }
            end += c.len_utf8();
        }
        if end == 0 {
            return Err(ParseError::EmptyAction { line: line_no });
        }
        Ok((
            Action {
                verb: rest[..end].to_string(),
                fields: Vec::new(),
            },
            &rest[end..],
        ))
    }
}

fn scan(line_no: usize, text: &str) -> Result<Vec<Seg>> {
    let chars: Vec<char> = text.chars().collect();
    let mut segs = Vec::new();
    let mut prose = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '@' | '#' | '_' => {
                if !prose.is_empty() {
                    segs.push(Seg::Prose(std::mem::take(&mut prose)));
                }
                let (tag, next) = parse_tag(line_no, &chars, i)?;
                segs.push(Seg::Tag(tag));
                i = next;
            }
            '=' if chars.get(i + 1) == Some(&'>') => {
                if !prose.is_empty() {
                    segs.push(Seg::Prose(std::mem::take(&mut prose)));
                }
                segs.push(Seg::Arrow);
                i += 2;
            }
            '→' => {
                if !prose.is_empty() {
                    segs.push(Seg::Prose(std::mem::take(&mut prose)));
                }
                segs.push(Seg::Arrow);
                i += 1;
            }
            '+' if standalone_plus(&chars, i) => {
                if !prose.is_empty() {
                    segs.push(Seg::Prose(std::mem::take(&mut prose)));
                }
                segs.push(Seg::Plus);
                i += 1;
            }
            '{' => return Err(ParseError::StrayBrace { line: line_no }),
            _ => {
                prose.push(c);
                i += 1;
            }
        }
    }
    if !prose.is_empty() {
        segs.push(Seg::Prose(prose));
    }
    Ok(segs)
}

fn standalone_plus(chars: &[char], i: usize) -> bool {
    i > 0 && chars[i - 1].is_whitespace() && chars.get(i + 1).is_some_and(|c| c.is_whitespace())
}

fn parse_tag(line_no: usize, chars: &[char], start: usize) -> Result<(Tag, usize)> {
    let sigil = match chars[start] {
        '@' => Sigil::At,
        '#' => Sigil::Hash,
        _ => Sigil::Underscore,
    };

    if chars.get(start + 1) == Some(&'{') {
        // Braced form: {name|field|field} with optional parallel groups.
        let mut i = start + 2;
        let mut optional = false;
        if chars.get(i) == Some(&'?') {
            optional = true;
            i += 1;
        }
        let body = collect_braced(line_no, chars, &mut i)?;
        let (name, fields) = split_name_fields(line_no, &body)?;
        if name.is_empty() {
            return Err(ParseError::EmptyTagName {
                line: line_no,
                sigil: sigil.as_char(),
            });
        }
        let mut alternatives = Vec::new();
        while chars.get(i) == Some(&'{') {
            i += 1;
            let abody = collect_braced(line_no, chars, &mut i)?;
            let (name, fields) = split_name_fields(line_no, &abody)?;
            if name.is_empty() {
                return Err(ParseError::EmptyTagName {
                    line: line_no,
                    sigil: sigil.as_char(),
                });
            }
            alternatives.push(AltGroup { name, fields });
        }
        Ok((
            Tag {
                sigil,
                optional,
                name,
                fields,
                alternatives,
            },
            i,
        ))
    } else {
        // Bare form: single-word name, no fields, no alternatives.
        let mut i = start + 1;
        let mut optional = false;
        if chars.get(i) == Some(&'?') {
            optional = true;
            i += 1;
        }
        let mut name = String::new();
        while i < chars.len() {
            let c = chars[i];
            if c.is_whitespace() || BARE_ENDS.contains(c) {
                break;
            }
            name.push(c);
            i += 1;
        }
        if name.is_empty() {
            return Err(ParseError::EmptyTagName {
                line: line_no,
                sigil: sigil.as_char(),
            });
        }
        // A bare tag never carries fields — braces are required for that.
        if chars.get(i) == Some(&'|') {
            return Err(ParseError::BareTagWithFields {
                line: line_no,
                sigil: sigil.as_char(),
            });
        }
        Ok((
            Tag {
                sigil,
                optional,
                name,
                fields: Vec::new(),
                alternatives: Vec::new(),
            },
            i,
        ))
    }
}

fn collect_braced(line_no: usize, chars: &[char], i: &mut usize) -> Result<String> {
    let mut body = String::new();
    loop {
        match chars.get(*i) {
            Some('}') => {
                *i += 1;
                return Ok(body);
            }
            Some(&c) => {
                body.push(c);
                *i += 1;
            }
            None => return Err(ParseError::UnterminatedBrace { line: line_no }),
        }
    }
}

/// Split a brace body into name plus classified fields, then check the
/// partition rule: quantities before notes, loud on violation.
fn split_name_fields(line_no: usize, body: &str) -> Result<(String, Vec<Field>)> {
    let (name, rest) = match body.split_once('|') {
        Some((n, r)) => (n, r),
        None => return Ok((body.to_string(), Vec::new())),
    };
    let mut fields = Vec::new();
    if !rest.is_empty() {
        for raw in rest.split('|') {
            fields.push(Field {
                raw: raw.to_string(),
                kind: classify(raw),
            });
        }
    }
    check_partition(line_no, &fields)?;
    Ok((name.to_string(), fields))
}
