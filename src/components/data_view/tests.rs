use super::*;
use crate::{KeyBindings, KeySpec};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusRequest, Key, KeyEvent, KeyModifiers, LayoutCtx, LayoutProposal, MouseEvent,
    MouseEventKind, Propagation, ScrollOffset, TreePath, TuiEvent, TuiNode, lerp_color, line_width,
    preset, theme,
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

#[derive(Debug, Clone)]
struct TransformRow {
    id: usize,
    task: &'static str,
    owner: &'static str,
    status: &'static str,
}

fn clear_transform_view() -> DataView<usize, usize> {
    DataView::new(1..=12, |id| *id).columns([
        Column::text(
            "value",
            "Value",
            Constraint::Percentage(50),
            |id: &usize| id.to_string(),
        )
        .filter_key(|id: &usize| id.to_string()),
        Column::text(
            "other",
            "Other",
            Constraint::Percentage(50),
            |id: &usize| id.to_string(),
        ),
    ])
}

fn assert_restored_highlight_is_visible(view: &DataView<usize, usize>, area: Rect) {
    let geometry = view.scroll_geometry(area);
    let target = view.scroll.target_offset().y;

    assert_eq!(view.highlighted_id(), Some(12));
    assert_eq!(view.highlighted, 11);
    assert_eq!(
        target,
        view.highlighted
            .saturating_add(1)
            .saturating_sub(geometry.viewport.height)
    );
}

fn assert_restored_highlight_is_centered(view: &DataView<usize, usize>, area: Rect) {
    let (geometry, rows) = view.scroll_geometry_and_row_geometry(area);
    let (row_start, row_end) = rows
        .span(view.highlighted)
        .expect("highlighted row should exist");
    let row_height = row_end.saturating_sub(row_start);
    let expected =
        row_start.saturating_sub(geometry.viewport.height.saturating_sub(row_height) / 2);

    assert_eq!(view.highlighted_id(), Some(7));
    assert_eq!(view.scroll.target_offset().y, expected);
}

fn assert_embedded_chip_styles(
    buffer: &ratatui::buffer::Buffer,
    chip_width: usize,
    foreground: Color,
    background: Color,
) {
    for x in [0, chip_width - 1] {
        let cap = buffer.cell((x as u16, 0)).unwrap();
        assert_eq!(cap.fg, foreground);
        assert_eq!(cap.bg, background);
    }
    let content = buffer.cell((1, 0)).unwrap();
    assert_eq!(content.fg, background);
    assert_eq!(content.bg, foreground);
}

#[test]
fn focused_event_precedence_can_be_disabled_for_app_hotkeys() {
    for (view, expected) in [
        (DataView::new([1], |id| *id), true),
        (
            DataView::new([1], |id| *id).focused_events_before_global_hotkeys(false),
            false,
        ),
    ] {
        let mut view: DataView<usize, usize> = view;
        let mut layout = LayoutCtx::new();

        <DataView<usize, usize> as TuiNode<()>>::layout(
            &mut view,
            Rect::new(0, 0, 20, 5),
            &mut layout,
        );

        assert_eq!(
            layout.focus_targets()[0].focused_events_before_global_hotkeys,
            expected
        );
    }
}

#[test]
fn resizing_the_viewport_keeps_the_highlighted_row_visible() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string());
    let old_area = Rect::new(0, 0, 20, 5);
    let resized_area = Rect::new(0, 0, 20, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut layout = LayoutCtx::new();

    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, old_area, &mut layout);
    view.highlighted = 9;
    view.ensure_highlight_visible(old_area, settings);
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, resized_area, &mut layout);

    assert_eq!(view.highlighted_id(), Some(9));
    assert_eq!(view.scroll.target_offset().y, 7);
}

#[test]
fn delegated_vertical_scroll_keeps_the_local_offset_at_zero_and_requests_reveal() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll();
    let area = Rect::new(0, 0, 20, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut layout = LayoutCtx::new();
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, area, &mut layout);
    <DataView<usize, usize> as TuiNode<()>>::focus(
        &mut view,
        None,
        true,
        &mut FocusCtx::new(settings),
    );
    let mut event = EventCtx::new(settings);

    let outcome = <DataView<usize, usize> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Key(KeyEvent::from(Key::Down)),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(view.scroll.offset().y, 0);
    assert_eq!(
        event.take_reveal_request().map(|(area, _)| area),
        Some(Rect::new(0, 1, 20, 1))
    );
    assert!(!event.layout_requested());
}

#[test]
fn repeated_wheel_input_snaps_data_view_without_animation() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string());
    let area = Rect::new(0, 0, 20, 5);
    let mut layout = LayoutCtx::new();
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, area, &mut layout);
    let mut ctx = EventCtx::new(AnimationSettings::default());

    for _ in 0..3 {
        let outcome = <DataView<usize, usize> as TuiNode<()>>::event(
            &mut view,
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        );
        assert_eq!(outcome, EventOutcome::Handled);
    }

    assert_eq!(view.scroll.offset().y, 3);
    assert_eq!(view.scroll.target_offset().y, 3);
    assert!(!view.scroll.is_active());
    assert!(!ctx.tick_requested());
}

#[test]
fn parent_delegated_data_view_bubbles_vertical_wheel_input() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll();
    let area = Rect::new(0, 0, 20, 5);
    let mut layout = LayoutCtx::new();
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, area, &mut layout);
    let mut ctx = EventCtx::new(AnimationSettings::default());

    let outcome = <DataView<usize, usize> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(ctx.propagation(), Propagation::Continue);
    assert_eq!(view.scroll.offset().y, 0);
}

#[test]
fn delegated_vertical_scroll_bubbles_navigation_at_its_upper_boundary() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll();
    let area = Rect::new(0, 0, 20, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut layout = LayoutCtx::new();
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, area, &mut layout);
    <DataView<usize, usize> as TuiNode<()>>::focus(
        &mut view,
        None,
        true,
        &mut FocusCtx::new(settings),
    );
    let mut event = EventCtx::new(settings);

    let outcome = <DataView<usize, usize> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Key(KeyEvent::from(Key::Up)),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(event.propagation(), Propagation::Continue);
    assert_eq!(event.take_reveal_request(), None);
}

#[test]
fn delegated_vertical_scroll_hides_the_local_vertical_scrollbar() {
    let view = DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll();

    let geometry = view.scroll_geometry(Rect::new(0, 0, 20, 3));

    assert_eq!(geometry.layout.vertical_bar, None);
}

#[test]
fn measured_height_reserves_a_row_for_an_overflowing_horizontal_scrollbar() {
    let view = DataView::new(0..20, |id| *id)
        .column(Column::text(
            "value",
            "Value",
            Constraint::Length(30),
            |id: &usize| id.to_string(),
        ))
        .parent_vertical_scroll();

    let measured = <DataView<usize, usize> as TuiNode<()>>::measure(
        &view,
        LayoutProposal::at_most(10, u16::MAX),
    );
    let geometry = view.scroll_geometry(Rect::new(0, 0, 10, measured.preferred.height));

    assert_eq!(measured.preferred.height, 21);
    assert!(geometry.layout.horizontal_bar.is_some());
    assert_eq!(geometry.viewport.height, 20);
}

#[test]
fn leaf_only_tree_omits_empty_chevron_gutter() {
    let view = DataView::list(
        [
            Row {
                id: 1,
                parent: None,
                name: "Alpha",
            },
            Row {
                id: 2,
                parent: None,
                name: "Beta",
            },
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .tree_glyphs(TreeGlyphs::ASCII)
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII);
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().symbol(),
        "["
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        "["
    );
}

#[test]
fn tree_with_branch_preserves_chevron_gutter_for_leaf_rows() {
    let view = DataView::list(
        [
            Row {
                id: 1,
                parent: None,
                name: "Parent",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "Child",
            },
            Row {
                id: 3,
                parent: None,
                name: "Leaf",
            },
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .tree_glyphs(TreeGlyphs::ASCII)
    .expanded([1])
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII);
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    assert_eq!(
        terminal.backend().buffer().cell((0, 2)).unwrap().symbol(),
        " "
    );
    assert_eq!(
        terminal.backend().buffer().cell((1, 2)).unwrap().symbol(),
        " "
    );
    assert_eq!(
        terminal.backend().buffer().cell((2, 2)).unwrap().symbol(),
        "["
    );
}

#[test]
fn left_gutter_marker_precedes_tree_indentation() {
    let view = DataView::list(
        [
            Row {
                id: 1,
                parent: None,
                name: "Parent",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "Child",
            },
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .tree_glyphs(TreeGlyphs::ASCII)
    .expanded([1])
    .left_gutter_marker_by(|row| (row.id == 2).then(|| Span::raw("┃")));
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().symbol(),
        "v"
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        "┃"
    );
}

#[test]
fn left_gutter_marker_repeats_for_wrapped_lines() {
    let view = DataView::list(
        [Row {
            id: 1,
            parent: None,
            name: "alpha bravo charlie",
        }],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .left_gutter_marker_by(|_| Some(Span::raw("┃")))
    .wrap_cells();
    let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    for y in 0..3 {
        assert_eq!(
            terminal.backend().buffer().cell((0, y)).unwrap().symbol(),
            "┃"
        );
    }
}

#[test]
fn row_update_preserves_order_and_resynchronizes_filtered_highlight() {
    let mut view = DataView::new(
        [(1, "Ada".to_string()), (2, "Grace".to_string())],
        |row: &(usize, String)| row.0,
    )
    .column(Column::text(
        "name",
        "Name",
        Constraint::Percentage(100),
        |row: &(usize, String)| row.1.clone(),
    ));
    view.set_search_query("Grace");
    assert_eq!(view.highlighted_id(), Some(2));

    view.update_row(&2, |row| row.1 = "Linus".to_string())
        .expect("row exists");

    assert_eq!(view.rows()[0].0, 1);
    assert_eq!(view.rows()[1], (2, "Linus".to_string()));
    assert_eq!(view.highlighted_id(), None);
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReorderRow {
    id: usize,
    rank: usize,
    group: &'static str,
}

fn reorder_view(rows: impl IntoIterator<Item = ReorderRow>) -> DataView<ReorderRow, usize> {
    let mut view = DataView::new(rows, |row| row.id).column(
        Column::text("rank", "Rank", Constraint::Fill(1), |row: &ReorderRow| {
            row.rank.to_string()
        })
        .search_key(|row| row.id.to_string())
        .filter_key(|row| row.group.to_string())
        .reorderable(|row| row.rank, |row, rank| row.rank = rank),
    );
    view.configure_reorder_sort("rank");
    view
}

fn reorder_rows() -> [ReorderRow; 2] {
    [
        ReorderRow {
            id: 1,
            rank: 10,
            group: "a",
        },
        ReorderRow {
            id: 2,
            rank: 20,
            group: "b",
        },
    ]
}

#[test]
fn reorder_allows_local_transforms_and_rejects_external_transforms() {
    let mut searched = reorder_view(reorder_rows());
    searched.set_search_query("1");
    assert_eq!(searched.reorder_snapshot("rank").unwrap().ids, vec![1, 2]);

    let mut filtered = reorder_view(reorder_rows());
    filtered.set_filter("rank", "a");
    assert_eq!(filtered.reorder_snapshot("rank").unwrap().ids, vec![1, 2]);

    let transforms: [fn(&mut DataView<ReorderRow, usize>); 2] = [
        |view: &mut DataView<ReorderRow, usize>| {
            view.set_search_query("1");
        },
        |view: &mut DataView<ReorderRow, usize>| {
            view.set_filter("rank", "a");
        },
    ];
    for configure_transform in transforms {
        let mut view = reorder_view(reorder_rows());
        view.set_transform_mode(DataViewTransformMode::External);
        configure_transform(&mut view);
        assert_eq!(
            view.reorder_snapshot("rank").err(),
            Some(ReorderUnavailableReason::TransformActive)
        );
    }
}

#[test]
fn reorder_rejects_paginated_tree_subset_and_duplicate_data() {
    assert_eq!(
        reorder_view(reorder_rows())
            .pagination(1)
            .reorder_snapshot("rank")
            .err(),
        Some(ReorderUnavailableReason::Paginated)
    );
    assert_eq!(
        reorder_view(reorder_rows())
            .tree(TreeAdapter::level(|_: &ReorderRow| 0))
            .reorder_snapshot("rank")
            .err(),
        Some(ReorderUnavailableReason::Tree)
    );
    assert_eq!(
        reorder_view(reorder_rows())
            .visible_row_ids([1])
            .reorder_snapshot("rank")
            .err(),
        Some(ReorderUnavailableReason::VisibleSubset)
    );
    assert_eq!(
        reorder_view([
            reorder_rows()[0].clone(),
            ReorderRow {
                id: 1,
                ..reorder_rows()[1].clone()
            }
        ])
        .reorder_snapshot("rank")
        .err(),
        Some(ReorderUnavailableReason::DuplicateRowIds)
    );
    assert_eq!(
        reorder_view([
            reorder_rows()[0].clone(),
            ReorderRow {
                rank: 10,
                ..reorder_rows()[1].clone()
            }
        ])
        .reorder_snapshot("rank")
        .err(),
        Some(ReorderUnavailableReason::DuplicateRankKeys)
    );
}

#[test]
fn reorder_commit_rejects_invalid_rank_setters_without_mutating_rows() {
    let setters: [fn(&mut ReorderRow, usize); 2] = [
        |_, _| {},
        |row, rank| {
            row.rank = rank;
            row.id += 100;
        },
    ];
    for setter in setters {
        let mut view = DataView::new(reorder_rows(), |row| row.id).column(
            Column::text("rank", "Rank", Constraint::Fill(1), |row: &ReorderRow| {
                row.rank.to_string()
            })
            .reorderable(|row| row.rank, setter),
        );
        view.configure_reorder_sort("rank");
        let snapshot = view.reorder_snapshot("rank").unwrap();
        let before = view.rows().to_vec();

        assert!(!view.commit_reorder("rank", &[2, 1], &snapshot));
        assert_eq!(view.rows(), before);
    }
}

#[test]
fn row_height_defaults_to_one_and_clamps_zero() {
    let mut view = DataView::list([1, 2], |row| *row, |row| row.to_string());

    assert_eq!(view.configured_row_height(), 1);
    view.set_row_height(0);
    assert_eq!(view.configured_row_height(), 1);
}

#[test]
fn dynamic_row_height_clamps_zero_and_fixed_height_replaces_policy() {
    let mut view = DataView::list([1, 2], |row| *row, |row| row.to_string())
        .row_height(3)
        .row_height_by(|row| if *row == 1 { 0 } else { 2 });

    assert_eq!(view.visible_row_geometry().total_height(), 3);
    assert_eq!(view.configured_row_height(), 3);
    view.set_row_height(4);
    assert_eq!(view.visible_row_geometry().total_height(), 8);
}

#[test]
fn multiline_cells_render_second_line_and_clip_beyond_row_height() {
    let view = DataView::new([1], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Fill(1),
            |_, _| {
                Text::from(vec![
                    Line::from("first"),
                    Line::from("second"),
                    Line::from("third"),
                ])
            },
        ))
        .row_height(2);
    let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "f");
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "s");
    assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), " ");
}

