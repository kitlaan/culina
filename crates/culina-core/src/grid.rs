//! The Cooking For Engineers grid (design.md, Output styles): the food DAG
//! drawn as a table. Ingredients sit in the left column, each step is a
//! cell spanning the rows of everything it combines, and a blank channel
//! carries a result right to the step that uses it.
//!
//! Built from the Document, not the CardModel: the grid needs per-use
//! ingredient notes and unit classification (weights, tool settings),
//! which the card flattens away.

use crate::amounts::Amount;
use crate::ast::{Document, Seg, Sigil, Step, Tag};
use crate::fields::FieldKind;
use crate::units::Dimension;

/// One recipe as a grid — target-independent, like the CardModel.
#[derive(Debug, Default)]
pub struct GridModel {
    pub title: String,
    /// Setup steps, outside the food graph ("preheat oven 450F"): a header.
    pub setup: Vec<String>,
    /// Trailing steps with no ingredient and no time ("serve"): a footer.
    pub footer: Vec<String>,
    /// Columns, the ingredient column included.
    pub width: usize,
    /// Cells by the row they start on, in column order.
    pub rows: Vec<Vec<GridCell>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridCell {
    pub col: usize,
    pub rowspan: usize,
    pub colspan: usize,
    pub kind: CellKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellKind {
    /// One use of an ingredient, in one unit: its weight when the author
    /// gave one, else its first amount. Empty for an implicit amount.
    Ingredient {
        amount: String,
        name: String,
        /// The use's notes ("diced", "frozen"), read with the name.
        notes: String,
        optional: bool,
    },
    /// A thread made earlier and already drawn into another step — a fork
    /// (`=> _roots + _leaves`) reaches its second consumer this way.
    Thread { name: String },
    /// One or more steps on the same thing. `rotate` is a layout fact: a
    /// multi-row cell past the first step column reads sideways.
    Step { ops: Vec<Op>, rotate: bool },
    /// Carries a result right. `into_step` is false for a result that no
    /// step uses, which runs to the table edge instead of opening into an L.
    Channel { into_step: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    pub verb: String,
    /// Tool settings the cook makes: temperatures ("450F") and word states
    /// ("high", "lid on").
    pub temps: Vec<String>,
    /// The action's time field, or its fuzzy note ("overnight").
    pub time: Option<String>,
    /// The authored sentence; the grid keeps only the verb, so renderers
    /// offer this on demand.
    pub prose: String,
}

#[derive(Debug)]
enum Leaf {
    Ingredient {
        amount: String,
        name: String,
        notes: String,
        optional: bool,
    },
    Thread(String),
}

#[derive(Debug)]
struct Node {
    leaf: Option<Leaf>,
    ops: Vec<Op>,
    children: Vec<usize>,
    timed: bool,
    consumed: bool,
}

impl Node {
    fn new() -> Node {
        Node {
            leaf: None,
            ops: Vec::new(),
            children: Vec::new(),
            timed: false,
            consumed: false,
        }
    }
}

/// What one step touches, read once from its segments.
struct StepParts<'a> {
    ingredients: Vec<&'a Tag>,
    tools: Vec<&'a Tag>,
    consumed: Vec<String>,
    produced: Vec<String>,
    time: Option<String>,
}

fn parts(step: &Step) -> StepParts<'_> {
    let segs: Vec<&Seg> = step
        .head
        .iter()
        .chain(step.continuations.iter().flat_map(|c| c.segs.iter()))
        .collect();
    let arrow = segs.iter().position(|s| matches!(s, Seg::Arrow));
    let mut p = StepParts {
        ingredients: Vec::new(),
        tools: Vec::new(),
        consumed: Vec::new(),
        produced: Vec::new(),
        time: None,
    };
    for (i, seg) in segs.iter().enumerate() {
        if let Seg::Tag(t) = seg {
            match t.sigil {
                Sigil::At => p.ingredients.push(t),
                Sigil::Hash => p.tools.push(t),
                Sigil::Underscore => {
                    if arrow.is_some_and(|a| i > a) {
                        p.produced.push(t.name.clone());
                    } else {
                        p.consumed.push(t.name.clone());
                    }
                }
            }
        }
    }
    let fields = &step.action.fields;
    p.time = fields
        .iter()
        .find(|f| f.quantity().is_some_and(|q| q.is_time()))
        .or_else(|| fields.iter().find(|f| !f.is_quantity()))
        .map(|f| f.raw.clone());
    p
}

/// A tool's size note: `18x13 in`, `9-inch`. Sizes are notes, not
/// quantities (the `x` breaks full consumption), so they need their own
/// test to stay out of the settings.
fn is_size(raw: &str) -> bool {
    let c: Vec<char> = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let by = c
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && (w[1] == 'x' || w[1] == '×') && w[2].is_ascii_digit());
    let unit = [" in", " cm", "-inch", " inch", " inches", "-in"]
        .iter()
        .any(|u| raw.ends_with(u));
    by || (unit && raw.chars().any(|c| c.is_ascii_digit()))
}

