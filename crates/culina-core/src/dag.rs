//! DAG validation: threads resolve, productions are well-formed, the graph
//! stays honest.
//!
//! Model (design.md, Threads and the DAG): steps are nodes, products are
//! threads. Consuming a thread without producing keeps it live — rests,
//! simmers, brushes. A step with a production clause (`=>`) ends the threads
//! it consumes and binds the ones it produces: rename is rebind, and a fork
//! names distinct groups. A step with neither `_` nor `=>` is a setup step
//! and stays outside the food DAG.
//!
//! Line numbers point at the step's head line — the unit the author edits.

use std::collections::HashMap;

use crate::ast::{Document, Seg, Sigil, Step, Tag};
use crate::error::{ParseError, Result};
use crate::fields::{FieldKind, Value};

pub fn validate(doc: &Document) -> Result<()> {
    let mut live: Vec<String> = Vec::new();
    // Dead threads and what they became, for honest error messages.
    let mut ended: HashMap<String, (usize, String)> = HashMap::new();

    for step in doc.steps() {
        let segs: Vec<&Seg> = step
            .head
            .iter()
            .chain(step.continuations.iter().flat_map(|c| c.segs.iter()))
            .collect();

        // '?' is for @ names only.
        for seg in &segs {
            if let Seg::Tag(t) = seg {
                if t.optional && t.sigil != Sigil::At {
                    return Err(ParseError::OptionalOutsideAt { line: step.line_no });
                }
            }
        }

        if segs.iter().filter(|s| matches!(s, Seg::Arrow)).count() > 1 {
            return Err(ParseError::SecondProduction { line: step.line_no });
        }
        let arrow_at = segs.iter().position(|s| matches!(s, Seg::Arrow));

        let (consumed, produced): (Vec<&Tag>, Vec<&Tag>) = match arrow_at {
            Some(at) => (
                segs[..at].iter().filter_map(|s| as_thread(s)).collect(),
                segs[at + 1..].iter().filter_map(|s| as_thread(s)).collect(),
            ),
            None => (
                segs.iter().filter_map(|s| as_thread(s)).collect(),
                Vec::new(),
            ),
        };

        // Consumptions must resolve against the live set, and must be bare —
        // piece selection is deferred; forks name their groups.
        for t in &consumed {
            if !t.fields.is_empty() {
                return Err(ParseError::MultiplicityOnConsumption {
                    line: step.line_no,
                    name: t.name.clone(),
                });
            }
            if !live.contains(&t.name) {
                if let Some((at_line, became)) = ended.get(&t.name) {
                    return Err(ParseError::ThreadEnded {
                        line: step.line_no,
                        name: t.name.clone(),
                        at_line: *at_line,
                        became: became.clone(),
                    });
                }
                return Err(ParseError::ThreadNeverCreated {
                    line: step.line_no,
                    name: t.name.clone(),
                });
            }
        }

        if arrow_at.is_some() && produced.is_empty() {
            return Err(ParseError::EmptyProduction { line: step.line_no });
        }

        // Productions: shape and liveness. Producing a live name is a
        // rename only when the step also consumes it.
        let consumed_names: Vec<&str> = consumed.iter().map(|t| t.name.as_str()).collect();
        for t in &produced {
            check_multiplicity(step, t)?;
            if live.contains(&t.name) && !consumed_names.contains(&t.name.as_str()) {
                return Err(ParseError::ThreadAlreadyLive {
                    line: step.line_no,
                    name: t.name.clone(),
                });
            }
        }

        // Liveness update: a producing step ends what it consumes; the
        // consumed threads become the produced ones.
        let became = produced
            .iter()
            .map(|t| format!("_{}", t.name))
            .collect::<Vec<_>>()
            .join(" + ");
        if arrow_at.is_some() {
            for t in &consumed {
                live.retain(|n| n != &t.name);
                ended.insert(t.name.clone(), (step.line_no, became.clone()));
            }
        }
        for t in &produced {
            ended.remove(&t.name);
            if !live.contains(&t.name) {
                live.push(t.name.clone());
            }
        }
    }
    Ok(())
}

fn as_thread(seg: &Seg) -> Option<&Tag> {
    match seg {
        Seg::Tag(t) if t.sigil == Sigil::Underscore => Some(t),
        _ => None,
    }
}

fn check_multiplicity(step: &Step, t: &Tag) -> Result<()> {
    if t.fields.is_empty() {
        return Ok(()); // implicit count of one
    }
    let ok = match t.fields.as_slice() {
        [f] => match &f.kind {
            FieldKind::Quantity(q) => matches!(q.value, Value::Single(n) if n.fract() == 0.0),
            _ => false,
        },
        _ => false,
    };
    if !ok {
        let found = t
            .fields
            .iter()
            .map(|f| f.raw.as_str())
            .collect::<Vec<_>>()
            .join("|");
        return Err(ParseError::BadMultiplicity {
            line: step.line_no,
            name: t.name.clone(),
            found,
        });
    }
    Ok(())
}
