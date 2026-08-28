use super::*;
use std::time::Duration;

use crate::{
    Animated, AnimationSettings, FocusCtx, KeyModifiers, LayoutCtx, LayoutProposal, LifecycleCtx,
    Propagation, ScrollOffset, TreeAdapter, TuiNode,
};
use ratatui::{Terminal, backend::TestBackend};

type Row = (usize, String, String, String);

fn table(row_count: usize) -> ListControl<Row, usize> {
    let rows = (0..row_count).map(|index| {
        (
            index,
            format!("Entity {index}"),
            format!("Owner {index}"),
            "Active".to_string(),
        )
    });
    ListControl::new_fields(
        rows,
        |row: &Row| row.0,
        [
            ListControlField::text("Entity"),
            ListControlField::text("Owner"),
            ListControlField::text("State"),
        ],
        |_, _| unreachable!("geometry test does not submit"),
    )
    .columns([
        Column::text(
            "entity",
            "Entity",
            Constraint::Percentage(45),
            |row: &Row| row.1.clone(),
        )
        .constrained(),
        Column::text("owner", "Owner", Constraint::Percentage(35), |row: &Row| {
            row.2.clone()
        })
        .constrained(),
        Column::text("state", "State", Constraint::Percentage(20), |row: &Row| {
            row.3.clone()
        })
        .constrained(),
    ])
    .headers(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeRow {
    id: usize,
    parent: Option<usize>,
}

#[test]
fn removing_tree_row_removes_its_complete_subtree() {
    let mut control: ListControl<TreeRow, usize> = ListControl::list(
        [
            TreeRow {
                id: 1,
                parent: None,
            },
            TreeRow {
                id: 2,
                parent: Some(1),
            },
            TreeRow {
                id: 3,
                parent: Some(2),
            },
            TreeRow {
                id: 4,
                parent: None,
            },
        ],
        |row: &TreeRow| row.id,
        |_| String::new(),
        |_, _| unreachable!("removal test does not add rows"),
    )
    .tree(TreeAdapter::parent_id(|row: &TreeRow| row.parent));
    control.data_view_mut().highlight_id(&1);

    assert!(control.remove_highlighted());

    assert_eq!(
        control.items(),
        &[TreeRow {
            id: 4,
            parent: None
        }]
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Removed { row_id: 1 }]
    );
}

#[test]
fn tree_reorder_inverts_the_moving_row_highlight() {
    let mut control: ListControl<TreeRow, usize> = ListControl::list(
        [
            TreeRow {
                id: 1,
                parent: None,
            },
            TreeRow {
                id: 2,
                parent: Some(1),
            },
            TreeRow {
                id: 3,
                parent: Some(1),
            },
        ],
        |row: &TreeRow| row.id,
        |row| row.id.to_string(),
        |_, _| unreachable!("tree reorder test does not add rows"),
    )
    .tree(TreeAdapter::mutable_parent_id(
        |row: &TreeRow| row.parent,
        |row, parent| row.parent = parent,
    ))
    .expanded([1]);
    control.data_view_mut().highlight_id(&3);
    control.data_view_mut().set_focused(true);
    let settings = AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    };
    let mut ctx = EventCtx::new(settings);

    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::Up), &mut ctx);

    assert_eq!(control.data_view().highlighted_id(), Some(3));
    assert!(control.data_view().row_has_reorder_highlight(&3));

    let mut terminal = Terminal::new(TestBackend::new(20, 3)).expect("terminal should build");
    terminal
        .draw(|frame| {
            control.data_view().render(frame, Rect::new(0, 0, 20, 3));
        })
        .expect("tree should render");
    let cell = terminal.backend().buffer().cell((0, 1)).unwrap();
    let theme = crate::theme();
    assert_eq!(cell.fg, theme.highlight_bg());
    assert_eq!(cell.bg, theme.highlight_fg());
}

fn layout_adding(control: &mut ListControl<Row, usize>, area: Rect) {
    control.begin_add(None);
    control.layout(area, &mut LayoutCtx::new());
}

fn expected_viewport_area(control: &ListControl<Row, usize>) -> Rect {
    let input = control.input_area;
    let columns = control
        .data_view
        .visible_column_rects(control.data_area, input.y, input.height);
    Rect::new(
        columns[0].x,
        input.y,
        columns.last().expect("state column").right() - columns[0].x,
        input.height,
    )
}

#[test]
fn active_input_spans_data_view_viewport_at_normal_and_narrow_widths() {
    for width in [80, 24] {
        let mut control = table(2);
        layout_adding(&mut control, Rect::new(0, 0, width, 10));

        assert_eq!(control.input_area, expected_viewport_area(&control));
    }
}

#[test]
fn active_input_uses_scrollbar_adjusted_data_view_viewport() {
    let mut control = table(20);
    let area = Rect::new(0, 0, 40, 8);
    layout_adding(&mut control, area);

    assert_eq!(control.input_area, expected_viewport_area(&control));
    assert!(control.input_area.right() < Panel::inner_area(area).right());
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RankedRow {
    id: usize,
    rank: usize,
}

fn ranked_control(rows: impl IntoIterator<Item = RankedRow>) -> ListControl<RankedRow, usize> {
    ListControl::new(rows, |row| row.id, |_, _| unreachable!())
        .columns([
            Column::text("rank", "Rank", Constraint::Fill(1), |row: &RankedRow| {
                row.rank.to_string()
            })
            .reorderable(|row| row.rank, |row, rank| row.rank = rank)
            .hidden(),
            Column::text("id", "ID", Constraint::Fill(1), |row: &RankedRow| {
                row.id.to_string()
            }),
        ])
        .reorderable_by("rank")
}

fn modified_key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}

fn ranked_rows(count: usize) -> Vec<RankedRow> {
    (0..count)
        .map(|id| RankedRow {
            id,
            rank: id.saturating_mul(10),
        })
        .collect()
}

fn start_reordering(control: &mut ListControl<RankedRow, usize>, moving_id: usize) {
    control.data_view.highlight_id(&moving_id);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
}

fn staged_ids(control: &ListControl<RankedRow, usize>) -> Vec<usize> {
    control
        .reorder
        .as_ref()
        .expect("reorder should be active")
        .staged
        .clone()
}

fn assert_reorder_scroll_matches_navigation(row_height: u16, moving_id: usize, keys: &[KeyEvent]) {
    let area = Rect::new(0, 0, 30, 9);
    let settings = AnimationSettings::default();
    let mut normal = ranked_control(ranked_rows(12)).row_height(row_height);
    let mut reorder = ranked_control(ranked_rows(12)).row_height(row_height);
    normal.layout(area, &mut LayoutCtx::new());
    reorder.layout(area, &mut LayoutCtx::new());
    normal.data_view.highlight_id(&moving_id);
    start_reordering(&mut reorder, moving_id);
    let mut ctx = EventCtx::new(settings);

    for key in keys {
        normal
            .data_view
            .on_key_with_settings(*key, normal.data_area, settings);
        reorder.handle_reorder_key(*key, &mut ctx);
    }

    assert_eq!(
        reorder.data_view.scroll_animation_state_for_test(),
        normal.data_view.scroll_animation_state_for_test(),
        "row_height={row_height}, moving_id={moving_id}, keys={keys:?}"
    );
}

fn assert_no_data_view_events(control: &mut ListControl<RankedRow, usize>) {
    assert!(control.data_view.take_events().is_empty());
    assert!(control.data_view.take_last_activated().is_none());
}

fn tick_scroll_until_idle(
    control: &mut ListControl<RankedRow, usize>,
    settings: AnimationSettings,
) {
    for _ in 0..20 {
        Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
        if !control.data_view.scroll_animation_state_for_test().2 {
            return;
        }
    }
    panic!("scroll animation did not finish");
}

