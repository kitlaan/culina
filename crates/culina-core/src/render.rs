//! Renderers — projections of the parsed document (design.md, Output
//! styles). Document → CardModel → a target.
//!
//! The CardModel is the semantic seam: title, meta rows, aggregated
//! ingredients, step prose, JSON-LD fields. HTML is the first target;
//! others (Hugo markdown, cook-mode) consume the same model without
//! re-deriving recipe semantics. The HTML fragment is semantic and
//! unstyled, with stable `culina-*` classes as the theming hooks —
//! collections style it with their own CSS.

use serde_json::json;

use crate::amounts::{summarize, Amount, Summary};
use crate::ast::{Block, Document, Seg, Sigil, Step};
use crate::fields::FieldKind;

/// One recipe, semantically rendered — target-independent.
#[derive(Debug, Default)]
pub struct CardModel {
    pub title: String,
    pub summary: Option<String>,
    pub serves: Option<String>,
    /// Display rows: (label, value) — cuisine, dish, source, …
    pub meta: Vec<(&'static str, String)>,
    /// Authored images in order; the first is the representative — the
    /// card hero. The rest are supporting shots, never step-anchored
    /// (see design.md, key semantics).
    pub photos: Vec<String>,
    /// Whole-recipe notes, one per authored `>> note:` line. Rendered after
    /// the method; never a meta row and never JSON-LD (schema.org has no
    /// honest slot — see design.md, key semantics).
    pub notes: Vec<String>,
    /// Section title (`None` for steps before any header) and its steps.
    pub sections: Vec<(Option<String>, Vec<StepModel>)>,
    /// Aggregated ingredients, one per name, amounts summed across uses.
    pub ingredients: Vec<Ingredient>,
}

/// One step: plain prose for text targets, plus the segment structure that
/// carries a reference's values — the data CSS hooks hang from.
#[derive(Debug)]
pub struct StepModel {
    pub text: String,
    pub segments: Vec<SegModel>,
    /// The action's time-classified field, raw — the cook-mode hook
    /// (`data-duration`). Fuzzy durations ("overnight") are notes and
    /// stay absent.
    pub duration: Option<String>,
    /// Threads this step creates (`=> _dough`). The production clause is
    /// data, not prose — a traditional card never reads "→ base." The
    /// `data-produces` attribute carries it for targets that want it.
    pub produces: Vec<String>,
}

#[derive(Debug)]
pub enum SegModel {
    Prose(String),
    Ref {
        kind: RefKind,
        name: String,
        /// Quantity (ingredient) or state (tool) fields, raw — for inline
        /// display hooks like `data-amounts`.
        values: Vec<String>,
    },
    Arrow,
    Plus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RefKind {
    Ingredient,
    Thread,
    Tool,
}

#[derive(Debug)]
pub struct Ingredient {
    pub name: String,
    /// Amounts as written, flat — raw field text keeps authored fractions
    /// ("1 1/3 tsp") and hedges ("about 110 g") intact.
    pub amounts: Vec<String>,
    /// The same amounts grouped per measured use: a use's equivalents
    /// ("2 1/2 cups", "600 g") stay together so uses can add up.
    pub uses: Vec<Vec<Amount>>,
    /// Total and breakdown across `uses` (see `amounts::summarize`).
    pub summary: Summary,
    pub notes: Vec<String>,
    pub optional: bool,
}

/// Project a validated document into the semantic card model.
pub fn card(doc: &Document) -> CardModel {
    let mut card = CardModel::default();
    let mut ingredients: Vec<Ingredient> = Vec::new();

    for m in doc.meta_lines() {
        match m.key.as_str() {
            "title" => card.title = m.value.clone(),
            "summary" => card.summary = Some(m.value.clone()),
            "servings" => card.serves = Some(m.value.clone()),
            "image" => card.photos.push(m.value.clone()),
            "note" => card.notes.push(m.value.clone()),
            _ => {
                if let Some(label) = meta_label(&m.key) {
                    card.meta.push((label, m.value.clone()));
                }
            }
        }
    }

    let mut section: Option<Option<String>> = None;
    for block in &doc.blocks {
        match block {
            Block::Section { title, .. } => {
                section = Some(Some(title.clone()));
                card.sections.push((Some(title.clone()), Vec::new()));
            }
            Block::Step(step) => {
                if section.is_none() {
                    section = Some(None);
                    card.sections.push((None, Vec::new()));
                }
                collect_ingredients(step, &mut ingredients);
                card.sections
                    .last_mut()
                    .expect("section opened above")
                    .1
                    .push(step_model(step));
            }
            _ => {}
        }
    }
    for i in &mut ingredients {
        i.summary = summarize(&i.uses);
    }
    card.ingredients = ingredients;
    card
}

/// What the ingredient list shows for an amount: the primary text, and a
/// breakdown when the primary is a total. One measured use shows as
/// written; several show their total with the breakdown behind it, or the
/// breakdown alone when no total sums cleanly.
fn amount_display(i: &Ingredient) -> (Option<String>, Option<String>) {
    if i.uses.len() >= 2 {
        match &i.summary.total {
            Some(t) => (Some(t.clone()), Some(i.summary.breakdown.clone())),
            None => (Some(i.summary.breakdown.clone()), None),
        }
    } else if i.amounts.is_empty() {
        (None, None)
    } else {
        (Some(i.amounts.join(", ")), None)
    }
}

/// URL shape per design.md's `>> source:` contract — "a value that parses
/// as a URL is the canonical link". http/https only: a scheme-relative or
/// bare `www.` string is an attribution statement, not a link.
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// The display host of a URL: authority without credentials or a leading
/// `www.`. Falls back to the whole string when no host can be read.
fn url_host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        url
    } else {
        host
    }
}