#[test]
fn wrapping_constrained_cells_expand_rows_without_horizontal_scrollbars() {
    let mut view = DataView::new(["A title that wraps at the viewport edge"], |title| *title)
        .column(
            Column::multiline(
                "title",
                "",
                Constraint::Percentage(100),
                |title: &&str, _| Text::from(*title),
            )
            .constrained(),
        )
        .wrap_cells();
    let area = Rect::new(0, 0, 12, 6);
    <DataView<_, _> as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
    let geometry = view.scroll_geometry(area);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    let text = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|position| terminal.backend().buffer().cell(position).unwrap().symbol())
        .collect::<String>();
    assert!(geometry.layout.horizontal_bar.is_none());
    assert!(geometry.content.height > 1);
    assert!(text.contains("viewport"));
    assert!(text.contains("edge"));
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        " "
    );
    assert_eq!(
        terminal.backend().buffer().cell((1, 1)).unwrap().symbol(),
        " "
    );
    assert_ne!(
        terminal.backend().buffer().cell((2, 1)).unwrap().symbol(),
        " "
    );
}

#[test]
fn wrapping_fill_cells_expand_rows_without_horizontal_scrollbars() {
    let mut view = DataView::new(["A title that wraps at the viewport edge"], |title| *title)
        .column(Column::multiline(
            "title",
            "",
            Constraint::Fill(1),
            |title: &&str, _| Text::from(*title),
        ))
        .wrap_cells();
    let area = Rect::new(0, 0, 12, 6);
    <DataView<_, _> as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
    let geometry = view.scroll_geometry(area);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    let text = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|position| terminal.backend().buffer().cell(position).unwrap().symbol())
        .collect::<String>();
    assert!(geometry.layout.horizontal_bar.is_none());
    assert!(geometry.content.height > 1);
    assert!(text.contains("viewport"));
    assert!(text.contains("edge"));
}

#[test]
fn wrapping_can_align_continuations_after_a_row_prefix() {
    let view = DataView::new([Row::new(1, "alpha bravo charlie")], |row| row.id)
        .column(
            Column::rich("title", "", Constraint::Fill(1), |row: &Row, _| {
                Line::from(vec![
                    Span::styled("ID-1", Style::default()),
                    Span::raw(format!(" {}", row.name)),
                ])
            })
            .wrap_continuation_indent_by(|_| 5),
        )
        .wrap_cells();
    let area = Rect::new(0, 0, 12, 4);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        " "
    );
    assert_eq!(
        terminal.backend().buffer().cell((4, 1)).unwrap().symbol(),
        " "
    );
    assert_eq!(
        terminal.backend().buffer().cell((5, 1)).unwrap().symbol(),
        "b"
    );
}

#[test]
fn multi_column_wrapping_excludes_inter_column_padding() {
    let view = DataView::new(["abcd e", "next"], |value| *value)
        .columns([
            Column::multiline("first", "", Constraint::Length(5), |value: &&str, _| {
                Text::from(*value)
            })
            .constrained(),
            Column::multiline("second", "", Constraint::Length(5), |_, _| Text::default()),
        ])
        .wrap_cells();
    let area = Rect::new(0, 0, 11, 3);
    let geometry = view.scroll_geometry(area);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    assert_eq!(geometry.content.height, 3);
    assert_eq!(
        terminal.backend().buffer().cell((2, 1)).unwrap().symbol(),
        "e"
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 2)).unwrap().symbol(),
        "n"
    );
}

#[test]
fn horizontal_clipping_uses_the_unclipped_wrapped_cell_width() {
    let mut view = DataView::new(["ab cd", "next"], |value| *value)
        .columns([
            Column::multiline("first", "", Constraint::Length(5), |value: &&str, _| {
                Text::from(*value)
            })
            .constrained(),
            Column::multiline("second", "", Constraint::Length(5), |_, _| Text::default()),
        ])
        .wrap_cells();
    let area = Rect::new(0, 0, 8, 3);
    let geometry = view.scroll_geometry(area);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    view.scroll.scroll_to(
        ScrollOffset::new(2, 0),
        geometry.viewport,
        geometry.content,
        settings,
    );
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    assert_eq!(geometry.content.height, 2);
    assert_eq!(
        terminal.backend().buffer().cell((1, 0)).unwrap().symbol(),
        "c"
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        "x"
    );
}

#[test]
fn partially_top_clipped_row_renders_its_continuation_line() {
    let mut view = DataView::new([1, 2], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Fill(1),
            |row, _| {
                Text::from(vec![
                    Line::from(format!("{row} first")),
                    Line::from(format!("{row} second")),
                ])
            },
        ))
        .row_height(2);
    let area = Rect::new(0, 0, 12, 2);
    let geometry = view.scroll_geometry(area);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    view.scroll.scroll_to(
        ScrollOffset::new(0, 1),
        geometry.viewport,
        geometry.content,
        settings,
    );
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().symbol(),
        "1"
    );
    assert_eq!(
        terminal.backend().buffer().cell((2, 0)).unwrap().symbol(),
        "s"
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().symbol(),
        "2"
    );
}

#[test]
fn intrinsic_width_uses_widest_multiline_continuation_with_prefix_gutter() {
    let view = DataView::new(
        [Row {
            id: 1,
            parent: None,
            name: "x",
        }],
        |row| row.id,
    )
    .column(Column::multiline(
        "value",
        "",
        Constraint::Length(1),
        |_, _| Text::from(vec![Line::from("x"), Line::from("long continuation")]),
    ))
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII)
    .row_height(2);

    assert_eq!(view.rendered_column_widths(), vec![21]);
}

#[test]
fn intrinsic_width_ignores_multiline_content_clipped_by_row_height() {
    let view = DataView::new([1], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Length(1),
            |_, _| Text::from(vec![Line::from("x"), Line::from("hidden continuation")]),
        ))
        .row_height(1);

    assert_eq!(view.rendered_column_widths(), vec![1]);
}

#[test]
fn empty_multiline_first_cell_still_renders_selection_gutter() {
    let view = DataView::new([1], |row| *row)
        .column(Column::multiline(
            "value",
            "",
            Constraint::Fill(1),
            |_, _| Text::default(),
        ))
        .selection_mode(SelectionMode::Multi)
        .selection_glyphs(SelectionGlyphs::ASCII);
    let mut terminal = Terminal::new(TestBackend::new(8, 1)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "[");
    assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), "]");
}

#[test]
fn multiline_continuations_align_after_ascii_tree_and_checkbox_gutters() {
    let view = DataView::new(
        [
            Row {
                id: 1,
                parent: None,
                name: "parent",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "child",
            },
        ],
        |row| row.id,
    )
    .column(Column::multiline(
        "value",
        "",
        Constraint::Fill(1),
        |row: &Row, _| Text::from(vec![Line::from(row.name), Line::from("metadata")]),
    ))
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .tree_glyphs(TreeGlyphs::ASCII)
    .expanded([1])
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII)
    .row_height(2);
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((6, 0)).unwrap().symbol(), "p");
    assert_eq!(buffer.cell((6, 1)).unwrap().symbol(), "m");
    assert_eq!(buffer.cell((8, 2)).unwrap().symbol(), "c");
    assert_eq!(buffer.cell((8, 3)).unwrap().symbol(), "m");
}