#[test]
fn reorder_navigation_uses_normal_data_view_scroll_anchoring() {
    let up = KeyEvent::from(Key::Up);
    let down = KeyEvent::from(Key::Down);
    let page_up = KeyEvent::from(Key::PageUp);
    let page_down = KeyEvent::from(Key::PageDown);
    let home = KeyEvent::from(Key::Home);
    let end = KeyEvent::from(Key::End);
    let gg = KeyEvent::from(Key::Char('g'));
    let bottom = modified_key(Key::Char('G'), KeyModifiers::SHIFT);

    for row_height in [1, 2] {
        for (moving_id, keys) in [
            (1, &[up][..]),
            (5, &[up][..]),
            (5, &[down][..]),
            (10, &[down][..]),
            (5, &[page_up][..]),
            (5, &[page_down][..]),
            (5, &[home][..]),
            (5, &[end][..]),
            (5, &[gg, gg][..]),
            (5, &[bottom][..]),
        ] {
            assert_reorder_scroll_matches_navigation(row_height, moving_id, keys);
        }
    }
}

#[test]
fn reorder_boundary_keys_do_not_retarget_scroll() {
    for (moving_id, key) in [(0, Key::Up), (11, Key::Down)] {
        let mut control = ranked_control(ranked_rows(12));
        control.layout(Rect::new(0, 0, 30, 9), &mut LayoutCtx::new());
        start_reordering(&mut control, moving_id);
        let before = control.data_view.scroll_animation_state_for_test();

        control.handle_reorder_key(KeyEvent::from(key), &mut EventCtx::default());

        assert_eq!(control.data_view.scroll_animation_state_for_test(), before);
    }
}

#[test]
fn reorder_preserves_on_navigate_selection_without_data_view_events() {
    for (mode, selected) in [
        (SelectionMode::Single, vec![1]),
        (SelectionMode::Multi, vec![1, 4]),
    ] {
        for commit in [false, true] {
            let mut control = ranked_control(ranked_rows(6))
                .selection_mode(mode)
                .selection_trigger(SelectionTrigger::OnNavigate)
                .activation_mode(ActivationMode::OnNavigate);
            control.data_view.highlight_id(&3);
            control.data_view.clear_selection();
            for id in &selected {
                control.data_view.select_id(*id);
            }
            control.data_view.take_events();
            control.data_view.take_last_activated();
            let mut ctx = EventCtx::default();

            control.handle_reorder_key(
                modified_key(Key::Char('m'), KeyModifiers::CONTROL),
                &mut ctx,
            );
            assert_eq!(control.data_view.selected_ids(), selected);
            assert_no_data_view_events(&mut control);

            control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
            assert_eq!(control.data_view.selected_ids(), selected);
            assert_no_data_view_events(&mut control);

            let finish = if commit { Key::Enter } else { Key::Esc };
            control.handle_reorder_key(KeyEvent::from(finish), &mut ctx);
            assert_eq!(control.data_view.selected_ids(), selected);
            assert_no_data_view_events(&mut control);
            if commit {
                assert!(matches!(
                    control.take_events().as_slice(),
                    [ListControlEvent::Reordered { .. }]
                ));
            } else {
                assert_eq!(
                    control.take_events(),
                    vec![ListControlEvent::ReorderCancelled { row_id: 3 }]
                );
            }
        }
    }
}

#[test]
fn reorder_gg_requires_two_presses_and_non_g_interrupts_sequence() {
    let mut control = ranked_control(ranked_rows(6));
    let mut ctx = EventCtx::default();
    start_reordering(&mut control, 3);

    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    assert_eq!(staged_ids(&control), vec![0, 1, 2, 3, 4, 5]);
    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    assert_eq!(staged_ids(&control), vec![3, 0, 1, 2, 4, 5]);

    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);
    assert_eq!(staged_ids(&control), vec![0, 3, 1, 2, 4, 5]);
    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    assert_eq!(staged_ids(&control), vec![0, 3, 1, 2, 4, 5]);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);
    start_reordering(&mut control, 3);
    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    assert_eq!(staged_ids(&control), vec![0, 1, 2, 3, 4, 5]);
}

#[test]
fn reorder_bottom_home_and_end_move_to_edges() {
    for (key, expected) in [
        (
            modified_key(Key::Char('G'), KeyModifiers::SHIFT),
            vec![0, 1, 3, 4, 2],
        ),
        (KeyEvent::from(Key::Home), vec![2, 0, 1, 3, 4]),
        (KeyEvent::from(Key::End), vec![0, 1, 3, 4, 2]),
    ] {
        let mut control = ranked_control(ranked_rows(5));
        start_reordering(&mut control, 2);

        control.handle_reorder_key(key, &mut EventCtx::default());

        assert_eq!(staged_ids(&control), expected);
    }
}

#[test]
fn reorder_page_keys_use_visible_page_step_and_clamp() {
    for key in [
        modified_key(Key::Char('u'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::PageUp),
    ] {
        let mut control = ranked_control(ranked_rows(10));
        control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
        start_reordering(&mut control, 5);

        control.handle_reorder_key(key, &mut EventCtx::default());
        assert_eq!(staged_ids(&control), vec![0, 1, 5, 2, 3, 4, 6, 7, 8, 9]);
        control.handle_reorder_key(key, &mut EventCtx::default());
        assert_eq!(staged_ids(&control), vec![5, 0, 1, 2, 3, 4, 6, 7, 8, 9]);
    }

    for key in [
        modified_key(Key::Char('d'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::PageDown),
    ] {
        let mut control = ranked_control(ranked_rows(10));
        control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
        start_reordering(&mut control, 5);

        control.handle_reorder_key(key, &mut EventCtx::default());
        assert_eq!(staged_ids(&control), vec![0, 1, 2, 3, 4, 6, 7, 8, 5, 9]);
        control.handle_reorder_key(key, &mut EventCtx::default());
        assert_eq!(staged_ids(&control), vec![0, 1, 2, 3, 4, 6, 7, 8, 9, 5]);
    }
}

#[test]
fn reorder_page_down_uses_underfilled_visible_item_count() {
    let mut control = ranked_control(ranked_rows(7));
    control.layout(Rect::new(0, 0, 30, 12), &mut LayoutCtx::new());
    start_reordering(&mut control, 0);

    control.handle_reorder_key(KeyEvent::from(Key::PageDown), &mut EventCtx::default());

    assert_eq!(staged_ids(&control), vec![1, 2, 3, 4, 5, 0, 6]);
}

#[test]
fn reorder_page_down_and_end_center_moving_row() {
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    start_reordering(&mut control, 5);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let mut ctx = EventCtx::new(settings);

    control.handle_reorder_key(KeyEvent::from(Key::PageDown), &mut ctx);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 5);

    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 5);
}

#[test]
fn reorder_page_up_and_gg_center_moving_row() {
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    start_reordering(&mut control, 5);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let mut ctx = EventCtx::new(settings);
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);

    control.handle_reorder_key(KeyEvent::from(Key::PageUp), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::PageUp), &mut ctx);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 1);

    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Char('g')), &mut ctx);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 0);
}

#[test]
fn reorder_cancel_restores_exact_scroll_offset_with_animations_disabled() {
    for cancel in [
        KeyEvent::from(Key::Esc),
        modified_key(Key::Char('['), KeyModifiers::CONTROL),
    ] {
        let mut control = ranked_control(ranked_rows(10));
        control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut ctx = EventCtx::new(settings);
        control.data_view.highlight_id(&5);
        control
            .data_view
            .ensure_highlight_visible(control.data_area, settings);
        let before = control.data_view.scroll_animation_state_for_test().1;
        control.handle_reorder_key(
            modified_key(Key::Char('m'), KeyModifiers::CONTROL),
            &mut ctx,
        );
        control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
        assert_ne!(
            control.data_view.scroll_animation_state_for_test().0,
            before
        );

        control.handle_reorder_key(cancel, &mut ctx);

        let restored = control.data_view.scroll_animation_state_for_test();
        assert_eq!(restored.0, before);
        assert_eq!(restored.1, before);
        assert!(!restored.2);
    }
}

#[test]
fn disabled_cancel_snaps_to_original_in_flight_target() {
    let settings = AnimationSettings::default();
    let mut disabled = settings;
    disabled.enabled = false;
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    control.data_view.highlight_id(&9);
    control
        .data_view
        .center_highlight(control.data_area, settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
    let saved = control.data_view.scroll_animation_state_for_test();
    assert!(saved.2);
    let mut ctx = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::Home), &mut ctx);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut EventCtx::new(disabled));

    assert_eq!(
        control.data_view.scroll_animation_state_for_test(),
        (saved.1, saved.1, false)
    );
}

