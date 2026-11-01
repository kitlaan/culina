//! Renderer conformance: the fixtures project into cards with stable
//! classes, aggregated ingredients, authored step prose, and the promised
//! schema.org JSON-LD.

use culina_core::fmt::prepare;
use culina_core::render::{card, html_fragment, jsonld};

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

fn fragment(src: &str) -> String {
    let (doc, _, _) = prepare(src, None).expect("fixtures prepare");
    html_fragment(&card(&doc))
}

#[test]
fn fixture_cards_render() {
    let html = fragment(CROISSANT);
    assert!(html.contains("culina-recipe"), "stable class: {html}");
    assert!(html.contains("culina-title\">Croissants"), "{html}");
    assert!(html.contains("culina-serves\">Serves 12"), "{html}");
    assert!(html.contains("culina-ingredient"), "{html}");
    assert!(html.contains("culina-ing-name\">flour"), "{html}");
    assert!(html.contains("500 g"), "{html}");
    // Sections render as headers; step prose strips sigils.
    assert!(html.contains("culina-section\">Lamination"), "{html}");
    assert!(
        html.contains("culina-step\">roll dough into an 8x16 in rectangle"),
        "{html}"
    );
    // The production arrow reads as prose.
    assert!(html.contains("→ sheet"), "{html}");

    let html = fragment(CURRY);
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