fn temps(tools: &[&Tag]) -> Vec<String> {
    tools
        .iter()
        .flat_map(|t| t.fields.iter())
        .filter(|f| f.quantity().and_then(|q| q.dim()) == Some(Dimension::Temperature))
        .map(|f| f.raw.clone())
        .collect()
}

/// The conditions a cook sets on a tool: temperatures and word states
/// (`high`, `lid on`). Sizes (`18x13 in`) describe the tool, not the step.
/// A one-word state names its tool (`sheet pan: parchment`): alone beside
/// the verb, "brush parchment" reads as the wrong thing.
fn settings(tools: &[&Tag]) -> Vec<String> {
    let mut out = Vec::new();
    for t in tools {
        for f in &t.fields {
            match f.quantity() {
                Some(q) if q.dim() == Some(Dimension::Temperature) => out.push(f.raw.clone()),
                Some(_) => {}
                None if is_size(&f.raw) => {}
                None if !f.raw.contains(char::is_whitespace) => {
                    out.push(format!("{}: {}", t.name, f.raw))
                }
                None => out.push(f.raw.clone()),
            }
        }
    }
    out
}

/// One unit per ingredient: the weight when the author gave one.
fn one_unit(t: &Tag) -> String {
    let amounts: Vec<Amount> = t.fields.iter().filter_map(Amount::from_field).collect();
    amounts
        .iter()
        .find(|a| a.quantity.dim() == Some(Dimension::Mass))
        .or(amounts.first())
        .map(|a| a.raw.clone())
        .unwrap_or_default()
}

fn ingredient_name(t: &Tag) -> String {
    let mut names = vec![t.name.clone()];
    names.extend(t.alternatives.iter().map(|a| a.name.clone()));
    names.join(" or ")
}