#[test]
fn mixed_row_heights_drive_measurement_content_reveal_centering_and_paging() {
    let area = Rect::new(0, 0, 20, 4);
    let mut view = DataView::list([1, 2, 3, 4], |row| *row, |row| row.to_string())
        .row_height_by(|row| [2, 1, 3, 1][*row - 1]);
    assert_eq!(view.visible_row_geometry().total_height(), 7);
    assert_eq!(view.scroll_geometry(area).content.height, 7);
    assert_eq!(
        <DataView<_, _> as TuiNode<()>>::measure(&view, LayoutProposal::unbounded())
            .preferred
            .height,
        7
    );
    assert_eq!(view.visible_page_step(area), 2);

    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    view.highlight_id(&3);
    view.ensure_highlight_visible(area, settings);
    assert_eq!(view.scroll.target_offset().y, 2);
    view.center_highlight(area, settings);
    assert_eq!(view.scroll.target_offset().y, 3);
}

#[test]
fn mixed_row_height_paging_uses_capacity_at_current_viewport() {
    let area = Rect::new(0, 0, 20, 4);
    let mut view = DataView::list(1..=6, |row| *row, |row| row.to_string())
        .row_height_by(|row| if *row == 1 { 4 } else { 1 });
    let geometry = view.scroll_geometry(area);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    view.highlight_id(&3);
    view.scroll.scroll_to(
        ScrollOffset::new(0, 4),
        geometry.viewport,
        geometry.content,
        settings,
    );

    assert_eq!(view.visible_page_step(area), 3);
    view.on_key_with_settings(Key::PageDown, area, settings);
    assert_eq!(view.highlighted_id(), Some(6));
    view.on_key_with_settings(Key::PageUp, area, settings);
    assert_eq!(view.highlighted_id(), Some(3));
}

#[test]
fn expanded_tree_content_height_sums_final_visible_rows() {
    let collapsed = tree_view().row_height_by(|row| if row.parent.is_some() { 2 } else { 1 });
    let expanded = tree_view()
        .expanded([1, 2, 3])
        .row_height_by(|row| if row.parent.is_some() { 2 } else { 1 });

    assert_eq!(collapsed.visible_row_geometry().total_height(), 1);
    assert_eq!(expanded.visible_row_geometry().total_height(), 13);
}

#[test]
fn row_height_changes_measurement_and_render_spacing() {
    let view = DataView::list([1, 2], |row| *row, |row| format!("row {row}"))
        .row_height(3)
        .focused(true);
    let measured =
        <DataView<usize, usize> as TuiNode<()>>::measure(&view, crate::LayoutProposal::unbounded());
    assert_eq!(measured.preferred.height, 6);

    let mut terminal = Terminal::new(TestBackend::new(12, 6)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "r");
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "r");
    for y in 0..3 {
        assert_eq!(
            buffer.cell((0, y)).unwrap().bg,
            crate::theme().highlight_bg()
        );
    }
}

#[test]
fn selected_non_cursor_row_normalizes_embedded_chip_colors() {
    let chip = crate::Chip::new("chip")
        .color_role(crate::ChipColorRole::Highlight)
        .line();
    let chip_width = line_width(&chip);
    let mut view = DataView::new([1], |row| *row)
        .columns([Column::rich(
            "chip",
            "",
            Constraint::Percentage(100),
            move |_, _| chip.clone(),
        )])
        .focused(false);
    view.selection_overlay = Some(SelectionOverlay {
        selected: vec![1],
        position: None,
        placeholder_depth: 0,
        placeholder_focused: false,
    });

    let mut terminal = Terminal::new(TestBackend::new(12, 1)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");

    let theme = theme();
    assert_embedded_chip_styles(
        terminal.backend().buffer(),
        chip_width,
        theme.selected_fg(),
        theme.selected_bg(),
    );
}

#[test]
fn row_height_uses_physical_lines_for_page_visibility() {
    let area = Rect::new(0, 0, 20, 4);
    let mut view = DataView::list(1..=8, |row| *row, |row| row.to_string()).row_height(2);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    view.on_key_with_settings(Key::PageDown, area, settings);

    assert_eq!(view.highlighted_id(), Some(3));
    assert_eq!(view.scroll.target_offset().y, 3);
}

#[test]
fn page_step_uses_visible_items_when_viewport_is_underfilled() {
    let area = Rect::new(0, 0, 20, 10);

    let underfilled = DataView::list(0..7, |row| *row, |row| row.to_string());
    let filled = DataView::list(0..10, |row| *row, |row| row.to_string());

    assert_eq!(underfilled.visible_page_step(area), 5);
    assert_eq!(filled.visible_page_step(area), 6);
}

#[test]
fn page_step_uses_item_capacity_and_visible_page_size_for_tall_rows() {
    let area = Rect::new(0, 0, 20, 10);
    let view = DataView::list(0..20, |row| *row, |row| row.to_string())
        .pagination(3)
        .row_height(2);

    assert_eq!(view.visible_page_step(area), 2);
}

#[test]
fn oversized_row_reveal_aligns_text_line_with_viewport_start() {
    let area = Rect::new(0, 0, 20, 3);
    let mut view = DataView::list([1, 2, 3], |row| *row, |row| format!("row {row}")).row_height(4);
    <DataView<_, _> as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());
    view.highlight_id(&3);

    view.reveal_highlighted();

    assert_eq!(view.scroll.target_offset().y, 8);
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");
    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().symbol(),
        "r"
    );
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
fn hidden_columns_support_alphabetical_and_numeric_automatic_sorting() {
    let alphabetical = DataView::new(
        [Row::new(1, "B"), Row::new(2, "A"), Row::new(3, "C")],
        |row| row.id,
    )
    .columns([
        Column::text("name", "Name", Constraint::Fill(1), |row: &Row| {
            row.name.to_string()
        })
        .sortable(|row: &Row| row.name.to_string())
        .hidden(),
        Column::text("id", "ID", Constraint::Fill(1), |row: &Row| {
            row.id.to_string()
        }),
    ])
    .sorted_by("name", SortDirection::Ascending);
    assert_eq!(visible_ids(&alphabetical), vec![2, 1, 3]);

    let numeric = DataView::new(
        [Row::new(10, "ten"), Row::new(2, "two"), Row::new(1, "one")],
        |row| row.id,
    )
    .columns([
        Column::text("id", "ID", Constraint::Fill(1), |row: &Row| {
            row.id.to_string()
        })
        .sortable(|row: &Row| row.id)
        .hidden(),
        Column::text("name", "Name", Constraint::Fill(1), |row: &Row| {
            row.name.to_string()
        }),
    ])
    .sorted_by("id", SortDirection::Ascending);
    assert_eq!(visible_ids(&numeric), vec![1, 2, 10]);
}

#[test]
fn hidden_columns_are_excluded_from_presentation_search_filters_and_measurement() {
    let mut view = DataView::new([Row::new(1, "secret")], |row| row.id)
        .columns([
            Column::text("hidden", "Hidden", Constraint::Length(40), |row: &Row| {
                row.name.to_string()
            })
            .filter_key(|row: &Row| row.name.to_string())
            .hidden(),
            Column::text("shown", "Shown", Constraint::Fill(1), |_| {
                String::from("public")
            }),
        ])
        .headers(true);

    assert_eq!(view.column_widths(12).len(), 1);
    assert_eq!(
        view.scroll_geometry(Rect::new(0, 0, 12, 2)).content.width,
        12
    );
    assert!(view.filterable_columns().is_empty());
    assert_eq!(view.filter_column_id_for_key('1'), None);
    view.set_search_query("secret");
    assert!(visible_ids(&view).is_empty());

    view.clear_search();
    let mut terminal = Terminal::new(TestBackend::new(12, 2)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Shown"));
    assert!(rendered.contains("public"));
    assert!(!rendered.contains("Hidden"));
    assert!(!rendered.contains("secret"));
}

#[test]
fn all_hidden_columns_still_render_action_bar_without_panicking() {
    let view = DataView::new([Row::new(1, "secret")], |row| row.id)
        .column(
            Column::text("hidden", "Hidden", Constraint::Length(40), |row: &Row| {
                row.name.to_string()
            })
            .hidden(),
        )
        .headers(true)
        .action_bar(true);
    assert!(view.column_widths(20).is_empty());
    assert_eq!(view.measurement_chrome_height(), 1);

    let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("all-hidden data view should render");
    assert!(
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .any(|cell| cell.symbol() != " ")
    );
    let action_bar = (0..20)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(action_bar.contains("Search..."));
}

#[test]
fn render_measures_each_visible_cell_once_per_pass() {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let renderer_calls = calls.clone();
    let view = DataView::new([Row::new(1, "A"), Row::new(2, "B")], |row| row.id).column(
        Column::rich("name", "Name", Constraint::Fill(1), move |row: &Row, _| {
            renderer_calls.set(renderer_calls.get() + 1);
            Line::from(row.name)
        }),
    );
    let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");

    assert_eq!(calls.get(), 4);
}

#[test]
fn line_navigation_and_rendering_measure_cells_linearly() {
    const ROWS: usize = 128;
    const VIEWPORT_HEIGHT: u16 = 5;

    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let renderer_calls = calls.clone();
    let mut view = DataView::new(0..ROWS, |row| *row).column(Column::rich(
        "value",
        "",
        Constraint::Fill(1),
        move |row: &usize, _| {
            renderer_calls.set(renderer_calls.get() + 1);
            Line::from(row.to_string())
        },
    ));
    let area = Rect::new(0, 0, 32, VIEWPORT_HEIGHT);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    view.on_key_with_settings(Key::Down, area, settings);
    assert_eq!(calls.get(), ROWS);

    calls.set(0);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|frame| view.render(frame, area)).unwrap();

    assert_eq!(calls.get(), ROWS + usize::from(VIEWPORT_HEIGHT));
}

#[test]
fn multi_select_width_measurement_accesses_source_rows_linearly() {
    fn measured_row_id_calls(rows: usize) -> usize {
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let row_id_calls = calls.clone();
        let view = DataView::new(0..rows, move |row| {
            row_id_calls.set(row_id_calls.get() + 1);
            *row
        })
        .column(Column::text(
            "value",
            "",
            Constraint::Fill(1),
            |row: &usize| row.to_string(),
        ))
        .selection_mode(SelectionMode::Multi)
        .selection_disabled_by(|row| row % 2 == 0);

        let _ = view.rendered_column_widths();
        calls.get()
    }

    let small = measured_row_id_calls(64);
    let large = measured_row_id_calls(128);

    assert!(large <= small * 3, "small={small}, large={large}");
}

#[test]
fn wrapped_line_navigation_measures_cells_a_fixed_number_of_times() {
    const ROWS: usize = 32;

    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let renderer_calls = calls.clone();
    let mut view = DataView::new(0..ROWS, |row| *row)
        .column(
            Column::rich(
                "value",
                "",
                Constraint::Percentage(100),
                move |row: &usize, _| {
                    renderer_calls.set(renderer_calls.get() + 1);
                    Line::from(format!("row {row} wraps across this narrow cell"))
                },
            )
            .constrained(),
        )
        .wrap_cells();
    let area = Rect::new(0, 0, 12, 5);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    view.on_key_with_settings(Key::Down, area, settings);

    assert_eq!(calls.get(), ROWS * 5);
}

#[test]
#[should_panic(expected = "must be sortable")]
fn plain_hidden_column_remains_invalid_for_automatic_sorting() {
    let _ = DataView::new([Row::new(1, "A")], |row| row.id)
        .column(
            Column::text("name", "Name", Constraint::Fill(1), |row: &Row| {
                row.name.to_string()
            })
            .hidden(),
        )
        .sorted_by("name", SortDirection::Ascending);
}

