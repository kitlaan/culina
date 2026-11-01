# AGENTS.md — Culina

## What this repo is

A recipe format and its reference implementation, in Rust. The spec
(`design.md`) and the code share a tree, and the spec is the contract —
an agent reads it before making grammar decisions, not after.

## Read first

`design.md`, at minimum: Design principles, the field grammar, Threads
and the DAG. The two fixtures in `crates/culina-core/tests/fixtures/`
are normative conformance tests and are byte-canonical: `culinafmt` is
a no-op on both.

## Build and verify

```
cargo fmt
cargo test
cargo clippy --all-targets
```

All three pass before every commit. `cargo fmt --check` too — this is a
formatter project; unformatted code is a bug here.

## Grammar changes

1. Write the rule in `design.md` first, with its reasoning.
2. Implement in `culina-core` — the parser, not the callers.
3. The worked-cases table in the spec becomes tests, case for case.
4. Both fixtures must round-trip byte-identical after the change.
5. If a fixture changes, the spec changes in the same commit. They move
   together or not at all.

## The phase pattern

Features land in phases: scaffold the structure, implement, test, fold
the decision back into `design.md`, regen the HTML page. A phase ends
green — `cargo test` passing, clippy clean, fixtures stable. The
round-trip test is the first thing that catches grammar bugs; reach for
it before anything else.

## Error philosophy

Errors are loud: they name the line, they tell the author what to do,
and they never silently degrade. Parse-or-note means unknown words
become notes, not errors — vagueness is a feature, not a failure to
parse. If an error doesn't tell the author how to fix it, the error is
the bug.

## Dependencies

Near-zero: `miette` (error rendering), `serde` + `serde_json`
(JSON-LD), `thiserror`, `toml` (lexicons and collection config). Don't
add more without a reason that survives review.

## Commit format

Four parts: abstract (one line), why (a short sentence), what (bullets
when the changes aren't tightly related), testing (explicit label,
with count). Keep the message proportional to the diff. Never label
sections with the literal words "Why:" or "What:".

## Things that bit us

- **The brace rule** — multiword tag names need braces (`_{fried paste}`,
  not `_fried paste`), and this mistake has been made three separate times
  in three different sigils (`#{parchment}`, `@{baguette}`, `_fried
  paste`). The scanner silently parses the bare single-word prefix and
  the rest becomes prose. When a rendered step reads oddly ("about 3 min
  paste."), check the braces first.
- The entity-decode channel: tool parameters that contain `&` or
  `<` arrive decoded. Build HTML entities from `char::from_u32(0x26)`
  at runtime.
- `zip` truncation: a byte-prefix match against a longer dictionary
  entry overflows the slice. Prefix matching must require full
  consumption.
- The production clause (`=> _dough`) is data, not prose. If it shows
  up in rendered step text or JSON-LD, the renderer has a bug.
- `cargo fmt` reformats code between reads — always read the file
  before editing if the last operation was a format pass.
