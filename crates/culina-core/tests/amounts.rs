//! Ingredient amount aggregation (design.md, Quantities and units): one
//! amount list per use, a total only when every use carries the bucket,
//! never a unit the author did not write.

use culina_core::amounts::{summarize, Amount, Summary};
use culina_core::fields::classify;
use culina_core::lexicon::Lexicon;

fn uses(lists: &[&[&str]]) -> Vec<Vec<Amount>> {
    let lex = Lexicon::english();
    lists
        .iter()
        .map(|u| {
            u.iter()
                .map(|raw| {
                    let kind = classify(&lex, raw);
                    Amount::from_field(&culina_core::fields::Field {
                        raw: raw.to_string(),
                        kind,
                    })
                    .unwrap_or_else(|| panic!("{raw} is a quantity"))
                })
                .collect()
        })
        .collect()
}

fn sum(lists: &[&[&str]]) -> Summary {
    summarize(&uses(lists))
}

#[test]
fn water_sums_cups_and_grams_separately() {
    // 2 5/6 cups is not a measure: three quarters, then the spoons.
    let s = sum(&[&["2 1/2 cups", "600 g"], &["1/3 cup", "80 g"]]);
    assert_eq!(
        s.total.as_deref(),
        Some("2 3/4 cups + 1 tbsp + 1 tsp, 680 g")
    );
    assert_eq!(s.breakdown, "2 1/2 cups (600 g) + 1/3 cup (80 g)");
}

#[test]
fn olive_oil_says_cups_and_spoons_the_way_a_cook_measures() {
    // 10-12 tablespoons, said as a half cup and two spoons up to three
    // quarters of a cup. The 50 g rides one use only, so no mass total.
    let s = sum(&[
        &["1/4 cup", "50 g"],
        &["2-3 tbsp"],
        &["2 tbsp"],
        &["2-3 tbsp"],
    ]);
    assert_eq!(s.total.as_deref(), Some("1/2 cup + 2 tbsp to 3/4 cup"));
    assert_eq!(s.breakdown, "1/4 cup (50 g) + 2-3 tbsp + 2 tbsp + 2-3 tbsp");
}

#[test]
fn spoons_stay_spoons() {
    // No cup was written, so the total never becomes one.
    let s = sum(&[&["2 tbsp", "18 g"], &["1 1/2 tsp", "5 g"]]);
    assert_eq!(s.total.as_deref(), Some("2 tbsp + 1 1/2 tsp, 23 g"));
}

#[test]
fn one_use_has_no_total() {
    let s = sum(&[&["1 1/3 tsp"]]);
    assert_eq!(s.total, None);
    assert_eq!(s.breakdown, "1 1/3 tsp");
}

#[test]
fn counts_sum_and_follow_their_plural() {
    assert_eq!(
        sum(&[&["2 cloves"], &["1 clove"]]).total.as_deref(),
        Some("3 cloves")
    );
    assert_eq!(
        sum(&[&["1 clove"], &["1 clove"]]).total.as_deref(),
        Some("2 cloves")
    );
    assert_eq!(sum(&[&["2"], &["1"]]).total.as_deref(), Some("3"));
}

#[test]
fn a_bucket_missing_from_a_use_is_left_out() {
    let s = sum(&[&["1 cup", "100 g"], &["1 cup"]]);
    assert_eq!(s.total.as_deref(), Some("2 cups"));
}

#[test]
fn unlike_uses_have_no_total() {
    let s = sum(&[&["1 cup"], &["100 g"]]);
    assert_eq!(s.total, None);
    assert_eq!(s.breakdown, "1 cup + 100 g");
}

#[test]
fn approximate_uses_make_an_approximate_total() {
    let s = sum(&[&["about 110 g"], &["50 g"]]);
    assert_eq!(s.total.as_deref(), Some("~160 g"));
}

#[test]
fn ranges_add_bound_by_bound() {
    let s = sum(&[&["2-3 tbsp"], &["1 tbsp"]]);
    assert_eq!(s.total.as_deref(), Some("3-4 tbsp"));
}

#[test]
fn whole_cups_stay_cups() {
    let s = sum(&[&["1/2 cup"], &["1/2 cup"]]);
    assert_eq!(s.total.as_deref(), Some("1 cup"));
    let s = sum(&[&["1/4 cup"], &["1/8 cup"]]);
    assert_eq!(s.total.as_deref(), Some("1/4 cup + 2 tbsp"));
}

#[test]
fn a_total_no_fraction_can_state_is_dropped() {
    // 1/7 cup is not a measure; the breakdown still carries it.
    let s = sum(&[&["1/7 cup"], &["1/7 cup"]]);
    assert_eq!(s.total, None);
    assert_eq!(s.breakdown, "1/7 cup + 1/7 cup");
}