#[test]
fn reorder_cancel_animates_from_current_offset_to_original_offset() {
    let settings = AnimationSettings::default();
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    let mut ctx = EventCtx::new(settings);
    start_reordering(&mut control, 0);
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    let current = control.data_view.scroll_animation_state_for_test().0;
    assert!(current.y > 0);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);

    let restoring = control.data_view.scroll_animation_state_for_test();
    assert_eq!(restoring.0, current);
    assert_eq!(restoring.1, ScrollOffset::default());
    assert!(restoring.2);
    tick_scroll_until_idle(&mut control, settings);
    assert_eq!(
        control.data_view.scroll_animation_state_for_test(),
        (ScrollOffset::default(), ScrollOffset::default(), false)
    );
}

#[test]
fn reorder_focus_loss_animates_scroll_restoration() {
    let mut control = ranked_control(ranked_rows(10));
    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 30, 7), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    let settings = AnimationSettings::default();
    let mut event = EventCtx::new(settings);
    start_reordering(&mut control, 0);
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut event);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    let current = control.data_view.scroll_animation_state_for_test().0;

    control.dispatch_focus(&data_target, false, &mut FocusCtx::new(settings));

    let restoring = control.data_view.scroll_animation_state_for_test();
    assert_eq!(restoring.0, current);
    assert!(restoring.2);
    tick_scroll_until_idle(&mut control, settings);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 0);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderCancelled { row_id: 0 }]
    );
}

#[test]
fn reorder_data_change_abort_animates_scroll_restoration() {
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    let settings = AnimationSettings::default();
    let mut ctx = EventCtx::new(settings);
    start_reordering(&mut control, 0);
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    let current = control.data_view.scroll_animation_state_for_test().0;
    control.data_view.set_rows(ranked_rows(11));

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);

    let restoring = control.data_view.scroll_animation_state_for_test();
    assert_eq!(restoring.0, current);
    assert!(restoring.2);
    tick_scroll_until_idle(&mut control, settings);
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 0);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged
        }]
    );
}

#[test]
fn reorder_commit_retains_moved_row_scroll_offset() {
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let mut ctx = EventCtx::new(settings);
    control.data_view.highlight_id(&0);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    let moved = control.data_view.vertical_scroll_offset_for_test();
    assert!(moved > 0);

    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), moved);
}

#[test]
fn reorder_cancel_restores_in_flight_scroll_animation() {
    let settings = AnimationSettings::default();
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    control.data_view.highlight_id(&9);
    control
        .data_view
        .center_highlight(control.data_area, settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
    let saved = control.data_view.scroll_animation_state_for_test();
    assert!(saved.2);
    let mut ctx = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::Home), &mut ctx);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    let current = control.data_view.scroll_animation_state_for_test().0;

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);

    assert_eq!(
        control.data_view.scroll_animation_state_for_test().0,
        current
    );
    let mut resumed_saved_target = false;
    for _ in 0..20 {
        Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
        let state = control.data_view.scroll_animation_state_for_test();
        resumed_saved_target |= state.1 == saved.1;
        if !state.2 {
            break;
        }
    }
    assert!(resumed_saved_target);
    assert_eq!(
        control.data_view.scroll_animation_state_for_test(),
        (saved.1, saved.1, false)
    );
}

#[test]
fn new_reorder_during_scroll_restoration_does_not_jump() {
    let settings = AnimationSettings::default();
    let mut control = ranked_control(ranked_rows(10));
    control.layout(Rect::new(0, 0, 30, 7), &mut LayoutCtx::new());
    let mut ctx = EventCtx::new(settings);
    start_reordering(&mut control, 0);
    control.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);
    Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
    let before = control.data_view.scroll_animation_state_for_test().0;

    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert!(control.is_reordering());
    assert_eq!(
        control.data_view.scroll_animation_state_for_test().0,
        before
    );
}

#[test]
fn unmodified_space_commits_reorder_but_modified_space_does_not() {
    let mut control =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(modified_key(Key::Char(' '), KeyModifiers::SHIFT), &mut ctx);
    assert!(control.is_reordering());

    control.handle_reorder_key(KeyEvent::from(Key::Char(' ')), &mut ctx);

    assert!(!control.is_reordering());
    assert!(matches!(
        control.take_events().as_slice(),
        [ListControlEvent::Reordered { .. }]
    ));
}

#[test]
fn flat_shift_range_selection_survives_only_shift_line_extension() {
    let mut control = ranked_control(ranked_rows(4));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );
    control.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );

    assert_eq!(
        control
            .flat_range_selection
            .as_ref()
            .expect("range should remain active")
            .selected,
        vec![1, 2, 3]
    );
    assert!(control.take_events().is_empty());
}

#[test]
fn flat_range_selection_survives_focus_loss_and_gain() {
    let mut control = table(4);
    control.data_view.highlight_id(&1);
    let mut event = EventCtx::default();
    control.handle_flat_range_selection_key(
        modified_key(Key::Down, KeyModifiers::SHIFT),
        &mut event,
    );
    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 30, 5), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    let settings = AnimationSettings::default();

    control.dispatch_focus(&data_target, false, &mut FocusCtx::new(settings));
    control.dispatch_focus(&data_target, true, &mut FocusCtx::new(settings));

    assert_eq!(control.transient_selected_ids(), vec![1, 2]);
    assert!(control.data_view.selection_overlay_active_for_test());
    assert!(!control.is_reordering());
}

#[test]
fn flat_selection_without_reordering_supports_bulk_actions() {
    let mut control = table(4);
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );

    assert_eq!(control.transient_selected_ids(), vec![1, 2]);

    let rows = control.items().to_vec();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert!(!control.is_reordering());
    assert_eq!(control.items(), rows);
    assert!(control.take_events().is_empty());
}

#[test]
fn transient_selected_ids_returns_shift_range_in_display_order() {
    let mut control = ranked_control(ranked_rows(4));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );
    control.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );

    assert_eq!(control.transient_selected_ids(), vec![1, 2, 3]);
    assert_eq!(control.transient_selected_ids(), vec![1, 2, 3]);
}

#[test]
fn flat_ctrl_navigation_selects_the_origin_and_ctrl_space_toggles_current_rows() {
    let mut control = ranked_control(ranked_rows(5));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_flat_range_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert_eq!(control.data_view.highlighted_id(), Some(3));
    assert_eq!(
        control
            .flat_range_selection
            .as_ref()
            .expect("ctrl selection should remain active")
            .selected,
        vec![1, 3]
    );
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        control
            .flat_block_move
            .as_ref()
            .expect("ctrl selection should start a flat block move")
            .selected,
        vec![1, 3]
    );
}

#[test]
fn transient_selected_ids_returns_sparse_ctrl_selection_in_display_order() {
    let mut control = ranked_control(ranked_rows(5));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_flat_range_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert_eq!(control.transient_selected_ids(), vec![1, 3]);
}

#[test]
fn clearing_transient_selection_retains_highlight() {
    let mut control = ranked_control(ranked_rows(5));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_flat_range_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(control.data_view.highlighted_id(), Some(3));

    control.clear_transient_selection();

    assert!(control.transient_selected_ids().is_empty());
    assert_eq!(control.data_view.highlighted_id(), Some(3));
    assert!(!control.data_view.selection_overlay_active_for_test());
}

#[test]
fn replacing_rows_preserves_transient_ctrl_selection_in_new_order() {
    let mut control = ranked_control(ranked_rows(5));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_flat_range_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control
        .handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_flat_range_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(control.transient_selected_ids(), vec![1, 3, 4]);

    control.set_rows([
        RankedRow { id: 3, rank: 0 },
        RankedRow { id: 1, rank: 10 },
        RankedRow { id: 2, rank: 20 },
    ]);

    assert_eq!(control.transient_selected_ids(), vec![3, 1]);
    assert_eq!(control.data_view.highlighted_id(), Some(3));
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        control
            .flat_block_move
            .as_ref()
            .expect("refreshed selection should start a block move")
            .selected,
        vec![3, 1]
    );
}

