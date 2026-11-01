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

## Tools

### culinafmt — the formatter

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

### culina-site — the static site generator

```
./target/debug/culina-site --in recipes/ --out public/
```

Walks a directory of `.culina` files and emits flat HTML pages, an index,
and CSS. Collection policy comes from a `culina.toml` at the input root
(`[site]` title, description; `[lexicon]` overlay path); a `culina.css` at
the root replaces the built-in default stylesheet. Broken recipes fail
the build. A `static/` directory is copied verbatim.

## Theming

The HTML culina-site emits is semantic and unstyled, with stable
`culina-*` classes. The built-in default is the classic recipe card
(paper, double rule, dotted leaders); restyling means writing CSS against
those classes — no HTML changes.

The renderer carries data on the elements it emits, as attributes:

| Attribute | On | Carries |
|---|---|---|
| `data-amounts` | ingredient `<span>` | the quantities as written |
| `data-state` | tool `<span>` | tool state — word or magnitude |
| `data-duration` | step `<li>` | the action's time field |
| `data-produces` | step `<li>` | what the step creates (`_name`, joined) |
| `data-cuisine` | the `<article>` | the `>> cuisine` value |

These are the theming API. Three example stylesheets in
`crates/culina-site/css/` demonstrate what the hooks enable:

- **`inline-measurements.css`** — amounts and tool states appear in the
  step prose: "fry paste in wok (medium heat)…"
- **`parallel-tracks.css`** — sections become side-by-side swimlanes;
  the parallelism visible at a glance
- **`index-card.css`** — a 4×6 recipe-box card with cuisine-colored tab
  and `@page` print mode for cardstock

Drop any of these in as your collection's `culina.css` and culina-site
uses it instead of the default. Or write your own against the same
classes — the hooks are the contract.

## Repository layout

This repo is a spec and its reference implementation in one tree:

- **`design.md`** — the specification. Grammar, type system, DAG semantics,
  metadata registry, lexicons. Normative.
- **`crates/culina-core`** — parser, validator, canonical printer, and the
  card model that renderers consume.
- **`crates/culinafmt`** — the formatter CLI.
- **`crates/culina-site`** — the static site generator CLI, with the
  classic card stylesheet built in and three alternatives in `css/`.
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
thread errors, the formatter, lexicons with the full resolution order,
the metadata key registry, the render pipeline (Document → CardModel →
HTML with schema.org JSON-LD), culina-site with the classic card
stylesheet built in, and three example stylesheets (inline measurements,
parallel tracks, index card).

Specced in `design.md`, not yet built: the scaling engine's exact
arithmetic, sub-recipe resolution across files, a WASM target (which
enables the cook-mode UI), per-language cores beyond English, and the
true CfE grid target (which needs its own HTML emission, not a CSS
approximation).

## Agent development

This repo is built agent-first — AI agents are the primary contributors.
An [AGENTS.md](AGENTS.md) carries the repo-specific instructions: what to
read before touching grammar, the verification commands, the commit
format, and the traps that bit previous agents.

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
