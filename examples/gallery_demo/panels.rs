use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{
    BorderKind, ChildKey, Dropdown, EventRoute, Flex, FlexItem, Grid, GridItem, GridTrack, Panel,
    PanelHost, PanelTitlePosition, Separator, SeparatorColorRole, Split, Tab, Tabs, TabsVariant,
};

use super::layouts::DemoBox;
use crate::Msg;

#[derive(Clone)]
pub(crate) struct PanelTitleChoice {
    pub(crate) id: &'static str,
    label: &'static str,
    enabled: bool,
}

fn panel_title_choices(position: PanelTitlePosition) -> Vec<PanelTitleChoice> {
    let enabled_label = match position {
        PanelTitlePosition::BottomRight => "show hotkey",
        _ => "show label",
    };
    vec![
        PanelTitleChoice {
            id: "none",
            label: "none",
            enabled: false,
        },
        PanelTitleChoice {
            id: "show",
            label: enabled_label,
            enabled: true,
        },
    ]
}

pub(crate) fn panel_demo() -> Panel {
    Panel::new().border(BorderKind::Plain).content([
        "Use dropdowns below to toggle each panel label or hotkey.",
        "Top labels use the standard - label - style.",
        "Bottom labels and hotkeys use the -| label |- inset style.",
    ])
}

pub(crate) fn panel_join_demo() -> PanelHost<Flex<Msg>> {
    Panel::new()
        .top_left("Joined separators")
        .bottom_left("Split + Grid + Flex composition")
        .border(BorderKind::Plain)
        .host(
            Flex::column()
                .separator(Separator::new().role(SeparatorColorRole::Subtle))
                .child("split", panel_join_split_demo(), FlexItem::fill(1))
                .child("grid", panel_join_grid_demo(), FlexItem::fill(2)),
        )
}

pub(crate) fn panel_tabs_join_demo() -> PanelHost<Tabs<Msg>> {
    Panel::new()
        .top_left("Tabs + separators")
        .bottom_left("Tab focuses tabs/body, not separator glyphs")
        .border(BorderKind::Plain)
        .host(panel_join_tabs_demo())
}

fn panel_join_split_demo() -> Split<DemoBox, DemoBox> {
    Split::horizontal(
        DemoBox::new("Split", "vertical separator joins panel top", 12, 3),
        DemoBox::new("PanelHost", "parent patches ┬/┴ where lines touch", 12, 3),
    )
    .separator(Separator::new().role(SeparatorColorRole::Subtle))
}

fn panel_join_grid_demo() -> Grid<Msg> {
    Grid::new()
        .columns([GridTrack::fill(1), GridTrack::fill(1), GridTrack::fill(1)])
        .rows([GridTrack::fill(1), GridTrack::fill(1)])
        .separator(Separator::new().role(SeparatorColorRole::Muted))
        .child(
            "one",
            DemoBox::new("Grid", "both axes create crosses", 10, 3),
            GridItem::new(0, 0),
        )
        .child(
            "two",
            DemoBox::new("Nested", "intersections stay inside", 10, 3),
            GridItem::new(0, 1),
        )
        .child(
            "three",
            DemoBox::new("Edges", "row line joins panel sides", 10, 3),
            GridItem::new(0, 2),
        )
        .child(
            "four",
            DemoBox::new("Span", "separator skips occupied span", 10, 3),
            GridItem::new(1, 0).span(3, 1),
        )
}

fn panel_join_tabs_demo() -> Tabs<Msg> {
    Tabs::new(vec![
        Tab::new(
            "Split",
            Split::horizontal(
                DemoBox::new("Tab body", "split separator inside tabs", 12, 3),
                DemoBox::new("Focus", "Tab targets Tabs, not separator", 12, 3),
            )
            .separator(Separator::new().role(SeparatorColorRole::Accent)),
        )
        .hotkey("s"),
        Tab::new(
            "Nested",
            Split::vertical(
                Split::horizontal(
                    DemoBox::new("Top left", "nested split", 10, 3),
                    DemoBox::new("Top right", "same separator type", 10, 3),
                )
                .separator(Separator::new().role(SeparatorColorRole::Muted)),
                DemoBox::new("Bottom", "nested separator joins inner split only", 24, 3),
            )
            .separator(Separator::new().role(SeparatorColorRole::Muted)),
        )
        .hotkey("n"),
    ])
    .variant(TabsVariant::Boxed)
    .bordered(true)
}