#[test]
fn replacing_rows_retains_surviving_shift_selection_anchor_and_highlight() {
    let mut control = ranked_control(ranked_rows(4));
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    control.handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.set_rows([
        RankedRow { id: 2, rank: 0 },
        RankedRow { id: 1, rank: 10 },
        RankedRow { id: 3, rank: 20 },
    ]);

    assert_eq!(control.transient_selected_ids(), vec![2, 1]);
    assert_eq!(
        control
            .flat_range_selection
            .as_ref()
            .expect("shift selection should remain active")
            .anchor,
        1
    );
    assert_eq!(control.data_view.highlighted_id(), Some(2));
}

#[test]
fn flat_shift_range_selection_clears_for_every_data_view_navigation() {
    let navigation = [
        ("up", vec![KeyEvent::from(Key::Up)]),
        ("down", vec![KeyEvent::from(Key::Down)]),
        ("left", vec![KeyEvent::from(Key::Left)]),
        ("right", vec![KeyEvent::from(Key::Right)]),
        ("page up", vec![KeyEvent::from(Key::PageUp)]),
        ("page down", vec![KeyEvent::from(Key::PageDown)]),
        (
            "control u",
            vec![modified_key(Key::Char('u'), KeyModifiers::CONTROL)],
        ),
        (
            "control d",
            vec![modified_key(Key::Char('d'), KeyModifiers::CONTROL)],
        ),
        ("home", vec![KeyEvent::from(Key::Home)]),
        ("end", vec![KeyEvent::from(Key::End)]),
        (
            "gg",
            vec![
                KeyEvent::from(Key::Char('g')),
                KeyEvent::from(Key::Char('g')),
            ],
        ),
        ("G", vec![modified_key(Key::Char('G'), KeyModifiers::SHIFT)]),
    ];

    for (name, keys) in navigation {
        let mut control = ranked_control(ranked_rows(4));
        control.data_view.highlight_id(&1);
        let mut ctx = EventCtx::default();
        control.handle_flat_range_selection_key(
            modified_key(Key::Char('j'), KeyModifiers::SHIFT),
            &mut ctx,
        );

        for key in keys {
            control.handle_flat_range_selection_key(key, &mut ctx);
        }

        assert!(
            control.flat_range_selection.is_none(),
            "{name} should clear range"
        );
        assert!(
            !control.data_view.selection_overlay_active_for_test(),
            "{name} should clear overlay"
        );
    }
}

fn seed_stale_flat_range(control: &mut ListControl<RankedRow, usize>) {
    control.flat_range_selection = Some(FlatRangeSelectionState {
        selected: vec![1, 2],
        anchor: 1,
        range_mode: true,
    });
    control
        .data_view
        .set_selection_overlay(vec![1, 2], None, 0, false);
}

fn assert_flat_range_cleared(control: &ListControl<RankedRow, usize>) {
    assert!(control.flat_range_selection.is_none());
    assert!(!control.data_view.selection_overlay_active_for_test());
}

fn start_flat_block_move(control: &mut ListControl<RankedRow, usize>, first_id: usize) {
    control.data_view.highlight_id(&first_id);
    let mut ctx = EventCtx::default();
    control.handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
}

fn assert_flat_block_cleared(control: &ListControl<RankedRow, usize>) {
    assert!(control.flat_block_move.is_none());
    assert_flat_range_cleared(control);
}

#[test]
fn flat_range_ctrl_m_starts_block_move_instead_of_single_reorder() {
    let mut control = ranked_control(ranked_rows(5));

    start_flat_block_move(&mut control, 1);

    let block = control
        .flat_block_move
        .as_ref()
        .expect("flat block move should be active");
    assert_eq!(block.selected, vec![1, 2]);
    assert_eq!(block.target_index, 1);
    assert!(control.reorder.is_none());
    assert_eq!(
        control.data_view.selection_placeholder_depth_for_test(),
        Some(0)
    );
}

#[test]
fn flat_block_move_moves_the_pseudo_target_through_unselected_gaps() {
    let mut control = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut control, 1);

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut EventCtx::default());

    assert_eq!(
        control
            .flat_block_move
            .as_ref()
            .expect("flat block move should remain active")
            .target_index,
        2
    );
    assert_eq!(
        control.data_view.selection_placeholder_depth_for_test(),
        Some(0)
    );
}

