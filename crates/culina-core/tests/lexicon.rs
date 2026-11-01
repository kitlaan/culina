//! Phase 7 conformance: lexicons — channels, loading, the +s plural rule,
//! overlay semantics, the document registry, language pre-scan, and the
//! doneness lift.

use culina_core::ast::{Block, Seg, Sigil};
use culina_core::fmt::{canonicalize, canonicalize_with};
use culina_core::lexicon::Lexicon;
use culina_core::parse::parse;
use culina_core::units::Dimension;

fn first_step(input: &str) -> culina_core::ast::Step {
    match parse(input).expect("parse").blocks.into_iter().next() {
        Some(Block::Step(s)) => s,
        other => panic!("expected a step, got {other:?}"),
    }
}

#[test]
fn english_core_loads() {
    let lex = Lexicon::english();
    assert_eq!(lex.lang, "en");
    assert!(lex.approx.words.contains(&"about".to_string()));
    assert!(lex.doneness.words.contains(&"until".to_string()));
    assert_eq!(lex.fuzzy("overnight").unwrap().min, 480.0);
    assert_eq!(lex.fuzzy("overnight").unwrap().max, 720.0);
    let knead = lex.verb("knead").unwrap();
    assert!(knead.timer);
    assert_eq!(knead.tense[2], "kneaded");
    assert!(!lex.verb("serve").unwrap().timer);
    let tsp = lex.units.get("tsp").unwrap();
    assert_eq!(tsp.dim, Dimension::Volume);
    assert_eq!(tsp.short.as_deref(), Some("t."));
    assert_eq!(tsp.long.as_deref(), Some("teaspoon"));
    let g = lex.units.get("g").unwrap();
    assert_eq!(g.factor, Some(1.0));
}

#[test]
fn plural_rule_matches_singular_canonical() {
    // "2 hours" parses as the canonical unit "hour", spelled "hours".
    let s = first_step(":{simmer|2 hours} _curry\n");
    let unit = s.action.fields[0].quantity().unwrap().unit.clone().unwrap();
    assert_eq!(unit.name, "hour");
    assert_eq!(unit.spelling, "hours");
    assert_eq!(unit.factor, Some(3600.0));

    let s = first_step(":x @{garlic|6 cloves}\n");
    let unit = s
        .head
        .iter()
        .find_map(|seg| match seg {
            Seg::Tag(t) if t.sigil == Sigil::At => t.amounts().first().map(|q| q.unit.clone()),
            _ => None,
        })
        .flatten()
        .unwrap();
    assert_eq!(unit.name, "clove");
    assert_eq!(unit.dim, Dimension::Count);
}

#[test]
fn overlay_replaces_word_level_and_unions_lists() {
    let base = Lexicon::english();
    let overlay_toml = r#"
[approx]
words = ["circa"]

[units.tsp]
dim = "volume"
factor = 9.9e-3
short = "tspx"
long = "teaspoon"
"#;
    let over = Lexicon::from_toml(overlay_toml).expect("overlay parses");
    let merged = base.overlay(&over);
    // list channels union
    assert!(merged.approx.words.contains(&"about".to_string()));
    assert!(merged.approx.words.contains(&"circa".to_string()));
    // map entries replace whole
    assert_eq!(merged.units.get("tsp").unwrap().factor, Some(9.9e-3));
    assert_eq!(
        merged.units.get("tsp").unwrap().short.as_deref(),
        Some("tspx")
    );
    // core units survive
    assert!(merged.units.contains_key("g"));
}

#[test]
fn document_registry_defines_units_through_existing_ones() {
    let input = ">> title: T\n>> unit: verre = 90 ml\n\n:season with @{wine|2 verre}\n";
    // The registry is part of the canonicalize pipeline, not plain parse:
    // resolve, then parse against the resolved lexicon.
    let lex = culina_core::fmt::resolve_lexicon(input, None).expect("registry resolves");
    let doc = culina_core::parse::parse_with(input, &lex).expect("registry parses");
    // Without the registry, "2 verre" would demote to a note; with it,
    // the quantity classifies and resolves through ml.
    match &doc.blocks[1] {
        Block::Step(s) => {
            let tag = s
                .head
                .iter()
                .find_map(|seg| match seg {
                    Seg::Tag(t) if t.sigil == Sigil::At => Some(t),
                    _ => None,
                })
                .unwrap();
            let q = tag.amounts()[0];
            let unit = q.unit.as_ref().unwrap();
            assert_eq!(unit.name, "verre");
            assert_eq!(unit.dim, Dimension::Volume);
            assert_eq!(unit.factor, Some(0.09));
        }
        other => panic!("expected a step, got {other:?}"),
    }

    // And the negative control: no registry, verre is folk prose.
    let plain = parse(":season with @{wine|2 verre}\n").expect("parse");
    match &plain.blocks[0] {
        Block::Step(s) => {
            let tag = s
                .head
                .iter()
                .find_map(|seg| match seg {
                    Seg::Tag(t) if t.sigil == Sigil::At => Some(t),
                    _ => None,
                })
                .unwrap();
            assert!(
                tag.amounts().is_empty(),
                "unregistered verre demotes to note"
            );
        }
        other => panic!("expected a step, got {other:?}"),
    }
}

#[test]
fn bad_registry_line_is_loud_in_canonicalize() {
    let err = canonicalize(
        ">> title: T\n>> unit: verre 90 ml\n\n:mix => _dough\n",
        None,
    )
    .expect_err("missing '=' must fail");
    assert!(err.to_string().contains("line 2"), "{err}");
    assert!(err.to_string().contains("verre"), "{err}");
}

#[test]
fn lang_prescan_falls_back_to_english() {
    // Only the English core ships; fr falls back without failing.
    let out = canonicalize(">> title: T\n>> lang: fr\n\n:mix => _dough\n", None)
        .expect("falls back to the English core");
    assert!(out.contains(">> lang: fr"));
}

#[test]
fn doneness_lift_attaches_prose_spans() {
    let s = first_step(":bake _loaf in #oven until golden.\n");
    assert_eq!(s.doneness, vec!["until golden.".to_string()]);

    // "or until" rides the same marker; the span starts at the marker word.
    let s = first_step(":bake _loaf, checking after 20 min or until deep golden.\n");
    assert_eq!(s.doneness, vec!["until deep golden.".to_string()]);

    // No marker, no lift.
    let s = first_step(":mix @{flour|200 g} until-done markerless.\n");
    assert!(s.doneness.is_empty());
}

#[test]
fn collection_overlay_flows_through_canonicalize() {
    let overlay = Lexicon::from_toml(
        r#"
[units.banane]
dim = "count"
long = "banana unit"
"#,
    )
    .expect("overlay parses");

    let input = ">> title: T\n\n:season with @{wine|2 banane}\n";
    let bare = canonicalize(input, None).expect("parses");
    assert!(bare.contains("2 banane"), "raw text preserved: {bare}");

    let _ = canonicalize_with(input, None, Some(&overlay)).expect("overlay parses the recipe");
}