#[test]
fn local_search_supports_default_fuzzy_and_explicit_contains_modes() {
    for (mode, query, expected) in [
        (SearchMode::Fuzzy, "api", vec![1, 3]),
        (SearchMode::Fuzzy, "cp", vec![2]),
        (SearchMode::Contains, "cp", vec![]),
    ] {
        let mut view = transform_view().search_mode(mode);
        assert!(view.set_search_query(query).changed);
        assert_eq!(visible_ids(&view), expected);
    }
}

#[test]
fn tree_search_keeps_matching_child_ancestors_visible() {
    let mut view = tree_view().expanded([1, 3]);

    view.set_search_query("task 3");

    assert_eq!(visible_ids(&view), vec![1, 3, 6]);
    assert_eq!(view.highlighted_id(), Some(6));
}

#[test]
fn tree_search_prefers_a_direct_parent_match() {
    let mut view = tree_view();

    view.set_search_query("root");

    assert_eq!(view.highlighted_id(), Some(1));
}

#[test]
fn tree_search_input_expands_all_nodes_only_on_initial_query() {
    let area = Rect::new(0, 0, 40, 6);
    let mut view = tree_view().action_bar(true);

    view.on_key(KeyEvent::from(Key::Char('/')), area);
    view.on_key(KeyEvent::from(Key::Char('t')), area);

    assert_eq!(view.expanded, HashSet::from([1, 2, 3]));

    view.collapse_all();
    view.on_key(KeyEvent::from(Key::Char('a')), area);

    assert!(view.expanded.is_empty());
}

#[test]
fn level_tree_search_keeps_matching_child_ancestors_visible() {
    let mut view = DataView::list(level_rows(), |row| row.id, |row| row.name.to_string())
        .tree(TreeAdapter::level(|row: &LevelRow| row.level))
        .expanded([1, 2]);

    view.set_search_query("z child");

    assert_eq!(visible_ids(&view), vec![1, 2, 3]);
}

#[test]
fn active_tree_transform_still_allows_node_toggle() {
    let mut view = tree_view().expanded([1, 3]);
    view.set_search_query("task 3");

    assert_eq!(visible_ids(&view), vec![1, 3, 6]);
    view.highlight_id(&3);
    view.toggle_highlighted_expansion(Rect::new(0, 0, 40, 5), AnimationSettings::default());

    assert_eq!(visible_ids(&view), vec![1, 3]);
}

#[test]
fn collapsing_tree_requests_layout_when_visible_row_count_changes() {
    let mut view = tree_view().expanded([1]);
    view.highlight_id(&1);
    view.set_focused(true);
    let expanded_height =
        <DataView<_, _> as TuiNode<()>>::measure(&view, LayoutProposal::unbounded())
            .preferred
            .height;
    let mut ctx = EventCtx::default();

    let outcome = <DataView<_, _> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Key(Key::Char(' ').into()),
        &mut ctx,
    );

    let collapsed_height =
        <DataView<_, _> as TuiNode<()>>::measure(&view, LayoutProposal::unbounded())
            .preferred
            .height;
    assert_eq!(outcome, EventOutcome::Handled);
    assert!(collapsed_height < expanded_height);
    assert!(ctx.layout_requested());
}

#[test]
fn revealing_programmatic_highlight_scrolls_it_into_view() {
    let area = Rect::new(0, 0, 20, 4);
    let mut view = clear_transform_view();
    <DataView<_, _> as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());

    view.highlight_id(&12);
    view.reveal_highlighted();

    assert_restored_highlight_is_visible(&view, area);
}

#[test]
fn search_matches_are_underlined_when_rendered() {
    let mut view = transform_view();
    view.set_search_query("api");

    let mut terminal = Terminal::new(TestBackend::new(40, 3)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 40, 3)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    for x in 0..3 {
        assert!(
            buffer
                .cell((x, 0))
                .unwrap()
                .style()
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }
}

#[test]
fn fuzzy_search_matches_are_underlined_when_rendered() {
    let mut view = transform_view();
    view.set_search_query("aa");

    let mut terminal = Terminal::new(TestBackend::new(40, 1)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 40, 1)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .cell((0, 0))
            .unwrap()
            .style()
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        buffer
            .cell((4, 0))
            .unwrap()
            .style()
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        !buffer
            .cell((1, 0))
            .unwrap()
            .style()
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
}

#[test]
fn unicode_search_highlight_uses_original_char_boundaries() {
    let mut view = DataView::new([Row::new(1, "İstanbul")], |row| row.id).column(Column::text(
        "name",
        "Name",
        Constraint::Percentage(100),
        |row: &Row| row.name.to_string(),
    ));
    view.set_transform_mode(DataViewTransformMode::External);
    view.set_search_query("i");

    let mut terminal = Terminal::new(TestBackend::new(20, 2)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 20, 2)))
        .expect("unicode search should render without panic");

    let mut rendered_row = false;
    for y in 0..2 {
        for x in 0..20 {
            rendered_row |= terminal.backend().buffer().cell((x, y)).unwrap().symbol() == "İ";
        }
    }
    assert!(rendered_row);
}

#[test]
fn routed_search_paste_updates_transform_query() {
    let mut view = transform_view().action_bar(true);
    view.set_focused(true);
    let mut layout = LayoutCtx::new();
    <DataView<TransformRow, usize> as TuiNode<()>>::layout(
        &mut view,
        Rect::new(0, 0, 60, 6),
        &mut layout,
    );
    view.event(
        &TuiEvent::Key(KeyEvent::from(Key::Char('/'))),
        &mut EventCtx::<()>::default(),
    );
    let route = EventRoute::new(TreePath::from_keys([ChildKey::new(SEARCH_SLOT)]));
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.dispatch_event(&route, &TuiEvent::Paste(String::from("api")), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(view.transform_state().search, "api");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn local_filters_are_exact_match_and_combined_with_and() {
    let mut view = transform_view();

    view.set_filter("owner", "Ada");
    view.set_filter("status", "Ready");

    assert_eq!(visible_ids(&view), vec![1]);
}

#[test]
fn filter_dropdown_transform_preserves_navigation_activation() {
    let mut view = transform_view().activation_mode(ActivationMode::OnNavigate);
    view.highlight_id(&2);
    let transform = view.set_filter("owner", "Ada");

    let outcome = view.transform_dropdown_outcome(
        transform,
        Rect::new(0, 0, 40, 3),
        AnimationSettings::default(),
    );

    assert!(outcome.activated);
    assert_eq!(
        view.take_last_activated().map(|event| event.row_id),
        Some(1)
    );
}

#[test]
fn empty_transform_result_renders_no_results_message() {
    let mut view = transform_view();
    view.set_search_query("not present");

    let mut terminal = Terminal::new(TestBackend::new(40, 3)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 40, 3)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    let message = (0..17)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();

    assert_eq!(message, "No results found.");
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, crate::theme().muted_fg());
}

#[test]
fn empty_message_can_be_overridden() {
    let view = DataView::list(Vec::<usize>::new(), |row| *row, |row| row.to_string())
        .empty_message("Nothing to show.");
    let mut terminal = Terminal::new(TestBackend::new(40, 3)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 40, 3)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    let message = (0..16)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();

    assert_eq!(message, "Nothing to show.");
}

#[test]
fn seasonal_empty_state_is_centered_and_clipped_in_small_areas() {
    let view = DataView::list(Vec::<usize>::new(), |row| *row, |row| row.to_string()).empty_state(
        crate::SeasonalEmptyState::new("Nothing here")
            .date(time::Date::from_calendar_date(2026, time::Month::December, 1).unwrap()),
    );
    let mut terminal = Terminal::new(TestBackend::new(20, 7)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("custom empty content should render");

    let buffer = terminal.backend().buffer();
    let line = |y| {
        (0..20)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect::<String>()
    };
    assert_eq!(line(2), "    Nothing here    ");
    assert_eq!(line(3), "                    ");
    assert_eq!(line(4).trim(), "╶┄ ✧ ·  · ✧ ┄╴");
    assert_eq!(buffer.cell((4, 2)).unwrap().fg, crate::theme().subtle_fg());

    let mut tiny = Terminal::new(TestBackend::new(4, 1)).expect("terminal should build");
    tiny.draw(|frame| view.render(frame, frame.area()))
        .expect("custom empty content should clip without panicking");
}

#[test]
fn visible_row_ids_remain_base_subset_when_local_filter_changes() {
    let mut view = transform_view().visible_row_ids([1, 2, 3]);

    view.set_filter("owner", "Ada");
    assert_eq!(visible_ids(&view), vec![1, 3]);

    view.clear_filter("owner");
    assert_eq!(visible_ids(&view), vec![1, 2, 3]);
}

#[test]
fn external_transform_mode_updates_state_without_local_filtering() {
    let mut view = transform_view();
    view.set_transform_mode(DataViewTransformMode::External);

    view.set_search_query("api");
    view.set_filter("owner", "Ada");

    assert_eq!(visible_ids(&view), vec![1, 2, 3, 4]);
    assert_eq!(view.transform_state().search, "api");
    assert_eq!(view.transform_state().filters.len(), 1);
}

#[test]
fn filter_header_label_includes_active_filter_icon() {
    let mut view = transform_view().headers(true);
    view.set_filter("owner", "Ada");

    let mut terminal = Terminal::new(TestBackend::new(40, 3)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 40, 3)))
        .expect("data view should render");

    let header = (0..40)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(header.contains(""));
}

#[test]
fn default_transform_keys_open_search_and_filters() {
    let bindings = KeyBindings::default();

    assert!(
        bindings
            .data_view()
            .search_matches(KeyEvent::from(Key::Char('/')))
    );
    assert!(
        bindings
            .data_view()
            .filter_matches(KeyEvent::from(Key::Char('f')))
    );
}

#[test]
fn search_hotkey_is_ignored_without_action_bar() {
    let mut view = transform_view();

    let outcome = view.on_key(KeyEvent::from(Key::Char('/')), Rect::new(0, 0, 40, 6));

    assert_eq!(outcome, DataViewOutcome::IDLE);
    assert_eq!(view.interaction, DataViewInteraction::Grid);
}

#[test]
fn clear_search_hotkey_clears_and_enters_insert_mode() {
    let area = Rect::new(0, 0, 40, 6);
    let mut view = clear_transform_view()
        .action_bar(true)
        .selection_mode(SelectionMode::Single)
        .selection_trigger(SelectionTrigger::OnNavigate)
        .selected([7]);
    view.highlight_id(&7);
    view.set_search_query("7");

    let outcome = view.on_key(
        KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::CONTROL,
        },
        area,
    );

    assert!(outcome.handled);
    assert!(outcome.changed);
    assert!(view.transform_state().search.is_empty());
    assert_eq!(view.interaction, DataViewInteraction::Search);
    assert!(view.search_input.insert_mode());
    assert_eq!(view.selected_ids(), vec![7]);
    assert_restored_highlight_is_centered(&view, area);
}

#[test]
fn unfocus_keys_clear_search_from_grid() {
    let area = Rect::new(0, 0, 40, 6);
    let keys = [
        KeyEvent::from(Key::Esc),
        KeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::CONTROL,
        },
    ];

    for key in keys {
        let mut view = clear_transform_view();
        view.highlight_id(&12);
        view.set_search_query("12");

        let outcome = view.on_key(key, area);

        assert!(outcome.handled);
        assert!(outcome.changed);
        assert!(view.transform_state().search.is_empty());
        assert_restored_highlight_is_visible(&view, area);
    }
}