pub fn grid(doc: &Document) -> GridModel {
    let mut model = GridModel::default();
    for m in doc.meta_lines() {
        if m.key == "title" {
            model.title = m.value.clone();
        }
    }

    let steps: Vec<&Step> = doc.steps().collect();
    let all: Vec<StepParts> = steps.iter().map(|s| parts(s)).collect();

    // The tail of steps with no ingredient, no time, and no join (serve,
    // store) reads better as a footer than as columns. A join stays: it is
    // where the tracks meet.
    let mut end = steps.len();
    while end > 0
        && all[end - 1].ingredients.is_empty()
        && all[end - 1].time.is_none()
        && all[end - 1].consumed.len() <= 1
    {
        end -= 1;
    }
    model.footer = steps[end..].iter().map(|s| s.action.verb.clone()).collect();

    let mut nodes: Vec<Node> = Vec::new();
    // Tools a setup step has set. Setting one again ("lower oven 375F"
    // between bakes) is a change mid-cook, so it rides the next step.
    let mut set_tools: Vec<String> = Vec::new();
    let mut carried: Vec<String> = Vec::new();
    // Thread name → the node that currently holds it.
    let mut live: Vec<(String, usize)> = Vec::new();
    let lookup = |live: &Vec<(String, usize)>, name: &str| {
        live.iter().rev().find(|(n, _)| n == name).map(|(_, i)| *i)
    };

    for (step, p) in steps[..end].iter().zip(&all[..end]) {
        let held: Vec<usize> = {
            let mut v = Vec::new();
            for n in &p.consumed {
                if let Some(i) = lookup(&live, n) {
                    if !v.contains(&i) {
                        v.push(i);
                    }
                }
            }
            v
        };

        if p.ingredients.is_empty() && held.is_empty() && p.produced.is_empty() {
            // Setup: outside the food graph. Name the tools that carry a
            // temperature; fall back to every tool.
            let hot: Vec<String> = p
                .tools
                .iter()
                .filter(|t| !temps(&[*t]).is_empty())
                .map(|t| format!("{} {}", t.name, temps(&[*t]).join(" ")))
                .collect();
            let reset: Vec<String> = p
                .tools
                .iter()
                .filter(|t| set_tools.contains(&t.name) && !temps(&[*t]).is_empty())
                .flat_map(|t| temps(&[*t]))
                .collect();
            if !reset.is_empty() {
                carried.extend(reset);
                continue;
            }
            set_tools.extend(p.tools.iter().map(|t| t.name.clone()));
            let tools = if hot.is_empty() {
                p.tools.iter().map(|t| t.name.clone()).collect()
            } else {
                hot
            };
            let mut line = step.action.verb.clone();
            if !tools.is_empty() {
                line.push(' ');
                line.push_str(&tools.join(", "));
            }
            model.setup.push(line);
            continue;
        }

        let mut step_temps = std::mem::take(&mut carried);
        step_temps.extend(settings(&p.tools));
        let op = Op {
            verb: step.action.verb.clone(),
            temps: step_temps,
            time: p.time.clone(),
            prose: crate::render::step_text(step),
        };

        let target = if p.ingredients.is_empty()
            && held.len() == 1
            && p.time.is_none()
            && !nodes[held[0]].timed
            && !nodes[held[0]].consumed
        {
            // A timeless run on one thing shares its column — unless that
            // thing already feeds another step (a fork's other branch).
            let i = held[0];
            nodes[i].ops.push(op);
            i
        } else {
            let mut node = Node::new();
            node.timed = p.time.is_some();
            node.ops.push(op);
            let id = nodes.len();
            nodes.push(node);

            let mut threads: Vec<usize> = Vec::new();
            for &h in &held {
                if nodes[h].consumed {
                    // Already drawn under another step: reference it by name.
                    let name = p
                        .consumed
                        .iter()
                        .find(|n| lookup(&live, n) == Some(h))
                        .cloned()
                        .unwrap_or_default();
                    let mut r = Node::new();
                    r.leaf = Some(Leaf::Thread(name));
                    threads.push(nodes.len());
                    nodes.push(r);
                } else {
                    nodes[h].consumed = true;
                    threads.push(h);
                }
            }
            let mut leaves: Vec<usize> = Vec::new();
            for t in &p.ingredients {
                let notes: Vec<&str> = t
                    .fields
                    .iter()
                    .filter(|f| matches!(f.kind, FieldKind::Note))
                    .map(|f| f.raw.as_str())
                    .collect();
                let mut leaf = Node::new();
                leaf.leaf = Some(Leaf::Ingredient {
                    amount: one_unit(t),
                    name: ingredient_name(t),
                    notes: notes.join(", "),
                    optional: t.optional,
                });
                leaves.push(nodes.len());
                nodes.push(leaf);
            }
            // The main thing stays on top: threads, biggest first, then the
            // ingredients that join it.
            threads.sort_by_key(|&t| std::cmp::Reverse(leaf_count(&nodes, t)));
            threads.extend(leaves);
            nodes[id].children = threads;
            id
        };

        let names = if p.produced.is_empty() {
            &p.consumed
        } else {
            &p.produced
        };
        for n in names {
            live.push((n.clone(), target));
        }
    }

    let roots: Vec<usize> = (0..nodes.len())
        .filter(|&i| !nodes[i].consumed && nodes[i].leaf.is_none() && !is_child(&nodes, i))
        .collect();
    model.width = roots.iter().map(|&r| col(&nodes, r)).max().unwrap_or(0) + 1;

    let mut row = 0;
    for &r in &roots {
        let span = lay(&nodes, r, row, &mut model.rows);
        // A result no step uses runs to the table edge.
        channel(
            &mut model.rows,
            row,
            span,
            col(&nodes, r),
            model.width,
            false,
        );
        row += span;
    }
    model
}

fn is_child(nodes: &[Node], i: usize) -> bool {
    nodes.iter().any(|n| n.children.contains(&i))
}

