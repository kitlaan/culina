//! Phase 3 conformance: inline sigils parse into segments, the printer
//! re-emits them byte-identically, and input sugar canonicalizes.

use culina_core::ast::{Block, Seg, Sigil};
use culina_core::{parse::parse, print::print};

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

fn first_step(input: &str) -> culina_core::ast::Step {
    let doc = parse(input).expect("parse");
    match doc.blocks.into_iter().next() {
        Some(Block::Step(s)) => s,
        other => panic!("expected a step, got {other:?}"),
    }
}

fn head_tags(s: &culina_core::ast::Step) -> Vec<&culina_core::ast::Tag> {
    s.head
        .iter()
        .filter_map(|seg| match seg {
            Seg::Tag(t) => Some(t),
            _ => None,
        })
        .collect()
}

#[test]
fn bare_and_braced_actions() {
    let s = first_step(":mix everything\n");
    assert_eq!(s.action.verb, "mix");
    assert!(s.action.fields.is_empty());

    let s = first_step(":{rest|45 min} _dough\n");
    assert_eq!(s.action.verb, "rest");
    assert_eq!(s.action.fields, vec!["45 min".to_string()]);
}

#[test]
fn braced_tag_fields_and_optional() {
    let s = first_step(":season with @{?makrut lime leaves|2}\n");
    let tags = head_tags(&s);
    assert_eq!(tags.len(), 1);
    let t = tags[0];
    assert_eq!(t.sigil, Sigil::At);
    assert!(t.optional);
    assert_eq!(t.name, "makrut lime leaves");
    assert_eq!(t.fields, vec!["2".to_string()]);
}

#[test]
fn alternatives_parse_as_parallel_groups() {
    let s = first_step(":stir with @{Thai basil|1 handful}{sweet basil|1 handful}\n");
    let tags = head_tags(&s);
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].alternatives.len(), 1);
    assert_eq!(tags[0].alternatives[0].name, "sweet basil");
    assert_eq!(
        tags[0].alternatives[0].fields,
        vec!["1 handful".to_string()]
    );
}

#[test]
fn bare_names_terminate_at_punctuation() {
    // `#oven` ends at the space; `_pains.` ends before the period.
    let s = first_step(":bake _pains in #oven until golden.\n");
    let tags = head_tags(&s);
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0].name, "pains");
    assert_eq!(tags[1].name, "oven");
}

#[test]
fn plus_joins_two_tags() {
    let s = first_step(":brush _croissants + _pains\n");
    assert!(s.head.iter().any(|seg| matches!(seg, Seg::Plus)));
    assert_eq!(head_tags(&s).len(), 2);
}

#[test]
fn arrow_input_sugar_canonicalizes() {
    let doc = parse(":serve _curry → _rice\n").expect("parse");
    let printed = print(&doc);
    assert!(printed.contains("=> _rice"));
    assert!(!printed.contains('\u{2192}'));
}

#[test]
fn unnecessary_braces_canonicalize_to_bare() {
    let doc = parse(":salt @{salt}\n").expect("parse");
    assert_eq!(print(&doc), ":salt @salt\n");
}

#[test]
fn continuation_lines_scan_tags() {
    let doc = parse(":roast _veg\n  (flip _veg halfway)\n").expect("parse");
    match &doc.blocks[0] {
        Block::Step(s) => {
            assert_eq!(s.continuations.len(), 1);
            assert_eq!(s.continuations[0].indent, "  ");
            assert!(s.continuations[0]
                .segs
                .iter()
                .any(|seg| matches!(seg, Seg::Tag(t) if t.name == "veg")));
        }
        other => panic!("expected a step, got {other:?}"),
    }
}

#[test]
fn loud_inline_errors() {
    assert!(parse(":\n").is_err(), "empty action");
    assert!(parse(":mix with @\n").is_err(), "tag without a name");
    assert!(parse(":mix @salt|2 g\n").is_err(), "bare tag with fields");
    assert!(parse(":mix {like this}\n").is_err(), "stray brace in prose");
    assert!(parse(":mix @{flour\n").is_err(), "unterminated brace");
}

#[test]
fn fixtures_idempotent() {
    for src in [CROISSANT, CURRY] {
        let once = print(&parse(src).expect("parse"));
        let twice = print(&parse(&once).expect("reparse"));
        assert_eq!(once, twice);
        assert_eq!(once, src, "canonical form must equal the fixture");
    }
}
