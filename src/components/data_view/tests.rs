use super::*;
use crate::{KeyBindings, KeySpec};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::Line;

use crate::{
    EventCtx, EventOutcome, Key, KeyEvent, KeyModifiers, LayoutCtx, Propagation, TuiEvent, TuiNode,
};

// Large cohesive behavior suite; private DataView state helpers stay local.

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
fn handled_key_stops_propagation() {
    let mut view = DataView::new([Row::new(1, "A"), Row::new(2, "B")], |row| row.id).column(
        Column::text("name", "Name", Constraint::Percentage(100), |row: &Row| {
            row.name.to_string()
        }),
    );
    let mut layout = LayoutCtx::new();
    <DataView<Row, usize> as TuiNode<()>>::layout(&mut view, Rect::new(0, 0, 10, 2), &mut layout);
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.event(&TuiEvent::Key(KeyEvent::from(Key::Down)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
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
fn shifted_horizontal_keys_follow_configured_navigation_keys() {
    let bindings = KeyBindings::new()
        .with_nav_line_left([
            KeySpec::key(Key::Left),
            KeySpec::plain('h'),
            KeySpec::plain('a'),
        ])
        .with_nav_line_right([
            KeySpec::key(Key::Right),
            KeySpec::plain('l'),
            KeySpec::plain('d'),
        ]);
    let mut view = DataView::new([Row::new(1, "ABCDEFGHIJKLMNOPQRST")], |row| row.id).column(
        Column::text("name", "Name", Constraint::Length(20), |row: &Row| {
            row.name.to_string()
        }),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 10, 2);

    let right = view.on_key_with_settings_and_bindings(
        KeyEvent {
            code: Key::Char('D'),
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
        &bindings,
    );
    assert!(right.handled);
    assert_eq!(view.scroll.offset().x, 8);

    let left = view.on_key_with_settings_and_bindings(
        KeyEvent {
            code: Key::Char('A'),
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
        &bindings,
    );
    assert!(left.handled);
    assert_eq!(view.scroll.offset().x, 0);
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
fn highlighted_row_style_is_applied_to_rendered_cell_content() {
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
    assert_eq!(cell.fg, theme.highlight_fg());
    assert_eq!(cell.bg, theme.highlight_bg());
}

#[test]
fn previous_highlight_background_is_cleared_after_navigation() {
    let mut view = DataView::list(
        [Row::new(1, "first"), Row::new(2, "second")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .focused(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 2)))
        .expect("data view should render");
    view.highlighted = 1;
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 2)))
        .expect("data view should render");

    let theme = crate::theme();
    let old_highlight_cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    let current_highlight_cell = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_ne!(old_highlight_cell.bg, theme.highlight_bg());
    assert_eq!(current_highlight_cell.bg, theme.highlight_bg());
}

#[test]
fn inactive_highlight_does_not_style_row() {
    let view = DataView::list(
        [Row::new(1, "selected")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut terminal = Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");

    let theme = crate::theme();
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_ne!(cell.fg, theme.highlight_fg());
    assert_ne!(cell.fg, theme.highlight_bg());
    assert_ne!(cell.bg, theme.highlight_bg());
}

#[test]
fn selected_row_style_is_applied_when_row_is_not_highlighted() {
    let view = DataView::list(
        [Row::new(1, "first"), Row::new(2, "second")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII)
    .selected([2]);
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 2)))
        .expect("data view should render");

    let theme = crate::theme();
    let content_cell = terminal.backend().buffer().cell((4, 1)).unwrap();
    assert_eq!(content_cell.fg, theme.selected_fg());
    assert_eq!(content_cell.bg, theme.selected_bg());
}

#[test]
fn single_selection_styles_row_without_selection_glyph() {
    let view = DataView::list(
        [Row::new(1, "first"), Row::new(2, "second")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Single)
    .selected([2]);
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 2)))
        .expect("data view should render");

    let theme = crate::theme();
    let first_content_cell = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(first_content_cell.symbol(), "s");
    assert_eq!(first_content_cell.fg, theme.selected_fg());
    assert_eq!(first_content_cell.bg, theme.selected_bg());
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
fn line_navigation_keeps_highlight_centered_without_scroll_animation() {
    let mut view = DataView::list(
        (0..20).map(Row::flat).collect::<Vec<_>>(),
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 20, 5);

    for _ in 0..3 {
        let _ = view.on_key_with_settings(
            KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            },
            area,
            settings,
        );
    }

    assert_eq!(view.highlighted, 3);
    assert_eq!(view.scroll.target_offset().y, 1);
    assert_eq!(view.scroll.offset().y, 1);
}

#[test]
fn page_navigation_centers_highlight_when_not_near_edges() {
    let mut view = DataView::list(
        (0..100).map(Row::flat).collect::<Vec<_>>(),
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 20, 21);

    let _ = view.on_key_with_settings(
        KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );

    assert_eq!(view.highlighted, 13);
    assert_eq!(view.scroll.target_offset().y, 3);
    assert_eq!(view.scroll.offset().y, 3);

    let _ = view.on_key_with_settings(
        KeyEvent {
            code: Key::PageDown,
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );

    assert_eq!(view.highlighted, 26);
    assert_eq!(view.scroll.target_offset().y, 16);
    assert_eq!(view.scroll.offset().y, 16);
}

#[test]
fn navigation_scrolls_up_when_highlight_moves_above_viewport_middle() {
    let mut view = DataView::list(
        (0..20).map(Row::flat).collect::<Vec<_>>(),
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 20, 5);

    for _ in 0..8 {
        let _ = view.on_key_with_settings(
            KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            },
            area,
            settings,
        );
    }
    assert_eq!(view.scroll.target_offset().y, 6);

    let _ = view.on_key_with_settings(
        KeyEvent {
            code: Key::Up,
            modifiers: KeyModifiers::NONE,
        },
        area,
        settings,
    );

    assert_eq!(view.highlighted, 7);
    assert_eq!(view.scroll.target_offset().y, 5);
    assert_eq!(view.scroll.offset().y, 5);
}

#[test]
fn held_navigation_advances_scroll_animation_before_key_repeat_stops() {
    let mut view = DataView::list(
        (0..40).map(Row::flat).collect::<Vec<_>>(),
        |row| row.id,
        |row| row.name.to_string(),
    );
    let settings = AnimationSettings::default();
    let area = Rect::new(0, 0, 20, 5);

    for _ in 0..8 {
        let _ = view.on_key_with_settings(
            KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            },
            area,
            settings,
        );
        let _ = Animated::tick(&mut view, settings.frame_duration(), settings);
    }

    assert_eq!(view.highlighted, 8);
    assert_eq!(view.scroll.target_offset().y, 6);
    assert_eq!(view.scroll.offset().y, 6);
    assert!(
        view.scroll.offset().y >= 2,
        "scroll offset should advance while navigation key is still repeating"
    );
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

#[test]
fn activation_mode_controls_key_and_navigation_activation() {
    let mut navigate = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .activation_mode(ActivationMode::OnNavigate);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = navigate.on_key_with_settings(down_key(), Rect::new(0, 0, 20, 2), settings);
    assert!(outcome.activated);
    assert_eq!(
        navigate.take_last_activated().map(|event| event.row_id),
        Some(2)
    );
    assert_eq!(
        navigate.take_events(),
        vec![
            DataViewTypedEvent::HighlightChanged { row_id: Some(2) },
            DataViewTypedEvent::Activated { row_id: 2 },
        ]
    );

    let mut manual = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .activation_mode(ActivationMode::Manual);
    let outcome = manual.on_key_with_settings(enter_key(), Rect::new(0, 0, 20, 1), settings);
    assert!(outcome.handled);
    assert!(!outcome.activated);
    assert!(manual.take_last_activated().is_none());
    assert!(manual.take_events().is_empty());
}

#[test]
fn manual_activation_mode_still_applies_activate_selection() {
    let mut view = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .activation_mode(ActivationMode::Manual)
    .selection_mode(SelectionMode::Single)
    .selection_trigger(SelectionTrigger::OnActivate);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(enter_key(), Rect::new(0, 0, 20, 1), settings);

    assert!(outcome.handled);
    assert!(outcome.changed);
    assert!(!outcome.activated);
    assert_eq!(view.selected_id(), Some(1));
    assert!(view.take_last_activated().is_none());
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::SelectionChanged {
            selected: vec![1],
            added: vec![1],
            removed: vec![],
        }]
    );
}