#[test]
fn flat_block_move_steps_through_selected_source_rows() {
    let mut control = ranked_control(ranked_rows(5)).headers(false);
    start_flat_block_move(&mut control, 1);
    let mut ctx = EventCtx::default();

    control.handle_reorder_key(KeyEvent::from(Key::Up), &mut ctx);

    assert_eq!(
        control
            .flat_block_move
            .as_ref()
            .expect("flat block move should remain active")
            .target_index,
        1
    );
    let mut terminal = Terminal::new(TestBackend::new(30, 6)).expect("terminal should build");
    terminal
        .draw(|frame| {
            control.data_view().render(frame, Rect::new(0, 0, 30, 6));
        })
        .expect("flat block move should render");
    let rows = (0..6)
        .map(|y| {
            (0..30)
                .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(rows, ["0", "1", "Moving 2 tasks", "2", "3", "4"]);

    control.handle_reorder_key(KeyEvent::from(Key::Up), &mut ctx);
    assert_eq!(
        control
            .flat_block_move
            .as_ref()
            .expect("flat block move should remain active")
            .target_index,
        1
    );
}

#[test]
fn flat_block_move_steps_through_sparse_selected_source_rows() {
    let mut control = ranked_control(ranked_rows(6)).headers(false);
    control.flat_range_selection = Some(FlatRangeSelectionState {
        selected: vec![0, 2, 4],
        anchor: 0,
        range_mode: false,
    });
    control
        .data_view
        .set_selection_overlay(vec![0, 2, 4], None, 0, false);
    control.data_view.highlight_id(&4);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    let render_rows = |control: &ListControl<RankedRow, usize>| {
        let mut terminal = Terminal::new(TestBackend::new(30, 7)).expect("terminal should build");
        terminal
            .draw(|frame| control.data_view().render(frame, Rect::new(0, 0, 30, 7)))
            .expect("flat block move should render");
        (0..7)
            .map(|y| {
                (0..30)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
    };

    control.handle_reorder_key(
        modified_key(Key::Char('k'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "1", "2", "3", "Moving 3 tasks", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('k'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "1", "2", "Moving 3 tasks", "3", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('k'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "1", "Moving 3 tasks", "2", "3", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('k'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "Moving 3 tasks", "1", "2", "3", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('k'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["Moving 3 tasks", "0", "1", "2", "3", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('j'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "Moving 3 tasks", "1", "2", "3", "4", "5"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('j'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["0", "1", "Moving 3 tasks", "2", "3", "4", "5"]
    );
}

#[test]
fn flat_block_move_returns_placeholder_before_following_row() {
    let mut control = ranked_control(ranked_rows(5)).headers(false);
    start_flat_block_move(&mut control, 1);
    let mut ctx = EventCtx::default();

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Up), &mut ctx);

    let mut terminal = Terminal::new(TestBackend::new(30, 6)).expect("terminal should build");
    terminal
        .draw(|frame| {
            control.data_view().render(frame, Rect::new(0, 0, 30, 6));
        })
        .expect("flat block move should render");
    let rows = (0..6)
        .map(|y| {
            (0..30)
                .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(rows, ["0", "1", "2", "Moving 2 tasks", "3", "4"]);
}

#[test]
fn flat_block_move_supports_page_and_edge_navigation_keys() {
    for (keys, expected_target) in [
        (vec![KeyEvent::from(Key::Home)], 0),
        (
            vec![
                KeyEvent::from(Key::Char('g')),
                KeyEvent::from(Key::Char('g')),
            ],
            0,
        ),
        (vec![modified_key(Key::Char('u'), KeyModifiers::CONTROL)], 0),
        (vec![KeyEvent::from(Key::PageUp)], 0),
        (vec![KeyEvent::from(Key::End)], 3),
        (vec![modified_key(Key::Char('G'), KeyModifiers::SHIFT)], 3),
        (vec![modified_key(Key::Char('d'), KeyModifiers::CONTROL)], 2),
        (vec![KeyEvent::from(Key::PageDown)], 2),
    ] {
        let mut control = ranked_control(ranked_rows(5));
        start_flat_block_move(&mut control, 1);
        let mut ctx = EventCtx::default();

        for key in keys {
            control.handle_reorder_key(key, &mut ctx);
        }

        assert_eq!(
            control
                .flat_block_move
                .as_ref()
                .expect("flat block move should remain active")
                .target_index,
            expected_target
        );
    }
}

#[test]
fn flat_block_move_commit_preserves_selected_source_order() {
    let mut control = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut control, 1);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);

    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert_eq!(control.data_view.reorder_visible_ids(), vec![0, 3, 1, 2, 4]);
    assert_flat_block_cleared(&control);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Reordered {
            row_ids: vec![0, 3, 1, 2, 4]
        }]
    );
}

#[test]
fn flat_block_move_commit_keeps_highlight_visible_in_narrow_viewport() {
    let mut control = ranked_control(ranked_rows(6));
    control.layout(Rect::new(0, 0, 30, 1), &mut LayoutCtx::new());
    control.data_view.highlight_id(&0);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let mut ctx = EventCtx::new(settings);
    control.handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.handle_flat_range_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert_eq!(control.data_view.highlighted_id(), Some(2));
    assert_eq!(control.data_view.vertical_scroll_offset_for_test(), 2);
}

#[test]
fn flat_block_move_cancel_keeps_rows_unchanged() {
    let mut control = ranked_control(ranked_rows(5));
    let before = control.items().to_vec();
    start_flat_block_move(&mut control, 1);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);

    assert_eq!(control.items(), before);
    assert_flat_block_cleared(&control);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderCancelled { row_id: 2 }]
    );
}

#[test]
fn flat_block_start_keeps_zero_selection_single_reorder_and_consumes_one_selection() {
    let mut zero_selection = ranked_control(ranked_rows(5));
    zero_selection.data_view.highlight_id(&3);
    zero_selection.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
    assert!(zero_selection.reorder.is_some());
    assert!(zero_selection.flat_block_move.is_none());

    let mut one_selection = ranked_control(ranked_rows(5));
    one_selection.flat_range_selection = Some(FlatRangeSelectionState {
        selected: vec![1],
        anchor: 1,
        range_mode: false,
    });
    one_selection
        .data_view
        .set_selection_overlay(vec![1], None, 0, false);
    one_selection.data_view.highlight_id(&3);
    for key in [
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::Char(' ')),
    ] {
        let mut ctx = EventCtx::default();
        assert_eq!(
            one_selection.handle_reorder_key(key, &mut ctx),
            Some(EventOutcome::Handled)
        );
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }
    assert!(one_selection.reorder.is_none());
    assert!(one_selection.flat_block_move.is_none());
    assert_eq!(
        one_selection.data_view.reorder_visible_ids(),
        vec![0, 1, 2, 3, 4]
    );
}

#[test]
fn local_search_rejects_flat_block_move_and_clears_selection() {
    let mut control = ranked_control(ranked_rows(12));
    control.data_view.set_search_query("1");
    let selected = control.data_view.reorder_visible_ids();
    assert_eq!(selected, vec![1, 10, 11]);
    control.flat_range_selection = Some(FlatRangeSelectionState {
        selected: selected[..2].to_vec(),
        anchor: selected[0],
        range_mode: true,
    });
    control
        .data_view
        .set_selection_overlay(selected[..2].to_vec(), None, 0, false);
    let mut ctx = EventCtx::default();

    assert_eq!(
        control.handle_reorder_key(
            modified_key(Key::Char('m'), KeyModifiers::CONTROL),
            &mut ctx,
        ),
        Some(EventOutcome::Handled)
    );

    assert!(!control.is_reordering());
    assert_flat_range_cleared(&control);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn starting_editor_clears_flat_range_selection() {
    let mut adding = ranked_control(ranked_rows(4));
    seed_stale_flat_range(&mut adding);
    adding.begin_add(None);
    assert_flat_range_cleared(&adding);

    let mut editing =
        ranked_control(ranked_rows(4)).editable(|row| vec![row.rank.to_string()], |_, _| {});
    editing.data_view.highlight_id(&1);
    seed_stale_flat_range(&mut editing);

    assert!(editing.begin_edit());
    assert_flat_range_cleared(&editing);
}

#[test]
fn flat_block_move_clears_state_for_focus_loss_unmount_mutation_and_conflict() {
    let settings = AnimationSettings::default();

    let mut focus_loss = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut focus_loss, 1);
    let mut layout = LayoutCtx::new();
    focus_loss.layout(Rect::new(0, 0, 30, 5), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    focus_loss.dispatch_focus(&data_target, false, &mut FocusCtx::new(settings));
    assert_flat_block_cleared(&focus_loss);

    let mut unmounted = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut unmounted, 1);
    unmounted.unmount(&mut LifecycleCtx::default());
    assert_flat_block_cleared(&unmounted);

    let mut mutable = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut mutable, 1);
    let _ = mutable.data_view_mut();
    assert_flat_block_cleared(&mutable);

    let mut conflicted = ranked_control(ranked_rows(5));
    start_flat_block_move(&mut conflicted, 1);
    conflicted.data_view.set_rows(ranked_rows(6));
    conflicted.handle_reorder_key(KeyEvent::from(Key::Down), &mut EventCtx::default());
    assert_flat_block_cleared(&conflicted);
    assert!(matches!(
        conflicted.take_events().as_slice(),
        [ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged
        }]
    ));
}

#[test]
fn flat_range_stays_visible_during_block_move_and_clears_when_reorder_exits() {
    let mut entering = ranked_control(ranked_rows(4));
    entering.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();
    entering.handle_flat_range_selection_key(
        modified_key(Key::Char('j'), KeyModifiers::SHIFT),
        &mut ctx,
    );
    entering.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(entering.flat_block_move.is_some());
    assert!(entering.flat_range_selection.is_some());
    assert!(entering.data_view.selection_overlay_active_for_test());

    let mut committed = ranked_control(ranked_rows(4));
    start_reordering(&mut committed, 1);
    seed_stale_flat_range(&mut committed);
    committed.handle_reorder_key(KeyEvent::from(Key::Enter), &mut EventCtx::default());
    assert_flat_range_cleared(&committed);

    let mut cancelled = ranked_control(ranked_rows(4));
    start_reordering(&mut cancelled, 1);
    seed_stale_flat_range(&mut cancelled);
    cancelled.handle_reorder_key(KeyEvent::from(Key::Esc), &mut EventCtx::default());
    assert_flat_range_cleared(&cancelled);

    let mut focus_lost = ranked_control(ranked_rows(4));
    start_reordering(&mut focus_lost, 1);
    seed_stale_flat_range(&mut focus_lost);
    focus_lost.cancel_reorder_for_focus_loss(AnimationSettings::default());
    assert_flat_range_cleared(&focus_lost);

    let mut unmounted = ranked_control(ranked_rows(4));
    start_reordering(&mut unmounted, 1);
    seed_stale_flat_range(&mut unmounted);
    unmounted.unmount(&mut LifecycleCtx::default());
    assert_flat_range_cleared(&unmounted);
}

#[test]
fn confirmed_remove_clears_flat_range_when_confirmation_opens_and_closes() {
    let mut control = ranked_control(ranked_rows(4)).confirm_remove("Remove", |_| String::new());
    control.data_view.highlight_id(&1);
    seed_stale_flat_range(&mut control);
    let mut ctx = EventCtx::default();

    assert!(control.request_remove_confirmation(&mut ctx));
    assert_flat_range_cleared(&control);

    seed_stale_flat_range(&mut control);
    control.confirmation_event(
        &EventRoute::new(TreePath::new()),
        &TuiEvent::Key(KeyEvent::from(Key::Char('d'))),
        &mut ctx,
    );

    assert!(!control.is_confirming_remove());
    assert_flat_range_cleared(&control);
}

#[test]
fn cancelled_remove_clears_flat_range_when_confirmation_closes() {
    let mut control = ranked_control(ranked_rows(4)).confirm_remove("Remove", |_| String::new());
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();

    assert!(control.request_remove_confirmation(&mut ctx));
    seed_stale_flat_range(&mut control);
    control.confirmation_event(
        &EventRoute::new(TreePath::new()),
        &TuiEvent::Key(KeyEvent::from(Key::Char('c'))),
        &mut ctx,
    );

    assert!(!control.is_confirming_remove());
    assert_flat_range_cleared(&control);
}

#[test]
fn reorder_highlight_eases_in_stays_active_and_eases_out_after_commit() {
    let mut control =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
    let settings = AnimationSettings::default();
    let mut ctx = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.0);

    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);

    Animated::tick(&mut control.data_view, Duration::from_secs(1), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);

    control.handle_reorder_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.0);
}

