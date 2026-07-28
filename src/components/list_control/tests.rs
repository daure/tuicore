use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    Animated, AnimationSettings, FocusCtx, KeyModifiers, LayoutCtx, LayoutProposal, ScrollOffset,
    TuiNode,
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

fn layout_adding(control: &mut ListControl<Row, usize>, area: Rect) {
    control.begin_add();
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
fn default_remove_binding_is_plain_x_only() {
    let bindings = ListControlKeyBindings::default();

    assert_eq!(bindings.remove, vec![KeySpec::plain('x')]);
}

#[test]
fn default_reorder_binding_requires_control_m() {
    let mut control = ranked_control([RankedRow { id: 1, rank: 10 }]);
    let mut ctx = EventCtx::default();

    assert!(
        control
            .handle_reorder_key(KeyEvent::from(Key::Char('m')), &mut ctx)
            .is_none()
    );
    assert!(!control.is_reordering());

    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(control.is_reordering());
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
fn large_reorder_moves_preserve_other_rows_and_commit_or_cancel_staged_order() {
    let mut committed = ranked_control(ranked_rows(6));
    let mut ctx = EventCtx::default();
    start_reordering(&mut committed, 1);
    committed.handle_reorder_key(KeyEvent::from(Key::End), &mut ctx);
    committed.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);
    assert_eq!(
        committed.take_events(),
        vec![ListControlEvent::Reordered {
            row_ids: vec![0, 2, 3, 4, 5, 1],
        }]
    );

    let mut cancelled = ranked_control(ranked_rows(6));
    start_reordering(&mut cancelled, 4);
    cancelled.handle_reorder_key(KeyEvent::from(Key::Home), &mut ctx);
    cancelled.handle_reorder_key(KeyEvent::from(Key::Esc), &mut ctx);
    assert_eq!(
        cancelled
            .items()
            .iter()
            .map(|row| row.rank)
            .collect::<Vec<_>>(),
        vec![0, 10, 20, 30, 40, 50]
    );
    assert_eq!(
        cancelled.take_events(),
        vec![ListControlEvent::ReorderCancelled { row_id: 4 }]
    );
}

#[test]
fn configured_reorder_stages_movement_and_commit_rewrites_only_rank_properties() {
    let mut control = ranked_control([
        RankedRow { id: 1, rank: 10 },
        RankedRow { id: 2, rank: 20 },
        RankedRow { id: 3, rank: 30 },
    ]);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(control.is_reordering());

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Char('j')), &mut ctx);
    assert_eq!(control.data_view.highlighted_id(), Some(1));
    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert!(!control.is_reordering());
    assert_eq!(
        control.items().iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        control
            .items()
            .iter()
            .map(|row| row.rank)
            .collect::<Vec<_>>(),
        vec![30, 10, 20]
    );
    assert_eq!(control.data_view.highlighted_id(), Some(1));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Reordered {
            row_ids: vec![2, 3, 1]
        }]
    );
}

