//! Renderer conformance: the fixtures project into cards with stable
//! classes, aggregated ingredients, authored step prose, and the promised
//! schema.org JSON-LD. Step prose carries refs with `data-amounts` and
//! `data-state` — the hooks CSS experiments hang from.

use culina_core::fmt::prepare;
use culina_core::render::{card, html_fragment, jsonld, RefKind};

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

fn fragment(src: &str) -> String {
    let (doc, _, _) = prepare(src, None).expect("fixtures prepare");
    html_fragment(&card(&doc))
}

#[test]
fn fixture_cards_render() {
    let (doc, _, _) = prepare(CROISSANT, None).unwrap();
    let model = card(&doc);
    let html = html_fragment(&model);

    assert!(html.contains("culina-recipe"), "stable class: {html}");
    assert!(html.contains("culina-title\">Croissants"), "{html}");
    assert!(html.contains("culina-serves\">Serves 12"), "{html}");
    assert!(html.contains("culina-ingredient"), "{html}");
    assert!(html.contains("culina-ing-name\">flour"), "{html}");
    assert!(html.contains("culina-ing-amounts\">500 g"), "{html}");
    assert!(html.contains("culina-section\">Lamination"), "{html}");

    // Plain step prose: verb + tags as names. The production clause is
    // data, not prose — a traditional card never reads "→ base".
    let step = &model.sections[2].1[0];
    assert!(
        step.text.contains("roll dough into an 8x16 in rectangle"),
        "{:?}",
        step.text
    );
    assert!(
        step.text.ends_with("rectangle."),
        "punctuation closes the sentence: {:?}",
        step.text
    );
    assert_eq!(step.produces, vec!["sheet".to_string()]);
    assert!(
        html.contains("data-produces=\"sheet\""),
        "produces ride the li: {html}"
    );
    assert!(!html.contains("→ sheet"), "no arrow in step prose: {html}");

    // The HTML target wraps refs in spans with values as data attributes.
    assert!(
        html.contains("data-amounts=\"500 g\""),
        "ingredient amounts ride the span: {html}"
    );
    assert!(
        html.contains("culina-ref-thread\">dough<"),
        "thread refs classed: {html}"
    );

    let (doc, _, _) = prepare(CURRY, None).unwrap();
    let model = card(&doc);
    let html = html_fragment(&model);
    assert!(html.contains("culina-title\">Red curry"), "{html}");
    assert!(html.contains("also known as"), "aka row: {html}");
    assert!(
        html.contains("culina-optional"),
        "makrut leaves optional: {html}"
    );
    assert!(
        html.contains("or sweet basil"),
        "alternatives join notes: {html}"
    );
    assert!(
        html.contains("data-state=\"medium heat\""),
        "tool state rides the span: {html}"
    );
}

#[test]
fn refs_carry_their_values() {
    let (doc, _, _) = prepare(CROISSANT, None).unwrap();
    let model = card(&doc);
    // The mix step's flour ref: ingredient kind, 500 g value.
    let mix = &model.sections[0].1[0];
    let flour = mix
        .segments
        .iter()
        .find_map(|s| match s {
            culina_core::render::SegModel::Ref {
                kind: RefKind::Ingredient,
                name,
                values,
            } if name == "flour" => Some(values.clone()),
            _ => None,
        })
        .expect("flour ref in the mix step");
    assert_eq!(flour, vec!["500 g".to_string()]);
}

#[test]
fn jsonld_is_schema_recipe() {
    let (doc, _, _) = prepare(CURRY, None).unwrap();
    let v = jsonld(&card(&doc));
    let s = serde_json::to_string(&v).unwrap();
    assert!(s.contains("schema.org"), "{s}");
    assert!(s.contains("Recipe"), "{s}");
    assert!(s.contains("recipeIngredient"), "{s}");
    assert!(s.contains("recipeInstructions"), "{s}");
    assert!(s.contains("recipeYield"), "serves maps to yield: {s}");
    assert!(s.contains("recipeCuisine"), "cuisine maps: {s}");
    assert!(s.contains("alternateName"), "aka maps: {s}");
    // The curry has no authored date; JSON-LD must not invent one.
    assert!(!s.contains("datePublished"), "{s}");
    // Instructions are clean prose — the production clause never leaks.
    assert!(!s.contains("→"), "no arrows in recipeInstructions: {s}");
}

#[test]
fn steps_carry_their_durations() {
    let html = fragment(">> title: T\n>> servings: 1\n\n:mix @{dough|1 cup} => _dough.\n:{bake|10 min} _dough until golden.\n");
    assert!(
        html.contains("data-duration=\"10 min\""),
        "cook-mode hook: {html}"
    );
    // Fuzzy durations are notes — no hook, no invented timer.
    let html =
        fragment(">> title: T\n>> servings: 1\n\n:mix @{flour|1 cup} => _dough.\n:{rest|overnight} _dough.\n");
    assert!(!html.contains("data-duration"), "{html}");
}

#[test]
fn production_clause_is_data_not_prose() {
    // A continuation after a production: the period closes the sentence,
    // then the continuation follows with a space.
    let (doc, _, _) = prepare(CURRY, None).unwrap();
    let model = card(&doc);
    let toss = &model.sections[0].1[0];
    assert!(
        toss.text
            .ends_with("fish sauce. (start this before the paste; it needs 15 min)"),
        "period + continuation: {:?}",
        toss.text
    );
    assert_eq!(toss.produces, vec!["chicken".to_string()]);
}

#[test]
fn cuisine_rides_the_article_for_print_targets() {
    let html = fragment(CURRY);
    assert!(
        html.contains("data-cuisine=\"Thai\""),
        "the color-coded-tab hook: {html}"
    );
    let html = fragment(">> title: Plain\n>> servings: 1\n\n:mix => _dough.\n");
    assert!(
        !html.contains("data-cuisine"),
        "absent when no cuisine: {html}"
    );
}

#[test]
fn authored_prose_is_escaped() {
    let html =
        fragment(">> title: <script>alert(1)</script>\n>> servings: 2\n\n:mix @{flour|1 cup} & @{water|1 cup} well.\n");
    let amp = char::from_u32(0x26).unwrap();
    let lt = format!("{amp}lt;");
    let gt = format!("{amp}gt;");
    let amp_entity = format!("{amp}amp;");
    assert!(
        html.contains(&format!("{lt}script{gt}alert(1){lt}/script{gt}")),
        "script tag escaped: {html}"
    );
    assert!(html.contains(&amp_entity), "ampersand escaped: {html}");
    assert!(!html.contains("<script>alert"), "no raw script tag: {html}");
}