#[test]
fn default_selection_key_is_not_handled_when_selection_is_disabled() {
    let mut view = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('x'),
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 20, 1),
        settings,
    );

    assert_eq!(outcome, DataViewOutcome::IDLE);
    assert!(view.take_events().is_empty());
}

#[test]
fn expansion_keys_are_idle_without_tree_actions() {
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let area = Rect::new(0, 0, 20, 3);
    let mut plain = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    );

    for key in [
        space_key(),
        z_key(KeyModifiers::NONE),
        z_key(KeyModifiers::SHIFT),
    ] {
        assert_eq!(
            plain.on_key_with_settings(key, area, settings),
            DataViewOutcome::IDLE
        );
    }

    let mut leaf = tree_view().expanded([1, 2]);
    leaf.highlighted = 2;
    assert_eq!(leaf.highlighted_id(), Some(4));
    assert_eq!(
        leaf.on_key_with_settings(space_key(), area, settings),
        DataViewOutcome::IDLE
    );

    let mut tree_without_children = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent));

    assert_eq!(
        tree_without_children.on_key_with_settings(z_key(KeyModifiers::NONE), area, settings),
        DataViewOutcome::IDLE
    );
    assert_eq!(
        tree_without_children.on_key_with_settings(z_key(KeyModifiers::SHIFT), area, settings),
        DataViewOutcome::IDLE
    );
}

