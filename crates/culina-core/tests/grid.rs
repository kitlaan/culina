//! The CfE grid (design.md, Output styles): the food DAG as a table —
//! ingredients left, each step a cell spanning what it combines, channels
//! carrying results right.

use culina_core::fmt::prepare;
use culina_core::grid::{grid, html_grid, CellKind, GridCell, GridModel};

const CROISSANT: &str = include_str!("fixtures/croissant.culina");
const CURRY: &str = include_str!("fixtures/red-curry.culina");

fn model(body: &str) -> GridModel {
    let src = format!(">> title: T\n>> servings: 1\n\n{body}");
    let (doc, _, _) = prepare(&src, None).expect("recipe prepares");
    grid(&doc)
}

fn cells(g: &GridModel) -> Vec<&GridCell> {
    g.rows.iter().flatten().collect()
}

fn step<'a>(g: &'a GridModel, verb: &str) -> &'a GridCell {
    cells(g)
        .into_iter()
        .find(
            |c| matches!(&c.kind, CellKind::Step { ops, .. } if ops.iter().any(|o| o.verb == verb)),
        )
        .unwrap_or_else(|| panic!("no step {verb}: {g:#?}"))
}

#[test]
fn joins_move_right_and_span_what_they_combine() {
    let g = model(
        ":mix @{flour|500 g}, @{salt|10 g} => _dry.\n:stir @{water|300 g}, @{yeast|5 g} => _wet.\n:combine _dry and _wet => _dough.\n",
    );
    assert_eq!(g.width, 3);
    let combine = step(&g, "combine");
    assert_eq!((combine.col, combine.rowspan), (2, 4));
    assert_eq!(step(&g, "mix").col, 1);
}

#[test]
fn a_late_ingredient_rides_a_channel_into_its_step() {
    let g = model(":mix @{flour|500 g}, @{water|300 g} => _a.\n:{rest|1 hour} _a.\n:add @{salt|10 g} to _a.\n");
    let add = step(&g, "add");
    assert_eq!(add.col, 3);
    // Salt rides a channel across columns 1 and 2, one strip per column.
    let salt_row = g.rows.len() - 1;
    let strips: Vec<usize> = g.rows[salt_row]
        .iter()
        .filter(|c| c.kind == CellKind::Channel { into_step: true })
        .map(|c| c.col)
        .collect();
    assert_eq!(strips, vec![1, 2]);
}

#[test]
fn setup_steps_head_the_grid() {
    let g = model(":preheat #{oven|450F}.\n:{bake|20 min} @{bread|1} in #oven.\n");
    assert_eq!(g.setup, vec!["preheat oven 450F"]);
    assert_eq!(g.width, 2);
}

#[test]
fn resetting_a_tool_rides_the_next_step() {
    let g = model(
        ":preheat #{oven|425F}.\n:{bake|10 min} @{bread|1} in #oven => _b.\n:lower #{oven|375F}.\n:{bake|15 min} _b.\n",
    );
    assert_eq!(g.setup, vec!["preheat oven 425F"]);
    let CellKind::Step { ops, .. } = &cells(&g)
        .into_iter()
        .rfind(|c| matches!(&c.kind, CellKind::Step { .. }))
        .unwrap()
        .kind
    else {
        unreachable!()
    };
    assert_eq!(ops[0].temps, vec!["375F"]);
}

#[test]
fn trailing_steps_without_ingredients_or_time_are_the_footer() {
    let g = model(":mix @{flour|1 cup} => _a.\n:{bake|20 min} _a.\n:cool _a.\n:serve _a.\n");
    assert_eq!(g.footer, vec!["cool", "serve"]);
}

#[test]
fn timeless_runs_share_a_column_and_timers_do_not() {
    let g = model(
        ":mix @{flour|1 cup} => _a.\n:pour _a.\n:stretch _a.\n:{rest|1 hour} _a.\n:{bake|20 min} _a.\n",
    );
    let CellKind::Step { ops, .. } = &step(&g, "mix").kind else {
        unreachable!()
    };
    let verbs: Vec<&str> = ops.iter().map(|o| o.verb.as_str()).collect();
    assert_eq!(verbs, vec!["mix", "pour", "stretch"]);
    assert_ne!(step(&g, "rest").col, step(&g, "bake").col);
}

#[test]
fn notes_ride_with_their_ingredient() {
    // Notes are read with the name ("green beans, frozen"); they take no
    // column of their own.
    let g = model(":cook @{onion|1|diced}, @{salt|1 tsp} => _a.\n");
    assert_eq!(step(&g, "cook").col, 1);
    let html = html_grid(&g);
    assert!(
        html.contains("onion<span class=\"culina-grid-note\">, diced</span>"),
        "{html}"
    );
}