#[test]
fn unfocus_keys_clear_search_when_leaving_search_input() {
    let area = Rect::new(0, 0, 40, 6);
    let keys = [
        KeyEvent::from(Key::Esc),
        KeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::CONTROL,
        },
    ];

    for key in keys {
        let mut view = clear_transform_view().action_bar(true);
        view.highlight_id(&12);
        view.on_key(KeyEvent::from(Key::Char('/')), area);
        view.set_search_query("12");

        let outcome = view.on_key(key, area);

        assert!(outcome.handled);
        assert!(outcome.changed);
        assert!(view.transform_state().search.is_empty());
        assert_eq!(view.interaction, DataViewInteraction::Grid);
        assert_restored_highlight_is_visible(&view, area);
    }
}

#[test]
fn clear_all_filters_preserves_highlight_and_scrolls_it_into_view() {
    let area = Rect::new(0, 0, 40, 6);
    let mut view = clear_transform_view().headers(true);
    view.highlight_id(&12);
    view.set_filter("value", "12");

    let outcome = view.on_key(
        KeyEvent {
            code: Key::Char('f'),
            modifiers: KeyModifiers::CONTROL,
        },
        area,
    );

    assert!(outcome.handled);
    assert!(outcome.changed);
    assert!(view.transform_state().filters.is_empty());
    assert_restored_highlight_is_visible(&view, area);
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
fn tree_selection_placeholder_sizes_and_scrolls_its_first_cell() {
    let mut view = DataView::new(
        [Row::new(1, "A"), Row::new(2, "B"), Row::new(3, "C")],
        |row| row.id,
    )
    .column(Column::text(
        "name",
        "Name",
        Constraint::Fill(1),
        |row: &Row| row.name.to_string(),
    ));
    view.set_selection_overlay(
        vec![1, 2],
        Some(SelectionOverlayPosition::After(2)),
        0,
        false,
    );
    let area = Rect::new(0, 0, 10, 10);
    let geometry = view.scroll_geometry(area);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    assert!(geometry.content.width >= "2 items selected".len());
    view.scroll.scroll_to(
        ScrollOffset::new(2, 0),
        geometry.viewport,
        geometry.content,
        settings,
    );
    let mut terminal = Terminal::new(TestBackend::new(10, 10)).expect("terminal should build");
    terminal
        .draw(|frame| view.render(frame, area))
        .expect("data view should render");

    let visible = (0..10)
        .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert_eq!(visible, "items sele");
}

#[test]
fn center_selection_placeholder_centers_the_moving_row() {
    let mut view = DataView::list(0..20, |id| *id, |id| id.to_string()).headers(false);
    view.set_selection_overlay(vec![15], Some(SelectionOverlayPosition::After(15)), 0, true);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    view.center_selection_placeholder(Rect::new(0, 0, 20, 5), settings);

    assert_eq!(view.vertical_scroll_offset_for_test(), 14);
}

#[test]
fn moving_placeholder_spans_visible_columns() {
    let mut view = DataView::new([Row::new(1, "A"), Row::new(2, "B")], |row| row.id)
        .headers(false)
        .column(Column::text("state", "", Constraint::Length(1), |_| {
            "•".to_string()
        }))
        .column(Column::text(
            "title",
            "",
            Constraint::Fill(1),
            |row: &Row| row.name.to_string(),
        ));
    view.set_selection_overlay(
        vec![1, 2],
        Some(SelectionOverlayPosition::After(2)),
        0,
        true,
    );
    let area = Rect::new(0, 0, 30, 10);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal.draw(|frame| view.render(frame, area)).unwrap();

    let placeholder = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(placeholder.contains("Moving 2 tasks"), "{placeholder:?}");
}

#[test]
fn focused_tree_selection_placeholder_uses_target_depth_and_reorder_style() {
    let mut view = DataView::new(
        [
            Row::new(1, "parent"),
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
        Constraint::Fill(1),
        |row: &Row| row.name.to_string(),
    ))
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .expanded([1]);
    view.set_focused(true);
    view.set_selection_overlay(vec![2], Some(SelectionOverlayPosition::After(2)), 1, true);
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 30, 4)))
        .expect("tree placeholder should render");

    let prefix_width = preset()
        .data_view()
        .tree_indent_width()
        .saturating_add(line_width(&Line::from(format!(
            "{} ",
            view.tree_glyphs.leaf
        ))));
    let cell = terminal
        .backend()
        .buffer()
        .cell((prefix_width as u16, 2))
        .expect("placeholder label should be indented");
    let theme = theme();
    assert_eq!(cell.symbol(), "M");
    assert_eq!(cell.fg, theme.highlight_bg());
    assert_eq!(cell.bg, theme.highlight_fg());
}

#[test]
fn tree_selection_placeholder_without_columns_has_no_rendered_widths() {
    let mut view = DataView::new([Row::new(1, "A")], |row| row.id);
    view.set_selection_overlay(vec![1], Some(SelectionOverlayPosition::After(1)), 0, false);

    assert_eq!(view.rendered_column_widths(), Vec::<usize>::new());
    assert_eq!(
        view.scroll_geometry(Rect::new(0, 0, 10, 10)).content.width,
        0
    );
}

#[test]
fn block_move_uses_dangling_parent_rows_as_root_insertion_anchors() {
    let mut view = DataView::new(
        [
            Row::new(3, "three"),
            Row::new(4, "four"),
            Row {
                id: 1,
                parent: Some(99),
                name: "dangling",
            },
            Row::new(2, "two"),
        ],
        |row| row.id,
    )
    .tree(TreeAdapter::mutable_parent_id(
        |row: &Row| row.parent,
        |row, parent| row.parent = parent,
    ));

    assert_eq!(
        view.move_tree_sibling_block(&[3, 4], None, None, 1)
            .map(|result| (result.parent_id, result.sibling_index)),
        Some((None, 1))
    );
    assert_eq!(
        view.rows().iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3, 4, 2]
    );
}

#[test]
fn block_move_reparents_only_selected_roots_and_rejects_selected_subtree_targets() {
    let rows = [
        Row::new(1, "one"),
        Row {
            id: 2,
            parent: Some(1),
            name: "one child",
        },
        Row::new(3, "three"),
        Row {
            id: 4,
            parent: Some(3),
            name: "three child",
        },
        Row::new(5, "target"),
        Row {
            id: 6,
            parent: Some(5),
            name: "target child",
        },
        Row::new(7, "seven"),
    ];
    let mut view = DataView::new(rows.clone(), |row: &Row| row.id).tree(
        TreeAdapter::mutable_parent_id(|row: &Row| row.parent, |row, parent| row.parent = parent),
    );

    assert_eq!(
        view.move_tree_sibling_block(&[1, 3], None, Some(2), 0),
        None
    );
    assert_eq!(
        view.rows()
            .iter()
            .map(|row| (row.id, row.parent))
            .collect::<Vec<_>>(),
        rows.iter()
            .map(|row| (row.id, row.parent))
            .collect::<Vec<_>>()
    );

    assert_eq!(
        view.move_tree_sibling_block(&[1, 3], None, Some(5), 1)
            .map(|result| (result.parent_id, result.sibling_index)),
        Some((Some(5), 1))
    );
    assert_eq!(
        view.rows()
            .iter()
            .map(|row| (row.id, row.parent))
            .collect::<Vec<_>>(),
        vec![
            (5, None),
            (6, Some(5)),
            (1, Some(5)),
            (2, Some(1)),
            (3, Some(5)),
            (4, Some(3)),
            (7, None),
        ]
    );
}

#[test]
fn cells_have_right_padding_except_for_the_last_column() {
    let view = DataView::new([Row::new(1, "A")], |row| row.id)
        .columns([
            Column::text("first", "X", Constraint::Length(1), |row: &Row| {
                row.name.to_string()
            }),
            Column::text("second", "Y", Constraint::Length(1), |_| String::from("B")),
        ])
        .headers(true);
    let mut terminal = Terminal::new(TestBackend::new(3, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 3, 2)))
        .expect("data view should render");

    let buffer = terminal.backend().buffer();
    let line = |y| {
        (0..3)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect::<String>()
    };
    assert_eq!(line(0), "X Y");
    assert_eq!(line(1), "A B");
}

#[test]
fn shifted_horizontal_keys_scroll_by_seventy_percent_of_assigned_width() {
    let new_view = |content_width| {
        DataView::new([Row::new(1, "A"), Row::new(2, "B")], |row| row.id).column(Column::text(
            "name",
            "Name",
            Constraint::Length(content_width),
            |row: &Row| row.name.to_string(),
        ))
    };
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let jump_right = KeyEvent {
        code: Key::Char('l'),
        modifiers: KeyModifiers::SHIFT,
    };
    let jump_left = KeyEvent {
        code: Key::Char('h'),
        modifiers: KeyModifiers::SHIFT,
    };
    let mut wide = new_view(100);

    let right = wide.on_key_with_settings(jump_right, Rect::new(0, 0, 50, 2), settings);
    assert!(right.handled);
    assert_eq!(wide.scroll.offset().x, 35);

    let mut wider_content = new_view(140);
    let _ = wider_content.on_key_with_settings(jump_right, Rect::new(0, 0, 50, 2), settings);
    assert_eq!(wider_content.scroll.offset().x, 35);

    let left = wide.on_key_with_settings(jump_left, Rect::new(0, 0, 50, 2), settings);
    assert!(left.handled);
    assert_eq!(wide.scroll.offset().x, 0);

    let mut reserved_scrollbar_gutter = new_view(100);
    let _ = reserved_scrollbar_gutter.on_key_with_settings(
        jump_right,
        Rect::new(0, 0, 10, 2),
        settings,
    );
    assert_eq!(reserved_scrollbar_gutter.scroll.offset().x, 7);

    let mut minimum = new_view(100);
    let _ = minimum.on_key_with_settings(jump_right, Rect::new(0, 0, 1, 2), settings);
    assert_eq!(minimum.scroll.offset().x, 1);

    let mut zero = new_view(100);
    let _ = zero.on_key_with_settings(jump_right, Rect::new(0, 0, 0, 2), settings);
    assert_eq!(zero.scroll.offset().x, 0);

    let old_right = wide.on_key_with_settings(
        KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::CONTROL,
        },
        Rect::new(0, 0, 50, 2),
        settings,
    );
    assert!(!old_right.handled);
    assert_eq!(wide.scroll.offset().x, 0);
}

