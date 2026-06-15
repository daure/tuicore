use super::*;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::ratatui::Terminal;
use tuirealm::ratatui::backend::TestBackend;
use tuirealm::ratatui::layout::{Constraint, Rect};
use tuirealm::ratatui::style::Style;
use tuirealm::ratatui::text::Line;

#[derive(Debug, Clone)]
struct Row {
    id: usize,
    parent: Option<usize>,
    name: &'static str,
}

#[derive(Debug, Clone)]
struct LevelRow {
    id: usize,
    level: usize,
    name: &'static str,
}

#[test]
fn parent_tree_places_children_under_each_parent() {
    let view = tree_view().expanded([1, 2, 3]);

    let rows = view.visible_rows();
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let depths = rows.iter().map(|row| row.depth).collect::<Vec<_>>();

    assert_eq!(ids, vec![1, 2, 4, 5, 3, 6, 7]);
    assert_eq!(depths, vec![0, 1, 2, 2, 1, 2, 2]);
}

#[test]
fn collapsing_middle_parent_keeps_later_sibling_children_with_that_sibling() {
    let view = tree_view().expanded([1, 3]);

    let rows = view.visible_rows();
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let depths = rows.iter().map(|row| row.depth).collect::<Vec<_>>();

    assert_eq!(ids, vec![1, 2, 3, 6, 7]);
    assert_eq!(depths, vec![0, 1, 1, 2, 2]);
}

#[test]
fn level_tree_sorts_siblings_without_reparenting_children() {
    let mut view = DataView::new(level_rows(), |row| row.id)
        .column(
            Column::text(
                "name",
                "Name",
                Constraint::Percentage(100),
                |row: &LevelRow| row.name.to_string(),
            )
            .sortable(|row: &LevelRow| row.name.to_string()),
        )
        .tree(TreeAdapter::level(|row: &LevelRow| row.level))
        .expanded([1, 2, 4]);

    let outcome = view.sort_by("name", SortDirection::Ascending);
    let rows = view.visible_rows();
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let depths = rows.iter().map(|row| row.depth).collect::<Vec<_>>();
    let parents = rows.iter().map(|row| row.parent_id).collect::<Vec<_>>();

    assert!(outcome.changed);
    assert_eq!(ids, vec![1, 4, 5, 2, 3]);
    assert_eq!(depths, vec![0, 1, 2, 1, 2]);
    assert_eq!(parents, vec![None, Some(1), Some(4), Some(1), Some(2)]);
}

#[test]
fn toggle_sort_cycles_from_ascending_to_descending_to_unsorted() {
    let mut view = DataView::new(
        [Row::new(1, "B"), Row::new(2, "A"), Row::new(3, "C")],
        |row| row.id,
    )
    .column(
        Column::text("name", "Name", Constraint::Percentage(100), |row: &Row| {
            row.name.to_string()
        })
        .sortable(|row: &Row| row.name.to_string()),
    );

    assert!(view.toggle_sort("name").changed);
    assert_eq!(visible_ids(&view), vec![2, 1, 3]);

    assert!(view.toggle_sort("name").changed);
    assert_eq!(visible_ids(&view), vec![3, 1, 2]);

    assert!(view.toggle_sort("name").changed);
    assert_eq!(view.sort, None);
    assert_eq!(visible_ids(&view), vec![1, 2, 3]);
}

#[test]
fn toggle_sort_can_target_any_sortable_column() {
    let mut view = DataView::new(
        [Row::new(1, "B"), Row::new(2, "A"), Row::new(3, "C")],
        |row| row.id,
    )
    .columns([
        Column::text("name", "Name", Constraint::Percentage(50), |row: &Row| {
            row.name.to_string()
        })
        .sortable(|row: &Row| row.name.to_string()),
        Column::text("id", "Id", Constraint::Percentage(50), |row: &Row| {
            row.id.to_string()
        })
        .sortable(|row: &Row| format!("{:02}", row.id)),
    ]);

    assert!(view.toggle_sort("name").changed);
    assert_eq!(visible_ids(&view), vec![2, 1, 3]);

    assert!(view.toggle_sort("id").changed);
    assert_eq!(visible_ids(&view), vec![1, 2, 3]);
}