#[test]
fn selected_builder_and_queries_ignore_selection_when_mode_is_none() {
    let view = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selected([1]);

    assert!(view.selected.is_empty());
    assert!(view.selected_ids().is_empty());
    assert_eq!(view.selected_id(), None);
    assert!(!view.is_selected(&1));
    assert_eq!(view.check_state(&1), CheckState::Unchecked);

    let view = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selected([1])
    .selection_mode(SelectionMode::None);

    assert!(view.selected.is_empty());
    assert!(view.selected_ids().is_empty());
    assert!(!view.is_selected(&1));
}

#[test]
fn page_change_emits_navigation_activation_when_highlighted_index_stays_same() {
    let mut view = DataView::list(
        [
            Row::new(1, "one"),
            Row::new(2, "two"),
            Row::new(3, "three"),
            Row::new(4, "four"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .pagination(2)
    .activation_mode(ActivationMode::OnNavigate);

    let outcome = view.next_page();

    assert!(outcome.activated);
    assert_eq!(view.highlighted, 0);
    assert_eq!(view.highlighted_id(), Some(3));
    assert_eq!(
        view.take_last_activated().map(|event| event.row_id),
        Some(3)
    );
    assert_eq!(
        view.take_events(),
        vec![
            DataViewTypedEvent::HighlightChanged { row_id: Some(3) },
            DataViewTypedEvent::Activated { row_id: 3 },
        ]
    );
}

#[test]
fn collapse_and_sort_emit_navigation_activation_when_row_changes_at_same_index() {
    let mut collapsed = DataView::list(
        [
            Row {
                id: 1,
                parent: None,
                name: "root",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "child",
            },
            Row {
                id: 3,
                parent: None,
                name: "sibling",
            },
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .expanded([1])
    .activation_mode(ActivationMode::OnNavigate);
    collapsed.highlighted = 1;

    let collapse_outcome = collapsed.collapse_all();

    assert!(collapse_outcome.activated);
    assert_eq!(collapsed.highlighted, 1);
    assert_eq!(collapsed.highlighted_id(), Some(3));
    assert_eq!(
        collapsed.take_events(),
        vec![
            DataViewTypedEvent::HighlightChanged { row_id: Some(3) },
            DataViewTypedEvent::Activated { row_id: 3 },
        ]
    );

    let mut sorted = DataView::new([Row::new(1, "B"), Row::new(2, "A")], |row| row.id)
        .column(
            Column::text("name", "Name", Constraint::Percentage(100), |row: &Row| {
                row.name.to_string()
            })
            .sortable(|row: &Row| row.name.to_string()),
        )
        .activation_mode(ActivationMode::OnNavigate);

    let sort_outcome = sorted.sort_by("name", SortDirection::Ascending);

    assert!(sort_outcome.activated);
    assert_eq!(sorted.highlighted, 0);
    assert_eq!(sorted.highlighted_id(), Some(2));
    assert_eq!(
        sorted.take_events(),
        vec![
            DataViewTypedEvent::HighlightChanged { row_id: Some(2) },
            DataViewTypedEvent::Activated { row_id: 2 },
        ]
    );
}

#[test]
fn activate_key_emits_legacy_and_typed_activation_by_default() {
    let mut view = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings(enter_key(), Rect::new(0, 0, 20, 1), settings);

    assert!(outcome.activated);
    assert_eq!(
        view.take_last_activated().map(|event| event.row_id),
        Some(1)
    );
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::Activated { row_id: 1 }]
    );
}

#[test]
fn configured_activate_key_emits_activation() {
    let bindings =
        KeyBindings::new().with_data_view_activate([KeySpec::key(Key::Enter), KeySpec::plain('a')]);
    let mut view = DataView::list(
        [Row::new(1, "one")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    let outcome = view.on_key_with_settings_and_bindings(
        KeyEvent {
            code: Key::Char('a'),
            modifiers: KeyModifiers::NONE,
        },
        Rect::new(0, 0, 20, 1),
        settings,
        &bindings,
    );

    assert!(outcome.activated);
    assert_eq!(
        view.take_last_activated().map(|event| event.row_id),
        Some(1)
    );
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::Activated { row_id: 1 }]
    );
}

#[test]
fn single_and_multi_selection_emit_stable_ordered_changes() {
    let mut single = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Single);

    assert!(single.select_id(2));
    assert!(single.select_id(1));

    assert_eq!(single.selected_id(), Some(1));
    assert_eq!(
        single.take_events(),
        vec![
            DataViewTypedEvent::SelectionChanged {
                selected: vec![2],
                added: vec![2],
                removed: vec![],
            },
            DataViewTypedEvent::SelectionChanged {
                selected: vec![1],
                added: vec![1],
                removed: vec![2],
            },
        ]
    );

    let mut multi = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two"), Row::new(3, "three")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selected([3]);

    assert!(multi.toggle_selected(1));
    assert!(multi.toggle_selected(3));

    assert_eq!(multi.selected_ids(), vec![1]);
    assert_eq!(
        multi.take_events(),
        vec![
            DataViewTypedEvent::SelectionChanged {
                selected: vec![1, 3],
                added: vec![1],
                removed: vec![],
            },
            DataViewTypedEvent::SelectionChanged {
                selected: vec![1],
                added: vec![],
                removed: vec![3],
            },
        ]
    );
}

#[test]
fn selection_rejects_unknown_ids_consistently() {
    let mut view = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selected([1, 99]);

    assert!(view.is_selected(&1));
    assert!(!view.is_selected(&99));
    assert_eq!(view.selected_ids(), vec![1]);
    assert!(view.take_events().is_empty());

    assert!(!view.select_id(99));
    assert!(!view.toggle_selected(99));
    assert!(!view.is_selected(&99));
    assert_eq!(view.selected_ids(), vec![1]);
    assert!(view.take_events().is_empty());

    assert!(view.clear_selection());
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::SelectionChanged {
            selected: vec![],
            added: vec![],
            removed: vec![1],
        }]
    );

    let changed = view.replace_selection([99].into_iter().collect());
    assert!(!changed);
    assert!(!view.is_selected(&99));
    assert!(view.selected_ids().is_empty());
    assert!(view.take_events().is_empty());

    let changed = view.replace_selection([1, 99].into_iter().collect());
    assert!(changed);
    assert!(view.is_selected(&1));
    assert!(!view.is_selected(&99));
    assert_eq!(view.selected_ids(), vec![1]);
    assert_eq!(
        view.take_events(),
        vec![DataViewTypedEvent::SelectionChanged {
            selected: vec![1],
            added: vec![1],
            removed: vec![],
        }]
    );

    view.selected.insert(99);
    assert!(!view.is_selected(&99));
    assert_eq!(view.selected_ids(), vec![1]);
    assert!(!view.replace_selection([1, 99].into_iter().collect()));
    assert!(!view.selected.contains(&99));
    assert!(view.take_events().is_empty());
}