#[test]
fn width_change_resets_horizontal_scroll_to_start() {
    let mut view = DataView::new([Row::new(1, "ABCDEFGHIJKLMNOPQRST")], |row| row.id).column(
        Column::text("name", "Name", Constraint::Length(20), |row: &Row| {
            row.name.to_string()
        }),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let narrow = Rect::new(0, 0, 10, 2);
    let wide = Rect::new(0, 0, 18, 2);
    let mut layout = LayoutCtx::new();

    <DataView<Row, usize> as TuiNode<()>>::layout(&mut view, narrow, &mut layout);
    let outcome = view.on_key_with_settings(
        KeyEvent {
            code: Key::Char('L'),
            modifiers: KeyModifiers::SHIFT,
        },
        narrow,
        settings,
    );
    assert!(outcome.handled);
    assert_eq!(view.scroll.offset().x, 7);

    <DataView<Row, usize> as TuiNode<()>>::layout(&mut view, wide, &mut layout);

    assert_eq!(view.scroll.offset().x, 0);
    assert_eq!(view.scroll.target_offset().x, 0);
}

#[test]
fn handled_key_stops_propagation() {
    let mut view = DataView::new([Row::new(1, "A"), Row::new(2, "B")], |row| row.id).column(
        Column::text("name", "Name", Constraint::Percentage(100), |row: &Row| {
            row.name.to_string()
        }),
    );
    view.set_focused(true);
    let mut layout = LayoutCtx::new();
    <DataView<Row, usize> as TuiNode<()>>::layout(&mut view, Rect::new(0, 0, 10, 2), &mut layout);
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.event(&TuiEvent::Key(KeyEvent::from(Key::Down)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn default_plain_j_and_k_move_highlight_down_and_up() {
    let mut view = DataView::list(
        [Row::new(1, "A"), Row::new(2, "B"), Row::new(3, "C")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let area = Rect::new(0, 0, 10, 3);

    assert!(view.on_key(Key::Char('j'), area).changed);
    assert_eq!(view.highlighted, 1);
    assert!(view.on_key(Key::Char('k'), area).changed);
    assert_eq!(view.highlighted, 0);
}

#[test]
fn multi_select_j_and_k_route_through_tui_node_events() {
    let area = Rect::new(0, 0, 10, 3);
    let mut view = DataView::list([1, 2, 3], |row| *row, |row| row.to_string())
        .selection_mode(SelectionMode::Multi)
        .selection_trigger(SelectionTrigger::OnNavigate);
    view.set_focused(true);
    <DataView<usize, usize> as TuiNode<()>>::layout(&mut view, area, &mut LayoutCtx::new());

    let down = <DataView<usize, usize> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Key(KeyEvent::from(Key::Char('j'))),
        &mut EventCtx::default(),
    );
    assert_eq!(down, EventOutcome::Handled);
    assert_eq!(view.highlighted_id(), Some(2));
    assert_eq!(view.selected_ids(), vec![2]);

    let up = <DataView<usize, usize> as TuiNode<()>>::event(
        &mut view,
        &TuiEvent::Key(KeyEvent::from(Key::Char('k'))),
        &mut EventCtx::default(),
    );
    assert_eq!(up, EventOutcome::Handled);
    assert_eq!(view.highlighted_id(), Some(1));
    assert_eq!(view.selected_ids(), vec![1, 2]);
}

#[test]
fn action_bar_search_registers_text_entry_focus_target() {
    let mut view = transform_view().action_bar(true);
    let mut layout = LayoutCtx::new();

    <DataView<TransformRow, usize> as TuiNode<()>>::layout(
        &mut view,
        Rect::new(0, 0, 60, 6),
        &mut layout,
    );
    let mut ctx = EventCtx::<()>::default();
    view.event(&TuiEvent::Key(KeyEvent::from(Key::Char('/'))), &mut ctx);
    let mut layout = LayoutCtx::new();
    <DataView<TransformRow, usize> as TuiNode<()>>::layout(
        &mut view,
        Rect::new(0, 0, 60, 6),
        &mut layout,
    );

    let target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path.keys() == [ChildKey::new(SEARCH_SLOT)])
        .expect("search child should register focus target");
    assert_eq!(target.id.as_str(), TEXT_INPUT_FOCUS);
    assert!(!target.tab_stop);
    assert!(target.hotkey_sequences.is_empty());
}

#[test]
fn opening_search_focuses_child_and_characters_stop_propagation() {
    let mut view = transform_view().action_bar(true);
    view.set_focused(true);
    let mut layout = LayoutCtx::new();
    <DataView<TransformRow, usize> as TuiNode<()>>::layout(
        &mut view,
        Rect::new(0, 0, 60, 6),
        &mut layout,
    );
    let mut ctx = EventCtx::<()>::default();

    let slash = view.event(&TuiEvent::Key(KeyEvent::from(Key::Char('/'))), &mut ctx);

    assert_eq!(slash, EventOutcome::Handled);
    assert_eq!(
        ctx.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: TreePath::from_keys([ChildKey::new(SEARCH_SLOT)]),
            id: FocusId::new(TEXT_INPUT_FOCUS),
        })
    );

    let route = EventRoute::new(TreePath::from_keys([ChildKey::new(SEARCH_SLOT)]));
    let mut ctx = EventCtx::<()>::default();
    let outcome = view.dispatch_event(
        &route,
        &TuiEvent::Key(KeyEvent::from(Key::Char('c'))),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert_eq!(view.transform_state().search, "c");
}

#[test]
fn clearing_search_focuses_the_search_child() {
    let mut view = transform_view().action_bar(true);
    view.set_focused(true);
    view.set_search_query("api");
    let mut layout = LayoutCtx::new();
    <DataView<TransformRow, usize> as TuiNode<()>>::layout(
        &mut view,
        Rect::new(0, 0, 60, 6),
        &mut layout,
    );
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('/'),
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: TreePath::from_keys([ChildKey::new(SEARCH_SLOT)]),
            id: FocusId::new(TEXT_INPUT_FOCUS),
        })
    );
}

#[test]
fn filter_picker_uses_dropdown_state() {
    let mut view = transform_view().headers(true).action_bar(true);

    assert!(
        view.on_key(KeyEvent::from(Key::Char('f')), Rect::new(0, 0, 60, 6))
            .changed
    );
    assert!(
        view.on_key(KeyEvent::from(Key::Char('2')), Rect::new(0, 0, 60, 6))
            .changed
    );
    assert!(matches!(
        view.interaction,
        DataViewInteraction::FilterValues { .. }
    ));
    assert!(
        view.filter_dropdown
            .as_ref()
            .is_some_and(|dropdown| dropdown.is_open())
    );
}

#[test]
fn disabled_filter_controls_ignore_hotkey_and_hide_action_bar_hint() {
    let mut view = transform_view()
        .headers(true)
        .action_bar(true)
        .filter_controls(false);
    let outcome = view.on_key(KeyEvent::from(Key::Char('f')), Rect::new(0, 0, 60, 6));
    assert_eq!(outcome, DataViewOutcome::IDLE);
    assert_eq!(view.interaction, DataViewInteraction::Grid);

    let mut terminal = Terminal::new(TestBackend::new(60, 6)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");

    let action_bar = (0..60)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(action_bar.contains("Search..."));
    assert!(!action_bar.contains("filters"));
}

#[test]
fn focus_target_registers_single_multiletter_and_cleared_hotkeys() {
    for (configured, clear, expected_key, expected_sequences) in [
        ("c", false, Some(KeyEvent::from(Key::Char('c'))), vec!["c"]),
        ("G G", false, None, vec!["gg"]),
        ("c", true, None, vec![]),
    ] {
        let mut view = DataView::list([Row::new(1, "A")], |row| row.id, |row| row.name.to_string())
            .hotkey(configured);
        if clear {
            view.clear_hotkey();
        }
        let mut layout = LayoutCtx::new();

        <DataView<Row, usize> as TuiNode<()>>::layout(
            &mut view,
            Rect::new(0, 0, 10, 2),
            &mut layout,
        );

        assert_eq!(layout.focus_targets()[0].hotkey, expected_key);
        assert_eq!(
            layout.focus_targets()[0].hotkey_sequences,
            expected_sequences
        );
    }
}

#[test]
fn shifted_horizontal_navigation_scrolls_tree_without_expanding() {
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
            modifiers: KeyModifiers::SHIFT,
        },
        Rect::new(0, 0, 8, 3),
        settings,
    );

    assert!(outcome.handled);
    assert!(!view.expanded.contains(&1));
    assert_eq!(view.scroll.offset().x, 5);
}

#[test]
fn shifted_horizontal_scrolling_uses_configured_navigation_keys() {
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
            modifiers: KeyModifiers::SHIFT,
        },
        area,
        settings,
        &bindings,
    );
    assert!(right.handled);
    assert_eq!(view.scroll.offset().x, 7);

    let left = view.on_key_with_settings_and_bindings(
        KeyEvent {
            code: Key::Char('A'),
            modifiers: KeyModifiers::SHIFT,
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
fn constrained_columns_fit_viewport_and_preserve_content_widths() {
    let view = DataView::new([Row::new(1, "A very long task title")], |row| row.id).columns([
        Column::text("state", "", Constraint::Length(1), |_| String::from("S")).constrained(),
        Column::text("priority", "", Constraint::Length(1), |_| String::from("P")).constrained(),
        Column::text("title", "Task", Constraint::Fill(1), |row: &Row| {
            row.name.to_string()
        })
        .constrained(),
        Column::text("size", "Size", Constraint::Length(4), |_| {
            String::from("MEDIUM")
        })
        .constrained(),
    ]);

    let widths = view.column_widths(20);

    assert_eq!(widths, vec![2, 2, 12, 4]);
    assert_eq!(widths.into_iter().sum::<usize>(), 20);
    assert_eq!(
        view.scroll_geometry(Rect::new(0, 0, 20, 2)).content.width,
        20
    );
}

#[test]
fn constrained_column_clips_oversized_content_without_displacing_next_column() {
    let view = DataView::new([Row::new(1, "ABCDEFG")], |row| row.id).columns([
        Column::text("first", "", Constraint::Length(4), |row: &Row| {
            row.name.to_string()
        })
        .constrained(),
        Column::text("second", "", Constraint::Length(3), |_| String::from("XYZ")).constrained(),
    ]);
    let mut terminal = Terminal::new(TestBackend::new(8, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 8, 1)))
        .expect("data view should render");

    let rendered = (0..8)
        .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert_eq!(rendered, "ABCD XYZ");
}

#[test]
fn constrained_fixed_columns_scroll_only_when_minimums_exceed_viewport() {
    let view = DataView::new([Row::new(1, "A")], |row| row.id).columns([
        Column::text("first", "First", Constraint::Length(8), |row: &Row| {
            row.name.to_string()
        })
        .constrained(),
        Column::text("second", "Second", Constraint::Length(8), |_| {
            String::from("B")
        })
        .constrained(),
    ]);

    assert_eq!(view.column_widths(10), vec![9, 8]);
    assert_eq!(
        view.scroll_geometry(Rect::new(0, 0, 10, 2)).content.width,
        17
    );
}

#[test]
fn mixed_columns_expand_only_intrinsic_content() {
    let view = DataView::new([Row::new(1, "ABCDEFGHI")], |row| row.id).columns([
        Column::text("fixed", "Fixed", Constraint::Length(4), |row: &Row| {
            row.name.to_string()
        })
        .constrained(),
        Column::text("intrinsic", "Value", Constraint::Length(1), |row: &Row| {
            row.name.to_string()
        }),
    ]);

    assert_eq!(view.column_widths(10), vec![5, 9]);
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
    assert!(cell.modifier.contains(Modifier::BOLD));
}

#[test]
fn focused_highlight_normalizes_embedded_chip_colors() {
    let chip = crate::Chip::new("chip")
        .color_role(crate::ChipColorRole::Highlight)
        .line();
    let chip_width = line_width(&chip);
    let view = DataView::new([1], |row| *row)
        .column(Column::rich(
            "chip",
            "",
            Constraint::Percentage(100),
            move |_, _| chip.clone(),
        ))
        .focused(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");

    let theme = theme();
    assert_embedded_chip_styles(
        terminal.backend().buffer(),
        chip_width,
        theme.highlight_fg(),
        theme.highlight_bg(),
    );
}

#[test]
fn unfocused_reorder_highlight_crossfades_only_moving_row_to_full_inverse_and_clears() {
    let mut view = DataView::list(
        [Row::new(1, "moving"), Row::new(2, "other")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let settings = AnimationSettings::default();
    view.start_reorder_highlight(1, settings);
    Animated::tick(&mut view, Duration::from_millis(100), settings);
    Animated::tick(&mut view, Duration::from_millis(25), settings);
    assert_eq!(view.reorder_highlight_progress_for_test(), 0.5);
    let mut terminal = Terminal::new(TestBackend::new(10, 2)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 2)))
        .expect("data view should render");

    let theme = crate::theme();
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(
        cell.fg,
        lerp_color(theme.highlight_fg(), theme.highlight_bg(), 0.5)
    );
    assert_eq!(
        cell.bg,
        lerp_color(theme.highlight_bg(), theme.highlight_fg(), 0.5)
    );
    assert!(cell.modifier.contains(Modifier::BOLD));
    assert!(!cell.modifier.contains(Modifier::REVERSED));
    assert!(!cell.modifier.contains(Modifier::UNDERLINED));
    let other = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(other.fg, Color::Reset);
    assert_eq!(other.bg, Color::Reset);

    Animated::tick(&mut view, Duration::from_millis(125), settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 2)))
        .expect("data view should render");
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, theme.highlight_bg());
    assert_eq!(cell.bg, theme.highlight_fg());

    view.clear_reorder_highlight(settings);
    Animated::tick(&mut view, Duration::from_millis(100), settings);
    Animated::tick(&mut view, Duration::from_millis(25), settings);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 2)))
        .expect("data view should render");
    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().fg,
        lerp_color(theme.highlight_fg(), theme.highlight_bg(), 0.5)
    );
    assert_eq!(
        terminal.backend().buffer().cell((0, 0)).unwrap().bg,
        lerp_color(theme.highlight_bg(), theme.highlight_fg(), 0.5)
    );

    Animated::tick(&mut view, Duration::from_millis(100), settings);
    Animated::tick(&mut view, Duration::from_millis(25), settings);
    assert_eq!(view.reorder_highlight_progress_for_test(), 0.0);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 2)))
        .expect("data view should render");
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, Color::Reset);
    assert_eq!(cell.bg, Color::Reset);
}