#[test]
fn disabled_animations_snap_reorder_progress_between_inverse_and_normal() {
    let mut control = ranked_control([RankedRow { id: 1, rank: 10 }]);
    let mut settings = AnimationSettings::default();
    settings.enabled = false;
    let mut ctx = EventCtx::new(settings);

    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert!(control.is_reordering());
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);

    control.handle_reorder_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.0);
}

#[test]
fn rapid_same_row_reorder_reentry_reverses_from_current_progress() {
    let mut control =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
    let settings = AnimationSettings::default();
    let mut ctx = EventCtx::new(settings);

    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(
        control.data_view.reorder_highlight_progress_for_test(),
        0.75
    );
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 1.0);
}

#[test]
fn cancelling_midway_through_entry_reverses_from_current_progress() {
    let mut control = ranked_control([RankedRow { id: 1, rank: 10 }]);
    let settings = AnimationSettings::default();
    let mut ctx = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);

    control.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);

    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(
        control.data_view.reorder_highlight_progress_for_test(),
        0.25
    );
}

#[test]
fn focus_loss_cancels_reorder_directly_to_ordinary_unfocused_row() {
    let mut control =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
    let settings = AnimationSettings::default();
    let mut event = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut event,
    );
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);

    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 30, 4), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    let mut focus = FocusCtx::new(settings);
    control.dispatch_focus(&data_target, false, &mut focus);

    assert!(!control.is_reordering());
    assert!(focus.redraw_requested());
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.0);
    assert!(!control.data_view.row_has_reorder_highlight(&1));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderCancelled { row_id: 1 }]
    );
}

#[test]
fn focus_loss_during_reorder_exit_clears_presentation_without_duplicate_event() {
    let rows = [RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }];
    let mut control = ranked_control(rows.clone());
    let mut ordinary = ranked_control(rows);
    let settings = AnimationSettings::default();
    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 30, 2), &mut layout);
    ordinary.layout(Rect::new(0, 0, 30, 2), &mut LayoutCtx::new());
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    control.dispatch_focus(&data_target, true, &mut FocusCtx::new(settings));
    let mut event = EventCtx::new(settings);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut event,
    );
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(50), settings);
    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut event);
    Animated::tick(&mut control.data_view, Duration::from_millis(100), settings);
    Animated::tick(&mut control.data_view, Duration::from_millis(25), settings);
    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.5);

    let mut focus = FocusCtx::new(settings);
    control.dispatch_focus(&data_target, false, &mut focus);

    assert_eq!(control.data_view.reorder_highlight_progress_for_test(), 0.0);
    assert!(!control.data_view.row_has_reorder_highlight(&1));
    assert!(focus.redraw_requested());
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Reordered {
            row_ids: vec![1, 2]
        }]
    );

    let mut control_terminal =
        Terminal::new(TestBackend::new(30, 2)).expect("terminal should build");
    let mut ordinary_terminal =
        Terminal::new(TestBackend::new(30, 2)).expect("terminal should build");
    control_terminal
        .draw(|frame| control.data_view.render(frame, Rect::new(0, 0, 30, 2)))
        .expect("reordered data view should render");
    ordinary_terminal
        .draw(|frame| ordinary.data_view.render(frame, Rect::new(0, 0, 30, 2)))
        .expect("ordinary data view should render");
    assert_eq!(
        control_terminal.backend().buffer(),
        ordinary_terminal.backend().buffer()
    );
}

#[test]
fn hidden_reorder_column_does_not_change_visible_layout() {
    let control = ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);

    assert_eq!(
        control
            .data_view
            .visible_column_rects(Rect::new(0, 0, 30, 3), 1, 1)
            .len(),
        1
    );
}

#[test]
#[should_panic(expected = "must be reorderable")]
fn reorder_configuration_rejects_non_reorderable_column() {
    let _ = ListControl::<RankedRow, usize>::new([], |row| row.id, |_, _| unreachable!())
        .column(
            Column::text("rank", "Rank", Constraint::Fill(1), |row: &RankedRow| {
                row.rank.to_string()
            })
            .hidden(),
        )
        .reorderable_by("rank");
}

fn start_and_layout<T, Id>(control: &mut ListControl<T, Id>, area: Rect)
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    control.begin_add(None);
    control.layout(area, &mut LayoutCtx::new());
}

fn control_enter<T, Id>(control: &mut ListControl<T, Id>, ctx: &mut EventCtx<()>)
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    control.handle_control_key(
        KeyEvent::from(Key::Enter),
        &EventRoute::new(TreePath::new()),
        ctx,
    );
}

#[test]
fn measure_grows_by_rows_and_caps_with_headers_chrome_and_draft() {
    let proposal = LayoutProposal::unbounded();
    let mut empty = table(0).row_height(2).max_rows(3);
    let one = table(1).row_height(2).max_rows(3);
    let many = table(8).row_height(2).max_rows(3);

    assert_eq!(empty.measure(proposal).preferred.height, 5);
    assert_eq!(one.measure(proposal).preferred.height, 5);
    assert_eq!(many.measure(proposal).preferred.height, 9);
    empty.begin_add(None);
    assert_eq!(empty.measure(proposal).preferred.height, 7);
}

#[test]
fn measure_sums_mixed_data_row_heights_up_to_max_rows() {
    let control = table(4).row_height(3).max_rows(3);
    let mut control = control;
    control
        .data_view_mut()
        .set_row_height_by(|row: &Row| if row.0 % 2 == 0 { 1 } else { 2 });

    assert_eq!(
        control
            .measure(LayoutProposal::unbounded())
            .preferred
            .height,
        7
    );
    control.begin_add(None);
    assert_eq!(
        control
            .measure(LayoutProposal::unbounded())
            .preferred
            .height,
        9
    );
}

#[test]
fn measure_reserves_height_for_horizontal_scrollbar() {
    let mut control: ListControl<_, _, ()> = ListControl::list(
        [(1, "https://example.com/a/very/long/link".to_string())],
        |row: &(usize, String)| row.0,
        |row| row.1.clone(),
        |value, _| (2, value),
    );

    let height = control
        .measure(LayoutProposal::at_most(20, u16::MAX))
        .preferred
        .height;
    assert_eq!(height, 4);

    control.layout(Rect::new(0, 0, 20, height), &mut LayoutCtx::new());
    let geometry = control.data_view.scroll_geometry(control.data_area);
    assert!(geometry.layout.horizontal_bar.is_some());
    assert!(geometry.layout.vertical_bar.is_none());
}

fn mutable_tree_control(rows: impl IntoIterator<Item = TreeRow>) -> ListControl<TreeRow, usize> {
    ListControl::list(
        rows,
        |row: &TreeRow| row.id,
        |row| row.id.to_string(),
        |_, _| unreachable!("tree selection test does not add rows"),
    )
    .tree(TreeAdapter::mutable_parent_id(
        |row: &TreeRow| row.parent,
        |row, parent| row.parent = parent,
    ))
}