#[test]
fn ingredients_show_one_unit_preferring_weight() {
    let g = model(":mix @{water|2 1/2 cups|600 g}, @{oil|2 tbsp}, @{egg} => _a.\n");
    let amounts: Vec<&str> = cells(&g)
        .iter()
        .filter_map(|c| match &c.kind {
            CellKind::Ingredient { amount, .. } => Some(amount.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(amounts, vec!["600 g", "2 tbsp", ""]);
}

#[test]
fn only_tall_cells_past_the_first_step_column_rotate() {
    let g = model(":mix @{a|1}, @{b|1} => _x.\n:{rest|1 hour} _x.\n:add @{c|1} to _x.\n");
    let rot = |v| matches!(step(&g, v).kind, CellKind::Step { rotate, .. } if rotate);
    assert!(!rot("mix"), "first step column reads across");
    assert!(rot("rest"));
    // The rotated cell carries its row count for the wrap cap.
    assert!(html_grid(&g).contains("style=\"--culina-grid-rows: "));
}

#[test]
fn a_fork_reaches_its_second_consumer_by_name() {
    let g = model(
        ":pick @{cilantro|1 bunch} into roots and leaves => _roots + _leaves.\n:pound _roots with @{garlic|2 cloves} => _paste.\n:serve _paste with _leaves.\n",
    );
    assert!(cells(&g).iter().any(|c| c.kind
        == CellKind::Thread {
            name: "leaves".into()
        }));
}

#[test]
fn fixtures_draw_one_tree_each() {
    for src in [CROISSANT, CURRY] {
        let (doc, _, _) = prepare(src, None).unwrap();
        let g = grid(&doc);
        // Every row starts with its ingredient or thread cell in column 0.
        for row in &g.rows {
            assert_eq!(row[0].col, 0, "{g:#?}");
        }
        let html = html_grid(&g);
        assert!(
            html.starts_with("<table class=\"culina-grid\" style=\"--culina-grid-rows: "),
            "{html}"
        );
    }
}

#[test]
fn html_keeps_the_sentence_behind_the_verb_and_escapes() {
    let g = model(":mix @{salt|1 tsp} & @{flour|1 cup} until <smooth> => _a.\n");
    let html = html_grid(&g);
    assert!(
        html.contains("class=\"culina-grid-verb\" title=\"mix salt"),
        "{html}"
    );
    assert!(html.contains("&lt;smooth&gt;"), "{html}");
    assert!(!html.contains("<smooth>"), "{html}");
}

#[test]
fn word_states_show_and_sizes_do_not() {
    let g = model(
        ":spread @{oil|1 tbsp} on #{sheet pan|18x13 in} => _pan.\n:heat @{oil|1 tbsp} in #{skillet|high} => _fat.\n:fry _pan and _fat => _a.\n",
    );
    let temps = |v| match &step(&g, v).kind {
        CellKind::Step { ops, .. } => ops[0].temps.clone(),
        _ => unreachable!(),
    };
    assert!(temps("spread").is_empty(), "a size describes the tool");
    assert_eq!(
        temps("heat"),
        vec!["skillet: high"],
        "a one-word state names its tool"
    );
}

#[test]
fn inputs_into_one_step_share_channel_strips() {
    // The cooked onion's channel (column 3) and the stock's channel
    // (columns 1-3) meet in column 3: one strip spans both rows there, so
    // no line splits two inputs to the same step.
    let g = model(
        ":mix @{a|1}, @{b|1} => _x.\n:{rest|1 hour} _x.\n:{bake|1 hour} _x.\n:slice @{onion|1} => _o.\n:{cook|5 min} _o.\n:simmer _x, _o, @{stock|1 cup} => _s.\n",
    );
    let strips = |k: usize| -> Vec<(usize, usize)> {
        g.rows
            .iter()
            .enumerate()
            .flat_map(|(r, cs)| cs.iter().map(move |c| (r, c)))
            .filter(|(_, c)| c.col == k && matches!(c.kind, CellKind::Channel { .. }))
            .map(|(r, c)| (r, c.rowspan))
            .collect()
    };
    assert_eq!(strips(3), vec![(2, 2)], "onion and stock share column 3");
    assert_eq!(strips(1), vec![(3, 1)], "only stock rides column 1");
}

#[test]
fn phrase_states_stand_alone() {
    let g = model(":{simmer|10 min} @{rice|1 cup} in #{pot|lid on} => _r.\n");
    let CellKind::Step { ops, .. } = &step(&g, "simmer").kind else {
        unreachable!()
    };
    assert_eq!(ops[0].temps, vec!["lid on"]);
}

#[test]
fn cells_box_their_content_and_slant_is_a_class() {
    use culina_core::grid::{html_grid_with, GridOptions};
    let g =
        model(":preheat #{oven|400F}.\n:mix @{flour|1 cup} => _a.\n:{bake|5 min} _a.\n:cool _a.\n");
    let plain = html_grid(&g);
    assert!(
        plain.starts_with("<table class=\"culina-grid\" style=\"--culina-grid-rows: "),
        "{plain}"
    );
    // Setup, ingredient, step, and footer cells each box their content.
    assert_eq!(plain.matches("culina-grid-content").count(), 5, "{plain}");
    let slant = html_grid_with(&g, &GridOptions { slant: true });
    assert!(
        slant.starts_with("<table class=\"culina-grid culina-grid-slant\""),
        "{slant}"
    );
}