#[test]
fn reorder_completion_leaves_consumer_panel_title_mutations_untouched() {
    for completion in [Key::Enter, Key::Esc] {
        let mut control =
            ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
        control.panel_mut().set_bottom_left("Before");
        let mut ctx = EventCtx::default();
        control.handle_reorder_key(
            modified_key(Key::Char('m'), KeyModifiers::CONTROL),
            &mut ctx,
        );
        control.panel_mut().set_bottom_left("Consumer update");

        control.handle_reorder_key(KeyEvent::from(completion), &mut ctx);

        assert_eq!(
            control
                .panel_ref()
                .title_text(PanelTitlePosition::BottomLeft),
            Some("Consumer update")
        );
    }
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
fn reorder_cancel_restores_order_and_unconfigured_key_propagates() {
    let mut plain = table(2);
    assert!(
        plain
            .handle_reorder_key(
                modified_key(Key::Char('m'), KeyModifiers::CONTROL),
                &mut EventCtx::default()
            )
            .is_none()
    );

    for cancel in [
        KeyEvent::from(Key::Esc),
        modified_key(Key::Char('['), KeyModifiers::CONTROL),
    ] {
        let mut control =
            ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
        let mut ctx = EventCtx::default();
        control.handle_reorder_key(
            modified_key(Key::Char('m'), KeyModifiers::CONTROL),
            &mut ctx,
        );
        control.handle_reorder_key(KeyEvent::from(Key::Up), &mut ctx);
        control.handle_reorder_key(cancel, &mut ctx);
        assert!(!control.is_reordering());
        assert_eq!(
            control
                .panel_ref()
                .title_text(PanelTitlePosition::BottomLeft),
            None
        );
        assert_eq!(control.items()[0].rank, 10);
        assert_eq!(
            control.take_events(),
            vec![ListControlEvent::ReorderCancelled { row_id: 1 }]
        );
    }
}

#[test]
fn reorder_rejects_duplicate_rank_keys_and_blocks_other_actions_while_active() {
    let mut duplicate =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 10 }]);
    duplicate.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
    assert!(!duplicate.is_reordering());
    assert_eq!(
        duplicate.take_events(),
        vec![ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DuplicateRankKeys
        }]
    );

    let mut control =
        ranked_control([RankedRow { id: 1, rank: 10 }, RankedRow { id: 2, rank: 20 }]);
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.handle_reorder_key(KeyEvent::from(Key::Char('+')), &mut ctx);
    assert!(control.is_reordering());
    assert!(!control.is_adding());
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

#[test]
#[should_panic(expected = "mutually exclusive")]
fn automatic_sort_and_reorder_configuration_are_exclusive() {
    let _ = ListControl::<RankedRow, usize>::new(
        [RankedRow { id: 1, rank: 10 }],
        |row| row.id,
        |_, _| unreachable!(),
    )
    .column(
        Column::text("rank", "Rank", Constraint::Fill(1), |row: &RankedRow| {
            row.rank.to_string()
        })
        .sortable(|row| row.rank)
        .reorderable(|row| row.rank, |row, rank| row.rank = rank),
    )
    .sorted_by("rank", SortDirection::Ascending)
    .reorderable_by("rank");
}

#[test]
#[should_panic(expected = "mutually exclusive")]
fn reorder_and_automatic_sort_configuration_are_exclusive() {
    let _ =
        ranked_control([RankedRow { id: 1, rank: 10 }]).sorted_by("rank", SortDirection::Ascending);
}

fn key(code: Key) -> TuiEvent {
    TuiEvent::Key(KeyEvent::from(code))
}

fn dropdown_control(
    fields: impl IntoIterator<Item = ListControlField>,
    submitted: Arc<Mutex<Vec<Vec<String>>>>,
) -> ListControl<Vec<String>, usize> {
    ListControl::new_fields(
        Vec::<Vec<String>>::new(),
        |row| row.len(),
        fields,
        move |values, _| {
            submitted
                .lock()
                .expect("submission lock")
                .push(values.clone());
            values
        },
    )
    .column(Column::text(
        "value",
        "",
        Constraint::Percentage(100),
        |row: &Vec<String>| row.join(" / "),
    ))
}

fn start_and_layout<T, Id>(control: &mut ListControl<T, Id>, area: Rect)
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    control.begin_add();
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

fn active_field_event<T, Id>(
    control: &mut ListControl<T, Id>,
    event: &TuiEvent,
    ctx: &mut EventCtx<()>,
) where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    let route = EventRoute::new(TreePath::from_keys([ListControl::<T, Id>::input_slot(
        control.active_field,
    )]));
    control.dispatch_event(&route, event, ctx);
}