#[test]
fn horizontal_scroll_offsets_rendered_cells() {
    let mut view = DataView::new([Row::new(1, "ABCDEFGHIJKL")], |row| row.id).column(Column::text(
        "name",
        "Name",
        Constraint::Length(12),
        |row: &Row| row.name.to_string(),
    ));
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 10, 2),
        settings,
    );
    assert!(outcome.handled);
    assert_eq!(view.scroll.offset().x, 1);

    let mut terminal = Terminal::new(TestBackend::new(10, 2)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 2)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    let visible = (0..10)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert_eq!(visible, "BCDEFGHIJK");
}

#[test]
fn shifted_horizontal_keys_jump_eight_columns() {
    let mut view = DataView::new([Row::new(1, "ABCDEFGHIJKLMNOPQRST")], |row| row.id).column(
        Column::text("name", "Name", Constraint::Length(20), |row: &Row| {
            row.name.to_string()
        }),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 10, 2);

    let right = view.on_key_with_settings(
        KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::SHIFT,
        },
        area,
        settings,
    );
    assert!(right.handled);
    assert_eq!(view.scroll.offset().x, 8);

    let left = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('H'),
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );
    assert!(left.handled);
    assert_eq!(view.scroll.offset().x, 0);
}

#[test]
fn shifted_horizontal_keys_scroll_tree_instead_of_expanding() {
    let mut view = DataView::new(
        [
            Row {
                id: 1,
                parent: None,
                name: "ABCDEFGHIJKLMNOPQRST",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "child",
            },
        ],
        |row| row.id,
    )
    .column(Column::text(
        "name",
        "Name",
        Constraint::Length(22),
        |row: &Row| row.name.to_string(),
    ))
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent));
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('L'),
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 8, 3),
        settings,
    );

    assert!(outcome.handled);
    assert!(!view.expanded.contains(&1));
    assert_eq!(view.scroll.offset().x, 8);
}

#[test]
fn horizontal_scroll_extent_uses_rendered_content_width() {
    let mut view = DataView::list(
        [Row::new(1, "ABCDEFGHIJKLMNO")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 10, 2);

    for _ in 0..10 {
        let _ = view.on_key_with_settings(
            KeyEvent {
                code: Key::Right,
                modifiers: KeyModifiers::NONE,
            },
            area,
            settings,
        );
    }

    assert_eq!(view.scroll.offset().x, 5);
}

#[test]
fn horizontal_scroll_extent_includes_percentage_column_expansion() {
    let view = DataView::new([Row::new(1, "A")], |row| row.id).columns([
        Column::text("first", "First", Constraint::Percentage(50), |row: &Row| {
            row.name.to_string()
        }),
        Column::text("second", "Second", Constraint::Percentage(50), |_| {
            String::from("B")
        }),
    ]);
    let area = Rect::new(0, 0, 10, 2);

    let geometry = view.scroll_geometry(area);
    let rendered_width = view
        .column_widths(geometry.layout.viewport.width as usize)
        .into_iter()
        .sum::<usize>();

    assert_eq!(geometry.content.width, rendered_width);
    assert_eq!(geometry.content.width, 10);
}

#[test]
fn selected_row_style_is_applied_to_rendered_cell_content() {
    let view = DataView::list(
        [Row::new(1, "selected")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .focused(true);
    let mut terminal = Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");

    let theme = crate::theme();
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, theme.selected_fg());
    assert_eq!(cell.bg, theme.selected_bg());
}

#[test]
fn tree_prefix_preserves_line_style_and_alignment() {
    let accent = crate::theme().accent_fg();
    let mut view = DataView::new([Row::new(1, "X"), Row::new(2, "Y")], |row| row.id)
        .column(Column::rich(
            "name",
            "Name",
            Constraint::Percentage(100),
            move |row: &Row, _| {
                Line::from(row.name)
                    .style(Style::default().fg(accent))
                    .centered()
            },
        ))
        .tree(TreeAdapter::parent_id(|row: &Row| row.parent));
    view.highlighted = 1;
    let mut terminal = Terminal::new(TestBackend::new(9, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 9, 2)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    let cell = buffer.cell((5, 0)).unwrap();
    assert_eq!(cell.symbol(), "X");
    assert_eq!(cell.fg, accent);
}

#[test]
fn tree_navigation_keeps_right_arrow_expansion_before_horizontal_scroll() {
    let mut view = tree_view();
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 8, 3),
        settings,
    );

    assert!(outcome.changed);
    assert!(view.expanded.contains(&1));
    assert_eq!(view.scroll.target_offset().x, 0);
}

#[test]
fn page_change_clamps_scroll_target_to_new_page() {
    let mut view = DataView::list(
        (0..13).map(Row::flat).collect::<Vec<_>>(),
        |row| row.id,
        |row| row.name.to_string(),
    )
    .pagination(10);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 20, 5);

    let _ = view.on_key_with_settings(
        KeyEvent {
            code: Key::End,
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );
    assert_eq!(view.scroll.target_offset().y, 5);

    let _ = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('n'),
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );

    assert_eq!(view.highlighted, 2);
    assert_eq!(view.scroll.target_offset().y, 0);
}

