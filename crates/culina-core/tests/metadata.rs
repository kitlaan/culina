//! Metadata key registry conformance — design.md's "What fails" block,
//! verbatim, plus the literal forms (servings ranges, the date grammar,
//! durations, enums) and the warn-and-pass-through rule.

use culina_core::fmt::canonicalize_full;

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

/// Errors when canonicalizing; warnings ride along.
fn check(input: &str) -> (Option<String>, Vec<String>) {
    match canonicalize_full(input, None, None) {
        Ok((_, warnings)) => (None, warnings),
        Err(e) => (Some(e.to_string()), Vec::new()),
    }
}

#[test]
fn fixtures_are_clean() {
    for src in [CROISSANT, CURRY] {
        let (text, warnings) = canonicalize_full(src, None, None).unwrap();
        assert_eq!(text, src);
        assert!(
            warnings.is_empty(),
            "fixtures carry no warnings: {warnings:?}"
        );
    }
}

// --- The "What fails" block, design.md, Worked example ---

#[test]
fn single_value_key_repeats_loudly() {
    let (err, _) = check(">> title: A\n>> title: B\n\n:mix => _dough\n");
    let err = err.expect("must fail");
    assert!(err.contains("line 2"), "{err}");
    assert!(err.contains("'title'"), "{err}");
}

#[test]
fn date_literal_must_parse() {
    let (err, _) = check(">> title: T\n>> date: last night\n\n:mix => _dough\n");
    let err = err.expect("must fail");
    assert!(err.contains("line 2"), "{err}");
    assert!(err.contains("date"), "{err}");
}

#[test]
fn reference_form_rejects_commas() {
    let (err, _) = check(">> title: T\n>> image: a.jpg, b.jpg\n\n:mix => _dough\n");
    let err = err.expect("must fail");
    assert!(err.contains("line 2"), "{err}");
    assert!(err.contains("image"), "{err}");
}

// --- Literal forms ---

#[test]
fn servings_accepts_numbers_and_ranges() {
    for v in ["4", "4-6", "12"] {
        let (err, _) = check(&format!(
            ">> title: T\n>> servings: {v}\n\n:mix => _dough\n"
        ));
        assert!(err.is_none(), "servings {v} should pass: {err:?}");
    }
    for v in ["many", "4 people", "0", ""] {
        let (err, _) = check(&format!(
            ">> title: T\n>> servings: {v}\n\n:mix => _dough\n"
        ));
        assert!(err.is_some(), "servings {v:?} should fail");
    }
}

#[test]
fn date_grammar() {
    let good = [
        "1961",
        "circa 1961",
        "early 1970s",
        "by 1950",
        "1961-06",
        "1961-06-14",
        "2011 — the farm weekend",
        "1970s - disco era",
    ];
    for v in good {
        let ok = date_ok(v);
        assert!(ok, "date {v:?} should parse");
    }
    let bad = [
        "last night",
        "1961-13",
        "1961-06-31 is weird",
        "summer",
        "19",
    ];
    for v in bad {
        assert!(!date_ok(v), "date {v:?} should fail");
    }
}

fn date_ok(v: &str) -> bool {
    let (err, _) = check(&format!(">> title: T\n>> date: {v}\n\n:mix => _dough\n"));
    err.is_none()
}

#[test]
fn enums_and_durations_and_tags() {
    let ok_cases = [
        ">> time: 45 min",
        ">> units: metric",
        ">> lang: fr-FR",
        ">> modified: 2026-11-01",
        ">> yield-scales: size",
        ">> lang: zh-Hans",
    ];
    for line in ok_cases {
        let (err, _) = check(&format!(">> title: T\n{line}\n\n:mix => _dough\n"));
        assert!(err.is_none(), "{line} should pass: {err:?}");
    }

    let bad_cases = [
        (">> time: tomorrow", "time"),
        (">> units: banana", "units"),
        (">> lang: 4", "lang"),
        (">> modified: yesterday", "modified"),
        (">> yield-scales: big", "yield-scales"),
    ];
    for (line, key) in bad_cases {
        let (err, _) = check(&format!(">> title: T\n{line}\n\n:mix => _dough\n"));
        let err = err.unwrap_or_else(|| panic!("{line} should fail"));
        assert!(err.contains(key), "{err}");
    }
}

#[test]
fn token_lists_reject_empty_tokens() {
    let (err, _) = check(">> title: T\n>> keywords: curry,, weeknight\n\n:mix => _dough\n");
    assert!(err.is_some(), "empty token must fail");
}

#[test]
fn unit_shape_needs_an_equals_sign() {
    let (err, _) = check(">> title: T\n>> unit: verre ninety ml\n\n:mix => _dough\n");
    let err = err.expect("registration shape must fail");
    assert!(err.contains("line 2"), "{err}");
}

// --- The note key (design.md, key semantics) ---

#[test]
fn note_repeats_freely_as_prose() {
    // Prose form, many-valued: each line is one note, commas are content,
    // and no warning rides — the key is in the registry.
    let input = ">> title: T\n>> note: brown-sugar variant (130 g brown sugar, 260 g water)\n>> note: save the bone\n\n:mix => _dough\n";
    let (err, warnings) = check(input);
    assert!(err.is_none(), "note is many-valued: {err:?}");
    assert!(warnings.is_empty(), "note is a known key: {warnings:?}");
}

#[test]
fn note_rejects_empty_values() {
    let (err, _) = check(">> title: T\n>> note:\n\n:mix => _dough\n");
    let err = err.expect("empty prose must fail");
    assert!(err.contains("line 2"), "{err}");
    assert!(err.contains("note"), "{err}");
}

// --- Warn and pass through ---

#[test]
fn unknown_keys_warn_and_stay_opaque() {
    let input = ">> title: T\n>> servings: 1\n>> blog-category: dessert\n\n:mix => _dough\n";
    let (err, warnings) = check(input);
    assert!(err.is_none(), "unknown keys never fail: {err:?}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("blog-category"), "{warnings:?}");
    assert!(warnings[0].contains("line 3"), "{warnings:?}");

    // The value prints verbatim — opaque.
    let (text, _) = canonicalize_full(input, None, None).unwrap();
    assert!(text.contains(">> blog-category: dessert"), "{text}");
}