#[test]
fn tree_cascade_selects_collapsed_descendants_and_reports_indeterminate_parent() {
    let mut view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants);

    assert!(view.toggle_selected(2));

    assert_eq!(view.selected_ids(), vec![2, 4, 5]);
    assert_eq!(view.check_state(&2), CheckState::Checked);
    assert_eq!(view.check_state(&1), CheckState::Indeterminate);
    assert_eq!(visible_ids(&view), vec![1]);
}

#[test]
fn cascade_check_state_uses_descendants_for_non_leaf_rows() {
    let checked = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selected([4, 5]);

    assert_eq!(checked.selected_ids(), vec![4, 5]);
    assert!(!checked.is_selected(&2));
    assert_eq!(checked.check_state(&2), CheckState::Checked);

    let partial = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selected([4]);

    assert_eq!(partial.check_state(&2), CheckState::Indeterminate);
}

#[test]
fn cascade_parent_is_checked_when_all_section_descendants_are_selected() {
    let mut view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .expanded([1, 2, 3]);

    assert!(view.toggle_selected(2));
    assert!(view.toggle_selected(3));

    assert_eq!(view.selected_ids(), vec![2, 3, 4, 5, 6, 7]);
    assert_eq!(view.check_state(&1), CheckState::Checked);
}