fn meta_label(key: &str) -> Option<&'static str> {
    Some(match key {
        "cuisine" => "cuisine",
        "course" => "course",
        "dish" => "dish",
        "diet" => "diet",
        "keywords" => "keywords",
        "source" => "source",
        "date" => "date",
        "author" => "by",
        "aka" => "also known as",
        "lang" => "language",
        _ => return None,
    })
}

fn collect_ingredients(step: &Step, out: &mut Vec<Ingredient>) {
    let segs = step
        .head
        .iter()
        .chain(step.continuations.iter().flat_map(|c| c.segs.iter()));
    for seg in segs {
        if let Seg::Tag(t) = seg {
            if t.sigil != Sigil::At {
                continue;
            }
            let mut amounts = Vec::new();
            let mut use_amounts = Vec::new();
            let mut notes = Vec::new();
            for f in &t.fields {
                match f.kind {
                    FieldKind::Quantity(_) => {
                        amounts.push(f.raw.clone());
                        use_amounts.extend(Amount::from_field(f));
                    }
                    FieldKind::Note => notes.push(f.raw.clone()),
                }
            }
            if let Some(alt) = t.alternatives.first() {
                notes.push(format!("or {}", alt.name));
            }
            match out.iter_mut().find(|i| i.name == t.name) {
                Some(existing) => {
                    existing.amounts.extend(amounts);
                    if !use_amounts.is_empty() {
                        existing.uses.push(use_amounts);
                    }
                    for n in notes {
                        if !existing.notes.contains(&n) {
                            existing.notes.push(n);
                        }
                    }
                }
                None => out.push(Ingredient {
                    name: t.name.clone(),
                    amounts,
                    uses: if use_amounts.is_empty() {
                        Vec::new()
                    } else {
                        vec![use_amounts]
                    },
                    summary: Summary {
                        total: None,
                        breakdown: String::new(),
                    },
                    notes,
                    optional: t.optional,
                }),
            }
        }
    }
}

/// A step's authored sentence, production clause split out — what the grid
/// offers on demand behind each verb.
pub(crate) fn step_text(step: &Step) -> String {
    step_model(step).text
}