fn tree_block_move_control() -> ListControl<TreeRow, usize> {
    let mut control = tree_selection_control();
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control
}

fn tree_selection_control() -> ListControl<TreeRow, usize> {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_tree_selection_key(
        modified_key(Key::Char(' '), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control
}

#[test]
fn ctrl_navigation_does_not_clear_tree_selection_without_quick_move() {
    let mut control = tree_block_move_control();
    let selected = control
        .tree_selection
        .as_ref()
        .expect("selection should be active")
        .selected
        .clone();

    assert_eq!(
        control.handle_quick_tree_move(modified_key(Key::Down, KeyModifiers::CONTROL)),
        None
    );
    assert_eq!(
        control
            .tree_selection
            .as_ref()
            .expect("selection should remain active")
            .selected,
        selected
    );
}

#[test]
fn block_move_start_does_not_start_single_row_reorder() {
    let control = tree_block_move_control();

    assert!(control.tree_block_move.is_some());
    assert!(control.tree_reorder.is_none());
    assert!(control.reorder.is_none());
}

#[test]
fn tree_block_line_move_redraws_without_relayout() {
    let mut control = tree_block_move_control();
    let mut ctx = EventCtx::default();

    control.handle_reorder_key(KeyEvent::from(Key::Char('j')), &mut ctx);

    assert!(ctx.redraw_requested());
    assert!(!ctx.layout_requested());
}

#[test]
fn tree_block_move_supports_page_and_edge_navigation_keys() {
    for (keys, expected_target) in [
        (vec![KeyEvent::from(Key::Home)], 0),
        (
            vec![
                KeyEvent::from(Key::Char('g')),
                KeyEvent::from(Key::Char('g')),
            ],
            0,
        ),
        (vec![modified_key(Key::Char('u'), KeyModifiers::CONTROL)], 0),
        (vec![KeyEvent::from(Key::PageUp)], 0),
        (vec![KeyEvent::from(Key::End)], 1),
        (vec![modified_key(Key::Char('G'), KeyModifiers::SHIFT)], 1),
    ] {
        let mut control = tree_block_move_control();
        let mut ctx = EventCtx::default();

        for key in keys {
            control.handle_reorder_key(key, &mut ctx);
        }

        assert_eq!(
            control
                .tree_block_move
                .as_ref()
                .expect("tree block move should remain active")
                .sibling_index,
            expected_target
        );
    }

    for key in [
        modified_key(Key::Char('d'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::PageDown),
    ] {
        let mut control = tree_block_move_control();
        let mut ctx = EventCtx::default();
        control.handle_reorder_key(KeyEvent::from(Key::Home), &mut ctx);
        control.handle_reorder_key(key, &mut ctx);

        assert_eq!(
            control
                .tree_block_move
                .as_ref()
                .expect("tree block move should remain active")
                .sibling_index,
            1
        );
    }
}

#[test]
fn tree_block_move_steps_before_and_after_a_contiguous_source_block() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
        TreeRow {
            id: 4,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&2);
    let mut ctx = EventCtx::default();
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    let render_rows = |control: &ListControl<TreeRow, usize>| {
        let mut terminal = Terminal::new(TestBackend::new(30, 5)).expect("terminal should build");
        terminal
            .draw(|frame| control.data_view().render(frame, Rect::new(0, 0, 30, 5)))
            .expect("tree block move should render");
        (0..5)
            .map(|y| {
                (0..30)
                    .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        render_rows(&control),
        ["1", "2", "3", "Moving 2 tasks", "4"]
    );

    control.handle_reorder_key(KeyEvent::from(Key::Char('k')), &mut ctx);
    assert_eq!(
        render_rows(&control),
        ["1", "2", "Moving 2 tasks", "3", "4"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('j'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["1", "2", "3", "Moving 2 tasks", "4"]
    );

    control.handle_reorder_key(KeyEvent::from(Key::Char('k')), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Char('k')), &mut ctx);
    assert_eq!(
        render_rows(&control),
        ["1", "Moving 2 tasks", "2", "3", "4"]
    );

    control.handle_reorder_key(
        modified_key(Key::Char('j'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert_eq!(
        render_rows(&control),
        ["1", "2", "Moving 2 tasks", "3", "4"]
    );
}

#[test]
fn one_item_tree_range_consumes_block_commands() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
    ]);
    control.tree_selection = Some(TreeSelectionState {
        selected: vec![1],
        anchor: None,
        range_mode: true,
    });

    for key in [
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::Char(' ')),
    ] {
        let mut ctx = EventCtx::default();

        assert_eq!(
            control.handle_reorder_key(key, &mut ctx),
            Some(EventOutcome::Handled)
        );
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(!control.is_reordering());
    }
}

#[test]
fn block_move_commit_and_cancel_clear_reorder_state() {
    for finish in [Key::Enter, Key::Esc] {
        let mut control = tree_block_move_control();

        control.handle_reorder_key(KeyEvent::from(finish), &mut EventCtx::default());

        assert!(!control.is_reordering());
    }
}

#[test]
fn tree_block_move_teardown_clears_modal_state() {
    let settings = AnimationSettings::default();
    let mut focus_control = tree_block_move_control();
    let mut layout = LayoutCtx::new();
    focus_control.layout(Rect::new(0, 0, 30, 5), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    let mut focus = FocusCtx::new(settings);

    focus_control.dispatch_focus(&data_target, false, &mut focus);

    assert!(!focus_control.is_reordering());
    assert!(focus.layout_requested());

    let mut mutable_control = tree_block_move_control();
    let _ = mutable_control.data_view_mut();
    assert!(!mutable_control.is_reordering());
    assert!(matches!(
        mutable_control.take_events().as_slice(),
        [ListControlEvent::TreeBlockMoveCancelled { .. }]
    ));

    let mut unmounted_control = tree_block_move_control();
    unmounted_control.unmount(&mut LifecycleCtx::default());
    assert!(!unmounted_control.is_reordering());
}

#[test]
fn tree_block_move_h_and_l_reparent_selected_roots() {
    let mut outdent = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: Some(1),
        },
        TreeRow {
            id: 3,
            parent: Some(1),
        },
        TreeRow {
            id: 4,
            parent: None,
        },
    ]);
    outdent.data_view_mut().expand_tree_row(1);
    outdent.data_view.highlight_id(&2);
    let mut ctx = EventCtx::default();
    outdent.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    outdent.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    outdent.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    outdent.handle_reorder_key(KeyEvent::from(Key::Char('h')), &mut ctx);
    assert_eq!(
        outdent
            .tree_block_move
            .as_ref()
            .expect("block move should remain active")
            .parent_id,
        None
    );
    outdent.handle_reorder_key(KeyEvent::from(Key::Char('j')), &mut ctx);
    outdent.handle_reorder_key(KeyEvent::from(Key::Char('k')), &mut ctx);
    outdent.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);
    assert_eq!(
        outdent.take_events(),
        vec![ListControlEvent::TreeBlockMoved {
            row_ids: vec![2, 3],
            parent_id: None,
            sibling_index: 1,
        }]
    );
    assert_eq!(
        outdent
            .items()
            .iter()
            .map(|row| (row.id, row.parent))
            .collect::<Vec<_>>(),
        vec![(1, None), (2, None), (3, None), (4, None)]
    );

    let mut indent = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
    ]);
    indent.data_view.highlight_id(&2);
    indent.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    indent.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    indent.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    indent.handle_reorder_key(KeyEvent::from(Key::Char('l')), &mut ctx);
    indent.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);
    assert_eq!(
        indent
            .items()
            .iter()
            .map(|row| (row.id, row.parent))
            .collect::<Vec<_>>(),
        vec![(1, None), (2, Some(1)), (3, Some(1))]
    );
}

