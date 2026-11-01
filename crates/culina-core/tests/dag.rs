//! Phase 5 conformance: DAG validation — every thread resolves, productions
//! are well-formed, and the fixtures stay valid.

use culina_core::dag::validate;
use culina_core::parse::parse;

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

fn check(input: &str) -> Result<(), culina_core::ParseError> {
    parse(input).and_then(|doc| validate(&doc))
}

#[test]
fn fixtures_validate() {
    for src in [CROISSANT, CURRY] {
        check(src).unwrap_or_else(|e| panic!("fixture must validate: {e}"));
    }
}

#[test]
fn typo_forks_loudly() {
    let err = check(":knead _dough\n").expect_err("unresolved thread must fail");
    assert!(err.to_string().contains("never created"), "{err}");
    assert!(err.to_string().contains("'dough'"), "{err}");
    assert!(err.to_string().contains("typo"), "{err}");
    assert!(err.to_string().contains("line 1"), "{err}");
}

#[test]
fn create_then_consume_passes() {
    check(":mix @{flour|200 g} => _dough\n:{rest|1 hour} _dough\n").expect("valid");
}

#[test]
fn rename_ends_the_old_name_and_reports_it() {
    let err =
        check(":mix @{flour|200 g} => _dough\n:shape _dough into a loaf => _loaf\n:bake _dough\n")
            .expect_err("dead thread must fail");
    let msg = err.to_string();
    assert!(msg.contains("no longer live"), "{msg}");
    assert!(msg.contains("ended at line 2"), "{msg}");
    assert!(msg.contains("becoming _loaf"), "{msg}");
}

#[test]
fn rename_to_same_name_is_a_rebind() {
    check(":mix @{flour|200 g} => _dough\n:shape _dough => _dough\n")
        .expect("consuming and producing the same name is a rename");
}

#[test]
fn second_creation_of_a_live_name_fails() {
    let err = check(":mix @{flour|200 g} => _dough\n:mix @{flour|200 g} => _dough\n")
        .expect_err("duplicate binding must fail");
    assert!(err.to_string().contains("already live"), "{err}");
}

#[test]
fn forks_name_distinct_groups_and_pass() {
    check(
        ":roll @{dough} => _panel\n:cut _panel => _{triangles|6} + _{rectangles|6}\n:roll _triangles\n:fill _rectangles => _filled\n",
    )
    .expect("a fork names its groups; both stay live until each ends");
}

#[test]
fn multiplicity_on_consumption_is_deferred() {
    let err = check(":mix @{flour|200 g} => _dough\n:cut _{dough|2}\n")
        .expect_err("piece selection must fail");
    assert!(
        err.to_string().contains("piece selection is deferred"),
        "{err}"
    );
}

#[test]
fn multiplicity_on_production_must_be_one_whole_count() {
    let created = ":mix @{flour|200 g} => _dough\n";

    check(&format!("{created}:divide _dough => _{{dough|2}}\n")).expect("integer count is valid");

    let err = check(&format!("{created}:divide _dough => _{{dough|2-4}}\n"))
        .expect_err("a range is not a count");
    assert!(err.to_string().contains("whole count"), "{err}");

    let err = check(&format!("{created}:divide _dough => _{{dough|2|extra}}\n"))
        .expect_err("two fields is not a count");
    assert!(err.to_string().contains("whole count"), "{err}");
}

#[test]
fn optional_is_for_at_names_only() {
    let err = check(":preheat #{?oven|425F}\n").expect_err("? on a tool must fail");
    assert!(err.to_string().contains("'?' is for @ names only"), "{err}");
}

#[test]
fn two_production_clauses_fail() {
    let err = check(":mix => _a => _b\n").expect_err("one arrow per step");
    assert!(err.to_string().contains("two production clauses"), "{err}");
}

#[test]
fn production_naming_nothing_fails() {
    let err =
        check(":mix @{flour|200 g} => bread\n").expect_err("missing underscore must fail loudly");
    assert!(err.to_string().contains("names nothing"), "{err}");
    assert!(err.to_string().contains("_"), "{err}");
}

#[test]
fn setup_steps_validate_without_threads() {
    check(":preheat #{oven|425F}\n").expect("setup steps are outside the food DAG");
}

#[test]
fn joins_consume_two_live_threads() {
    check(":whisk @{egg|1} => _wash\n:mix => _a\n:mix => _b\n:brush _a + _b with _wash\n")
        .expect("plus joins two live threads");
}