/// One step's segment structure, plus the folded plain text. The production
/// clause (`=> _dough`) splits out as `produces` — the traditional card
/// never reads "→ base", and the trailing punctuation stays with the prose.
fn step_model(step: &Step) -> StepModel {
    let map = |seg: &Seg| match seg {
        Seg::Prose(p) => SegModel::Prose(p.clone()),
        Seg::Tag(t) => SegModel::Ref {
            kind: match t.sigil {
                Sigil::At => RefKind::Ingredient,
                Sigil::Hash => RefKind::Tool,
                Sigil::Underscore => RefKind::Thread,
            },
            name: t.name.clone(),
            values: match t.sigil {
                // Tool state is whatever the author wrote — magnitude or
                // word. Ingredient values are amounts only; notes stay in
                // the ingredient list.
                Sigil::Hash => t.fields.iter().map(|f| f.raw.clone()).collect(),
                _ => t
                    .fields
                    .iter()
                    .filter(|f| f.is_quantity())
                    .map(|f| f.raw.clone())
                    .collect(),
            },
        },
        Seg::Plus => SegModel::Plus,
        Seg::Arrow => SegModel::Arrow,
    };

    let mut raw = vec![SegModel::Prose(step.action.verb.clone())];
    for seg in &step.head {
        raw.push(map(seg));
    }
    for cont in &step.continuations {
        raw.push(SegModel::Prose(" ".into()));
        for seg in &cont.segs {
            raw.push(map(seg));
        }
    }

    // Split the production clause: everything from the Arrow onward is
    // data. Thread refs after the arrow are the produces; prose after
    // them accumulates as the tail — the closing period, then any
    // continuation text. All pieces survive.
    let mut segments: Vec<SegModel> = Vec::new();
    let mut produces: Vec<String> = Vec::new();
    let mut in_production = false;
    let mut tail = String::new();
    for seg in raw {
        match seg {
            SegModel::Arrow => in_production = true,
            SegModel::Ref {
                kind: RefKind::Thread,
                name,
                ..
            } if in_production => produces.push(name),
            SegModel::Plus if in_production => {}
            SegModel::Prose(p) if in_production => {
                let trimmed = p.trim_start();
                if !trimmed.is_empty() {
                    if !tail.is_empty() {
                        tail.push(' ');
                    }
                    tail.push_str(trimmed);
                }
            }
            other => segments.push(other),
        }
    }
    if !tail.is_empty() {
        // Trim trailing whitespace from the preceding prose so the tail
        // closes without a gap: "sauce." not "sauce ."
        while segments
            .last()
            .is_some_and(|s| matches!(s, SegModel::Prose(p) if p.trim().is_empty()))
        {
            segments.pop();
        }
        if let Some(SegModel::Prose(p)) = segments.last_mut() {
            while p.ends_with(char::is_whitespace) {
                p.pop();
            }
        }
        // Closing punctuation (.,;!?"'") attaches tight to the preceding
        // word; opening content (parens, words) gets a separator space.
        let closes = tail.chars().next().is_some_and(|c| ".,;!?\"'".contains(c));
        if closes {
            segments.push(SegModel::Prose(tail));
        } else {
            segments.push(SegModel::Prose(format!(" {tail}")));
        }
    }

    StepModel {
        text: fold_text(&segments),
        segments,
        duration: step
            .action
            .fields
            .iter()
            .find_map(|f| f.quantity().filter(|q| q.is_time()).map(|_| f.raw.clone())),
        produces,
    }
}

/// Segments → plain prose: refs as bare names, `=>` as "→", `+` as "+".
fn fold_text(segments: &[SegModel]) -> String {
    let mut text = String::new();
    for seg in segments {
        match seg {
            SegModel::Prose(p) => text.push_str(p),
            SegModel::Ref { name, .. } => text.push_str(name),
            SegModel::Arrow => text.push('→'),
            SegModel::Plus => text.push('+'),
        }
    }
    text.trim().to_string()
}

/// The schema.org Recipe payload the spec promised at JSON-LD emit time.
pub fn jsonld(card: &CardModel) -> serde_json::Value {
    let mut v = json!({
        "@context": "https://schema.org",
        "@type": "Recipe",
        "name": card.title,
    });
    let obj = v.as_object_mut().expect("json! builds an object");
    if let Some(s) = &card.summary {
        obj.insert("description".into(), json!(s));
    }
    if let Some(s) = &card.serves {
        obj.insert("recipeYield".into(), json!(s));
    }
    // schema.org accepts a single URL or an array; first is representative
    // either way. One image stays a string — the common real-world emit.
    match card.photos.as_slice() {
        [] => {}
        [only] => {
            obj.insert("image".into(), json!(only));
        }
        many => {
            obj.insert("image".into(), json!(many));
        }
    }
    if !card.ingredients.is_empty() {
        obj.insert(
            "recipeIngredient".into(),
            json!(card
                .ingredients
                .iter()
                .map(ingredient_line)
                .collect::<Vec<_>>()),
        );
    }
    let steps: Vec<&str> = card
        .sections
        .iter()
        .flat_map(|(_, s)| s.iter().map(|m| m.text.as_str()))
        .collect();
    if !steps.is_empty() {
        obj.insert("recipeInstructions".into(), json!(steps));
    }
    for (label, value) in &card.meta {
        let key = match *label {
            "cuisine" => "recipeCuisine",
            "course" => "recipeCategory",
            "keywords" => "keywords",
            "by" => "author",
            "date" => "datePublished",
            "language" => "inLanguage",
            "also known as" => "alternateName",
            _ => continue,
        };
        obj.insert(key.into(), json!(value));
    }
    v
}

