use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{
    ChildKey, Dropdown, DropdownCommitMode, DropdownLabelPosition, DropdownSearchMode,
    DropdownVariant, EventRoute,
};

#[derive(Clone)]
pub(crate) struct DropdownDemoItem {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
}

fn dropdown_items() -> Vec<DropdownDemoItem> {
    vec![
        DropdownDemoItem {
            id: "alpha",
            label: "Alpha backlog",
        },
        DropdownDemoItem {
            id: "beta",
            label: "Beta build",
        },
        DropdownDemoItem {
            id: "gamma",
            label: "Gamma release",
        },
        DropdownDemoItem {
            id: "delta",
            label: "Delta docs",
        },
        DropdownDemoItem {
            id: "omega",
            label: "Omega ops",
        },
    ]
}

pub(crate) fn dropdown_fuzzy_single() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick release lane...")
        .label("Lane")
        .hotkey("1")
}

pub(crate) fn dropdown_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::multi(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick workstreams...")
        .search_mode(DropdownSearchMode::Contains)
        .selected(["alpha", "delta"])
        .label("Work")
        .hotkey("2")
}

pub(crate) fn dropdown_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Immediate lane...")
        .search_mode(DropdownSearchMode::None)
        .commit_mode(DropdownCommitMode::Immediate)
        .centered(true)
        .selected_one("beta")
        .label("Immediate")
        .hotkey("3")
}

pub(crate) fn dropdown_filled_fuzzy_single() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_fuzzy_single()
        .selected_one("gamma")
        .variant(DropdownVariant::Filled)
        .label("Lane")
        .hotkey("4")
        .alt_style(true)
        .label_position(DropdownLabelPosition::Inline)
}

pub(crate) fn dropdown_filled_multi_contains() -> Dropdown<DropdownDemoItem, &'static str> {
    dropdown_multi_contains()
        .variant(DropdownVariant::Filled)
        .label("Work")
        .hotkey("5")
        .alt_style(true)
}

pub(crate) fn dropdown_filled_no_search_immediate() -> Dropdown<DropdownDemoItem, &'static str> {
    Dropdown::single(dropdown_items(), |row| row.id, |row| row.label.to_string())
        .placeholder("Pick immediate lane...")
        .search_mode(DropdownSearchMode::None)
        .commit_mode(DropdownCommitMode::Immediate)
        .no_selection_text("--None--")
        .variant(DropdownVariant::Filled)
        .label("Immediate")
        .hotkey("6")
        .alt_style(true)
}

pub(crate) fn dropdown_preview_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Fill(1)])
        .areas(area)
}

fn dropdown_columns(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .areas(area)
}

pub(crate) fn dropdown_grid_areas(area: Rect) -> [Rect; 6] {
    let rows: [Rect; 2] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .areas(area);
    let bordered = dropdown_columns(rows[0]);
    let filled = dropdown_columns(rows[1]);
    [
        bordered[0],
        bordered[1],
        bordered[2],
        filled[0],
        filled[1],
        filled[2],
    ]
}

pub(crate) fn dropdown_column_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(10),
            Constraint::Fill(1),
        ])
        .areas(area)
}

pub(crate) fn dropdown_area(area: Rect) -> Rect {
    dropdown_column_layout(area)[1]
}

pub(crate) fn dropdown_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("dropdown-{index}"))
}

pub(crate) fn dropdown_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("dropdown-")?
        .parse()
        .ok()
        .filter(|index| *index < 6)
}

pub(crate) fn dropdown_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = dropdown_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}
