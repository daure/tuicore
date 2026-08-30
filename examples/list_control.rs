use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Constraint, Rect},
};
use tuicore::{
    AnimationSettings, Column, EventCtx, EventOutcome, EventRoute, Flex, FlexItem, FocusCtx,
    FocusId, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx,
    ListControl, ListControlField, RenderCtx, SortDirection, Tab, Tabs, TickResult, TreeAdapter,
    TuiEvent, TuiNode,
};

const FIRST: &str = "first";
const SECOND: &str = "second";
const THIRD: &str = "third";

#[derive(Debug, Clone)]
pub(crate) struct ListDemoRow {
    pub(crate) id: usize,
    pub(crate) parent_id: Option<usize>,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) state: String,
    pub(crate) rank: usize,
}

pub(crate) type ListControlShowcase<M> = Flex<M>;

struct ShowcaseListControl<M> {
    control: ListControl<ListDemoRow, usize, M>,
}

impl<M: 'static> ShowcaseListControl<M> {
    fn new(control: ListControl<ListDemoRow, usize, M>) -> Self {
        Self { control }
    }

    fn discard_events(&mut self) {
        self.control.take_events();
    }
}

impl<M: 'static> TuiNode<M> for ShowcaseListControl<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.control.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.control.layout(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.control.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let outcome = self.control.event(event, ctx);
        self.discard_events();
        outcome
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let outcome = self.control.dispatch_event(route, event, ctx);
        self.discard_events();
        outcome
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.control.focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.control.dispatch_focus(target, focused, ctx);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.control.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.destroy(ctx);
    }
}

fn showcase<M, const N: usize>(
    controls: [ListControl<ListDemoRow, usize, M>; N],
) -> ListControlShowcase<M>
where
    M: 'static,
{
    controls
        .into_iter()
        .enumerate()
        .fold(Flex::column().gap(1), |layout, (index, control)| {
            let key = match index {
                0 => FIRST.to_string(),
                1 => SECOND.to_string(),
                2 => THIRD.to_string(),
                _ => format!("control-{index}"),
            };
            layout.child(
                key,
                ShowcaseListControl::new(control),
                FlexItem::fit_content(),
            )
        })
}

pub(crate) fn compact_names<M: 'static>() -> ListControlShowcase<M> {
    showcase(compact_name_controls())
}

pub(crate) fn compact_name_controls<M: 'static>() -> [ListControl<ListDemoRow, usize, M>; 2] {
    let text_rows = rows(["Ada", "Grace", "Linus", "Mina", "Ken", "Margaret"]);
    let mut text_id = next_id(&text_rows);
    let text = ListControl::list(
        text_rows,
        |row| row.id,
        |row| row.name.clone(),
        move |name, _| new_row(&mut text_id, name),
    )
    .title("Text names")
    .hotkey("lt")
    .confirm_remove("Remove name?", |row| {
        format!(
            "Remove {}? Current owner is {} and state is {}.",
            row.name, row.owner, row.state
        )
    })
    .max_rows(4);

    let dropdown_rows = rows(["Ada", "Grace", "Linus", "Mina"]);
    let mut dropdown_id = next_id(&dropdown_rows);
    let dropdown = ListControl::new_fields(
        dropdown_rows,
        |row| row.id,
        [ListControlField::dropdown(
            "Choose name",
            ["Barbara", "Donald", "Edsger", "Frances", "Guido"],
        )],
        move |mut values, _| new_row(&mut dropdown_id, values.remove(0)),
    )
    .column(Column::text(
        "name",
        "",
        Constraint::Percentage(100),
        |row: &ListDemoRow| row.name.clone(),
    ))
    .title("Dropdown names")
    .hotkey("ld")
    .max_rows(3);

    [text, dropdown]
}

pub(crate) fn entity_table<M: 'static>() -> ListControlShowcase<M> {
    showcase(entity_controls())
}

pub(crate) fn reorder_mode<M: 'static>() -> ListControlShowcase<M> {
    Flex::column()
        .gap(1)
        .child(
            FIRST,
            ShowcaseListControl::new(reorder_control()),
            FlexItem::fit_content(),
        )
        .child(
            SECOND,
            ShowcaseListControl::new(short_reorder_control()),
            FlexItem::fixed(12),
        )
}

pub(crate) fn reorder_mode_tree<M: 'static>() -> ListControlShowcase<M> {
    showcase([tree_reorder_control()])
}

pub(crate) fn reorder_control<M: 'static>() -> ListControl<ListDemoRow, usize, M> {
    reorder_list(
        rows([
            "Plan",
            "Build",
            "Review",
            "Ship",
            "Research",
            "Design",
            "Prototype",
            "Validate",
            "Document",
            "Test",
            "Package",
            "Release",
            "Monitor",
            "Measure",
            "Refine",
            "Secure",
            "Optimize",
            "Integrate",
            "Deploy",
            "Observe",
            "Support",
            "Audit",
            "Archive",
            "Retrospect",
        ]),
        "Shift+J/K range · Ctrl+J/K navigate + Space select · Ctrl+M move block · ↑↓ move target · Enter commit · Esc clear/cancel",
        "lr",
    )
}