#[test]
fn tree_block_placeholder_indented_under_root_has_depth_one() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&2);
    let mut ctx = EventCtx::default();
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );

    control.handle_reorder_key(KeyEvent::from(Key::Char('l')), &mut ctx);

    let depth = control
        .data_view
        .selection_placeholder_depth_for_test()
        .expect("tree block move should create a placeholder");
    assert_eq!(depth, 1);

    let mut terminal = Terminal::new(TestBackend::new(30, 4)).expect("terminal should build");
    terminal
        .draw(|frame| control.data_view().render(frame, Rect::new(0, 0, 30, 4)))
        .expect("tree block move should render");
    let rows = (0..4)
        .map(|y| {
            (0..30)
                .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(rows, ["  1", "    Moving 2 tasks", "  2", "  3"]);
}

#[test]
fn tree_block_indent_expands_collapsed_target_until_commit_or_cancel() {
    let rows = [
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 4,
            parent: Some(1),
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
    ];
    let mut committed = mutable_tree_control(rows.clone());
    committed.data_view.highlight_id(&2);
    let mut ctx = EventCtx::default();
    committed.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    committed.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    committed.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    committed.handle_reorder_key(KeyEvent::from(Key::Char('l')), &mut ctx);

    assert!(committed.data_view.tree_expansion_snapshot().contains(&1));
    assert_eq!(
        committed
            .tree_block_move
            .as_ref()
            .expect("block move should remain active")
            .parent_id,
        Some(1)
    );

    committed.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);
    assert!(committed.data_view.tree_expansion_snapshot().contains(&1));
    assert_eq!(committed.data_view.highlighted_id(), Some(2));

    let mut cancelled = mutable_tree_control(rows);
    cancelled.data_view.highlight_id(&2);
    cancelled.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    cancelled.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    cancelled.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    cancelled.handle_reorder_key(KeyEvent::from(Key::Char('l')), &mut ctx);
    cancelled.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);

    assert!(!cancelled.data_view.tree_expansion_snapshot().contains(&1));
}

#[test]
fn mutable_data_view_access_clears_pre_move_tree_selection() {
    let mut control = tree_selection_control();

    control.data_view_mut().set_visible_row_ids([1]);

    assert!(control.tree_selection.is_none());
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
    assert!(!control.is_reordering());
}

#[test]
fn block_move_start_clears_selection_when_it_becomes_unavailable() {
    for key in [
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        KeyEvent::from(Key::Char(' ')),
    ] {
        let mut control = tree_selection_control();
        control.data_view.set_visible_row_ids([1]);

        control.handle_reorder_key(key, &mut EventCtx::default());

        assert!(
            control.tree_selection.is_none(),
            "{key:?} should clear selection"
        );
        assert!(
            !control.is_reordering(),
            "{key:?} should not start block move"
        );
    }
}

#[test]
fn tree_ctrl_navigation_selects_the_origin_and_space_toggles_current_siblings() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
        TreeRow {
            id: 4,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&2);
    let mut ctx = EventCtx::default();

    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    control.handle_tree_selection_key(KeyEvent::from(Key::Char(' ')), &mut ctx);
    assert_eq!(control.data_view.highlighted_id(), Some(4));
    assert_eq!(
        control
            .tree_selection
            .as_ref()
            .expect("ctrl selection should remain active")
            .selected,
        vec![2, 4]
    );
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert_eq!(
        control.items().iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3, 2, 4]
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::TreeBlockMoved {
            row_ids: vec![2, 4],
            parent_id: None,
            sibling_index: 2,
        }]
    );
}

#[test]
fn tree_shift_selection_stays_within_sibling_parent() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: Some(1),
        },
        TreeRow {
            id: 3,
            parent: Some(1),
        },
        TreeRow {
            id: 4,
            parent: None,
        },
    ])
    .expanded([1]);
    control.data_view.highlight_id(&2);

    control.handle_tree_selection_key(
        modified_key(Key::Down, KeyModifiers::SHIFT),
        &mut EventCtx::default(),
    );

    assert_eq!(
        control
            .tree_selection
            .as_ref()
            .expect("shift starts selection")
            .selected,
        vec![2, 3]
    );
    assert_eq!(control.data_view.highlighted_id(), Some(3));
}

#[test]
fn tree_range_selection_survives_focus_loss_and_gain() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
        TreeRow {
            id: 3,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&1);
    control.handle_tree_selection_key(
        modified_key(Key::Down, KeyModifiers::SHIFT),
        &mut EventCtx::default(),
    );
    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 30, 5), &mut layout);
    let data_target = layout
        .focus_targets()
        .iter()
        .find(|target| target.path == TreePath::from_keys([ChildKey::new(DATA_SLOT)]))
        .expect("data focus target should exist")
        .clone();
    let settings = AnimationSettings::default();

    control.dispatch_focus(&data_target, false, &mut FocusCtx::new(settings));
    control.dispatch_focus(&data_target, true, &mut FocusCtx::new(settings));

    assert_eq!(
        control
            .tree_selection
            .as_ref()
            .expect("tree selection should remain active")
            .selected,
        vec![1, 2]
    );
    assert!(control.data_view.selection_overlay_active_for_test());
    assert!(!control.is_reordering());
}

#[test]
fn plain_data_view_navigation_clears_tree_range_selection() {
    let navigation_keys = [
        KeyEvent::from(Key::Up),
        KeyEvent::from(Key::Down),
        KeyEvent::from(Key::Left),
        KeyEvent::from(Key::Right),
        KeyEvent::from(Key::PageUp),
        KeyEvent::from(Key::PageDown),
        KeyEvent::from(Key::Home),
        KeyEvent::from(Key::End),
        KeyEvent::from(Key::Char('g')),
        modified_key(Key::Char('g'), KeyModifiers::SHIFT),
    ];

    for key in navigation_keys {
        let mut control = mutable_tree_control([
            TreeRow {
                id: 1,
                parent: None,
            },
            TreeRow {
                id: 2,
                parent: None,
            },
        ]);
        control.data_view.highlight_id(&1);
        let mut ctx = EventCtx::default();
        control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);

        control.handle_tree_selection_key(key, &mut ctx);

        assert!(
            control.tree_selection.is_none(),
            "{key:?} should clear range"
        );
    }
}

#[test]
fn plain_navigation_keeps_tree_ctrl_selection() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::CONTROL), &mut ctx);
    let selected = control
        .tree_selection
        .as_ref()
        .expect("ctrl selection should be active")
        .selected
        .clone();

    control.handle_tree_selection_key(KeyEvent::from(Key::PageDown), &mut ctx);

    assert_eq!(
        control
            .tree_selection
            .as_ref()
            .expect("ctrl selection should remain active")
            .selected,
        selected
    );
}

#[test]
fn escape_clears_tree_selection_after_it_becomes_unavailable() {
    let mut control = mutable_tree_control([
        TreeRow {
            id: 1,
            parent: None,
        },
        TreeRow {
            id: 2,
            parent: None,
        },
    ]);
    control.data_view.highlight_id(&1);
    let mut ctx = EventCtx::default();
    control.handle_tree_selection_key(modified_key(Key::Down, KeyModifiers::SHIFT), &mut ctx);
    control.data_view.set_visible_row_ids([1]);

    let outcome = control.handle_tree_selection_key(KeyEvent::from(Key::Esc), &mut ctx);

    assert_eq!(outcome, Some(EventOutcome::Handled));
    assert!(control.tree_selection.is_none());
}

#[test]
fn adding_past_max_rows_keeps_height_capped_and_reveals_new_row() {
    let rows = (0..5).map(|index| (index, format!("Item {index}")));
    let mut control = ListControl::list(
        rows,
        |row: &(usize, String)| row.0,
        |row| row.1.clone(),
        |name, _| (99, name),
    )
    .max_rows(3);
    let proposal = LayoutProposal::unbounded();
    let capped_height = control.measure(proposal).preferred.height;
    start_and_layout(&mut control, Rect::new(0, 0, 40, capped_height));
    let ListControlInput::Text(input) = &mut control.inputs[0] else {
        panic!("field should be text");
    };
    input.set_value("Newest");

    control_enter(&mut control, &mut EventCtx::default());
    control.tick(Duration::from_secs(1), AnimationSettings::default());

    assert_eq!(control.measure(proposal).preferred.height, capped_height);
    assert_eq!(control.data_view.highlighted_id(), Some(99));
    assert!(control.data_view.vertical_scroll_offset_for_test() > 0);
}
