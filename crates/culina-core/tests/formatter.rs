//! Phase 6 conformance: canonicalize — stamp-on-change, indent
//! normalization, and idempotence. The fixtures must already be canonical:
//! culinafmt is a no-op on them from day one.

use culina_core::fmt::canonicalize;

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

#[test]
fn fixtures_are_already_canonical() {
    // No stamp injected, no churn — the corpus is byte-canonical.
    assert_eq!(
        canonicalize(CROISSANT, Some("2099-01-01")).unwrap(),
        CROISSANT
    );
    assert_eq!(canonicalize(CURRY, Some("2099-01-01")).unwrap(), CURRY);
}

#[test]
fn stamping_only_happens_on_change() {
    let input = ">> title: T\n>> servings: 1\n\n:mix => _dough\n\n\n";
    let out = canonicalize(input, Some("2026-11-01")).unwrap();
    assert!(out.contains(">> modified: 2026-11-01"), "{out}");

    // Re-running with a later date must not dirty an already-canonical file.
    let out2 = canonicalize(&out, Some("2099-12-31")).unwrap();
    assert_eq!(out, out2);
    assert!(out2.contains("2026-11-01"), "{out2}");
}

#[test]
fn existing_stamp_refreshes_in_place() {
    let input = ">> title: T\n>> modified: 2020-01-01\n>> servings: 1\n\n:mix => _dough\n\n\n";
    let out = canonicalize(input, Some("2026-11-01")).unwrap();
    // Position preserved (second line), value refreshed.
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[1], ">> modified: 2026-11-01");
    assert_eq!(lines[2], ">> servings: 1");
}

#[test]
fn step_only_fragments_are_not_recipes() {
    // Required metadata (design.md, Metadata): a doc with no `>>` block
    // fails validation. The formatter maintains stamps; it never
    // creates metadata — and now it never gets the chance to.
    let err = canonicalize(":a\n\n\n:b\n", None).unwrap_err().to_string();
    assert!(err.contains("missing required metadata"), "{err}");
    assert!(err.contains("title"), "{err}");
    assert!(err.contains("servings"), "{err}");
}

#[test]
fn trailing_metadata_block_stamps_too() {
    // Metadata may trail the steps (design.md, Metadata): the stamp
    // lands in the first metadata block, not the first block.
    let input = ":mix => _dough\n\n\n>> title: T\n>> servings: 1\n";
    let out = canonicalize(input, Some("2026-11-01")).unwrap();
    assert!(out.contains(">> modified: 2026-11-01"), "{out}");
    // The stamp joins the trailing block, after the last authored line.
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.last(), Some(&">> modified: 2026-11-01"));
}

#[test]
fn stdin_mode_never_stamps() {
    let out = canonicalize(">> title: T\n>> servings: 1\n\n\n:mix => _dough\n\n", None).unwrap();
    assert!(!out.contains("modified"), "{out}");
}

#[test]
fn continuation_indent_normalizes_to_two_spaces() {
    let tabbed = ">> title: T\n>> servings: 1\n\n:mix => _dough\n\tsome note\n";
    let out = canonicalize(tabbed, None).unwrap();
    assert!(out.contains("\n  some note\n"), "{out}");

    let four = ">> title: T\n>> servings: 1\n\n:mix => _dough\n    some note\n";
    let out = canonicalize(four, None).unwrap();
    assert!(out.contains("\n  some note\n"), "{out}");
}

#[test]
fn canonicalize_is_idempotent_across_spelling_variants() {
    // All spellings converge to one canonical form.
    let prefix = ">> title: T\n>> servings: 1\n\n";
    let variants = [
        format!("{prefix}:mix => _dough.\n"),
        format!("{prefix}:mix → _dough.\n"),
        format!("{prefix}:mix => _dough.  \n"),
        format!("{prefix}:{{mix}} => _dough.\n"),
        format!("{prefix}:mix => _{{dough}}.\n"),
    ];
    for v in &variants {
        let once = canonicalize(v, None).unwrap();
        let twice = canonicalize(&once, None).unwrap();
        assert_eq!(once, twice, "idempotence on {v:?}");
    }
    let first = canonicalize(&variants[0], None).unwrap();
    for v in &variants[1..] {
        assert_eq!(
            canonicalize(v, None).unwrap(),
            first,
            "all spellings converge: {v:?}"
        );
    }
}