fn leaf_count(nodes: &[Node], i: usize) -> usize {
    if nodes[i].children.is_empty() {
        1
    } else {
        nodes[i]
            .children
            .iter()
            .map(|&c| leaf_count(nodes, c))
            .sum()
    }
}

fn col(nodes: &[Node], i: usize) -> usize {
    if nodes[i].children.is_empty() {
        0
    } else {
        1 + nodes[i]
            .children
            .iter()
            .map(|&c| col(nodes, c))
            .max()
            .unwrap_or(0)
    }
}

fn place(rows: &mut Vec<Vec<GridCell>>, row: usize, cell: GridCell) {
    if rows.len() <= row {
        rows.resize_with(row + 1, Vec::new);
    }
    rows[row].push(cell);
    rows[row].sort_by_key(|c| c.col);
}

/// A channel from column `from` to the step in column `to`, over `span`
/// rows starting at `row`.
fn channel(
    rows: &mut Vec<Vec<GridCell>>,
    row: usize,
    span: usize,
    from: usize,
    to: usize,
    into_step: bool,
) {
    if to > from + 1 {
        place(
            rows,
            row,
            GridCell {
                col: from + 1,
                rowspan: span,
                colspan: to - from - 1,
                kind: CellKind::Channel { into_step },
            },
        );
    }
}

