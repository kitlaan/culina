//! Phase 4 conformance: the field grammar — split, classify, partition.
//! The worked-cases table in design.md (The field grammar) is reproduced
//! here test for test, plus classification for actions and tool states.

use culina_core::ast::{Block, Seg, Sigil};
use culina_core::fields::{Quantity, Value};
use culina_core::units::Dimension;
use culina_core::{parse::parse, print::print};

fn first_step(input: &str) -> culina_core::ast::Step {
    let doc = parse(input).expect("parse");
    match doc.blocks.into_iter().next() {
        Some(Block::Step(s)) => s,
        other => panic!("expected a step, got {other:?}"),
    }
}

fn first_at(input: &str) -> culina_core::ast::Tag {
    let s = first_step(input);
    s.head
        .iter()
        .find_map(|seg| match seg {
            Seg::Tag(t) if t.sigil == Sigil::At => Some(t),
            _ => None,
        })
        .cloned()
        .expect("an @ tag in the head")
}

fn first_hash(input: &str) -> culina_core::ast::Tag {
    let s = first_step(input);
    s.head
        .iter()
        .find_map(|seg| match seg {
            Seg::Tag(t) if t.sigil == Sigil::Hash => Some(t),
            _ => None,
        })
        .cloned()
        .expect("a # tag in the head")
}

fn q(f: &culina_core::fields::Field) -> &Quantity {
    f.quantity().expect("field should be a quantity")
}

// --- The worked-cases table, design.md, "The field grammar" ---

#[test]
fn equivalency_range_and_unit() {
    let t = first_at(":x @{paprika|1-2 g|1 tsp}\n");
    assert_eq!(t.fields.len(), 2);
    let a = q(&t.fields[0]);
    assert_eq!(a.value, Value::Range(1.0, 2.0));
    assert_eq!(a.unit.as_ref().unwrap().name, "g");
    assert_eq!(a.dim(), Some(Dimension::Mass));
    let b = q(&t.fields[1]);
    assert_eq!(b.value, Value::Single(1.0));
    assert_eq!(b.unit.as_ref().unwrap().name, "tsp");
}

#[test]
fn amount_then_note() {
    let t = first_at(":x @{onion|2|diced}\n");
    assert!(t.fields[0].is_quantity());
    assert_eq!(t.fields[0].raw, "2");
    assert!(!t.fields[1].is_quantity());
    assert_eq!(t.fields[1].raw, "diced");
}

#[test]
fn trailing_word_kills_quantity_classification() {
    // Full consumption: "1/4-inch dice" has a trailing word, so it is a
    // note — the quantity grammar never partially matches.
    let t = first_at(":x @{potato|2|1/4-inch dice}\n");
    assert!(t.fields[0].is_quantity());
    assert!(!t.fields[1].is_quantity());
    assert_eq!(t.fields[1].raw, "1/4-inch dice");
}

#[test]
fn count_mass_equivalency() {
    let t = first_at(":x @{onion|1|110 g}\n");
    let a = q(&t.fields[0]);
    assert_eq!(a.value, Value::Single(1.0));
    assert!(a.unit.is_none());
    let b = q(&t.fields[1]);
    assert_eq!(b.value, Value::Single(110.0));
    assert_eq!(b.unit.as_ref().unwrap().name, "g");
}

#[test]
fn approx_prefix_strips_and_flags() {
    let t = first_at(":x @{onion|1|about 110 g}\n");
    let b = q(&t.fields[1]);
    assert!(b.approx);
    assert_eq!(b.value, Value::Single(110.0));
    assert_eq!(b.unit.as_ref().unwrap().name, "g");
}

#[test]
fn count_unit_then_provenance_note() {
    let t = first_at(":x @{tomato sauce|1 batch|from r}\n");
    let a = q(&t.fields[0]);
    assert_eq!(a.value, Value::Single(1.0));
    assert_eq!(a.unit.as_ref().unwrap().dim, Dimension::Count);
    assert!(!t.fields[1].is_quantity());
    assert_eq!(t.fields[1].raw, "from r");
}