#[test]
fn dropdown_add_opens_and_selection_enter_submits_immediately() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut control = dropdown_control(
        [ListControlField::dropdown("Owner", ["Ada", "Grace"])],
        Arc::clone(&submitted),
    );
    start_and_layout(&mut control, Rect::new(0, 0, 40, 8));
    let mut ctx = EventCtx::default();

    assert!(control.inputs[0].dropdown_is_open());
    active_field_event(&mut control, &key(Key::Enter), &mut ctx);
    assert!(!control.inputs[0].dropdown_is_open());

    assert_eq!(
        submitted.lock().expect("submission lock").as_slice(),
        &[vec!["Ada".to_string()]]
    );
    assert_eq!(control.items(), &[vec!["Ada".to_string()]]);
}

#[test]
fn mixed_fields_submit_ordered_values_after_each_confirmation() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut control = dropdown_control(
        [
            ListControlField::text("Entity"),
            ListControlField::dropdown("Owner", ["Ada", "Grace"]),
            ListControlField::dropdown("State", ["Ready", "Paused"]),
        ],
        Arc::clone(&submitted),
    );
    start_and_layout(&mut control, Rect::new(0, 0, 50, 8));
    let ListControlInput::Text(input) = &mut control.inputs[0] else {
        panic!("first field should be text");
    };
    input.set_value("Gateway");
    let mut ctx = EventCtx::default();

    control_enter(&mut control, &mut ctx);
    assert_eq!(control.active_field, 1);
    assert!(control.inputs[1].dropdown_is_open());
    active_field_event(&mut control, &key(Key::Enter), &mut ctx);
    assert_eq!(control.active_field, 2);
    assert!(control.inputs[2].dropdown_is_open());
    active_field_event(&mut control, &key(Key::Enter), &mut ctx);

    assert_eq!(
        submitted.lock().expect("submission lock").as_slice(),
        &[vec![
            "Gateway".to_string(),
            "Ada".to_string(),
            "Ready".to_string()
        ]]
    );
}

#[test]
fn escape_closes_dropdown_and_cancels_draft() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut control = dropdown_control([ListControlField::dropdown("Owner", ["Ada"])], submitted);
    start_and_layout(&mut control, Rect::new(0, 0, 40, 8));
    let mut ctx = EventCtx::default();
    active_field_event(&mut control, &key(Key::Esc), &mut ctx);
    assert!(!control.is_adding());
    assert_eq!(control.take_events(), vec![ListControlEvent::AddCancelled]);
}

#[test]
fn submitted_dropdown_is_clear_for_next_add() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut control = dropdown_control([ListControlField::dropdown("Owner", ["Ada"])], submitted);
    start_and_layout(&mut control, Rect::new(0, 0, 40, 8));
    let mut ctx = EventCtx::default();
    active_field_event(&mut control, &key(Key::Enter), &mut ctx);

    control.begin_add();
    assert!(control.inputs[0].value().is_empty());
}

#[test]
fn internal_field_transition_does_not_cancel_draft() {
    let submitted = Arc::new(Mutex::new(Vec::new()));
    let mut control = dropdown_control(
        [
            ListControlField::text("Entity"),
            ListControlField::dropdown("Owner", ["Ada"]),
        ],
        submitted,
    );
    start_and_layout(&mut control, Rect::new(0, 0, 40, 8));
    let ListControlInput::Text(input) = &mut control.inputs[0] else {
        panic!("first field should be text");
    };
    input.set_value("Gateway");
    control_enter(&mut control, &mut EventCtx::default());

    let mut layout = LayoutCtx::new();
    control.layout(Rect::new(0, 0, 40, 8), &mut layout);
    assert!(layout.focus_targets().iter().all(|target| {
        target.path != TreePath::from_keys([ListControl::<Vec<String>, usize>::input_slot(0)])
    }));
    assert!(control.is_adding());
    assert!(control.take_events().is_empty());
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
    empty.begin_add();
    assert_eq!(empty.measure(proposal).preferred.height, 7);
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