/// Lay out node `i` from `row`; returns the rows it spans. The parent draws
/// its children's channels one column at a time: in each column, every
/// row carrying an input to this step joins one strip, so no line splits
/// inputs into the same step. Strips meet side by side through their
/// hidden right borders, so a channel still reads as one band.
fn lay(nodes: &[Node], i: usize, row: usize, rows: &mut Vec<Vec<GridCell>>) -> usize {
    let n = &nodes[i];
    let c = col(nodes, i);
    if let Some(leaf) = &n.leaf {
        let kind = match leaf {
            Leaf::Ingredient {
                amount,
                name,
                notes,
                optional,
            } => CellKind::Ingredient {
                amount: amount.clone(),
                name: name.clone(),
                notes: notes.clone(),
                optional: *optional,
            },
            Leaf::Thread(name) => CellKind::Thread { name: name.clone() },
        };
        place(
            rows,
            row,
            GridCell {
                col: 0,
                rowspan: 1,
                colspan: 1,
                kind,
            },
        );
        return 1;
    }
    let mut r = row;
    // The column each row's input reaches before its channel starts.
    let mut reach: Vec<usize> = Vec::new();
    for &ch in &n.children {
        let cc = col(nodes, ch);
        let s = lay(nodes, ch, r, rows);
        reach.extend(std::iter::repeat_n(cc, s));
        r += s;
    }
    for k in 1..c {
        let mut start: Option<usize> = None;
        for (j, &cc) in reach.iter().enumerate() {
            match (cc < k, start) {
                (true, None) => start = Some(j),
                (false, Some(s)) => {
                    channel(rows, row + s, j - s, k - 1, k + 1, true);
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            channel(rows, row + s, reach.len() - s, k - 1, k + 1, true);
        }
    }
    // A step with no inputs still takes a row.
    let span = (r - row).max(1);
    let kind = CellKind::Step {
        ops: n.ops.clone(),
        // The first step column reads across; later columns turn sideways
        // once a cell spans rows.
        rotate: c >= 2 && span > 1,
    };
    place(
        rows,
        row,
        GridCell {
            col: c,
            rowspan: span,
            colspan: 1,
            kind,
        },
    );
    span
}

/// The grid as an HTML fragment: a `<table class="culina-grid">` with
/// stable `culina-grid-*` classes, unstyled like the card fragment.
pub fn html_grid(g: &GridModel) -> String {
    html_grid_with(g, &GridOptions::default())
}

/// How to emit the grid. Layout stays in the GridModel; these are
/// presentation switches the stylesheet acts on.
#[derive(Debug, Default, Clone)]
pub struct GridOptions {
    /// Lean the columns into a forward slash (`culina-grid-slant`).
    pub slant: bool,
}

/// Every cell's content sits in one `culina-grid-content` box, so a
/// stylesheet can transform the content apart from its cell (the slant
/// shears the table and shears each box back upright).
fn content(inner: &str) -> String {
    format!("<span class=\"culina-grid-content\">{inner}</span>")
}

pub fn html_grid_with(g: &GridModel, opts: &GridOptions) -> String {
    let esc = crate::render::esc;
    // The row count lets a stylesheet size the slant's lean (rows times row
    // height times tan 15deg) without measuring the rendered table.
    let rows = g.rows.len() + usize::from(!g.setup.is_empty()) + usize::from(!g.footer.is_empty());
    let cls = if opts.slant {
        "culina-grid culina-grid-slant"
    } else {
        "culina-grid"
    };
    let mut out = format!("<table class=\"{cls}\" style=\"--culina-grid-rows: {rows}\">\n");
    if !g.setup.is_empty() {
        out.push_str(&format!(
            "<thead><tr><td class=\"culina-grid-setup\" colspan=\"{}\">{}</td></tr></thead>\n",
            g.width,
            content(&esc(&g.setup.join(" · ")))
        ));
    }
    out.push_str("<tbody>\n");
    for cells in &g.rows {
        out.push_str("<tr>");
        for c in cells {
            let span = format!(
                "{}{}",
                if c.rowspan > 1 {
                    format!(" rowspan=\"{}\"", c.rowspan)
                } else {
                    String::new()
                },
                if c.colspan > 1 {
                    format!(" colspan=\"{}\"", c.colspan)
                } else {
                    String::new()
                }
            );
            match &c.kind {
                CellKind::Ingredient {
                    amount,
                    name,
                    notes,
                    optional,
                } => {
                    let cls = if *optional {
                        "culina-grid-ingredient culina-optional"
                    } else {
                        "culina-grid-ingredient"
                    };
                    let amt = if amount.is_empty() {
                        String::new()
                    } else {
                        format!("<span class=\"culina-grid-amount\">{}</span> ", esc(amount))
                    };
                    let note = if notes.is_empty() {
                        String::new()
                    } else {
                        format!("<span class=\"culina-grid-note\">, {}</span>", esc(notes))
                    };
                    out.push_str(&format!(
                        "<td class=\"{cls}\"{span}>{}</td>",
                        content(&format!("{amt}{}{note}", esc(name)))
                    ));
                }
                CellKind::Thread { name } => out.push_str(&format!(
                    "<td class=\"culina-grid-thread\"{span}>{}</td>",
                    content(&esc(name))
                )),
                CellKind::Channel { into_step } => {
                    let cls = if *into_step {
                        "culina-grid-channel"
                    } else {
                        "culina-grid-channel culina-grid-edge"
                    };
                    out.push_str(&format!("<td class=\"{cls}\"{span}></td>"))
                }
                CellKind::Step { ops, rotate } => {
                    let cls = if *rotate {
                        "culina-grid-step culina-grid-rotate"
                    } else {
                        "culina-grid-step"
                    };
                    let body = ops
                        .iter()
                        .map(|o| {
                            let mut s = format!(
                                "<span class=\"culina-grid-verb\" title=\"{}\">{}</span>",
                                esc(&o.prose),
                                esc(&o.verb)
                            );
                            for t in &o.temps {
                                s.push_str(&format!(
                                    " <span class=\"culina-grid-temp\">{}</span>",
                                    esc(t)
                                ));
                            }
                            if let Some(t) = &o.time {
                                s.push_str(&format!(
                                    " <span class=\"culina-grid-time\">{}</span>",
                                    esc(t)
                                ));
                            }
                            s
                        })
                        .collect::<Vec<_>>()
                        .join("<span class=\"culina-grid-sep\"> · </span>");
                    // A rotated label wraps rather than stretch its rows; the
                    // stylesheet caps its length at this many rows.
                    let rows = if *rotate {
                        format!(" style=\"--culina-grid-rows: {}\"", c.rowspan)
                    } else {
                        String::new()
                    };
                    out.push_str(&format!(
                        "<td class=\"{cls}\"{span}{rows}>{}</td>",
                        content(&format!("<span class=\"culina-grid-ops\">{body}</span>"))
                    ));
                }
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n");
    if !g.footer.is_empty() {
        out.push_str(&format!(
            "<tfoot><tr><td class=\"culina-grid-footer\" colspan=\"{}\">{}</td></tr></tfoot>\n",
            g.width,
            content(&format!("then {}", esc(&g.footer.join(" · "))))
        ));
    }
    out.push_str("</table>\n");
    out
}