#[test]
fn disabled_animation_snaps_reorder_render_to_inverse_and_normal() {
    let mut view = DataView::list(
        [Row::new(1, "moving")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");
    let theme = crate::theme();

    view.start_reorder_highlight(1, settings);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, theme.highlight_bg());
    assert_eq!(cell.bg, theme.highlight_fg());

    view.clear_reorder_highlight(settings);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");
    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, Color::Reset);
    assert_eq!(cell.bg, Color::Reset);
}

#[test]
fn highlighted_row_forces_readable_foreground_and_preserves_rich_modifiers() {
    let semantic_color = crate::theme().error_fg();
    let view = DataView::new([Row::new(1, "BIG")], |row| row.id)
        .column(Column::rich(
            "size",
            "Size",
            Constraint::Length(5),
            move |row: &Row, _| {
                Line::from(Span::styled(
                    row.name,
                    Style::default()
                        .fg(semantic_color)
                        .add_modifier(Modifier::UNDERLINED),
                ))
            },
        ))
        .focused(true);
    let mut terminal = Terminal::new(TestBackend::new(5, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 5, 1)))
        .expect("data view should render");

    let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    assert_eq!(cell.fg, crate::theme().highlight_fg());
    assert_eq!(cell.bg, crate::theme().highlight_bg());
    assert!(cell.modifier.contains(Modifier::BOLD));
    assert!(cell.modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn moving_rich_row_overrides_span_color_and_preserves_modifiers_at_full_inverse() {
    let semantic_color = crate::theme().error_fg();
    let settings = AnimationSettings::default();
    let theme = crate::theme();

    for focused in [false, true] {
        let mut view = DataView::new([Row::new(1, "moving")], |row| row.id)
            .column(Column::rich(
                "name",
                "Name",
                Constraint::Length(10),
                move |row: &Row, _| {
                    Line::from(Span::styled(
                        row.name,
                        Style::default()
                            .fg(semantic_color)
                            .add_modifier(Modifier::UNDERLINED),
                    ))
                },
            ))
            .focused(focused);
        let mut terminal = Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");

        view.start_reorder_highlight(1, settings);
        Animated::tick(&mut view, Duration::from_secs(1), settings);
        Animated::tick(&mut view, Duration::from_secs(1), settings);
        Animated::tick(&mut view, Duration::from_secs(1), settings);
        terminal
            .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
            .expect("data view should render");

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "m");
        assert_eq!(cell.fg, theme.highlight_bg());
        assert_eq!(cell.bg, theme.highlight_fg());
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
    }
}

