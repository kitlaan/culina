# Culina

Recipes as a dataflow graph. Culina is a markdown-like language for recipes
where steps are nodes and products are threads — dough becomes a loaf, a
loaf becomes bread — and `culinafmt`, the canonical formatter that keeps
recipe files clean the way gofmt keeps Go clean.

The point: most recipe formats hold text. Culina holds a graph, so the
things you want to do with recipes — step cards with timers, scaling,
searching, rendering — read structure instead of guessing at prose.

## A 30-second example

```
>> title: Garlic bread
>> servings: 4

:mix @{butter|50 g}, @{garlic|2 cloves|minced} => _paste.
:spread _paste on @{baguette|1}.
:{bake|10 min} @baguette in #{oven|400F} until golden.
```

That file knows, as data:

- `_paste` is a product of the first step, consumed by the second — the
  graph is checkable, so typos are loud, not silent:

  ```
  x line 5: thread 'past' was never created — a typo silently forks the graph
  ```

- the bake step takes **10 minutes** (its `|` field classified as a time
  quantity) and the oven is **at 400°F** (tool state, which later steps
  inherit);
- "until golden" is a lifted doneness span — a typed field whose contents
  stay your words;
- the recipe serves 4, so scaling has a baseline.

Vagueness is a feature: "1 medium onion" stays a note; "110 g" is an
amount that scales. The parser never invents precision the author does not
have.

## Usage

```
cargo build
./target/debug/culinafmt --help
```

```
culinafmt 0.1.0 — canonicalize .culina recipes

Usage: culinafmt [--lexicon PATH] [-l|-d] [file ...]

With files: rewrite each in place when its form differs.
  -l            list files whose form differs; do not write
  -d            print unified diffs; do not write
  --lexicon     a collection overlay on the tool core (TOML)
With no files: read stdin, print canonical text to stdout (no stamping).
```

Authors choose their own line breaks; culinafmt never wraps and normalizes
the continuation indent. Files carry `>> modified:` — stamped only when a
run changes the file, so re-running never dirties anything. Unknown
metadata keys warn and pass through; malformed ones fail loudly with the
offending line rendered.

## Repository layout

This repo is a spec and its reference implementation in one tree:

- **`design.md`** — the specification. Grammar, type system, DAG semantics,
  metadata registry, lexicons. Normative.
- **`crates/culina-core`** — parser, validator, and canonical printer.
  `parse → validate → print` round-trips byte-identically.
- **`crates/culinafmt`** — the CLI.
- **`crates/culina-core/tests/fixtures/`** — the two stress fixtures
  (croissants, red curry). They are conformance tests and are
  byte-canonical: `culinafmt` is a no-op on both.
- **`crates/culina-core/lexicons/`** — the embedded English lexicon.
  Channels (units, verbs, fuzzy durations, doneness markers) as TOML;
  collections overlay with `--lexicon`, documents register units with
  `>> unit: verre = 90 ml`.

## Status

Implemented: block and inline grammar, the field grammar (quantities,
equivalency, approx prefixes, the `%` escape), DAG validation with honest
thread errors, the formatter, lexicons with the full resolution order, and
the metadata key registry.

Specced in `design.md`, not yet built: renderers (traditional card, the
Cooking For Engineers grid, cook-mode), the scaling engine, sub-recipe
resolution across files, a WASM target, and per-language cores beyond
English.

## Origins

Culina started as a riff on [Cooklang](https://cooklang.org) and grew into
its own language. The `@` ingredient and `#` cookware sigils are
Cooklang's; the data model is not — Cooklang keeps steps as prose
paragraphs, while Culina makes food a graph (steps as nodes, products as
threads), so structure never lives inside prose. Credit where due:
Cooklang proved a plain-text recipe format could be readable and
machine-processable at once.

## License

MIT — see [LICENSE](LICENSE).
