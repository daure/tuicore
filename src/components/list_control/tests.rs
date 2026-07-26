use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{AnimationSettings, KeyModifiers, LayoutCtx, LayoutProposal, TuiNode};

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

#[test]
fn configured_reorder_stages_movement_and_commit_rewrites_only_rank_properties() {
    let mut control = ranked_control([
        RankedRow { id: 1, rank: 10 },
        RankedRow { id: 2, rank: 20 },
        RankedRow { id: 3, rank: 30 },
    ]);
    control.panel_mut().set_bottom_left(" Ready ");
    let mut ctx = EventCtx::default();
    control.handle_reorder_key(
        modified_key(Key::Char('m'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(control.is_reordering());
    assert_eq!(
        control
            .panel_ref()
            .title_text(PanelTitlePosition::BottomLeft),
        Some("Moving")
    );

    control.handle_reorder_key(KeyEvent::from(Key::Down), &mut ctx);
    control.handle_reorder_key(KeyEvent::from(Key::Char('j')), &mut ctx);
    assert_eq!(control.data_view.highlighted_id(), Some(1));
    control.handle_reorder_key(KeyEvent::from(Key::Enter), &mut ctx);

    assert!(!control.is_reordering());
    assert_eq!(
        control
            .panel_ref()
            .title_text(PanelTitlePosition::BottomLeft),
        Some(" Ready ")
    );
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
