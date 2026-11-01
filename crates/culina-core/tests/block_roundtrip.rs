//! Phase 2 conformance: the stress fixtures parse at block level and the
//! canonical printer reproduces them byte-for-byte. Idempotence
//! (format(format(x)) == format(x)) follows from round-trip + determinism.

use culina_core::ast::Block;
use culina_core::{parse::parse, print::print};

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

#[test]
fn croissant_roundtrips() {
    let doc = parse(CROISSANT).expect("croissant fixture must parse");
    assert_eq!(print(&doc), CROISSANT);
}

#[test]
fn curry_roundtrips() {
    let doc = parse(CURRY).expect("curry fixture must parse");
    assert_eq!(print(&doc), CURRY);
}

#[test]
fn croissant_block_counts() {
    let doc = parse(CROISSANT).expect("parse");
    assert_eq!(doc.section_count(), 5);
    assert_eq!(doc.step_count(), 30);
    assert_eq!(doc.continuation_count(), 0);
    assert_eq!(doc.meta_lines().count(), 4);
}

#[test]
fn curry_block_counts() {
    let doc = parse(CURRY).expect("parse");
    assert_eq!(doc.section_count(), 5);
    assert_eq!(doc.step_count(), 15);
    assert_eq!(doc.continuation_count(), 2);
    assert_eq!(doc.meta_lines().count(), 7);
}

#[test]
fn curry_metadata_keys_in_order() {
    let doc = parse(CURRY).expect("parse");
    let keys: Vec<&str> = doc.meta_lines().map(|m| m.key.as_str()).collect();
    assert_eq!(
        keys,
        ["title", "servings", "cuisine", "dish", "aka", "aka", "summary"]
    );
}

#[test]
fn blocks_self_delimit_without_blanks() {
    // Lenient input: missing blank lines still parse; the printer adds them.
    let doc = parse(":mix => _dough\n# Bake\n:{bake|10 min} _dough").expect("parse");
    assert_eq!(doc.blocks.len(), 3);
    assert_eq!(
        print(&doc),
        ":mix => _dough\n\n# Bake\n\n:{bake|10 min} _dough\n"
    );
}

#[test]
fn blanks_between_steps_collapse_to_none() {
    // Canonical form runs steps adjacent; blank input spacing is lenient.
    let doc = parse(":a\n\n\n\n:b").expect("parse");
    assert_eq!(print(&doc), ":a\n:b\n");
}

#[test]
fn crlf_input_normalizes_to_lf() {
    let doc = parse(">> a: 1\r\n\r\n:step one\r\n").expect("parse");
    assert_eq!(print(&doc), ">> a: 1\n\n:step one\n");
}

#[test]
fn continuation_outside_step_errors() {
    let err = parse("  orphan line").expect_err("must fail");
    assert!(
        err.to_string().contains("line 1"),
        "error should name the line: {err}"
    );
}

#[test]
fn stray_line_errors() {
    assert!(parse("just prose at column zero").is_err());
}

#[test]
fn metadata_without_colon_errors() {
    assert!(parse(">> broken").is_err());
}

#[test]
fn section_without_space_or_title_errors() {
    assert!(parse("#Dough").is_err());
    assert!(parse("# ").is_err());
}

#[test]
fn blank_between_meta_lines_collapses() {
    let doc = parse(">> a: 1\n\n>> b: 2").expect("parse");
    match &doc.blocks[..] {
        [Block::Meta(lines)] => assert_eq!(lines.len(), 2),
        other => panic!("expected one meta block, got {other:?}"),
    }
}
