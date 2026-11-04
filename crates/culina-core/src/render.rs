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
    /// First (representative) image path, if the recipe has one.
    pub photo: Option<String>,
    /// Whole-recipe notes, one per authored `>> note:` line. Rendered after
    /// the method; never a meta row and never JSON-LD (schema.org has no
    /// honest slot — see design.md, key semantics).
    pub notes: Vec<String>,
    /// Section title (`None` for steps before any header) and its steps.
    pub sections: Vec<(Option<String>, Vec<StepModel>)>,
    /// Aggregated ingredients: (name, amounts, notes, optional).
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
    /// Amounts as written — raw field text keeps authored fractions
    /// ("1 1/3 tsp") and hedges ("about 110 g") intact. Exact summing
    /// waits for the scaling engine's arithmetic.
    pub amounts: Vec<String>,
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
            "image" => {
                if card.photo.is_none() {
                    card.photo = Some(m.value.clone());
                }
            }
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
    card.ingredients = ingredients;
    card
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
            let mut notes = Vec::new();
            for f in &t.fields {
                match f.kind {
                    FieldKind::Quantity(_) => amounts.push(f.raw.clone()),
                    FieldKind::Note => notes.push(f.raw.clone()),
                }
            }
            if let Some(alt) = t.alternatives.first() {
                notes.push(format!("or {}", alt.name));
            }
            match out.iter_mut().find(|i| i.name == t.name) {
                Some(existing) => {
                    existing.amounts.extend(amounts);
                    for n in notes {
                        if !existing.notes.contains(&n) {
                            existing.notes.push(n);
                        }
                    }
                }
                None => out.push(Ingredient {
                    name: t.name.clone(),
                    amounts,
                    notes,
                    optional: t.optional,
                }),
            }
        }
    }
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
    if let Some(p) = &card.photo {
        obj.insert("image".into(), json!(p));
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
    if !i.amounts.is_empty() {
        line.push_str(": ");
        line.push_str(&i.amounts.join(", "));
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
    if let Some(photo) = &card.photo {
        out.push_str(&format!(
            "<img class=\"culina-photo\" src=\"{}\" alt=\"{}\" loading=\"lazy\">\n",
            esc(photo),
            esc(&card.title)
        ));
    }
    if !card.meta.is_empty() {
        out.push_str("<dl class=\"culina-meta\">\n");
        for (label, value) in &card.meta {
            out.push_str(&format!(
                "<dt class=\"culina-meta-key\">{}</dt><dd class=\"culina-meta-value\">{}</dd>\n",
                esc(label),
                esc(value)
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
            if !i.amounts.is_empty() {
                out.push_str(&format!(
                    " <span class=\"culina-ing-amounts\">{}</span>",
                    esc(&i.amounts.join(", "))
                ));
            }
            if !i.notes.is_empty() {
                out.push_str(&format!(
                    " <span class=\"culina-ing-notes\">{}</span>",
                    esc(&i.notes.join(", "))
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
                                    "culina-ref-ingredient",
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
fn esc(s: &str) -> String {
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