#[test]
fn ansi_and_indexed_reorder_colors_snap_inverse_until_animated_exit_finishes() {
    let settings = AnimationSettings::default();

    for (foreground, background) in [
        (Color::White, Color::Blue),
        (Color::Indexed(231), Color::Indexed(24)),
    ] {
        let mut view = DataView::list(
            [Row::new(1, "moving")],
            |row| row.id,
            |row| row.name.to_string(),
        );
        view.start_reorder_highlight_with_colors(1, settings, foreground, background);
        assert_eq!(view.reorder_highlight_progress_for_test(), 1.0);
        let style = view.reorder_highlighted_row_style_with_colors(foreground, background);
        assert_eq!(style.fg, Some(background));
        assert_eq!(style.bg, Some(foreground));

        Animated::tick(&mut view, Duration::from_secs(1), settings);
        assert_eq!(view.reorder_highlight_progress_for_test(), 1.0);
        view.clear_reorder_highlight(settings);
        Animated::tick(&mut view, Duration::from_millis(100), settings);
        Animated::tick(&mut view, Duration::from_millis(100), settings);
        assert_eq!(view.reorder_highlight_progress_for_test(), 1.0);
        let style = view.reorder_highlighted_row_style_with_colors(foreground, background);
        assert_eq!(style.fg, Some(background));
        assert_eq!(style.bg, Some(foreground));

        Animated::tick(&mut view, Duration::from_millis(50), settings);
        assert_eq!(view.reorder_highlight_progress_for_test(), 0.0);
        assert!(!view.row_has_reorder_highlight(&1));
    }

    let disabled = AnimationSettings {
        enabled: false,
        ..settings
    };
    let mut view = DataView::list(
        [Row::new(1, "moving")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    view.start_reorder_highlight_with_colors(1, disabled, Color::Indexed(231), Color::Indexed(24));
    assert_eq!(view.reorder_highlight_progress_for_test(), 1.0);
    view.clear_reorder_highlight(disabled);
    assert_eq!(view.reorder_highlight_progress_for_test(), 0.0);
    assert!(!view.row_has_reorder_highlight(&1));
}

#[test]
fn focused_reorder_exit_clears_at_normal_highlight_without_extra_frame() {
    let mut view = DataView::list(
        [Row::new(1, "moving")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .focused(true);
    let settings = AnimationSettings::default();
    let mut terminal = Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");
    let theme = crate::theme();

    view.start_reorder_highlight(1, settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    view.clear_reorder_highlight(settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);
    Animated::tick(&mut view, Duration::from_secs(1), settings);

    assert!(!view.row_has_reorder_highlight(&1));
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");
    let endpoint = terminal.backend().buffer().cell((0, 0)).unwrap().clone();
    assert_eq!(endpoint.fg, theme.highlight_fg());
    assert_eq!(endpoint.bg, theme.highlight_bg());

    Animated::tick(&mut view, Duration::from_millis(1), settings);
    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("data view should render");
    assert_eq!(terminal.backend().buffer().cell((0, 0)).unwrap(), &endpoint);
}

#[test]
fn immediate_focus_loss_endpoint_matches_ordinary_unfocused_row() {
    let mut moving = DataView::list(
        [Row::new(1, "moving")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .focused(true);
    let ordinary = DataView::list(
        [Row::new(1, "moving")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let settings = AnimationSettings::default();
    moving.start_reorder_highlight(1, settings);
    Animated::tick(&mut moving, Duration::from_secs(1), settings);
    moving.set_focused(false);

    let mut moving_terminal =
        Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");
    let mut ordinary_terminal =
        Terminal::new(TestBackend::new(10, 1)).expect("terminal should build");
    moving_terminal
        .draw(|frame| moving.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("moving data view should render");
    ordinary_terminal
        .draw(|frame| ordinary.render(frame, Rect::new(0, 0, 10, 1)))
        .expect("ordinary data view should render");

    assert_eq!(
        moving_terminal.backend().buffer(),
        ordinary_terminal.backend().buffer()
    );
}

#[test]
fn changing_reorder_row_starts_new_presentation_from_base() {
    let mut view = DataView::list(
        [Row::new(1, "first"), Row::new(2, "second")],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let settings = AnimationSettings::default();

    view.start_reorder_highlight(1, settings);
    Animated::tick(&mut view, Duration::from_millis(100), settings);
    Animated::tick(&mut view, Duration::from_millis(25), settings);
    assert_eq!(view.reorder_highlight_progress_for_test(), 0.5);

    view.start_reorder_highlight(2, settings);

    assert_eq!(view.reorder_highlight_progress_for_test(), 0.0);
    assert!(!view.row_has_reorder_highlight(&1));
    assert!(view.row_has_reorder_highlight(&2));
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
    assert_ne!(cell.fg, theme.text_fg());
    assert_ne!(cell.fg, theme.surface_bg());
    assert_ne!(cell.bg, theme.surface_bg());
}

#[test]
fn inactive_highlight_uses_selected_style_when_enabled() {
    let view = DataView::list(
        [Row::new(1, "selected")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .show_inactive_highlight(true);
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
fn inactive_highlight_normalizes_embedded_chip_colors() {
    let chip = crate::Chip::new("chip")
        .color_role(crate::ChipColorRole::Highlight)
        .line();
    let chip_width = line_width(&chip);
    let view = DataView::new([1], |row| *row)
        .column(Column::rich(
            "chip",
            "",
            Constraint::Percentage(100),
            move |_, _| chip.clone(),
        ))
        .show_inactive_highlight(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .expect("data view should render");

    let theme = theme();
    assert_embedded_chip_styles(
        terminal.backend().buffer(),
        chip_width,
        theme.selected_fg(),
        theme.selected_bg(),
    );
}

#[test]
fn checked_row_uses_checkbox_as_its_only_indicator() {
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

    let unchecked_cell = terminal.backend().buffer().cell((4, 0)).unwrap();
    let content_cell = terminal.backend().buffer().cell((4, 1)).unwrap();
    assert_eq!(content_cell.fg, unchecked_cell.fg);
    assert_eq!(content_cell.bg, unchecked_cell.bg);
    assert_eq!(content_cell.modifier, unchecked_cell.modifier);
}

#[test]
fn focused_selected_cursor_uses_focus_style_and_keeps_selection_glyph() {
    let view = DataView::list(
        [Row::new(1, "selected")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selection_glyphs(SelectionGlyphs::ASCII)
    .selected([1])
    .focused(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 1)).expect("terminal should build");

    terminal
        .draw(|frame| view.render(frame, Rect::new(0, 0, 12, 1)))
        .expect("data view should render");

    let theme = crate::theme();
    let glyph_cell = terminal.backend().buffer().cell((0, 0)).unwrap();
    let content_cell = terminal.backend().buffer().cell((4, 0)).unwrap();
    assert_eq!(glyph_cell.symbol(), "[");
    assert_eq!(glyph_cell.fg, theme.highlight_fg());
    assert_eq!(content_cell.bg, theme.highlight_bg());
    assert!(content_cell.modifier.contains(Modifier::BOLD));
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
    let mut child = Row::new(2, "Y");
    child.parent = Some(1);
    let mut view = DataView::new([Row::new(1, "X"), child], |row| row.id)
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
        .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
        .expanded([1]);
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
fn unbound_selection_key_is_not_handled_when_selection_is_disabled() {
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
fn disabled_checkbox_rows_are_muted_and_cannot_be_selected() {
    let mut view = DataView::list([1, 2], |row| *row, |row| format!("row {row}"))
        .selection_mode(SelectionMode::Multi)
        .selection_glyphs(SelectionGlyphs::ASCII)
        .selection_disabled_by(|row| *row == 2)
        .selection_disabled_glyph("󱋭")
        .selected([1, 2]);

    assert_eq!(view.selected_ids(), vec![1]);
    assert!(view.is_selection_disabled(&2));
    assert!(!view.toggle_selected(2));
    assert_eq!(view.check_state(&2), CheckState::Unchecked);

    let mut terminal = Terminal::new(TestBackend::new(20, 2)).unwrap();
    terminal
        .draw(|frame| view.render(frame, frame.area()))
        .unwrap();

    let disabled = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(disabled.symbol(), "󱋭");
    assert_eq!(disabled.fg, theme().muted_fg());
}

#[test]
fn disabled_tree_descendants_leave_parent_unchecked_and_are_removed_after_row_updates() {
    let mut view = DataView::list(
        [
            Row {
                id: 1,
                parent: None,
                name: "parent",
            },
            Row {
                id: 2,
                parent: Some(1),
                name: "child",
            },
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
    .selection_mode(SelectionMode::Multi)
    .selection_propagation(SelectionPropagation::CascadeDescendants)
    .selection_disabled_by(|row| row.parent.is_some())
    .selected([1, 2]);

    assert_eq!(view.selected_ids(), vec![1]);
    assert_eq!(view.check_state(&1), CheckState::Unchecked);

    view.update_row(&1, |row| row.parent = Some(99));

    assert!(view.selected_ids().is_empty());
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
fn collapse_emits_navigation_activation_but_sort_preserves_highlighted_id() {
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
    assert_eq!(collapsed.highlighted, 0);
    assert_eq!(collapsed.highlighted_id(), Some(1));
    assert_eq!(
        collapsed.take_events(),
        vec![
            DataViewTypedEvent::HighlightChanged { row_id: Some(1) },
            DataViewTypedEvent::Activated { row_id: 1 },
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

    assert!(!sort_outcome.activated);
    assert_eq!(sorted.highlighted, 1);
    assert_eq!(sorted.highlighted_id(), Some(1));
    assert!(sorted.take_events().is_empty());
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
fn select_all_shortcut_toggles_multi_selection() {
    let mut view = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two"), Row::new(3, "three")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi);
    let area = Rect::new(0, 0, 20, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    let selected = view.on_key_with_settings(Key::Char('a'), area, settings);

    assert!(selected.handled);
    assert!(selected.changed);
    assert_eq!(view.selected_ids(), vec![1, 2, 3]);

    let cleared = view.on_key_with_settings(Key::Char('a'), area, settings);

    assert!(cleared.handled);
    assert!(cleared.changed);
    assert!(view.selected_ids().is_empty());

    let mut single = DataView::list(
        [Row::new(1, "one"), Row::new(2, "two")],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Single)
    .selected([1]);

    let ignored = single.on_key_with_settings(Key::Char('a'), area, settings);

    assert!(!ignored.handled);
    assert_eq!(single.selected_ids(), vec![1]);
}

#[test]
fn tree_bulk_shortcut_expands_when_collapsed_and_collapses_when_expanded() {
    let mut view = tree_view();
    let area = Rect::new(0, 0, 20, 6);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };

    let expanded = view.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(expanded.handled);
    assert!(expanded.changed);
    assert!(!view.expanded.is_empty());

    let mut partial = tree_view().expanded([1]);
    let completed = partial.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(completed.handled);
    assert!(completed.changed);
    assert_eq!(
        partial.expanded,
        partial
            .expandable_ids()
            .collect::<std::collections::HashSet<_>>()
    );

    let collapsed = view.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(collapsed.handled);
    assert!(collapsed.changed);
    assert!(view.expanded.is_empty());

    let mut stale = tree_view().expanded([1, 2, 3, 99]);
    let collapsed = stale.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(collapsed.changed);
    assert!(stale.expanded.is_empty());
}

#[test]
fn tree_bulk_shortcut_restores_the_closest_visible_ancestor_and_scrolls_to_it() {
    let area = Rect::new(0, 0, 20, 1);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut view = tree_view().expanded([1, 2, 3]);
    view.highlighted = 3;
    view.ensure_highlight_visible(area, settings);

    let collapsed = view.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(collapsed.changed);
    assert_eq!(view.highlighted_id(), Some(1));
    assert_eq!(view.scroll.target_offset().y, 0);
}

#[test]
fn tree_bulk_shortcut_centers_the_highlighted_item_when_expanding() {
    let area = Rect::new(0, 0, 20, 3);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut view = tree_view().expanded([1]);
    view.highlighted = 2;

    let expanded = view.on_key_with_settings(Key::Char('z'), area, settings);

    assert!(expanded.changed);
    assert_eq!(view.highlighted_id(), Some(3));
    assert_eq!(view.scroll.target_offset().y, 3);
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

#[test]
fn filtered_row_ids_apply_ranked_visible_order() {
    let view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .visible_row_ids([3, 1]);

    assert_eq!(visible_ids(&view), vec![3, 1]);
}

#[test]
fn replacing_rows_preserves_visible_subset_order_by_id_after_reorder() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .visible_row_ids([3, 1]);
    view.highlight_id(&1);

    view.set_rows([
        Row::new(1, "Alpha updated"),
        Row::new(3, "Gamma updated"),
        Row::new(2, "Beta updated"),
    ]);

    assert_eq!(visible_ids(&view), vec![3, 1]);
    assert_eq!(view.highlighted_id(), Some(1));
}

#[test]
fn replacing_rows_removes_missing_visible_ids_and_synchronizes_highlight() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .visible_row_ids([3, 1]);
    view.highlight_id(&3);

    view.set_rows([Row::new(2, "Beta updated"), Row::new(1, "Alpha updated")]);

    assert_eq!(visible_ids(&view), vec![1]);
    assert_eq!(view.highlighted_id(), Some(1));
}

#[test]
fn filtering_preserves_highlight_by_row_id() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
            Row::new(4, "Delta"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    view.highlight_line_with_settings(2, Rect::new(0, 0, 20, 4), settings);
    let outcome = view.set_visible_row_ids([4, 3]);

    assert!(outcome.changed);
    assert_eq!(visible_ids(&view), vec![4, 3]);
    assert_eq!(view.highlighted_id(), Some(3));
}

#[test]
fn filtering_falls_back_to_first_visible_row_when_highlight_is_hidden() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    );
    let mut settings = AnimationSettings::default();
    settings.enabled = false;

    view.highlight_line_with_settings(2, Rect::new(0, 0, 20, 3), settings);
    view.set_visible_row_ids([2]);

    assert_eq!(visible_ids(&view), vec![2]);
    assert_eq!(view.highlighted_id(), Some(2));
}

#[test]
fn empty_filter_has_no_highlight_and_next_nonempty_filter_selects_first_visible_row() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    );

    view.set_visible_row_ids([]);
    assert_eq!(visible_ids(&view), Vec::<usize>::new());
    assert_eq!(view.highlighted_id(), None);

    view.set_visible_row_ids([3, 1]);
    assert_eq!(visible_ids(&view), vec![3, 1]);
    assert_eq!(view.highlighted_id(), Some(3));
}

#[test]
fn hidden_selected_item_is_retained_when_filter_changes() {
    let mut view = DataView::list(
        [
            Row::new(1, "Alpha"),
            Row::new(2, "Beta"),
            Row::new(3, "Gamma"),
        ],
        |row| row.id,
        |row| row.name.to_string(),
    )
    .selection_mode(SelectionMode::Multi)
    .selected([2]);

    view.set_visible_row_ids([1, 3]);

    assert_eq!(visible_ids(&view), vec![1, 3]);
    assert_eq!(view.selected_ids(), vec![2]);
    assert!(view.is_selected(&2));
}

fn tree_view() -> DataView<Row, usize> {
    DataView::list(rows(), |row| row.id, |row| row.name.to_string())
        .tree(TreeAdapter::parent_id(|row: &Row| row.parent))
}

fn transform_view() -> DataView<TransformRow, usize> {
    DataView::new(transform_rows(), |row| row.id).columns([
        Column::text(
            "task",
            "Task",
            Constraint::Percentage(40),
            |row: &TransformRow| row.task.to_string(),
        )
        .sortable(|row| row.task.to_string())
        .search_key(|row| row.task.to_string()),
        Column::text(
            "owner",
            "Owner",
            Constraint::Percentage(30),
            |row: &TransformRow| row.owner.to_string(),
        )
        .filter_key(|row| row.owner.to_string()),
        Column::text(
            "status",
            "Status",
            Constraint::Percentage(30),
            |row: &TransformRow| row.status.to_string(),
        )
        .filter_key(|row| row.status.to_string()),
    ])
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

fn transform_rows() -> Vec<TransformRow> {
    vec![
        TransformRow {
            id: 1,
            task: "API auth",
            owner: "Ada",
            status: "Ready",
        },
        TransformRow {
            id: 2,
            task: "CLI polish",
            owner: "Lin",
            status: "Active",
        },
        TransformRow {
            id: 3,
            task: "API docs",
            owner: "Ada",
            status: "Blocked",
        },
        TransformRow {
            id: 4,
            task: "TUI layout",
            owner: "Mia",
            status: "Ready",
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

#[test]
fn typed_sort_orders_numbers_and_preserves_equal_ties_in_both_directions() {
    #[derive(Clone)]
    struct NumericRow {
        id: usize,
        value: usize,
    }
    let rows = [
        NumericRow { id: 1, value: 10 },
        NumericRow { id: 2, value: 2 },
        NumericRow { id: 3, value: 100 },
        NumericRow { id: 4, value: 10 },
    ];
    let mut view = DataView::new(rows, |row| row.id).column(
        Column::text("value", "Value", Constraint::Fill(1), |row: &NumericRow| {
            row.value.to_string()
        })
        .sortable(|row| row.value),
    );

    view.sort_by("value", SortDirection::Ascending);
    assert_eq!(visible_ids(&view), vec![2, 1, 4, 3]);
    view.sort_by("value", SortDirection::Descending);
    assert_eq!(visible_ids(&view), vec![3, 1, 4, 2]);
}

#[test]
fn custom_comparator_and_sort_change_preserve_highlighted_id() {
    let mut view = DataView::new(rows(), |row| row.id).column(
        Column::text("name", "Name", Constraint::Fill(1), |row: &Row| {
            row.name.to_string()
        })
        .sortable_by(|left, right| left.name.len().cmp(&right.name.len())),
    );
    view.highlight_id(&3);
    view.take_events();

    view.sort_by("name", SortDirection::Ascending);

    assert_eq!(view.highlighted_id(), Some(3));
    assert!(view.take_events().is_empty());
}

#[test]
#[should_panic(expected = "must be sortable")]
fn automatic_sort_rejects_non_sortable_column() {
    let _ = DataView::list(rows(), |row| row.id, |row| row.name.to_string())
        .sorted_by("label", SortDirection::Ascending);
}