fn short_reorder_control<M: 'static>() -> ListControl<ListDemoRow, usize, M> {
    reorder_list(
        rows([
            "Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf",
        ]),
        "7 items in a 10-row viewport · PageDown from Alpha",
        "ls",
    )
}

fn tree_reorder_control<M: 'static>() -> ListControl<ListDemoRow, usize, M> {
    let tree_rows = vec![
        tree_row(1, None, "Roadmap"),
        tree_row(2, Some(1), "Discovery"),
        tree_row(3, Some(2), "Interview users"),
        tree_row(4, Some(2), "Map workflows"),
        tree_row(5, Some(1), "Delivery"),
        tree_row(6, Some(5), "Build prototype"),
        tree_row(7, Some(5), "Run validation"),
        tree_row(8, None, "Operations"),
        tree_row(9, Some(8), "Monitor rollout"),
        tree_row(10, Some(8), "Collect feedback"),
    ];
    let mut next_id = next_id(&tree_rows);
    ListControl::list(
        tree_rows,
        |row| row.id,
        |row| row.name.clone(),
        move |name, _| new_row(&mut next_id, name),
    )
    .tree(TreeAdapter::mutable_parent_id(
        |row: &ListDemoRow| row.parent_id,
        |row, parent_id| row.parent_id = parent_id,
    ))
    .expanded([1, 2, 5, 8])
    .title("Shift+J/K range · Ctrl+J/K navigate + Space select · Ctrl+M move block · Esc cancel")
    .hotkey("lrt")
    .max_rows(12)
}

fn reorder_list<M: 'static>(
    reorder_rows: Vec<ListDemoRow>,
    title: &str,
    hotkey: &str,
) -> ListControl<ListDemoRow, usize, M> {
    let mut reorder_id = next_id(&reorder_rows);
    ListControl::list(
        reorder_rows,
        |row| row.id,
        |row| row.name.clone(),
        move |name, _| new_row(&mut reorder_id, name),
    )
    .column(
        Column::text("rank", "", Constraint::Length(0), |row: &ListDemoRow| {
            row.rank.to_string()
        })
        .reorderable(|row| row.rank, |row, rank| row.rank = rank)
        .hidden(),
    )
    .reorderable_by("rank")
    .action_bar(true)
    .title(title)
    .hotkey(hotkey)
    .max_rows(10)
}

pub(crate) fn entity_controls<M: 'static>() -> [ListControl<ListDemoRow, usize, M>; 3] {
    [
        entity_control(false, "All-text fields · Ctrl+X confirms delete", "le"),
        entity_control(true, "Mixed fields", "lm"),
        people_control(),
    ]
}

fn people_control<M: 'static>() -> ListControl<ListDemoRow, usize, M> {
    let rows = [
        ("Katherine", "Johnson"),
        ("Alan", "Turing"),
        ("Radia", "Perlman"),
        ("Margaret", "Hamilton"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, surname))| ListDemoRow {
        id: index + 1,
        parent_id: None,
        name: name.to_string(),
        owner: surname.to_string(),
        state: format!("{name} {surname}"),
        rank: (index + 1) * 10,
    })
    .collect::<Vec<_>>();
    let mut next_id = next_id(&rows);
    ListControl::new_fields(
        rows,
        |row| row.id,
        [
            ListControlField::text("First name"),
            ListControlField::text("Surname"),
        ],
        move |values, _| {
            let mut values = values.into_iter();
            let name = values.next().expect("first name exists");
            let owner = values.next().expect("surname exists");
            let row = ListDemoRow {
                id: next_id,
                parent_id: None,
                state: format!("{name} {owner}"),
                rank: next_id * 10,
                name,
                owner,
            };
            next_id += 1;
            row
        },
    )
    .editable(
        |row| vec![row.name.clone(), row.owner.clone()],
        |row, values| {
            row.name.clone_from(&values[0]);
            row.owner.clone_from(&values[1]);
            row.state = format!("{} {}", row.name, row.owner);
        },
    )
    .columns([
        Column::text(
            "initials",
            "",
            Constraint::Length(3),
            |row: &ListDemoRow| {
                format!(
                    "{}{}",
                    row.name.chars().next().unwrap_or_default(),
                    row.owner.chars().next().unwrap_or_default()
                )
            },
        )
        .constrained(),
        Column::text(
            "name",
            "Name",
            Constraint::Percentage(25),
            |row: &ListDemoRow| row.name.clone(),
        )
        .constrained(),
        Column::text(
            "surname",
            "Surname",
            Constraint::Percentage(25),
            |row: &ListDemoRow| row.owner.clone(),
        )
        .constrained(),
        Column::text(
            "full-name",
            "Full name",
            Constraint::Fill(1),
            |row: &ListDemoRow| row.state.clone(),
        )
        .constrained(),
    ])
    .copy_with(|row| row.state.clone())
    .headers(true)
    .title("Derived people · yy copies full name")
    .hotkey("lp")
    .max_rows(3)
}