pub(crate) fn panel_title_dropdown(
    position: PanelTitlePosition,
) -> Dropdown<PanelTitleChoice, &'static str> {
    Dropdown::single(
        panel_title_choices(position),
        |row| row.id,
        |row| row.label.to_string(),
    )
    .placeholder(panel_title_placeholder(position))
    .selected_one("show")
    .label(panel_title_control_label(position))
    .hotkey(panel_title_control_hotkey(position))
}

fn panel_title_placeholder(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "Top left title...",
        PanelTitlePosition::TopRight => "Top right title...",
        PanelTitlePosition::BottomLeft => "Bottom left title...",
        PanelTitlePosition::BottomRight => "Panel hotkey...",
    }
}

fn panel_title_control_label(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "Top left",
        PanelTitlePosition::TopRight => "Top right",
        PanelTitlePosition::BottomLeft => "Bottom left",
        PanelTitlePosition::BottomRight => "Hotkey",
    }
}

fn panel_title_control_hotkey(position: PanelTitlePosition) -> &'static str {
    match position {
        PanelTitlePosition::TopLeft => "q",
        PanelTitlePosition::TopRight => "w",
        PanelTitlePosition::BottomLeft => "e",
        PanelTitlePosition::BottomRight => "r",
    }
}

pub(crate) fn apply_panel_choice(
    panel: &mut Panel,
    position: PanelTitlePosition,
    selected: Option<&'static str>,
) {
    let Some(choice) = panel_title_choices(position)
        .into_iter()
        .find(|choice| Some(choice.id) == selected)
    else {
        return;
    };
    if !choice.enabled {
        panel.clear_title(position);
        return;
    }

    match position {
        PanelTitlePosition::TopLeft => panel.set_top_left("top left"),
        PanelTitlePosition::TopRight => panel.set_top_right("top right"),
        PanelTitlePosition::BottomLeft => panel.set_bottom_left("bottom left"),
        PanelTitlePosition::BottomRight => panel.set_hotkey("p"),
    }
}

pub(crate) fn panel_preview_layout(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Fill(1),
        ])
        .areas(area)
}

pub(crate) fn panel_separator_preview_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .areas(area)
}

pub(crate) fn panel_title_control_areas(area: Rect) -> [Rect; 4] {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .areas(area)
}

pub(crate) fn panel_title_column_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area)
}

pub(crate) fn panel_title_dropdown_area(area: Rect) -> Rect {
    panel_title_column_layout(area)[1]
}

pub(crate) const PANEL_TITLE_CONTROL_COUNT: usize = 4;

pub(crate) fn panel_demo_child_key() -> ChildKey {
    ChildKey::new("panel-demo")
}

pub(crate) fn panel_join_demo_child_key() -> ChildKey {
    ChildKey::new("panel-join-demo")
}

pub(crate) fn panel_tabs_join_demo_child_key() -> ChildKey {
    ChildKey::new("panel-tabs-join-demo")
}

pub(crate) fn panel_title_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("panel-title-{index}"))
}

pub(crate) fn panel_title_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("panel-title-")?
        .parse()
        .ok()
        .filter(|index| *index < PANEL_TITLE_CONTROL_COUNT)
}

pub(crate) fn panel_title_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = panel_title_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

pub(crate) fn panel_demo_child_route(route: &EventRoute) -> Option<EventRoute> {
    route
        .path
        .without_first_if(&panel_demo_child_key())
        .map(EventRoute::new)
}

pub(crate) fn panel_join_demo_child_route(route: &EventRoute) -> Option<EventRoute> {
    route
        .path
        .without_first_if(&panel_join_demo_child_key())
        .map(EventRoute::new)
}

pub(crate) fn panel_tabs_join_demo_child_route(route: &EventRoute) -> Option<EventRoute> {
    route
        .path
        .without_first_if(&panel_tabs_join_demo_child_key())
        .map(EventRoute::new)
}