#[test]
fn percent_escape_forces_unknown_unit() {
    let t = first_at(":x @{flour|1%glug|sifted}\n");
    let a = q(&t.fields[0]);
    assert_eq!(a.value, Value::Single(1.0));
    assert_eq!(a.unit.as_ref().unwrap().name, "glug");
    assert_eq!(a.dim(), Some(Dimension::Other));
    assert!(!t.fields[1].is_quantity());
    assert_eq!(t.fields[1].raw, "sifted");
}

#[test]
fn implicit_one_when_no_amount() {
    let t = first_at(":x @{onion|diced}\n");
    assert!(t.amounts().is_empty());
    assert!(!t.fields[0].is_quantity());
}

#[test]
fn quantity_after_note_is_a_loud_error() {
    let err = parse(":x @{onion|diced|2}\n").expect_err("must fail loudly");
    assert!(
        err.to_string().contains("line 1"),
        "error names the line: {err}"
    );
    assert!(
        err.to_string().contains("'2'"),
        "error names the field: {err}"
    );
}

// --- Classification beyond the table ---

#[test]
fn glued_units_parse() {
    let t = first_hash(":x #{oven|350F}\n");
    let s = q(&t.fields[0]);
    assert_eq!(s.value, Value::Single(350.0));
    assert_eq!(s.dim(), Some(Dimension::Temperature));

    let t = first_at(":x @{flour|20g}\n");
    let s = q(&t.fields[0]);
    assert_eq!(s.value, Value::Single(20.0));
    assert_eq!(s.dim(), Some(Dimension::Mass));
}

#[test]
fn mixed_numbers_and_ascii_fractions() {
    let t = first_at(":x @{water|1 1/3 tsp}\n");
    assert_eq!(q(&t.fields[0]).value, Value::Single(4.0 / 3.0));

    let t = first_at(":x @{panel|1/8 in}\n");
    assert_eq!(q(&t.fields[0]).value, Value::Single(0.125));
    assert_eq!(q(&t.fields[0]).dim(), Some(Dimension::Length));
}

#[test]
fn approx_prefix_with_glued_unit_and_tilde() {
    let t = first_at(":x @{flour|about 20g}\n");
    assert!(q(&t.fields[0]).approx);

    let t = first_at(":x @{water|~2 cups}\n");
    let a = q(&t.fields[0]);
    assert!(a.approx);
    assert_eq!(a.value, Value::Single(2.0));
}

#[test]
fn action_time_fields_classify_as_time() {
    let s = first_step(":{rest|1 hour} _dough\n");
    assert!(q(&s.action.fields[0]).is_time());

    let s = first_step(":{simmer|2 hours} _curry\n");
    assert!(q(&s.action.fields[0]).is_time());
}

#[test]
fn action_conditions_and_tool_word_states_are_notes() {
    let s = first_step(":{rest|doubled} _dough\n");
    assert!(!s.action.fields[0].is_quantity());

    let t = first_hash(":x #{pot|lid on}\n");
    assert!(!t.fields[0].is_quantity());

    let t = first_hash(":x #{wok|medium}\n");
    assert!(!t.fields[0].is_quantity());
}

#[test]
fn longest_match_prefers_tbsp_and_plurals() {
    let t = first_at(":x @{sugar|1 tbsp}\n");
    assert_eq!(q(&t.fields[0]).unit.as_ref().unwrap().name, "tbsp");

    let s = first_step(":{simmer|2 hours} _curry\n");
    assert_eq!(q(&s.action.fields[0]).unit.as_ref().unwrap().name, "hours");
}

#[test]
fn lenient_range_spacing() {
    let t = first_at(":x @{paprika|1 - 2 g}\n");
    assert_eq!(q(&t.fields[0]).value, Value::Range(1.0, 2.0));
}

#[test]
fn partition_error_in_alternative_group() {
    assert!(parse(":x @{A|1}{B|diced|2}\n").is_err());
}

#[test]
fn fixtures_still_roundtrip_and_still_parse() {
    let croissant = include_str!("fixtures/croissant.culina");
    let curry = include_str!("fixtures/red-curry.culina");
    for src in [croissant, curry] {
        let doc = parse(src).expect("fixtures must parse with field classification");
        assert_eq!(print(&doc), src);
    }
}