fn entity_control<M: 'static>(
    mixed: bool,
    title: &str,
    hotkey: &str,
) -> ListControl<ListDemoRow, usize, M> {
    let mut rows = rows(["Gateway", "Worker", "Scheduler", "Indexer"]);
    if mixed {
        for (index, row) in rows.iter_mut().enumerate() {
            row.name = if index % 2 == 0 { "Person" } else { "Service" }.into();
            if row.name == "Person" {
                row.state.clear();
            } else {
                row.owner.clear();
            }
        }
    }
    let mut next_id = next_id(&rows);
    let kind = if mixed {
        ListControlField::dropdown("Kind", ["Person", "Service"])
    } else {
        ListControlField::text("Entity")
    };
    let owner = if mixed {
        ListControlField::text("Person name").visible_when(0, ["Person"])
    } else {
        ListControlField::text("Owner")
    };
    let state = if mixed {
        ListControlField::text("Service URL").visible_when(0, ["Service"])
    } else {
        ListControlField::text("State")
    };
    let control = ListControl::new_fields(
        rows,
        |row| row.id,
        [kind, owner, state],
        move |values, _| {
            let mut values = values.into_iter();
            let row = ListDemoRow {
                id: next_id,
                parent_id: None,
                name: values.next().expect("entity field exists"),
                owner: values.next().expect("owner field exists"),
                state: values.next().expect("state field exists"),
                rank: next_id * 10,
            };
            next_id += 1;
            row
        },
    )
    .editable(
        |row| vec![row.name.clone(), row.owner.clone(), row.state.clone()],
        |row, mut values| {
            row.name = values.remove(0);
            row.owner = values.remove(0);
            row.state = values.remove(0);
        },
    )
    .columns(entity_columns())
    .headers(true)
    .title(title)
    .hotkey(hotkey)
    .max_rows(4);
    if mixed {
        control.reorderable_by("rank")
    } else {
        control
            .confirm_remove("Remove entity?", |row| {
                format!(
                    "Remove {}? Current owner is {} and state is {}.",
                    row.name, row.owner, row.state
                )
            })
            .sorted_by("rank", SortDirection::Descending)
    }
}

fn entity_columns() -> [Column<ListDemoRow, usize>; 4] {
    [
        Column::text("rank", "#", Constraint::Length(5), |row: &ListDemoRow| {
            row.rank.to_string()
        })
        .sortable(|row| row.rank)
        .reorderable(|row| row.rank, |row, rank| row.rank = rank)
        .hidden()
        .constrained(),
        Column::text(
            "name",
            "Entity",
            Constraint::Percentage(45),
            |row: &ListDemoRow| row.name.clone(),
        )
        .constrained(),
        Column::text(
            "owner",
            "Owner",
            Constraint::Percentage(35),
            |row: &ListDemoRow| row.owner.clone(),
        )
        .constrained(),
        Column::text(
            "state",
            "State",
            Constraint::Percentage(20),
            |row: &ListDemoRow| row.state.clone(),
        )
        .constrained(),
    ]
}

fn new_row(next_id: &mut usize, name: String) -> ListDemoRow {
    let row = ListDemoRow {
        id: *next_id,
        parent_id: None,
        name,
        owner: "You".to_string(),
        state: "Active".to_string(),
        rank: *next_id * 10,
    };
    *next_id += 1;
    row
}

fn rows<const N: usize>(names: [&str; N]) -> Vec<ListDemoRow> {
    names
        .into_iter()
        .enumerate()
        .map(|(index, name)| ListDemoRow {
            id: index + 1,
            parent_id: None,
            name: name.to_string(),
            owner: ["Ada", "Grace", "Linus", "Mina"][index % 4].to_string(),
            state: ["Active", "Ready", "Paused", "Running"][index % 4].to_string(),
            rank: (index + 1) * 10,
        })
        .collect()
}

fn tree_row(id: usize, parent_id: Option<usize>, name: &str) -> ListDemoRow {
    ListDemoRow {
        id,
        parent_id,
        name: name.to_string(),
        owner: "Gallery".to_string(),
        state: "Ready".to_string(),
        rank: id * 10,
    }
}

fn next_id(rows: &[ListDemoRow]) -> usize {
    rows.iter().map(|row| row.id).max().unwrap_or(0) + 1
}

#[allow(dead_code)]
fn main() -> tuicore::Result<()> {
    tuicore::init();

    let tabs = Tabs::new(vec![
        Tab::<()>::new("Compact", compact_names()).hotkey("c"),
        Tab::new("Entities", entity_table()).hotkey("e"),
        Tab::new("Reorder", reorder_mode()).hotkey("r"),
        Tab::new("Tree reorder", reorder_mode_tree()).hotkey("t"),
    ]);

    tuicore::TreeApp::new(tabs).run()
}
