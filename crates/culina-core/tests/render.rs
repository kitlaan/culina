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

// --- Notes (design.md, key semantics) ---

#[test]
fn notes_render_after_the_method() {
    let html = fragment(
        ">> title: T\n>> servings: 1\n>> note: Make a day ahead; the braise deepens overnight.\n>> note: Save the bone & the oil\n\n:mix @{flour|1 cup} => _dough.\n",
    );
    assert!(html.contains("<aside class=\"culina-notes\">"), "{html}");
    assert!(
        html.contains(
            "<p class=\"culina-note\">Make a day ahead; the braise deepens overnight.</p>"
        ),
        "one paragraph per note line: {html}"
    );
    // Note prose escapes like all authored prose.
    let amp = char::from_u32(0x26).unwrap();
    assert!(
        html.contains(&format!(
            "<p class=\"culina-note\">Save the bone {amp}amp; the oil</p>"
        )),
        "ampersand escaped: {html}"
    );
    // The aside trails the method div.
    let method = html.find("culina-method").expect("method div");
    let notes = html.find("culina-notes").expect("notes aside");
    assert!(method < notes, "notes after the method: {html}");
    // Notes are body content, not a meta row.
    assert!(
        !html.contains("<dt class=\"culina-meta-key\">note</dt>"),
        "notes never ride the meta dl: {html}"
    );
    // No authored notes, no aside.
    let bare = fragment(">> title: T\n>> servings: 1\n\n:mix => _dough.\n");
    assert!(!bare.contains("culina-notes"), "{bare}");
}

#[test]
fn note_commas_are_content() {
    // Prose never splits — "A, B, C" is one note, not three.
    let (doc, _, _) = prepare(
        ">> title: T\n>> servings: 1\n>> note: A, B, C\n\n:mix => _dough.\n",
        None,
    )
    .unwrap();
    let model = card(&doc);
    assert_eq!(model.notes, vec!["A, B, C".to_string()]);
}

#[test]
fn notes_stay_out_of_jsonld() {
    // schema.org has no honest slot for notes — the JSON-LD omits them.
    let (doc, _, _) = prepare(
        ">> title: T\n>> servings: 1\n>> note: save the bone\n\n:mix => _dough.\n",
        None,
    )
    .unwrap();
    let s = serde_json::to_string(&jsonld(&card(&doc))).unwrap();
    assert!(!s.contains("save the bone"), "{s}");
}

// --- Meta rows (design.md, `>> source:`) ---

#[test]
fn url_meta_values_carry_the_wrap_class() {
    // A URL is one unbreakable word; the class is the hook stylesheets
    // use to let it wrap instead of stretching the row past the card.
    let html = fragment(
        ">> title: T\n>> servings: 1\n>> source: https://example.com/a-long-source-link\n>> dish: soup\n\n:mix => _dough.\n",
    );
    assert!(
        html.contains(
            "<dd class=\"culina-meta-value culina-meta-url\">https://example.com/a-long-source-link</dd>"
        ),
        "url value tagged: {html}"
    );
    assert!(
        html.contains("<dd class=\"culina-meta-value\">soup</dd>"),
        "plain values stay untagged: {html}"
    );
    assert_eq!(html.matches("culina-meta-url").count(), 1, "{html}");
}

// --- Images (design.md, key semantics) ---

#[test]
fn first_image_is_the_hero_and_the_rest_trail() {
    let html = fragment(
        ">> title: T\n>> servings: 1\n>> image: photos/top.jpg\n>> image: photos/braise.jpg\n>> note: save the bone\n\n:mix => _dough.\n",
    );
    assert!(
        html.contains("<img class=\"culina-photo\" src=\"photos/top.jpg\""),
        "first image is the hero: {html}"
    );
    assert!(
        html.contains("<img class=\"culina-gallery-item\" src=\"photos/braise.jpg\""),
        "second image rides the gallery: {html}"
    );
    // The gallery trails the notes — words finish as one run, then plates.
    let notes = html.find("culina-notes").expect("notes aside");
    let gallery = html.find("culina-gallery").expect("gallery div");
    assert!(notes < gallery, "gallery after the notes: {html}");
    // The hero never duplicates into the gallery.
    assert_eq!(html.matches("culina-gallery-item").count(), 1, "{html}");
}

#[test]
fn single_image_needs_no_gallery() {
    let html = fragment(">> title: T\n>> servings: 1\n>> image: a.jpg\n\n:mix => _dough.\n");
    assert!(
        html.contains("class=\"culina-photo\" src=\"a.jpg\" alt=\"T\" loading=\"eager\""),
        "hero loads eager — it is the largest contentful paint: {html}"
    );
    assert!(
        !html.contains("culina-gallery"),
        "no gallery div for one image: {html}"
    );
}

#[test]
fn image_jsonld_is_a_string_for_one_and_an_array_for_more() {
    let (doc, _, _) = prepare(
        ">> title: T\n>> servings: 1\n>> image: a.jpg\n\n:mix => _dough.\n",
        None,
    )
    .unwrap();
    assert_eq!(jsonld(&card(&doc))["image"], serde_json::json!("a.jpg"));

    let (doc, _, _) = prepare(
        ">> title: T\n>> servings: 1\n>> image: a.jpg\n>> image: b.jpg\n\n:mix => _dough.\n",
        None,
    )
    .unwrap();
    assert_eq!(
        jsonld(&card(&doc))["image"],
        serde_json::json!(["a.jpg", "b.jpg"])
    );
}