fn ingredient_line(i: &Ingredient) -> String {
    let mut line = i.name.clone();
    if let (Some(amounts), _) = amount_display(i) {
        line.push_str(": ");
        line.push_str(&amounts);
    }
    if !i.notes.is_empty() {
        line.push_str(", ");
        line.push_str(&i.notes.join(", "));
    }
    line
}

/// The HTML target: a semantic, unstyled fragment with stable classes.
/// Meta values ride the article as data attributes — `data-cuisine` is the
/// color-coded-tab hook for print targets.
pub fn html_fragment(card: &CardModel) -> String {
    // Ingredients used in two or more measured steps (see `Ingredient::uses`).
    let summed: std::collections::HashSet<&str> = card
        .ingredients
        .iter()
        .filter(|i| i.uses.len() >= 2)
        .map(|i| i.name.as_str())
        .collect();
    let cuisine = card
        .meta
        .iter()
        .find(|(label, _)| *label == "cuisine")
        .map(|(_, v)| format!(" data-cuisine=\"{}\"", esc(v)))
        .unwrap_or_default();
    let mut out = String::new();
    out.push_str(&format!("<article class=\"culina-recipe\"{cuisine}>\n"));
    out.push_str(&format!(
        "<h1 class=\"culina-title\">{}</h1>\n",
        esc(&card.title)
    ));
    if let Some(serves) = &card.serves {
        out.push_str(&format!(
            "<p class=\"culina-serves\">Serves {}</p>\n",
            esc(serves)
        ));
    }
    if let Some(summary) = &card.summary {
        out.push_str(&format!(
            "<p class=\"culina-summary\">{}</p>\n",
            esc(summary)
        ));
    }
    if let Some(hero) = card.photos.first() {
        // Eager, not lazy: the hero is above the fold and usually the
        // largest contentful paint — lazy-loading it delays the thing the
        // reader came for.
        out.push_str(&format!(
            "<img class=\"culina-photo\" src=\"{}\" alt=\"{}\" loading=\"eager\">\n",
            esc(hero),
            esc(&card.title)
        ));
    }
    if !card.meta.is_empty() {
        out.push_str("<dl class=\"culina-meta\">\n");
        for (label, value) in &card.meta {
            // A URL is one unbreakable word: screens get a link whose
            // text is the host, and the full address rides the href for
            // print stylesheets (design.md, `>> source:`).
            let (value_cls, value_html) = if is_url(value) {
                (
                    "culina-meta-value culina-meta-url",
                    format!("<a href=\"{}\">{}</a>", esc(value), esc(url_host(value))),
                )
            } else {
                ("culina-meta-value", esc(value))
            };
            out.push_str(&format!(
                "<div class=\"culina-meta-item\"><dt class=\"culina-meta-key\">{}</dt><dd class=\"{}\">{}</dd></div>\n",
                esc(label),
                value_cls,
                value_html
            ));
        }
        out.push_str("</dl>\n");
    }
    if !card.ingredients.is_empty() {
        out.push_str("<ul class=\"culina-ingredients\">\n");
        for i in &card.ingredients {
            let optional = if i.optional { " culina-optional" } else { "" };
            out.push_str(&format!(
                "<li class=\"culina-ingredient{optional}\"><span class=\"culina-ing-name\">{}</span>",
                esc(&i.name)
            ));
            let (amounts, breakdown) = amount_display(i);
            if let Some(a) = amounts {
                out.push_str(&format!(
                    " <span class=\"culina-ing-amounts\">{}</span>",
                    esc(&a)
                ));
            }
            if !i.notes.is_empty() {
                out.push_str(&format!(
                    " <span class=\"culina-ing-notes\">{}</span>",
                    esc(&i.notes.join(", "))
                ));
            }
            if let Some(b) = breakdown {
                out.push_str(&format!(
                    " <span class=\"culina-ing-breakdown\">{}</span>",
                    esc(&b)
                ));
            }
            out.push_str("</li>\n");
        }
        out.push_str("</ul>\n");
    }
    if !card.sections.is_empty() {
        out.push_str("<div class=\"culina-method\">\n");
        for (title, steps) in &card.sections {
            if let Some(t) = title {
                out.push_str(&format!("<h3 class=\"culina-section\">{}</h3>\n", esc(t)));
            }
            out.push_str("<ol class=\"culina-steps\">\n");
            for step in steps {
                let dur = step
                    .duration
                    .as_ref()
                    .map(|d| format!(" data-duration=\"{}\"", esc(d)))
                    .unwrap_or_default();
                let prod = if step.produces.is_empty() {
                    String::new()
                } else {
                    format!(" data-produces=\"{}\"", esc(&step.produces.join(" + ")))
                };
                out.push_str(&format!("<li class=\"culina-step\"{dur}{prod}>"));
                for seg in &step.segments {
                    match seg {
                        SegModel::Prose(p) => out.push_str(&esc(p)),
                        SegModel::Ref { kind, name, values } => {
                            let (class, attr) = match kind {
                                RefKind::Ingredient => (
                                    // A summed ingredient's list line shows
                                    // the total, so each step's own portion
                                    // is the hook sheets show inline.
                                    if !values.is_empty() && summed.contains(name.as_str()) {
                                        "culina-ref-ingredient culina-ref-summed"
                                    } else {
                                        "culina-ref-ingredient"
                                    },
                                    if values.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" data-amounts=\"{}\"", esc(&values.join(", ")))
                                    },
                                ),
                                RefKind::Tool => (
                                    "culina-ref-tool",
                                    if values.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" data-state=\"{}\"", esc(&values.join(", ")))
                                    },
                                ),
                                RefKind::Thread => ("culina-ref-thread", String::new()),
                            };
                            out.push_str(&format!(
                                "<span class=\"culina-ref {class}\"{attr}>{}</span>",
                                esc(name)
                            ));
                        }
                        SegModel::Arrow => out.push('→'),
                        SegModel::Plus => out.push('+'),
                    }
                }
                out.push_str("</li>\n");
            }
            out.push_str("</ol>\n");
        }
        out.push_str("</div>\n");
    }
    if !card.notes.is_empty() {
        out.push_str("<aside class=\"culina-notes\">\n");
        for n in &card.notes {
            out.push_str(&format!("<p class=\"culina-note\">{}</p>\n", esc(n)));
        }
        out.push_str("</aside>\n");
    }
    // Supporting shots trail the notes — the card's words finish as one
    // run, then the media plates (design.md, `>> image:` semantics).
    if card.photos.len() > 1 {
        out.push_str("<div class=\"culina-gallery\">\n");
        for img in &card.photos[1..] {
            out.push_str(&format!(
                "<img class=\"culina-gallery-item\" src=\"{}\" alt=\"{}\" loading=\"lazy\">\n",
                esc(img),
                esc(&card.title)
            ));
        }
        out.push_str("</div>\n");
    }
    let mut ld = serde_json::to_string(&jsonld(card)).expect("jsonld serializes");
    // serde_json does not escape angle brackets; a raw "</script" inside a
    // string would close the element. JSON allows \u003c, which is inert
    // in HTML — the standard JSON-LD embedding defense.
    ld = ld.replace('<', "\\u003c");
    out.push_str(&format!(
        "<script type=\"application/ld+json\">{ld}</script>\n"
    ));
    out.push_str("</article>\n");
    out
}

/// HTML-escape. Entities are built from a codepoint at runtime so the raw
/// sequences never appear in source text — some tooling in this project's
/// pipeline decodes entities in transit.
pub(crate) fn esc(s: &str) -> String {
    let amp = char::from_u32(0x26).unwrap();
    let lt = format!("{amp}lt;");
    let gt = format!("{amp}gt;");
    let quot = format!("{amp}quot;");
    let amp_entity = format!("{amp}amp;");
    s.replace('&', &amp_entity)
        .replace('<', &lt)
        .replace('>', &gt)
        .replace('"', &quot)
}
