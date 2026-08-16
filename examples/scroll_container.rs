use ratatui::layout::Constraint;
use tuicore::{
    Button, Column, DataView, Flex, FlexItem, Panel, ScrollContainer, SelectionGlyphs,
    SelectionMode, SelectionTrigger, TextInput, TreeAdapter, TreeGlyphs,
};

#[derive(Clone)]
struct BacklogItem {
    id: usize,
    parent: Option<usize>,
    title: String,
}

fn main() -> tuicore::Result<()> {
    tuicore::init();

    let mixed_content = Flex::<()>::column()
        .gap(1)
        .child(
            "intro",
            Panel::new().top_left("Mixed content").content([
                "One outer viewport owns scrolling for panels, inputs, buttons, and tables.",
                "Tab into a control below the fold to reveal it.",
            ]),
            FlexItem::fit_content(),
        )
        .child(
            "name",
            TextInput::new().placeholder("Project name"),
            FlexItem::fit_content(),
        )
        .child("save", Button::new("Save draft"), FlexItem::fit_content())
        .child(
            "details",
            Panel::new()
                .top_left("Details")
                .content((0..12).map(|index| format!("Detail row {index}"))),
            FlexItem::fit_content(),
        )
        .child("table", task_table(), FlexItem::fit_content());

    let backlog = Flex::<()>::column()
        .gap(1)
        .child(
            "sprint-5",
            sprint_view("Sprint 5", 8),
            FlexItem::fit_content(),
        )
        .child(
            "sprint-6",
            sprint_view("Sprint 6", 8),
            FlexItem::fit_content(),
        )
        .child(
            "sprint-7",
            sprint_view("Sprint 7", 8),
            FlexItem::fit_content(),
        )
        .child(
            "backlog",
            sprint_view("Backlog", 120).hotkey("shift+b"),
            FlexItem::fit_content(),
        );

    let root = Flex::<()>::column()
        .gap(1)
        .child(
            "mixed",
            ScrollContainer::vertical(mixed_content),
            FlexItem::fill(1),
        )
        .child(
            "backlog",
            ScrollContainer::vertical(backlog),
            FlexItem::fill(1),
        );

    tuicore::TreeApp::new(root).run()
}

fn task_table() -> DataView<BacklogItem, usize> {
    DataView::new(backlog_rows("Table", 20), |item| item.id).column(
        Column::<BacklogItem, usize>::text("title", "Task", Constraint::Percentage(100), |item| {
            item.title.clone()
        }),
    )
}

fn sprint_view(title: &str, children: usize) -> DataView<BacklogItem, usize> {
    DataView::list(
        backlog_rows(title, children),
        |item| item.id,
        |item| item.title.clone(),
    )
    .tree(TreeAdapter::parent_id(|item: &BacklogItem| item.parent))
    .expanded([])
    .tree_glyphs(TreeGlyphs::NERD_FONT)
    .selection_mode(SelectionMode::Multi)
    .selection_trigger(SelectionTrigger::OnActivate)
    .selection_glyphs(SelectionGlyphs::NERD_FONT)
    .parent_vertical_scroll()
}

fn backlog_rows(title: &str, children: usize) -> Vec<BacklogItem> {
    let root_id = children.saturating_mul(10).saturating_add(title.len());
    std::iter::once(BacklogItem {
        id: root_id,
        parent: None,
        title: title.to_owned(),
    })
    .chain((1..=children).map(|index| BacklogItem {
        id: root_id.saturating_add(index),
        parent: Some(root_id),
        title: format!("{title} item {index}"),
    }))
    .collect()
}
