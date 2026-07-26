use ratatui::layout::Constraint;
use tuicore::{Column, Flex, FlexItem, ListControl, ListControlField};

use crate::Msg;

const FIRST: &str = "first";
const SECOND: &str = "second";
const THIRD: &str = "third";

#[derive(Debug, Clone)]
pub(crate) struct ListDemoRow {
    pub(crate) id: usize,
    pub(crate) name: String,
    pub(crate) owner: String,
    pub(crate) state: String,
}

pub(crate) type ListControlShowcase = Flex<Msg>;

fn showcase<const N: usize>(
    controls: [ListControl<ListDemoRow, usize, Msg>; N],
) -> ListControlShowcase {
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
            layout.child(key, control, FlexItem::fit_content())
        })
}

pub(crate) fn compact_names() -> ListControlShowcase {
    showcase(compact_name_controls())
}

pub(crate) fn compact_name_controls() -> [ListControl<ListDemoRow, usize, Msg>; 2] {
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

pub(crate) fn entity_table() -> ListControlShowcase {
    showcase(entity_controls())
}

pub(crate) fn entity_controls() -> [ListControl<ListDemoRow, usize, Msg>; 3] {
    [
        entity_control(false, "All-text fields", "le"),
        entity_control(true, "Mixed fields", "lm"),
        people_control(),
    ]
}

fn people_control() -> ListControl<ListDemoRow, usize, Msg> {
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
        name: name.to_string(),
        owner: surname.to_string(),
        state: format!("{name} {surname}"),
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
                state: format!("{name} {owner}"),
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
    .headers(true)
    .title("Derived people")
    .hotkey("lp")
    .max_rows(3)
}

fn entity_control(mixed: bool, title: &str, hotkey: &str) -> ListControl<ListDemoRow, usize, Msg> {
    let rows = rows(["Gateway", "Worker", "Scheduler", "Indexer"]);
    let mut next_id = next_id(&rows);
    let owner = if mixed {
        ListControlField::text("Optional owner").optional()
    } else {
        ListControlField::text("Owner")
    };
    let state = if mixed {
        ListControlField::dropdown("Optional state", ["Active", "Ready", "Paused", "Running"])
            .optional()
    } else {
        ListControlField::text("State")
    };
    ListControl::new_fields(
        rows,
        |row| row.id,
        [ListControlField::text("Entity"), owner, state],
        move |values, _| {
            let mut values = values.into_iter();
            let row = ListDemoRow {
                id: next_id,
                name: values.next().expect("entity field exists"),
                owner: values.next().expect("owner field exists"),
                state: values.next().expect("state field exists"),
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
    .max_rows(4)
}

fn entity_columns() -> [Column<ListDemoRow, usize>; 3] {
    [
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
        name,
        owner: "You".to_string(),
        state: "Active".to_string(),
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
            name: name.to_string(),
            owner: ["Ada", "Grace", "Linus", "Mina"][index % 4].to_string(),
            state: ["Active", "Ready", "Paused", "Running"][index % 4].to_string(),
        })
        .collect()
}

fn next_id(rows: &[ListDemoRow]) -> usize {
    rows.iter().map(|row| row.id).max().unwrap_or(0) + 1
}