#[test]
fn cascade_parent_is_checked_when_all_leaf_descendants_are_selected() {
    let view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selected([4, 5, 6, 7]);

    assert_eq!(view.selected_ids(), vec![4, 5, 6, 7]);
    assert_eq!(view.check_state(&2), CheckState::Checked);
    assert_eq!(view.check_state(&3), CheckState::Checked);
    assert_eq!(view.check_state(&1), CheckState::Checked);
}

#[test]
fn cascade_selection_builder_expands_parent_ids() {
    let view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selected([2]);

    assert_eq!(view.selected_ids(), vec![2, 4, 5]);
    assert_eq!(view.check_state(&2), CheckState::Checked);
}

#[test]
fn enabling_cascade_selection_expands_existing_parent_ids() {
    let view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selected([2])
        .selection_propagation(SelectionPropagation::CascadeDescendants);

    assert_eq!(view.selected_ids(), vec![2, 4, 5]);
    assert_eq!(view.check_state(&2), CheckState::Checked);
}

#[test]
fn cascade_selection_is_ignored_in_single_selection_mode() {
    let mut view = tree_view()
        .selection_mode(SelectionMode::Single)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selection_glyphs(SelectionGlyphs::ASCII)
        .expanded([1]);

    assert!(view.select_id(2));

    assert_eq!(view.selected_ids(), vec![2]);
    assert_eq!(view.check_state(&1), CheckState::Unchecked);
    assert_eq!(view.check_state(&2), CheckState::Checked);
    assert_eq!(view.check_state(&4), CheckState::Unchecked);

    let visible = view.visible_rows();
    let root = visible.iter().find(|row| row.id == 1).unwrap();
    let section = visible.iter().find(|row| row.id == 2).unwrap();
    assert_eq!(view.selection_glyph(root), "[ ]");
    assert_eq!(view.selection_glyph(section), "[x]");

    assert!(view.toggle_selected(1));
    assert_eq!(view.selected_ids(), vec![1]);
    assert_eq!(view.check_state(&1), CheckState::Checked);
    assert_eq!(view.check_state(&2), CheckState::Unchecked);
}

#[test]
fn selection_prefix_contributes_render_width_and_shows_indeterminate_glyph() {
    let view = tree_view()
        .selection_mode(SelectionMode::Multi)
        .selection_propagation(SelectionPropagation::CascadeDescendants)
        .selection_glyphs(SelectionGlyphs::ASCII)
        .selected([4])
        .expanded([1]);

    assert_eq!(view.column_widths(1), vec![17]);

    let mut terminal = Terminal::new(TestBackend::new(12, 2)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 2)))
        .expect("data view should render");

    let visible = (0..12)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert_eq!(visible, " [-] root ┃");
}

fn tree_view() -> DataView<Row, usize> {
    DataView::list(rows(), |row| row.id, |row| row.name.to_string())
        .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
}

fn visible_ids<T>(view: &DataView<T, usize>) -> Vec<usize> {
    view.visible_rows().iter().map(|row| row.id).collect()
}

fn down_key() -> KeyEvent {
    KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }
}

fn enter_key() -> KeyEvent {
    KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }
}

fn space_key() -> KeyEvent {
    KeyEvent {
        code: Key::Char(' '),
        modifiers: KeyModifiers::NONE,
    }
}

fn z_key(modifiers: KeyModifiers) -> KeyEvent {
    let shifted = modifiers.contains(KeyModifiers::SHIFT);
    KeyEvent {
        code: Key::Char(if shifted { 'Z' } else { 'z' }),
        modifiers,
    }
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
