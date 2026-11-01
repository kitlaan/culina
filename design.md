# Culina — design note

First draft, from ideation sessions on 2026-10-31 and 2026-11-01. It started as a Cooklang
alternative and diverged: the core idea is a **dataflow graph of food**, which
Cooklang does not have. The grammar has survived two stress tests (a laminated
dough and a mise-en-place curry); both are embedded at the bottom as fixtures.

**Contents**

- [Summary](#summary)
- [Design principles](#design-principles)
- [Document structure](#document-structure)
- [Metadata](#metadata)
- [Provenance and dates](#provenance-and-dates)
- [Tags and sigils](#tags-and-sigils)
- [The field grammar](#the-field-grammar)
- [Quantities and units](#quantities-and-units)
- [Threads and the DAG](#threads-and-the-dag)
- [Tools](#tools)
- [Verbs](#verbs)
- [Lexicons](#lexicons)
- [Prep: note or thread?](#prep-note-or-thread)
- [Divided ingredients](#divided-ingredients)
- [Sub-recipe references](#sub-recipe-references)
- [Portioning: yield vs servings](#portioning-yield-vs-servings)
- [Repetition](#repetition)
- [Timing, doneness, anchors](#timing-doneness-anchors)
- [Output styles](#output-styles)
- [Worked examples](#worked-examples)
- [Open items](#open-items)

## Summary

Culina is a markdown-like SDL for recipes. Source is dense and structured;
renderers project it to recipe cards, step cards, grids, and timelines.
Recipe files use the `.culina` extension; the reference formatter is
`culinafmt`.

Three consumers drive every decision:

- **Cook-mode UI** — step cards, timers, branch on the graph, tool state.
- **Scaling + units engine** — resize by servings, unit conversion, SI output.
- **Human reading** — print, search, archival. Authored prose stays canonical.

## Design principles

**Dense source, normalized output.** Authors type terse structured text. A
formatter canonicalizes it (the gofmt posture): parse → format round-trips,
and the continuation indent is machine-owned — authors choose their own break
points and culinafmt never wraps. The source is for the author; renderings
are for everyone else.

**Vagueness is a feature.** Recipes are folklore, not lab protocols. The
format carries the author's real confidence: "1 medium onion" stays a note;
"110 g" is an amount that scales. Parse-or-note gives fuzzy quantities a
first-class home instead of a validation error. Precision is opt-in, via
equivalency fields.

**One field grammar.** Every tag — `@`, `#`, `:`, `_` — shares one `|` field
grammar. A field is a quantity if the quantity grammar consumes it fully;
otherwise it is a note. Meaning differs per sigil; syntax never does.

**The step is the dial.** The author picks step granularity, and the choice
carries meaning. Inline prep gets no card; prep in its own step does. Expanded
repetitions are real nodes; collapsed ones are a single node. There is no
language construct for any of this — the author's structure *is* the semantics.

**Authored prose is canonical.** "Until doubled" and "in a warm spot" stay the
author's words. Print styles quote them; cook-mode and grids generate from
structure.

## Document structure

- A block starting `# ` is a section header. Sections are presentation only;
  the DAG does not use them. They often mark parallel tracks, and the join step
  encodes the sync.
- A block starting `:` is one step: one action.
- Indented physical lines continue the same step.
- **Steps run adjacent** — no blank line between consecutive steps; the
  dense wall is canonical. A blank line separates section headers and
  metadata blocks from everything else. Input spacing is lenient;
  `culinafmt` emits the canonical rule.

The `#` collision (header vs. tool) resolves positionally: steps start with
`:`, so a line-initial `# ` is always a section and `#foo` mid-step is always
a tool.

## Metadata

```
>> title: Red curry with chicken
>> servings: 4
```

- `>>` prefix, `key: value`.
- **Required:** `title`, `servings`. Servings is the scaling baseline.
- Metadata is also the recipe's public interface: a referencing parent scales
  against `>> servings` and consults the yield policy. See
  [Sub-recipe references](#sub-recipe-references).

### The type system: two axes

Every key declares **cardinality** (one or many) and **value form**. The form
governs parsing and separation; the cardinality governs repetition.

| Form | Keys | Behavior |
|---|---|---|
| Token | `keywords`, `cuisine`, `course`, `diet`, `dish`, `author` | short, comma-free; comma lists |
| Prose | `title`, `summary`, `source`, `aka` | may contain commas; line lists |
| Reference | `image` | path or URL, validated; line lists |
| Literal | `servings` (number or range), `date` (date + qualifier lexicon), `time` (duration), `units` (enum), `lang` (BCP 47), `modified` (timestamp) | machine-validated; bad form errors |
| Definition | `unit` (`name = value`) | line list with internal structure |

Repetition rules:

- Repetition is the universal list primitive. All many-valued keys append per
  occurrence.
- Comma-splitting is sugar, available **only to token-form keys** — everywhere
  else commas are content, so prose, reference, and definition keys take lines
  only. Rule of thumb: a value that may contain a comma repeats per line.
- Single-value keys error on repeat — loud, same rule as quantity-after-note.
- The formatter emits one canonical form. Lenient input, one output — the
  gofmt posture again.

### Key registry

The form table above is the derivation rule — how comma-splitting gets
decided. This registry is the normative statement: what a conformance test
reads. If the two ever drift, regenerate the registry from the rule.

| Key | Form | Values | Comma sugar | Note |
|---|---|---|---|---|
| `title` | Prose | one | — | required |
| `servings` | Literal | one | — | required; number or range |
| `author` | Token | many | ✓ | `Name <email or URL>` recommended |
| `date` | Literal | one | — | content origin |
| `source` | Prose | many | ✗ | URL or attribution, one per line |
| `summary` | Prose | one | — | |
| `keywords` | Token | many | ✓ | |
| `cuisine` | Token | many | ✓ | |
| `course` | Token | many | ✓ | |
| `diet` | Token | many | ✓ | |
| `dish` | Token | many | ✓ | "makes both," not "also called" |
| `aka` | Prose | many | ✗ | script-dependent separators |
| `image` | Reference | many | ✗ | first = representative |
| `units` | Literal | one | — | `metric` / `imperial` / `us` |
| `lang` | Literal | one | — | |
| `makes` | Token | many | ✓ | ordered: export/binding order; also display order |
| `time` | Literal | one | — | override |
| `yield-scales` | Literal | one | — | `count` (default) or `size` |
| `modified` | Literal | one | — | formatter-stamped |
| `unit` | Definition | many | ✗ | `name = value` |

Unknown keys: **warn and pass through.** A collection's `>> blog-category:`
should not fork the spec, but silence would hide typos (`>> servngs: 6`), so
the parser warns and keeps the value opaque.

### Key classes

| Class | Keys | Owner |
|---|---|---|
| Required | `title`, `servings` | author |
| Descriptive | `author`, `date`, `source`, `summary`, `keywords`, `cuisine`, `course`, `diet`, `dish`, `aka`, `image`, `units`, `lang`, `makes` | author |
| Tool-maintained | `modified` | formatter |
| Overrides | `time`, `yield-scales`; further scaling policy reserved | author |
| Registry | `unit` | author |

Two admission tests decide what may exist:

1. **Author keys** — a fact the body cannot derive. This is why `equipment` is
   rejected: the `#` tags already hold it.
2. **Tool-maintained keys** — a fact the tool can witness. The formatter
   witnesses canonicalization, so `modified` is true by construction. It
   cannot witness creation, so `created` does not exist.

### Key semantics

- `>> time:` is an optional override; total time derives from the DAG's
  critical path.
- `>> unit: glug = 30 ml` registers a custom unit. An unregistered unit is
  opaque: it scales arithmetically, never converts.
- `>> units:` seeds the units dial, so cup-forward recipes render cup-forward
  by default.
- `>> summary:` — one line. Doubles as HTML meta description and card preview.
- `>> servings:` accepts a range (`4-6`); the scaling engine uses the midpoint,
  renderers print the range. Same vagueness posture as `1-2 g`.
- `>> source:` — prose, line list. A value that parses as a URL is the
  canonical link; anything else is an attribution statement ("adapted from
  Julia Child, MtAoFC — halved the time"). Parse-or-note; one key covers both.
  Commas inside a value are content, so repetition is the only way to add a
  source.
- `>> image:` — reference, line list. First listed is the representative
  image (schema.org array semantics).
- `>> author:` — token list. Recommended shape `Name <email or URL>`, never
  mandated: parse-or-note extracts name + contact when the shape matches, and
  "grandma" renders as free text. Keep display names in natural order
  ("Ted Lin", not "Lin, Ted") so commas stay separators; splitting is
  bracket-aware. Prefer a URL in the brackets — emails rot, and on a served
  collection they are spam bait.
- `>> dish:` — the canonical dish name(s), bare tokens: the normalizer-facing
  join key. A comma list here means "makes both" (`croissant, pain au
  chocolat`), not "also called."
- `>> aka:` — prose, line list. Alternate names with whatever punctuation they
  need (`aka: 红烧肉 (red-braised pork belly)`). Three consumers: search
  indexes read `title + aka + keywords` identically; renderers may show
  "also known as"; the dish normalizer clusters renditions across languages
  through shared aliases. Script-dependent separators (，、) are the reason
  aka never comma-splits.
- `>> date:` — content origin. Semantics in
  [Provenance and dates](#provenance-and-dates).
- `>> diet:` — self-declared flags only. Deriving diet from ingredients is a
  rabbit hole; declaration wins.
- `>> lang:` — BCP 47 (`fr-FR`, `zh-Hans`). Declares the document's language,
  which drives the lexicons: the verb lexicon, fuzzy durations ("overnight"
  is an English word), and unit spellings. See [Multilingual](#multilingual).
- `>> yield-scales:` — `count` (default) or `size`; how terminal multiplicity
  reacts to a scale factor. See [Portioning](#portioning-yield-vs-servings).
  Further policy keys stay reserved.
- `>> makes:` — the exported products in binding order (an ordered token
  list), so a parent can destructure a multi-output child; also pins display
  order. A single-terminal child needs none — its only terminal is the
  implicit export.
- Rejected keys: `difficulty` (bad signal), `equipment` (derivable from `#`
  tags), `created` (no witness), added-date (container-relative — a repo
  fact, never a file fact).

### Multilingual

Locale is a document property, not a field axis — a French title with English
steps is incoherent, so recipes translate whole, not field by field:

- `>> lang:` declares the file's language and drives its lexicons.
- Translated variants are **sibling files**, the Hugo convention
  (`coq-au-vin.culina` / `coq-au-vin.fr.culina`); the renderer picks by reader.
  No `title[fr]:` sub-grammar; the type system stays two-axes.
- A one-off gloss rides prose: `>> title: Coq au Vin (rooster in wine)`.
- Cross-language clustering is `dish` + `aka` feeding the ecosystem
  normalizer — identity lives in the data, not in field grammar.

### Worked example

The Child coq au vin, faithful transcription, touching every form:

```
>> title: Coq au Vin (Julia Child's)
>> servings: 6
>> author: Ted Lin
>> dish: coq au vin
>> cuisine: French
>> course: main
>> diet: gluten-free
>> keywords: braise, wine, chicken, weekend
>> summary: Chicken braised in red wine with lardons and mushrooms, the 1961 Child method.
>> date: 1961
>> source: Julia Child, Mastering the Art of French Cooking, 1961
>> source: https://archive.org/details/masteringartoffr01chil
>> image: photos/coq-top.jpg
>> image: photos/braise.jpg
>> unit: verre = 90 ml
>> modified: 2026-11-01
```

What fails:

```
>> title: A second title        error — single-value key repeated
>> date: last night             error — literal does not parse
>> image: a.jpg, b.jpg          error — reference form validates path/URL shape
```

One silent non-error to know: `>> source: A, B, C` parses as **one**
attribution, because prose never splits. Two sources are two lines.

Key names stay human. The HTML renderer maps to schema.org at JSON-LD emit
time (`summary` → `description`, `date` → `datePublished`, `cuisine` →
`recipeCuisine`, `course` → `recipeCategory`, `aka` → `alternateName`,
`lang` → `inLanguage`). Renaming keys to match an SEO schema is tail wagging
dog.

## Provenance and dates

The work-vs-rendition split: the dish lives in the cultural commons; a file is
one expression of it. Different keys carry each half.

### Dish identity

- `>> dish: coq au vin` — the canonical name, the join key. Renditions cluster
  on it: a Child transcription, a Kenji riff, grandma's version.
- `>> aka:` is the in-file alias channel: alternate names (pinyin, kanji,
  glosses) that feed the dish normalizer, so an English rendition and a
  Chinese-language sibling cluster without a central registry.
- `title` stays display text ("Coq au Vin — Julia's, adapted"). Same split as
  title-vs-filename.
- This is the copyright-honest shape: the dish is a method, not ownable; the
  source author's prose is; a DSL re-expression with attribution respects
  both.

### Version provenance

- `author` — who authored **this file**. The transcriber, not Julia Child.
- `source` — where **this rendition** came from. Repeatable; sources carry
  their own dates in prose, so the chain goes as deep as needed.
- `date` — content origin, not file lifecycle. **Date travels with the
  content.** A faithful transcription inherits it (Child's 1961 → `date:
  1961`); an adaptation mints a new date and demotes the old one into the
  source line. Same shape as scholarly editions.
- A small qualifier lexicon parses for fuzzy dates (`circa 1961`, `early
  1970s`, `by 1950`); anything else demotes to a note. Era-sorting is the
  archival use case.
- A recipe with no author, source, or date is valid — a folk recipe. `dish:`
  alone is enough identity.

### File lifecycle

- `>> modified:` — culinafmt stamps it when a run changes the file; an
  already-canonical file keeps its old stamp, so re-running never dirties a
  file. It refreshes an existing `modified` line in place and never creates
  metadata for a file without any. Do not hand-edit; it means last canonical
  revision, and unformatted edits are git's business.
- Detached from a repo (zip, email, blog post) the file still self-describes —
  that is the whole argument for the key. Hugo does this dance with `lastmod`.
- No `created:`. Two independent proofs: the formatter cannot witness birth
  (mtime lies after any copy), and the fact itself is container-relative.

### Collection chronology

"When this recipe entered the collection" is a property of the container, not
the file. The same file is "added 2015" in one repo and "added today" in the
next, so it can never be a file key. It is a build-time query against the
containing repo (first-commit date).

Three facts, three owners, three sort orders:

| Fact | Key or source | Owner | Sort order |
|---|---|---|---|
| Content era | `>> date:` | author | "the 1987 recipes" |
| Freshness | `>> modified:` | formatter | "touched recently" |
| Collection chronology | git first-commit | repo, build time | "new here" |

### Export

A zip exports files, not the container, so container-owned facts stay behind.
`date:` and `modified:` travel in-band; first-commit does not.

- **Honest default:** the importing collection mints fresh added-dates —
  correct under the container-relative rule.
- **Sidecar manifest:** an export tool may write a `MANIFEST.csv`
  (`path, first-commit-date`) beside the files — container metadata about the
  export, never inside it. An importer that honors it recreates history with
  preserved author dates and defuses the bulk-import spike.
- **Export the container:** `git bundle` instead of a zip, when full history
  must travel.

Build-time caveats: rebases soften first-commit dates; a bulk import without a
manifest lands everything "added" on one day.

## Tags and sigils

| Sigil | Meaning | `\|` fields mean |
|---|---|---|
| `:` | action, one per step | quantities classify by unit (time → duration); the rest are conditions |
| `@` | ingredient | amount list, then prep/provenance notes |
| `#` | tool, stateful | quantity = state magnitude (`350F`); note = word state (`lid on`) |
| `_` | thread reference | multiplicity, at creation only |
| `=>` | names a product | creates or renames a thread |
| `?` | optional, on the `@` name | |
| `{...}{...}` | alternatives | each group is a complete parallel tag |
| `+` | joins references | sugar for multiple `_` |
| `%` | unit escape | forces quantity classification |

Braces when a tag is multiword or carries `|`. Bare otherwise: `@salt`,
`#oven`, `_dough`.

`=>` is the canonical production sigil; `→` is accepted input sugar, and the
formatter rewrites it to `=>` on canonicalization. With this, the grammar is
pure ASCII: every non-ASCII byte in a recipe file is data — a Thai alias, a
rendered ⅛ — never syntax.

## The field grammar

Split, classify, partition:

1. **Split** braced content on `|`.
2. **Classify** each field independently. A field is a quantity only if the
   quantity grammar consumes it fully — a number (range, mixed number, ASCII
   fraction), an optional unit from the dictionary, no remainder. `350F` ✓.
   `1-2 g` ✓. `1/4-inch dice` ✗ — the trailing word makes it a note. This
   full-consumption rule is what makes parse-or-note deterministic.
3. **Partition.** The leading run of quantities is the amount list. The first
   note ends it. **A quantity after a note is a syntax error**, not a demotion.
   `@{onion|diced|2}` fails loudly, because silent demotion would drop the
   amount from scaling.

Rules that follow:

- Multiple quantities = **equivalency**: `@{paprika|1-2 g|1 tsp}`. Renderers
  pick per the units dial; the scaling engine converts between them.
- A missing amount on `@` = **implicit 1**. `@{onion|diced}` is 1 onion, diced.
  Scaling works on the implicit count for free.
- A field may open with an **approx prefix** (`about`, `around`, `roughly`,
  `~` — per-language lexicon): it strips and sets an approx flag, and the
  quantity inside still parses. `:{rest|about 1 hour}` keeps its timer;
  renderers print the hedge.
- `%` forces a quantity with an unknown unit: `@{flour|1%glug|sifted}`.
  Without it, an unknown unit word demotes the whole field to a note and the
  amount vanishes.
- Units infer from the dictionary by longest match. Glued forms parse (`20g`,
  `350F`), as do ranges (`1-2 g`) and mixed numbers (`1 1/3 tsp`).

Worked cases:

```
@{paprika|1-2 g|1 tsp}           amount [1-2 g, 1 tsp]   equivalency
@{onion|2|diced}                 amount [2], notes [diced]
@{potato|2|1/4-inch dice}        amount [2], notes [1/4-inch dice]
@{onion|1|110 g}                 amount [1, 110 g]       equivalency ("1, about 110 g")
@{tomato sauce|1 batch|from r}   amount [1 batch], notes [from r]
@{flour|1%glug|sifted}           amount [1 glug], notes [sifted]
@{onion|1|about 110 g}           amount [1, ~110 g]       approx flag
@{onion|diced}                   implicit 1, notes [diced]
@{onion|diced|2}                 syntax error
```

## Quantities and units

- Dictionary plus longest match; `%` as the escape hatch for glued or exotic
  units.
- **ASCII in source, unicode in render:** type `1/8 in`, print `⅛`.
- Size words ("medium") are notes. There is no standard onion size; do not
  manufacture grams the author does not have. When precision matters, opt in:
  `@{onion|1|110 g|diced}`.
- Folk amounts ("to taste", "a handful") are notes. Scaling leaves them alone.
- Scaling sees the difference: mass scales exactly; count-with-size rounds to
  cook-sensible steps, or prints "1½ medium onions" and lets the human judge.
- Output dials are orthogonal to layout: spelling length (`t.` / `tsp` /
  `teaspoon`), unit system (SI: `1050 g` → `1.05 kg`), scale factor.

## Threads and the DAG

The core idea. Steps are nodes; products are threads.

- **Named at creation:** `=> _dough` on the producing step. Every `_name`
  resolves or it is an error. A typo must not silently fork the graph.
- **Explicit consumption, no implicit parents.** A step with no `_` consumes
  nothing. This is deliberate: "parent is previous step" misfires on setup
  steps (preheat) and hides typos. `_dough` is cheap to type and the formatter
  elides the repetition on render.
- **A step with no `_` and no `=>` is a setup step**, outside the food DAG:
  `:preheat #{oven|425F}`. Tool-state sets are the other example:
  `:lower #{oven|375F}` between two bakes.
- **Evolution without renames:** `:{rest|1 hour} _dough` touches the live
  thread. Three fold/rest cycles keep the name `_laminate`.
- **Rename = rebind:** `:shape _dough into a loaf => _loaf`.
- **Fork:** multiple outputs, `=> _bagels + _buns`; or unnamed multiplicity,
  `=> _{dough|2}`.
- **Join:** multiple `_` in one step (`+` is sugar). Multi-consume is not
  necessarily a merge — outputs are whatever `=>` says.
- **Groups:** forks name their groups — `=> _{triangles|6} + _{rectangles|6}`.
  Counts ride on the thread; later references are bare (`_triangles`).
  Multiplicity flows through transforms (6 triangles → 6 croissants).
  **No mid-group slicing:** forks name groups instead. Piece selection is
  deferred until a real recipe needs it.

## Tools

- **Temp attaches to tools; time attaches to actions.** `#{oven|425F}` is oven
  state; `:{bake|10 min}` is step time.
- State is a word (`#{pot|lid on}`, `#{wok|medium}`, `#{skillet|dry}`) or a
  quantity (`#{oven|350F}`) — same field grammar, classified the same way.
- Cook-mode derives a step's ambient conditions from tool-state history: the
  oven set at preheat holds through the bake. Print renderers need the same
  derivation, because the temp can sit sections away from its use.

## Verbs

- Free-form. A **known-verb lexicon** (bake, knead, rest, make, use, …) earns
  first-class behavior: timer templates, tense handling, step-card titles.
  Unknown verbs degrade to plain prose. Do not lock the vocabulary.
- On sub-recipe references, the verb carries acquisition mode: `make` = cook
  it now, `use` = assume pre-made.

## Lexicons

Per-language word tables the parser consults — the thesaurus behind
parse-or-note. The spec defines the channels, their payload shapes, and one
resolution order; tools ship per-language cores (selected by `>> lang:`),
collections overlay config, and documents override locally through Registry
keys — new Registry keys admitted only when a real document needs one.
Unknown words degrade to prose: the frame is grammar, the contents are
folklore.

| Channel | Maps | Payload |
|---|---|---|
| Verbs | `knead` → behavior | timer rule, tense, card title |
| Fuzzy durations | `overnight` → range | 8–12 h |
| Approx prefixes | `about` → flag | (none) |
| Doneness markers | `until` → span lift | (frame only; contents stay folk) |
| Date qualifiers | `circa` → pattern | parse marker |
| Units | `glug` → magnitude | 30 ml |

One resolution order, every channel:

> document registry (`>> unit:` today) → collection config → tool core (per
> language) → miss, and the word degrades to prose.

Seed contents are deliberately tiny — the English core ships `overnight`
(8–12 h) and `a few hours` (2–4 h) for fuzzy durations. Everything else is
prose until the ecosystem grows it.

### Lexicon files

- Format: **TOML**, one file per language — `lexicon.en.toml` — channels as
  sections. The English core is embedded in the tool; collections pass an
  overlay (`culinafmt --lexicon path.toml`).
- Overlay semantics are word-level: a map entry replaces the core entry
  whole, a missing word is added, and list channels (approx, doneness,
  dates) union. Removal is not supported until a real collection needs it.
- Matching: unit input spellings are the key, `short`, `long`, and `alt`
  forms, each plus its `+s` plural ("2 hours" matches "hour"); one spelling
  maps to exactly one unit. Fuzzy and verb words match whole words only.
- `>> unit: verre = 90 ml` registers through existing units — 90 × ml's
  factor — so the registry needs no data format of its own.
- `note` is a standard optional field on any entry: seed wisdom is data
  ("fridge rest, not counter"), comments stay for maintainers.
- `factor` is to the dimension base (grams, liters, meters, seconds);
  countables and temperatures carry none.

### Collection config

A `culina.toml` at the collection root is the collection's policy file:
`[site]` carries generator settings (title, description), `[lexicon]`
`file = "overlay.toml"` points at the lexicon overlay. Fields are added
when real collections need them — the same admission discipline as the
metadata registry.

## Prep: note or thread?

Identity is a property of crossing step boundaries. Prep used in the same step
is a note (`@{shallot|2|halved}`). Prep that needs its own attention or clock —
toasting spices — becomes a thread (`:toast … => _spices`). The author's step
granularity is the dial, and cook-mode reads it for free: thread = a card,
inline note = no card.

## Divided ingredients

Portioning one thing across steps is an ingredient→thread fork. No "divided"
flag exists, and none is needed:

```
:pick @{cilantro|1 bunch} into roots and leaves => _{cilantro roots} + _{cilantro leaves}.
```

Grocery sums stay exact. Contrast: repeated *quantified* uses (fish sauce,
1 tbsp, twice) stay plain tags and simply sum.

## Sub-recipe references

A recipe is a function: ingredients in, steps, products out. A reference is a
call — one opaque node that produces a thread:

```
:make @{tomato sauce|1 batch|from recipes/tomato-sauce} => _sauce.
:use  @{tomato sauce|1 batch|from recipes/tomato-sauce} => _sauce.
```

- `from <path>` rides as a provenance note. Without it:
  `@{tomato sauce|1 batch|store-bought}` — a plain ingredient, no link.
- **Addressing:** relative paths, resolved from the referencing file's
  directory. An extension is optional, not excluded — resolvers try the
  literal path first, then extension candidates (`.culina`). URLs are
  permitted for shared collections; fetch policy is tooling's.
- The parent binds its own name via `=>`; the child's threads stay scoped.
- **Opaque node, not transclusion.** Expansion is a render projection —
  cook-mode offers "tap to expand", print shows a pointer. Transclusion as
  source structure would couple edit histories and inject the child's
  overnight anchors into the parent's schedule.
- Scaling propagates: parent factor × batch amount → child rendered scaled.
  The child's yield policy is consulted; a parse-time consistency check
  verifies parent request against child yield.
- Grocery resolution is recursive with a cycle guard (cake → frosting → cake
  crumbs → cake is legal provenance, not an infinite traversal).
- Node duration = the child's critical path, so collapsed scheduling still
  works.
- **Multi-output children:** the child declares its exports in binding order
  via `>> makes: _stock, _meat` (also the display order), and the parent
  binds positionally with its own names — `:use @{roast chicken|1 batch|from
  recipes/roast-chicken} => _stock + _meat.` A single-terminal child needs
  no `makes:` (its only terminal is the implicit export). A multi-terminal
  child without one still parses and renders, but a parent reference errors
  loudly. Amounts scale the whole child.

## Portioning: yield vs servings

- **Yield** = terminal multiplicity. "Makes 12 croissants" is simply
  `=> _{croissant|12}`.
- **Servings** = `>> servings`. Scaling keys on it.
- They diverge — a curry is one pot serving four — and that is fine: servings
  is the baseline, multiplicity is a display fact.
- **Display rule:** "Serves N" always renders, from metadata. "Makes …"
  renders when terminal multiplicity is greater than one, joining terminals
  with "and" — a one-pot curry correctly shows only "serves 4."
- **Scaling:** `>> yield-scales: count | size`. `count` (the default when
  absent) multiplies yield by the scale factor, rounded to whole, never below
  one — 12 buns become 18; one loaf becomes two, and paired with a sized
  tool the scaler can say "you now need 2 pans." `size` fixes the count and
  lets each item absorb the scale — the rare "one bigger celebration cake."

## Repetition

No repeat construct. The survey of real cooking:

- **Transformative cycles** cap at ~6 (puff pastry turns; double folds exist
  to halve the count). Pasta-roller passes look like repetition but are not —
  the setting changes each pass.
- **Accumulation** (crêpes, cookie scoops, dumplings) is one step in the
  recipe's own mind, and multiplicity already covers it:

```
:cook _batter one ladle at a time in #{crepe pan} until the batter runs out => _{crepes|20}.
```

The author picks per case — trivial repeats collapse to one step with the loop
in a note (`:fold _dough four times, 30 minutes apart.`), meaningful cycles
expand because cook-mode wants real cards and clocks. Typing tedium belongs to
an editor macro that expands `×3` on save, not to the grammar.

## Timing, doneness, anchors

- Durations are time-classified fields on actions. Fuzzy durations ("overnight")
  are notes; the fuzzy-duration lexicon channel maps them (overnight ≈ 8–12 h)
  so cook-mode still gets timers.
- Doneness stays folk, but the frame is lifted: the doneness-markers channel
  recognizes "until X" spans and attaches them to the step as a typed field,
  without interpreting X. "Until doubled" becomes a distinct card element
  and a search target; the contents stay folklore. One sub-case is
  machine-readable for free: a span containing a quantity ("until 165F
  internal") parses via the ordinary quantity grammar — probe-thermometer
  alarms, done-prediction against the timer. Zero new syntax; the braced form
  already carried conditions (`:{rest|doubled}` — non-quantity `|` fields on
  actions are conditions).
- Anchors ("the night before", "start the rice when the curry first simmers")
  stay prose hints. The DAG plus durations does the scheduling.

## Output styles

Every style is a projection of four channels — quantities, procedure prose,
timing, structure:

| Style | Quantities | Prose | Timing | Structure |
|---|---|---|---|---|
| Traditional card | aggregated list up front | authored, numbered | absent | absent |
| CfE grid | inline per step, aligned to first use | generated imperatives | inline | indentation |
| Inline info | embedded in sentences | authored, units normalized | in sentences | absent |
| Cook-mode cards | per card | authored fragments | timers | branching UI |
| Timeline | — | — | Gantt of parallel tracks | the DAG, drawn |

The Cooking For Engineers grid is the validation case: it tabulates exactly the
step→ingredient and step→duration edges the format holds as data, with thread
lifetimes as carried rows.

The render pipeline: **Document → CardModel → target.** The CardModel is the
semantic seam — title, meta rows, aggregated ingredients, step prose, JSON-LD
fields — and HTML is the first target: an unstyled fragment with stable
`culina-*` classes, so collections theme with their own CSS and other targets
(Hugo markdown, cook-mode) consume the same model without re-deriving recipe
semantics.

## Worked examples

Both are stress fixtures. The croissant exercises parallel threads, a join,
repeated transforms, tool-state history, and mid-bake state changes. The curry
exercises the prep boundary, a divided ingredient, staggered additions, and a
two-track join at serve.

### Croissants

```
>> title: Croissants & pains au chocolat
>> servings: 12
>> cuisine: French
>> dish: croissant, pain au chocolat

# Dough

:mix @{flour|500 g}, @{water|290 g}, @{sugar|55 g}, @{salt|11 g}, @{yeast|9 g} in #{stand mixer} until a shaggy dough forms => _dough.
:{knead|5 min} _dough in #{stand mixer|speed 2} until smooth but still slack.
:{rest|overnight} _dough in #{fridge|40F}.

# Butter block

:pound @{butter|280 g} cold between #parchment into an 8-inch square => _{butter block}.
:{rest|1 hour} _{butter block} in #{fridge|40F}.

# Lamination

:roll _dough into an 8x16 in rectangle => _sheet.
:enclose _{butter block} in _sheet => _laminate.
:roll _laminate to 6x18 in.
:fold _laminate into thirds, like a letter.
:{rest|45 min} _laminate in #{fridge|40F}.
:roll _laminate to 6x18 in.
:fold _laminate into thirds, like a letter.
:{rest|45 min} _laminate in #{fridge|40F}.
:roll _laminate to 6x18 in.
:fold _laminate into thirds, like a letter.
:{rest|overnight} _laminate in #{fridge|40F}.

# Shape

:roll _laminate into a 1/8 in-thick panel => _panel.
:cut _panel into 12 pieces, 6 triangles and 6 rectangles => _{triangles|6} + _{rectangles|6}.
:fill _rectangles with @{chocolate|60 g|chopped} => _filled.
:roll _triangles into crescents => _croissants.
:roll _filled into cylinders => _pains.

# Proof and bake

:whisk @{egg|1} with @{milk|1 tbsp} => _{egg wash}.
:brush _croissants + _pains with _{egg wash}, arranged on #{sheet pan|parchment}.
:{proof|2 hours} _croissants + _pains in a warm spot until jiggly and nearly doubled.
:brush _croissants + _pains with _{egg wash} again.
:preheat #{oven|425F} during the last 20 min of the proof.
:{bake|10 min} _croissants + _pains in #oven until the tops set.
:lower #{oven|375F}.
:{bake|15 min} _croissants + _pains until deep golden.
:cool _croissants + _pains on #{wire rack} until room temperature.
```

### Red curry with chicken

```
>> title: Red curry with chicken
>> servings: 4
>> cuisine: Thai
>> dish: red curry
>> aka: gaeng phet
>> aka: แกงเผ็ด
>> summary: Weeknight curry from a scratch paste, with a parallel rice track.

# Chicken

:toss @{chicken|500 g|bite-sized} with @{fish sauce|1 tbsp} => _chicken.
  (start this before the paste; it needs 15 min)

# Paste

:toast @{coriander|1 tsp}, @{white peppercorn|1 tsp} in #{skillet|dry} until fragrant, then cool => _spices.
:pick @{cilantro|1 bunch} into roots and leaves => _{cilantro roots} + _{cilantro leaves}.
:pound @{shallot|2|halved}, @{garlic|6 cloves}, @{red chili|4}, @{lemongrass|1 stalk|cored}, @{galangal|1 in|sliced}, _{cilantro roots}, _spices in #mortar to a fine paste => _paste.

# Curry

:fry _paste in #{wok|medium} until darkened and fragrant, about 3 min => _base.
:{fry|3 min} _chicken in _base until sealed outside => _curry.
:stir @{coconut milk|2 cans} into _curry.
:{simmer|7 min} _curry.
:add @{green beans|150 g|trimmed} to _curry.
:{simmer|3 min} _curry.
:season _curry with @{fish sauce|1 tbsp|to taste}, @{palm sugar|1 tsp}, @{?makrut lime leaves|2}.
:stir _curry off the heat with @{Thai basil|1 handful}{sweet basil|1 handful}.

# Rice

:rinse @{jasmine rice|300 g} in #colander until the water runs clear => _rice.
:{simmer|15 min} _rice with @{water|450 ml} in #{pot|lid on} until the water is absorbed.
  (start this when the curry first simmers)

# Serve

:serve _curry over _rice with _{cilantro leaves} + @{lime|1|cut into wedges}.
```

## Open items

- Piece selection inside groups — deferred. Reopens when a recipe must
  consume an unspecified subset of an existing group (the sacrifice
  pattern: "fry one spring roll first to test the oil"). If it ever fires,
  the natural syntax is consumption-side `|n` — "n unspecified members."