#[test]
fn collapsing_tree_clamps_page_to_remaining_rows() {
    let mut view = tree_view().expanded([1, 2, 3]).pagination(3);

    assert!(view.next_page().changed);
    assert!(view.next_page().changed);
    assert_eq!(view.pagination.as_ref().unwrap().page, 2);

    let outcome = view.collapse_all();
    let visible = view.visible_rows();
    let ids = visible.iter().map(|row| row.id).collect::<Vec<_>>();

    assert!(outcome.changed);
    assert_eq!(view.pagination.as_ref().unwrap().page, 0);
    assert_eq!(ids, vec![1]);
}

fn tree_view() -> DataView<Row, usize> {
    DataView::list(rows(), |row| row.id, |row| row.name.to_string())
        .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
}

fn visible_ids<T>(view: &DataView<T, usize>) -> Vec<usize> {
    view.visible_rows().iter().map(|row| row.id).collect()
}

impl Row {
    fn new(id: usize, name: &'static str) -> Self {
        Self {
            id,
            parent: None,
            name,
        }
    }

    fn flat(id: usize) -> Self {
        Self::new(id, "row")
    }
}

fn rows() -> Vec<Row> {
    vec![
        Row {
            id: 1,
            parent: None,
            name: "root",
        },
        Row {
            id: 2,
            parent: Some(1),
            name: "section 1",
        },
        Row {
            id: 3,
            parent: Some(1),
            name: "section 2",
        },
        Row {
            id: 4,
            parent: Some(2),
            name: "task 1",
        },
        Row {
            id: 5,
            parent: Some(2),
            name: "task 2",
        },
        Row {
            id: 6,
            parent: Some(3),
            name: "task 3",
        },
        Row {
            id: 7,
            parent: Some(3),
            name: "task 4",
        },
    ]
}

fn level_rows() -> Vec<LevelRow> {
    vec![
        LevelRow {
            id: 1,
            level: 0,
            name: "root",
        },
        LevelRow {
            id: 2,
            level: 1,
            name: "z parent",
        },
        LevelRow {
            id: 3,
            level: 2,
            name: "z child",
        },
        LevelRow {
            id: 4,
            level: 1,
            name: "a parent",
        },
        LevelRow {
            id: 5,
            level: 2,
            name: "a child",
        },
    ]
}
