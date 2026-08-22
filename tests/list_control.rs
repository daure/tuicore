use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::Modifier;
use tuicore::{
    AnimationSettings, ChildKey, Column, ConfirmationDialogKeyBindings, DataView, DispatchEffects,
    EventCtx, EventRoute, FocusCtx, FocusManager, FocusRequest, HotkeyEvent, Key, KeyEvent,
    KeyModifiers, KeySpec, LayoutCtx, LayoutEngine, ListControl, ListControlEvent,
    ListControlField, ListControlKeyBindings, ListControlReorderUnavailable, Panel, RenderCtx,
    SortDirection, TreeAdapter, TreeDispatcher, TreePath, TuiEvent, TuiNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    id: usize,
    name: String,
}

fn control(rows: impl IntoIterator<Item = Row>) -> ListControl<Row, usize> {
    ListControl::new(
        rows,
        |row| row.id,
        |name, rows| Row {
            id: rows.iter().map(|row| row.id).max().unwrap_or(0) + 1,
            name,
        },
    )
    .column(Column::text(
        "name",
        "Name",
        Constraint::Percentage(100),
        |row: &Row| row.name.clone(),
    ))
    .hotkey("l")
}

fn data_route() -> EventRoute {
    EventRoute::new(TreePath::from_keys([ChildKey::new("data")]))
}

fn input_route() -> EventRoute {
    EventRoute::new(TreePath::from_keys([ChildKey::new("add-input")]))
}

fn field_route(index: usize) -> EventRoute {
    let slot = if index == 0 {
        "add-input".to_string()
    } else {
        format!("add-input-{index}")
    };
    EventRoute::new(TreePath::from_keys([ChildKey::new(slot)]))
}

fn multi_control() -> ListControl<(usize, String, String, String), usize> {
    ListControl::new_fields(
        [],
        |row: &(usize, String, String, String)| row.0,
        [
            ListControlField::text("Entity"),
            ListControlField::text("Owner"),
            ListControlField::text("State"),
        ],
        |values, rows| {
            let mut values = values.into_iter();
            (
                rows.len() + 1,
                values.next().expect("entity exists"),
                values.next().expect("owner exists"),
                values.next().expect("state exists"),
            )
        },
    )
}

fn dropdown_control() -> ListControl<Row, usize> {
    ListControl::new_fields(
        [],
        |row: &Row| row.id,
        [ListControlField::dropdown("Name", ["Ada", "Grace"])],
        |mut values, rows| Row {
            id: rows.len() + 1,
            name: values.remove(0),
        },
    )
    .column(Column::text(
        "name",
        "Name",
        Constraint::Percentage(100),
        |row: &Row| row.name.clone(),
    ))
}

fn editable_control() -> ListControl<(usize, String, String), usize> {
    ListControl::new_fields(
        [
            (1, "Ada".into(), "Ready".into()),
            (2, "Grace".into(), "Paused".into()),
        ],
        |row: &(usize, String, String)| row.0,
        [
            ListControlField::text("Name"),
            ListControlField::dropdown("State", ["Ready", "Paused"]),
        ],
        |values, rows| (rows.len() + 1, values[0].clone(), values[1].clone()),
    )
    .editable(
        |row| vec![row.1.clone(), row.2.clone()],
        |row, values| {
            row.1.clone_from(&values[0]);
            row.2.clone_from(&values[1]);
        },
    )
}

fn edit_key() -> TuiEvent {
    key(Key::Char('e'), KeyModifiers::NONE)
}

struct ListRuntime {
    control: ListControl<Row, usize>,
    layout: LayoutEngine,
    focus: FocusManager,
    dispatcher: TreeDispatcher,
    area: Rect,
}

impl ListRuntime {
    fn new() -> Self {
        let area = Rect::new(0, 0, 40, 8);
        let mut runtime = Self {
            control: dropdown_control(),
            layout: LayoutEngine::new(),
            focus: FocusManager::new(),
            dispatcher: TreeDispatcher::new(),
            area,
        };
        runtime.layout.layout(&mut runtime.control, area);
        let request = FocusRequest::TargetAt {
            path: data_route().path,
            id: tuicore::FocusId::new("data-view"),
        };
        runtime.apply_focus_request(request);
        runtime
    }

    fn current_id(&self) -> Option<&str> {
        self.focus.current().map(|target| target.id.as_str())
    }

    fn dispatch(&mut self, event: TuiEvent) {
        let route = EventRoute::new(self.focus.current_path());
        self.dispatch_at(route, event);
    }

    fn dispatch_at(&mut self, route: EventRoute, event: TuiEvent) {
        let effects: DispatchEffects<()> = self.dispatcher.dispatch_event(
            &mut self.control,
            &route,
            &event,
            AnimationSettings::default(),
        );
        self.reconcile(effects.layout, effects.focus_request);
    }

    fn reconcile(&mut self, layout_requested: bool, focus_request: Option<FocusRequest>) {
        if layout_requested {
            self.layout.layout(&mut self.control, self.area);
        }
        if let Some(request) = focus_request {
            self.apply_focus_request(request);
        } else if layout_requested {
            self.validate_focus();
        }
    }

    fn apply_focus_request(&mut self, request: FocusRequest) {
        if let Some(transition) = self
            .focus
            .apply_request(&request, self.layout.focus_targets())
        {
            let effects: DispatchEffects<()> = self.dispatcher.dispatch_focus(
                &mut self.control,
                transition,
                AnimationSettings::default(),
            );
            if effects.layout {
                self.layout.layout(&mut self.control, self.area);
                self.validate_focus();
            }
        }
    }

    fn validate_focus(&mut self) {
        if let Some(transition) = self.focus.validate(self.layout.focus_targets()) {
            let effects: DispatchEffects<()> = self.dispatcher.dispatch_focus(
                &mut self.control,
                transition,
                AnimationSettings::default(),
            );
            if effects.layout {
                self.layout.layout(&mut self.control, self.area);
            }
        }
    }
}

fn key(code: Key, modifiers: KeyModifiers) -> TuiEvent {
    TuiEvent::Key(KeyEvent { code, modifiers })
}

fn reorder_key() -> TuiEvent {
    key(Key::Char('m'), KeyModifiers::CONTROL)
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
            .filter_key(|row| row.rank.to_string())
            .sortable(|row| row.rank)
            .reorderable(|row| row.rank, |row, rank| row.rank = rank)
            .hidden(),
            Column::text("id", "ID", Constraint::Fill(1), |row: &RankedRow| {
                row.id.to_string()
            }),
        ])
        .reorderable_by("rank")
}

fn ranked_rows() -> [RankedRow; 3] {
    [
        RankedRow { id: 1, rank: 10 },
        RankedRow { id: 2, rank: 20 },
        RankedRow { id: 3, rank: 30 },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeRow {
    id: usize,
    parent: Option<usize>,
    name: String,
}

fn tree_control() -> ListControl<TreeRow, usize> {
    ListControl::list(
        [
            TreeRow {
                id: 1,
                parent: None,
                name: "Release".into(),
            },
            TreeRow {
                id: 2,
                parent: Some(1),
                name: "Test".into(),
            },
            TreeRow {
                id: 3,
                parent: Some(1),
                name: "Package".into(),
            },
            TreeRow {
                id: 4,
                parent: None,
                name: "Publish".into(),
            },
        ],
        |row| row.id,
        |row| row.name.clone(),
        |name, rows| TreeRow {
            id: rows.iter().map(|row| row.id).max().unwrap_or(0) + 1,
            parent: None,
            name,
        },
    )
    .tree(TreeAdapter::mutable_parent_id(
        |row: &TreeRow| row.parent,
        |row, parent| row.parent = parent,
    ))
    .expanded([1, 4])
}

#[test]
fn backslash_adds_child_to_highlighted_row() {
    let mut control = tree_control();
    control.data_view_mut().highlight_id(&1);

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('\\'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert!(control.is_adding());
    control.dispatch_event(
        &input_route(),
        &TuiEvent::Paste("Document".into()),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(control.items().last().map(|row| row.parent), Some(Some(1)));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::AddedChild {
            row_id: 5,
            parent_id: 1,
        }]
    );
}

#[test]
fn plus_adds_sibling_with_same_parent_as_highlighted_row() {
    let mut control = tree_control();
    control.data_view_mut().highlight_id(&2);

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('+'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &input_route(),
        &TuiEvent::Paste("Document".into()),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(control.items().last().map(|row| row.parent), Some(Some(1)));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::AddedChild {
            row_id: 5,
            parent_id: 1,
        }]
    );
}

#[test]
fn tree_move_mode_supports_arrows_and_hjkl_reparenting() {
    let mut control = tree_control();
    control.data_view_mut().highlight_id(&3);
    control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    control.dispatch_event(
        &data_route(),
        &key(Key::Up, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('j'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('l'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .parent,
        Some(2)
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::TreeMoved {
            row_id: 3,
            parent_id: Some(2),
            sibling_index: 0,
        }]
    );

    control.data_view_mut().highlight_id(&2);
    control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    control.dispatch_event(
        &data_route(),
        &key(Key::Left, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('j'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    let moved = control.items().iter().find(|row| row.id == 2).unwrap();
    assert_eq!(moved.parent, None);
    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .parent,
        Some(2)
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::TreeMoved {
            row_id: 2,
            parent_id: None,
            sibling_index: 2,
        }]
    );
}

#[test]
fn angle_keys_immediately_indent_and_outdent_highlighted_subtree() {
    let mut control = tree_control();
    control.data_view_mut().highlight_id(&3);

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('>'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .parent,
        Some(2)
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::TreeMoved {
            row_id: 3,
            parent_id: Some(2),
            sibling_index: 0,
        }]
    );

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('<'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .parent,
        Some(1)
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::TreeMoved {
            row_id: 3,
            parent_id: Some(1),
            sibling_index: 1,
        }]
    );
}

#[test]
fn canceling_tree_move_restores_parent_and_source_order() {
    let mut control = tree_control();
    let original = control.items().to_vec();
    control.data_view_mut().highlight_id(&2);
    control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('h'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Esc, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(control.items(), original);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderCancelled { row_id: 2 }]
    );
}

#[test]
fn tree_move_conflict_restores_staged_change_without_overwriting_external_parent_edit() {
    let mut control = tree_control();
    control.data_view_mut().highlight_id(&3);
    control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('l'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    control
        .data_view_mut()
        .update_row(&4, |row| row.parent = Some(1));

    control.dispatch_event(
        &data_route(),
        &key(Key::Down, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 3)
            .unwrap()
            .parent,
        Some(1)
    );
    assert_eq!(
        control
            .items()
            .iter()
            .find(|row| row.id == 4)
            .unwrap()
            .parent,
        Some(1)
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        }]
    );
}

#[test]
fn routed_reorder_entry_movement_commit_cancel_and_active_blocking() {
    for route in [EventRoute::new(TreePath::new()), data_route()] {
        let mut control = ranked_control(ranked_rows());
        assert_eq!(
            control.dispatch_event(&route, &reorder_key(), &mut EventCtx::default()),
            tuicore::EventOutcome::Handled
        );
        assert!(control.is_reordering());
        control.dispatch_event(
            &data_descendant_route("search"),
            &add_key(),
            &mut EventCtx::default(),
        );
        control.dispatch_event(
            &data_descendant_route("search"),
            &key(Key::Char('x'), KeyModifiers::NONE),
            &mut EventCtx::default(),
        );
        assert!(control.is_reordering());
        assert!(!control.is_adding());
        assert_eq!(control.items().len(), 3);
        control.dispatch_event(
            &data_route(),
            &key(Key::Down, KeyModifiers::NONE),
            &mut EventCtx::default(),
        );
        control.dispatch_event(
            &data_route(),
            &key(Key::Down, KeyModifiers::NONE),
            &mut EventCtx::default(),
        );
        control.dispatch_event(
            &data_route(),
            &key(Key::Enter, KeyModifiers::NONE),
            &mut EventCtx::default(),
        );
        assert_eq!(
            control
                .items()
                .iter()
                .map(|row| row.rank)
                .collect::<Vec<_>>(),
            vec![30, 10, 20]
        );
        assert_eq!(
            control.take_events(),
            vec![ListControlEvent::Reordered {
                row_ids: vec![2, 3, 1]
            }]
        );

        control.dispatch_event(&route, &reorder_key(), &mut EventCtx::default());
        control.dispatch_event(
            &route,
            &key(Key::Esc, KeyModifiers::NONE),
            &mut EventCtx::default(),
        );
        assert!(!control.is_reordering());
        assert!(matches!(
            control.take_events().as_slice(),
            [ListControlEvent::ReorderCancelled { .. }]
        ));
    }

    let mut plain = control([]);
    assert_eq!(
        plain.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default()),
        tuicore::EventOutcome::Ignored
    );
}

#[test]
fn reorder_key_exits_leave_consumer_panel_titles_untouched() {
    for exit in [Key::Enter, Key::Esc, Key::Char('[')] {
        let mut control = ranked_control(ranked_rows()).panel(Panel::new().bottom_left("Before"));
        control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
        assert_eq!(
            control
                .panel_ref()
                .title_text(tuicore::PanelTitlePosition::BottomLeft),
            Some("Before")
        );
        control.panel_mut().set_bottom_left("Consumer update");

        let modifiers = if exit == Key::Char('[') {
            KeyModifiers::CONTROL
        } else {
            KeyModifiers::NONE
        };
        control.dispatch_event(
            &data_route(),
            &key(exit, modifiers),
            &mut EventCtx::default(),
        );
        assert_eq!(
            control
                .panel_ref()
                .title_text(tuicore::PanelTitlePosition::BottomLeft),
            Some("Consumer update")
        );
    }
}

#[test]
fn reorder_status_restores_after_focus_loss_and_data_change_rejection() {
    let mut blurred = ranked_control(ranked_rows()).panel(Panel::new().bottom_left("Caller help"));
    blurred.layout(Rect::new(0, 0, 40, 8), &mut LayoutCtx::new());
    blurred.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    let mut layout = LayoutCtx::new();
    blurred.layout(Rect::new(0, 0, 40, 8), &mut layout);
    let target = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == "data-view")
        .expect("data view target should exist")
        .clone();
    blurred.dispatch_focus(&target, false, &mut FocusCtx::default());
    assert_eq!(
        blurred
            .panel_ref()
            .title_text(tuicore::PanelTitlePosition::BottomLeft),
        Some("Caller help")
    );

    let mut changed = ranked_control(ranked_rows());
    changed.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    changed.data_view_mut().update_row(&2, |row| row.rank = 25);
    changed.dispatch_event(
        &data_route(),
        &key(Key::Down, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert_eq!(
        changed
            .panel_ref()
            .title_text(tuicore::PanelTitlePosition::BottomLeft),
        None
    );
    assert_eq!(
        changed.take_events(),
        vec![ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged
        }]
    );
}

#[test]
fn routed_custom_reorder_binding_replaces_default() {
    let mut control = ranked_control(ranked_rows())
        .keybindings(ListControlKeyBindings::default().reorder([KeySpec::plain('r')]));
    control.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    assert!(!control.is_reordering());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('r'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert!(control.is_reordering());
}

#[test]
fn routed_search_editor_dropdown_and_confirmation_own_reorder_binding() {
    let mut search = ranked_control(ranked_rows()).action_bar(true);
    search.data_view_mut().set_focused(true);
    search.dispatch_event(
        &data_route(),
        &key(Key::Char('/'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    search.dispatch_event(
        &data_descendant_route("search"),
        &reorder_key(),
        &mut EventCtx::default(),
    );
    assert!(!search.is_reordering());

    let mut editor = ranked_control(ranked_rows());
    editor.dispatch_event(&data_route(), &add_key(), &mut EventCtx::default());
    editor.dispatch_event(&input_route(), &reorder_key(), &mut EventCtx::default());
    assert!(editor.is_adding());
    assert!(!editor.is_reordering());

    let mut dropdown = ListControl::<RankedRow, usize>::new_fields(
        ranked_rows(),
        |row: &RankedRow| row.id,
        [ListControlField::dropdown("Rank", ["10", "20"])],
        |_, _| unreachable!(),
    )
    .column(
        Column::text("rank", "Rank", Constraint::Fill(1), |row: &RankedRow| {
            row.rank.to_string()
        })
        .reorderable(|row| row.rank, |row, rank| row.rank = rank),
    )
    .reorderable_by("rank");
    dropdown.dispatch_event(&data_route(), &add_key(), &mut EventCtx::default());
    dropdown.dispatch_event(&input_route(), &reorder_key(), &mut EventCtx::default());
    assert!(dropdown.is_adding());
    assert!(!dropdown.is_reordering());

    let mut confirmation =
        ranked_control(ranked_rows()).confirm_remove("Remove?", |_| "row".into());
    confirmation.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
    confirmation.dispatch_event(
        &EventRoute::new(TreePath::from_keys([ChildKey::new("remove-confirmation")])),
        &reorder_key(),
        &mut EventCtx::default(),
    );
    assert!(confirmation.is_confirming_remove());
    assert!(!confirmation.is_reordering());
}

#[test]
#[should_panic(expected = "mutually exclusive")]
fn public_sort_then_reorder_configuration_is_rejected() {
    let _ = ListControl::<RankedRow, usize>::new(
        ranked_rows(),
        |row: &RankedRow| row.id,
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
fn public_reorder_then_sort_configuration_is_rejected() {
    let _ = ranked_control(ranked_rows()).sorted_by("rank", SortDirection::Ascending);
}

fn add_key() -> TuiEvent {
    key(Key::Char('+'), KeyModifiers::NONE)
}

fn data_descendant_route(slot: &str) -> EventRoute {
    EventRoute::new(TreePath::from_keys([
        ChildKey::new("data"),
        ChildKey::new(slot),
    ]))
}

fn assert_action_bar_search_character_does_not_mutate_rows(character: char) {
    let mut control = control([Row {
        id: 1,
        name: "Ada".into(),
    }])
    .action_bar(true);
    control.data_view_mut().set_focused(true);
    control.layout(Rect::new(0, 0, 40, 8), &mut LayoutCtx::new());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('/'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    control.dispatch_event(
        &data_descendant_route("search"),
        &key(Key::Char(character), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(
        control.data_view().transform_state().search,
        character.to_string()
    );
    assert_eq!(control.items().len(), 1);
    assert!(!control.is_adding());
    assert!(control.take_events().is_empty());
}

#[test]
fn action_bar_search_owns_list_control_action_characters() {
    for character in ['+', 'x', 'e'] {
        assert_action_bar_search_character_does_not_mutate_rows(character);
    }
}

fn assert_header_filter_receives_list_binding(character: char) {
    let mut control = control([Row {
        id: 1,
        name: "Ada".into(),
    }])
    .column(
        Column::text(
            "filterable",
            "Filterable",
            Constraint::Length(12),
            |row: &Row| row.name.clone(),
        )
        .filter_key(|row| row.name.clone()),
    )
    .headers(true);
    control.data_view_mut().set_focused(true);
    control.layout(Rect::new(0, 0, 40, 8), &mut LayoutCtx::new());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('f'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    let outcome = control.dispatch_event(
        &data_route(),
        &key(Key::Char(character), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );

    assert_eq!(outcome, tuicore::EventOutcome::Handled);
    assert_eq!(control.items().len(), 1);
    assert!(!control.is_adding());
    assert!(!control.is_editing());
    assert!(control.take_events().is_empty());
}

#[test]
fn header_filter_owns_list_control_action_characters() {
    for character in ['e', '+', 'x'] {
        assert_header_filter_receives_list_binding(character);
    }
}

#[test]
fn add_enters_inline_input_and_submit_appends_and_highlights() {
    let mut control = control([Row {
        id: 1,
        name: "Ada".into(),
    }]);
    let mut ctx = EventCtx::default();

    control.dispatch_event(&data_route(), &add_key(), &mut ctx);

    assert!(control.is_adding());
    assert!(!control.data_view().is_focused());
    assert!(
        matches!(ctx.focus_request(), Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "input")
    );

    let mut submit_ctx = EventCtx::default();
    control.dispatch_event(
        &input_route(),
        &TuiEvent::Paste("Grace".into()),
        &mut submit_ctx,
    );
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut submit_ctx,
    );

    assert!(!control.is_adding());
    assert!(control.data_view().is_focused());
    assert_eq!(control.items().len(), 2);
    assert_eq!(control.items()[1].name, "Grace");
    assert_eq!(control.data_view().highlighted_id(), Some(2));
    assert!(submit_ctx.layout_requested());
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Added { row_id: 2 }]
    );
}

#[test]
fn escape_cancels_and_blank_enter_stays_editing() {
    let mut control = control([]);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);

    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    assert!(control.is_adding());
    assert!(control.items().is_empty());

    let mut cancel_ctx = EventCtx::default();
    control.dispatch_event(
        &input_route(),
        &key(Key::Esc, KeyModifiers::NONE),
        &mut cancel_ctx,
    );
    assert!(!control.is_adding());
    assert!(cancel_ctx.layout_requested());
    assert_eq!(control.take_events(), vec![ListControlEvent::AddCancelled]);
}

#[test]
fn sequential_fields_advance_then_submit_exact_values_once() {
    let mut control = multi_control();
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);

    for (index, value) in ["Gateway", "Ada", "Ready"].into_iter().enumerate() {
        let route = field_route(index);
        control.dispatch_event(&route, &TuiEvent::Paste(value.into()), &mut ctx);
        control.dispatch_event(&route, &key(Key::Enter, KeyModifiers::NONE), &mut ctx);
        if index < 2 {
            assert!(control.is_adding());
            assert!(control.items().is_empty());
            assert!(matches!(
                ctx.focus_request(),
                Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(index + 1).path
            ));
        }
    }

    assert_eq!(
        control.items(),
        &[(
            1,
            "Gateway".to_string(),
            "Ada".to_string(),
            "Ready".to_string()
        )]
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Added { row_id: 1 }]
    );
}

#[test]
fn edit_prefills_all_fields_cycles_and_updates_highlighted_row_in_place() {
    let mut control = editable_control();
    control.data_view_mut().highlight_id(&2);
    let mut ctx = EventCtx::default();

    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);
    assert!(control.is_editing());
    assert!(!control.is_adding());
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(0).path
    ));
    control.dispatch_event(
        &field_route(0),
        &TuiEvent::Paste(" Hopper".into()),
        &mut ctx,
    );
    control.dispatch_event(
        &field_route(0),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    assert!(control.is_editing());
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(1).path
    ));
    control.dispatch_event(
        &field_route(1),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert!(!control.is_editing());
    assert_eq!(control.items()[0], (1, "Ada".into(), "Ready".into()));
    assert_eq!(
        control.items()[1],
        (2, "Grace Hopper".into(), "Paused".into())
    );
    assert_eq!(control.data_view().highlighted_id(), Some(2));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Edited { row_id: 2 }]
    );
}

#[test]
fn typing_in_prefilled_edit_field_appends_at_end() {
    let mut control = editable_control();
    control.data_view_mut().highlight_id(&2);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);

    control.dispatch_event(
        &field_route(0),
        &key(Key::Char('!'), KeyModifiers::NONE),
        &mut ctx,
    );
    control.dispatch_event(
        &field_route(0),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    control.dispatch_event(
        &field_route(1),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(control.items()[1].1, "Grace!");
}

#[test]
fn escape_cancels_edit_without_mutating_row() {
    let mut control = editable_control();
    control.data_view_mut().highlight_id(&2);
    let original = control.items().to_vec();
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);
    control.dispatch_event(
        &field_route(0),
        &TuiEvent::Paste(" changed".into()),
        &mut ctx,
    );

    control.dispatch_event(
        &field_route(0),
        &key(Key::Esc, KeyModifiers::NONE),
        &mut ctx,
    );

    assert!(!control.is_editing());
    assert_eq!(control.items(), original);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::EditCancelled { row_id: 2 }]
    );
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
    ));
}

#[test]
fn submitting_edit_after_row_removal_cancels_and_restores_data_focus() {
    let mut control = editable_control();
    control.data_view_mut().highlight_id(&2);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);
    control.data_view_mut().remove_row(&2);
    control.dispatch_event(
        &field_route(0),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    let outcome = control.dispatch_event(
        &field_route(1),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(outcome, tuicore::EventOutcome::Handled);
    assert!(!control.is_editing());
    assert!(control.data_view().is_focused());
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::EditCancelled { row_id: 2 }]
    );
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
    ));
}

#[test]
fn custom_edit_binding_replaces_default() {
    let mut control = editable_control()
        .keybindings(ListControlKeyBindings::default().edit([KeySpec::plain('i')]));

    control.dispatch_event(&data_route(), &edit_key(), &mut EventCtx::default());
    assert!(!control.is_editing());
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('i'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert!(control.is_editing());
}

fn optional_control(
    rows: impl IntoIterator<Item = (usize, String, String, String)>,
) -> ListControl<(usize, String, String, String), usize> {
    ListControl::new_fields(
        rows,
        |row: &(usize, String, String, String)| row.0,
        [
            ListControlField::text("Name"),
            ListControlField::text("Note").optional(),
            ListControlField::dropdown("State", ["Ready", "Paused"]).optional(),
        ],
        |values, rows| {
            (
                rows.len() + 1,
                values[0].clone(),
                values[1].clone(),
                values[2].clone(),
            )
        },
    )
    .editable(
        |row| vec![row.1.clone(), row.2.clone(), row.3.clone()],
        |row, values| {
            row.1.clone_from(&values[0]);
            row.2.clone_from(&values[1]);
            row.3.clone_from(&values[2]);
        },
    )
}

fn conditional_control(
    rows: impl IntoIterator<Item = (usize, String, String, String)>,
) -> ListControl<(usize, String, String, String), usize> {
    ListControl::new_fields(
        rows,
        |row: &(usize, String, String, String)| row.0,
        [
            ListControlField::dropdown("Kind", ["Person", "Service"]),
            ListControlField::text("Person name").visible_when(0, ["Person"]),
            ListControlField::text("Service URL").visible_when(0, ["Service"]),
        ],
        |values, rows| {
            (
                rows.len() + 1,
                values[0].clone(),
                values[1].clone(),
                values[2].clone(),
            )
        },
    )
    .editable(
        |row| vec![row.1.clone(), row.2.clone(), row.3.clone()],
        |row, values| {
            row.1.clone_from(&values[0]);
            row.2.clone_from(&values[1]);
            row.3.clone_from(&values[2]);
        },
    )
}

fn filter_dropdown(control: &mut impl TuiNode<()>, query: &str, ctx: &mut EventCtx<()>) {
    for character in query.chars() {
        control.dispatch_event(
            &field_route(0),
            &key(Key::Char(character), KeyModifiers::NONE),
            ctx,
        );
    }
    control.dispatch_event(&field_route(0), &key(Key::Enter, KeyModifiers::NONE), ctx);
}

#[test]
fn dropdown_kind_selects_required_visible_follow_up_and_preserves_vector_shape() {
    for (query, kind, field, value, expected) in [
        (
            "Pers",
            "Person",
            1,
            "Ada",
            (1, "Person".into(), "Ada".into(), "".into()),
        ),
        (
            "Serv",
            "Service",
            2,
            "https://api",
            (1, "Service".into(), "".into(), "https://api".into()),
        ),
    ] {
        let mut control = conditional_control([]);
        let mut ctx = EventCtx::default();
        control.dispatch_event(&data_route(), &add_key(), &mut ctx);

        filter_dropdown(&mut control, query, &mut ctx);

        assert!(control.is_adding());
        assert!(matches!(
            ctx.focus_request(),
            Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(field).path
        ));
        control.dispatch_event(
            &field_route(field),
            &TuiEvent::Paste(value.into()),
            &mut ctx,
        );
        control.dispatch_event(
            &field_route(field),
            &key(Key::Enter, KeyModifiers::NONE),
            &mut ctx,
        );

        assert_eq!(control.items(), &[expected], "kind={kind}");
        assert!(!control.is_adding());
    }
}

#[test]
fn edit_prefill_hides_and_clears_values_for_newly_inactive_branch() {
    let mut control = conditional_control([(7, "Person".into(), "Ada".into(), "stale".into())]);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);

    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(0).path
    ));
    filter_dropdown(&mut control, "Serv", &mut ctx);
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { path, .. }) if path == &field_route(2).path
    ));
    control.dispatch_event(
        &field_route(2),
        &TuiEvent::Paste("https://new".into()),
        &mut ctx,
    );
    control.dispatch_event(
        &field_route(2),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(
        control.items(),
        &[(7, "Service".into(), "".into(), "https://new".into())]
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Edited { row_id: 7 }]
    );
}

#[test]
#[should_panic(expected = "visibility conditions must reference an earlier field")]
fn field_visibility_rejects_self_reference_at_construction() {
    let _ = ListControl::<Row, usize>::new_fields(
        [],
        |row| row.id,
        [ListControlField::text("Name").visible_when(0, ["show"])],
        |_, _| unreachable!(),
    );
}

#[test]
#[should_panic(expected = "visibility conditions must reference an earlier field")]
fn field_visibility_rejects_later_reference_at_construction() {
    let _ = ListControl::<Row, usize>::new_fields(
        [],
        |row| row.id,
        [
            ListControlField::text("Name").visible_when(1, ["show"]),
            ListControlField::text("State"),
        ],
        |_, _| unreachable!(),
    );
}

#[test]
fn optional_empty_text_and_dropdown_submit_in_add_flow() {
    let mut control = optional_control([]);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.dispatch_event(&field_route(0), &TuiEvent::Paste("API".into()), &mut ctx);
    for index in 0..3 {
        control.dispatch_event(
            &field_route(index),
            &key(Key::Enter, KeyModifiers::NONE),
            &mut ctx,
        );
    }

    assert_eq!(control.items(), &[(1, "API".into(), "".into(), "".into())]);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Added { row_id: 1 }]
    );
}

#[test]
fn optional_dropdown_can_clear_prefilled_edit_value() {
    let mut control = optional_control([(7, "API".into(), "".into(), "Paused".into())]);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &edit_key(), &mut ctx);
    for index in 0..2 {
        control.dispatch_event(
            &field_route(index),
            &key(Key::Enter, KeyModifiers::NONE),
            &mut ctx,
        );
    }
    for _ in 0..3 {
        control.dispatch_event(
            &field_route(2),
            &key(Key::Char('k'), KeyModifiers::CONTROL),
            &mut ctx,
        );
    }
    control.dispatch_event(
        &field_route(2),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(control.items(), &[(7, "API".into(), "".into(), "".into())]);
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Edited { row_id: 7 }]
    );
}

#[test]
#[should_panic(
    expected = "ListControl dropdown option strings must be non-empty because \"\" represents no selection"
)]
fn required_dropdown_rejects_empty_option_string() {
    let _ = ListControl::<Row, usize>::new_fields(
        [],
        |row| row.id,
        [ListControlField::dropdown("Name", [""])],
        |_, _| unreachable!(),
    );
}

#[test]
#[should_panic(
    expected = "ListControl dropdown option strings must be non-empty because \"\" represents no selection"
)]
fn optional_dropdown_rejects_empty_option_string() {
    let _ = ListControl::<Row, usize>::new_fields(
        [],
        |row| row.id,
        [ListControlField::dropdown("Name", [""]).optional()],
        |_, _| unreachable!(),
    );
}

#[test]
fn blank_intermediate_and_final_fields_block_progress_and_submission() {
    let mut control = multi_control();
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.dispatch_event(&field_route(0), &TuiEvent::Paste("Entity".into()), &mut ctx);
    control.dispatch_event(
        &field_route(0),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    control.dispatch_event(
        &field_route(1),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    assert!(control.is_adding());
    assert!(control.items().is_empty());

    control.dispatch_event(&field_route(1), &TuiEvent::Paste("Owner".into()), &mut ctx);
    control.dispatch_event(
        &field_route(1),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    control.dispatch_event(
        &field_route(2),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    assert!(control.is_adding());
    assert!(control.items().is_empty());
    assert!(control.take_events().is_empty());
}

#[test]
fn escape_from_later_fields_cancels_whole_draft() {
    for cancel_index in [1, 2] {
        let mut control = multi_control();
        let mut ctx = EventCtx::default();
        control.dispatch_event(&data_route(), &add_key(), &mut ctx);
        for index in 0..cancel_index {
            control.dispatch_event(
                &field_route(index),
                &TuiEvent::Paste(format!("value-{index}")),
                &mut ctx,
            );
            control.dispatch_event(
                &field_route(index),
                &key(Key::Enter, KeyModifiers::NONE),
                &mut ctx,
            );
        }

        control.dispatch_event(
            &field_route(cancel_index),
            &key(Key::Esc, KeyModifiers::NONE),
            &mut ctx,
        );

        assert!(!control.is_adding());
        assert!(control.items().is_empty());
        assert_eq!(control.take_events(), vec![ListControlEvent::AddCancelled]);
    }
}

#[test]
fn field_focus_transition_does_not_cancel_draft() {
    let mut control = multi_control();
    let area = Rect::new(0, 0, 60, 8);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.layout(area, &mut LayoutCtx::new());
    control.dispatch_event(&field_route(0), &TuiEvent::Paste("Entity".into()), &mut ctx);
    control.dispatch_event(
        &field_route(0),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    let mut layout = LayoutCtx::new();
    control.layout(area, &mut layout);
    assert!(
        layout
            .focus_targets()
            .iter()
            .all(|target| target.path != field_route(0).path)
    );
    assert!(
        layout
            .focus_targets()
            .iter()
            .any(|target| target.path == field_route(1).path)
    );
    assert!(control.is_adding());
    assert!(control.take_events().is_empty());
}

#[test]
fn remove_selects_nearest_survivor_and_empty_remove_is_noop() {
    let mut control = control([
        Row {
            id: 1,
            name: "one".into(),
        },
        Row {
            id: 2,
            name: "two".into(),
        },
        Row {
            id: 3,
            name: "three".into(),
        },
    ]);
    control.data_view_mut().highlight_id(&2);
    let mut ctx = EventCtx::default();
    let remove = key(Key::Char('x'), KeyModifiers::CONTROL);

    control.dispatch_event(&data_route(), &remove, &mut ctx);

    assert!(ctx.layout_requested());
    assert_eq!(
        control.items().iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(control.data_view().highlighted_id(), Some(3));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Removed { row_id: 2 }]
    );

    control.data_view_mut().remove_row(&1);
    control.data_view_mut().remove_row(&3);
    let mut empty_ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &remove, &mut empty_ctx);
    assert!(!empty_ctx.layout_requested());
    assert!(control.take_events().is_empty());
}

fn confirmed_control() -> ListControl<Row, usize> {
    control([
        Row {
            id: 10,
            name: "Ada · Active".into(),
        },
        Row {
            id: 20,
            name: "Grace · Paused".into(),
        },
    ])
    .confirm_remove("Remove item?", |row| {
        let (name, state) = row.name.split_once(" · ").expect("seeded row format");
        format!("Remove {name} while state is {state}?")
    })
}

#[test]
fn confirmation_opens_without_removing_and_renders_selected_row_details() {
    let mut control = confirmed_control();
    control.data_view_mut().highlight_id(&20);
    let area = Rect::new(0, 0, 50, 8);
    let mut ctx = EventCtx::default();

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(control.is_confirming_remove());
    assert_eq!(control.items().len(), 2);
    assert!(matches!(
        ctx.focus_request(),
        Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "dialog"
    ));
    let mut navigation_ctx = EventCtx::default();
    control.dispatch_event(
        &EventRoute::new(TreePath::from_keys([ChildKey::new("remove-confirmation")])),
        &key(Key::Char('j'), KeyModifiers::CONTROL),
        &mut navigation_ctx,
    );
    assert!(control.is_confirming_remove());
    assert!(navigation_ctx.focus_request().is_none());

    let mut layout = LayoutCtx::new();
    control.layout(area, &mut layout);
    let dialog = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == "dialog")
        .expect("confirmation dialog focus target");
    assert!(dialog.focused_events_before_global_hotkeys);
    assert!(dialog.suppress_global_hotkeys);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            control.render(frame, area, &mut render);
            render.flush(frame);
        })
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Remove Grace while state is Paused?"));
}

#[test]
fn confirmation_centers_and_renders_in_screen_overlay_bounds() {
    let mut control = confirmed_control();
    let component_area = Rect::new(1, 1, 12, 4);
    let overlay_bounds = Rect::new(0, 0, 80, 24);
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );

    let mut layout = LayoutCtx::new();
    layout.with_overlay_bounds(overlay_bounds, |ctx| {
        control.layout(component_area, ctx);
    });
    let dialog = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == "dialog")
        .expect("confirmation dialog focus target");
    assert_eq!(
        dialog.area.x,
        overlay_bounds.x + (overlay_bounds.width - dialog.area.width) / 2
    );
    assert_eq!(
        dialog.area.y,
        overlay_bounds.y + (overlay_bounds.height - dialog.area.height) / 2
    );
    assert!(dialog.area.x >= component_area.right());
    assert_eq!(layout.overlays()[0].area, dialog.area);
    assert_eq!(layout.overlays()[0].bounds, overlay_bounds);

    let mut terminal = Terminal::new(TestBackend::new(
        overlay_bounds.width,
        overlay_bounds.height,
    ))
    .unwrap();
    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            control.render(frame, component_area, &mut render);
            render.flush(frame);
        })
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("Remove item?"));
    assert!(rendered.contains("Delete"));
    assert!(rendered.contains("Cancel"));
}

#[test]
fn confirmation_removes_pending_stable_id_once() {
    let mut control = confirmed_control();
    control.data_view_mut().highlight_id(&20);
    let mut ctx = EventCtx::default();
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.data_view_mut().highlight_id(&10);

    let mut confirm_ctx = EventCtx::default();
    control.dispatch_event(
        &EventRoute::new(TreePath::from_keys([ChildKey::new("remove-confirmation")])),
        &key(Key::Char('d'), KeyModifiers::NONE),
        &mut confirm_ctx,
    );

    assert!(!control.is_confirming_remove());
    assert_eq!(
        control.items().iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![10]
    );
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Removed { row_id: 20 }]
    );
    assert!(matches!(
        confirm_ctx.focus_request(),
        Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
    ));
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('d'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert!(control.take_events().is_empty());
}

#[test]
fn confirmation_cancel_keys_keep_item_and_restore_data_focus() {
    for cancel in [
        key(Key::Char('c'), KeyModifiers::NONE),
        key(Key::Esc, KeyModifiers::NONE),
        key(Key::Char('['), KeyModifiers::CONTROL),
        key(Key::Char('x'), KeyModifiers::NONE),
    ] {
        let mut control = confirmed_control();
        let mut ctx = EventCtx::default();
        control.dispatch_event(
            &data_route(),
            &key(Key::Char('x'), KeyModifiers::CONTROL),
            &mut ctx,
        );
        let mut cancel_ctx = EventCtx::default();
        control.dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::new("remove-confirmation")])),
            &cancel,
            &mut cancel_ctx,
        );

        assert!(!control.is_confirming_remove());
        assert_eq!(control.items().len(), 2);
        assert!(control.take_events().is_empty());
        assert!(matches!(
            cancel_ctx.focus_request(),
            Some(FocusRequest::TargetAt { id, .. }) if id.as_str() == "data-view"
        ));
    }
}

#[test]
fn custom_confirmation_bindings_confirm_and_cancel_removal() {
    let bindings = ConfirmationDialogKeyBindings {
        yes: Some(KeySpec::plain('y')),
        no: Some(KeySpec::plain('n')),
    };
    for (choice, expected_ids, expected_events) in [
        (
            'y',
            vec![20],
            vec![ListControlEvent::Removed { row_id: 10 }],
        ),
        ('n', vec![10, 20], vec![]),
    ] {
        let mut control = confirmed_control().confirmation_keybindings(bindings);
        control.dispatch_event(
            &data_route(),
            &key(Key::Char('x'), KeyModifiers::CONTROL),
            &mut EventCtx::default(),
        );

        control.dispatch_event(
            &EventRoute::new(TreePath::from_keys([ChildKey::new("remove-confirmation")])),
            &key(Key::Char(choice), KeyModifiers::NONE),
            &mut EventCtx::default(),
        );

        assert_eq!(
            control.items().iter().map(|row| row.id).collect::<Vec<_>>(),
            expected_ids
        );
        assert_eq!(control.take_events(), expected_events);
    }
}

#[test]
fn default_bindings_accept_ctrl_x_only_for_remove() {
    let mut control = control([Row {
        id: 1,
        name: "one".into(),
    }]);
    let mut ctx = EventCtx::default();

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('+'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::NONE),
        &mut ctx,
    );
    control.dispatch_event(
        &data_route(),
        &key(Key::Char('-'), KeyModifiers::NONE),
        &mut ctx,
    );

    assert!(!control.is_adding());
    assert_eq!(control.items().len(), 1);
    assert!(control.take_events().is_empty());
    assert!(!ctx.layout_requested());

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut ctx,
    );
    assert!(control.items().is_empty());
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Removed { row_id: 1 }]
    );
}

#[test]
fn custom_minus_remove_binding_replaces_default_ctrl_x() {
    let mut control = control([Row {
        id: 1,
        name: "one".into(),
    }])
    .keybindings(ListControlKeyBindings::default().remove([KeySpec::plain('-')]));

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('x'), KeyModifiers::CONTROL),
        &mut EventCtx::default(),
    );
    assert_eq!(control.items().len(), 1);

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('-'), KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    assert!(control.items().is_empty());
}

#[test]
fn custom_add_binding_replaces_defaults() {
    let mut control =
        control([]).keybindings(ListControlKeyBindings::default().add([KeySpec::plain('a')]));
    let mut ctx = EventCtx::default();

    control.dispatch_event(
        &data_route(),
        &key(Key::Char('a'), KeyModifiers::NONE),
        &mut ctx,
    );

    assert!(control.is_adding());
}

#[test]
fn submit_and_cancel_layout_cycles_remove_input_and_restore_data_tab_stop() {
    for submit in [true, false] {
        let mut control = control([]);
        let area = Rect::new(0, 0, 30, 8);
        control.layout(area, &mut LayoutCtx::new());
        let mut ctx = EventCtx::default();
        control.dispatch_event(&data_route(), &add_key(), &mut ctx);

        let mut adding_layout = LayoutCtx::new();
        control.layout(area, &mut adding_layout);
        let data = adding_layout
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "data-view")
            .expect("data target should remain registered");
        assert!(!data.tab_stop);
        assert!(
            adding_layout
                .focus_targets()
                .iter()
                .any(|target| target.id.as_str() == "input")
        );

        let mut finish_ctx = EventCtx::default();
        if submit {
            control.dispatch_event(
                &input_route(),
                &TuiEvent::Paste("new".into()),
                &mut finish_ctx,
            );
            control.dispatch_event(
                &input_route(),
                &key(Key::Enter, KeyModifiers::NONE),
                &mut finish_ctx,
            );
        } else {
            control.dispatch_event(
                &input_route(),
                &key(Key::Esc, KeyModifiers::NONE),
                &mut finish_ctx,
            );
        }
        assert!(finish_ctx.layout_requested());

        let mut finished_layout = LayoutCtx::new();
        control.layout(area, &mut finished_layout);
        assert!(
            !finished_layout
                .focus_targets()
                .iter()
                .any(|target| target.id.as_str() == "input")
        );
        assert!(
            finished_layout
                .focus_targets()
                .iter()
                .find(|target| target.id.as_str() == "data-view")
                .expect("data target should be restored")
                .tab_stop
        );
    }
}

#[test]
fn hidden_insert_emits_inserted_id_without_changing_visible_highlight() {
    let mut control = control([Row {
        id: 1,
        name: "Ada".into(),
    }]);
    control.data_view_mut().set_search_query("Ada");
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.dispatch_event(&input_route(), &TuiEvent::Paste("Grace".into()), &mut ctx);
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(control.data_view().highlighted_id(), Some(1));
    assert_eq!(
        control.take_events(),
        vec![ListControlEvent::Added { row_id: 2 }]
    );
}

#[test]
fn added_visible_row_is_revealed_in_constrained_viewport() {
    let mut control = control((1..=4).map(|id| Row {
        id,
        name: format!("row {id}"),
    }));
    let area = Rect::new(0, 0, 20, 5);
    control.layout(area, &mut LayoutCtx::new());
    let mut ctx = EventCtx::new(AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    });
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.layout(area, &mut LayoutCtx::new());
    control.dispatch_event(&input_route(), &TuiEvent::Paste("row 5".into()), &mut ctx);
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );
    control.layout(area, &mut LayoutCtx::new());

    let mut terminal = Terminal::new(TestBackend::new(20, 5)).expect("terminal should build");
    terminal
        .draw(|frame| {
            let mut render = RenderCtx::new();
            control.render(frame, area, &mut render);
            render.flush(frame);
        })
        .expect("control should render");
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(rendered.contains("row 5"));
}

#[test]
fn input_blur_cancels_add_and_requests_layout() {
    let mut control = control([]);
    let area = Rect::new(0, 0, 30, 8);
    let mut event_ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut event_ctx);
    let mut layout = LayoutCtx::new();
    control.layout(area, &mut layout);
    let input = layout
        .focus_targets()
        .iter()
        .find(|target| target.id.as_str() == "input")
        .expect("input target should exist")
        .clone();
    let mut focus_ctx = FocusCtx::default();

    control.dispatch_focus(&input, false, &mut focus_ctx);

    assert!(!control.is_adding());
    assert!(focus_ctx.layout_requested());
    assert_eq!(control.take_events(), vec![ListControlEvent::AddCancelled]);
}

#[test]
fn runtime_dropdown_add_opens_search_and_enter_submits() {
    let mut runtime = ListRuntime::new();
    runtime.dispatch(add_key());
    assert_eq!(runtime.current_id(), Some("input"));

    runtime.dispatch(key(Key::Char('G'), KeyModifiers::NONE));

    let nested_search_route = EventRoute::new(
        runtime
            .focus
            .current_path()
            .child(ChildKey::new("dropdown-search")),
    );
    runtime.dispatch_at(nested_search_route, key(Key::Enter, KeyModifiers::NONE));

    assert!(!runtime.control.is_adding());
    assert_eq!(runtime.current_id(), Some("data-view"));
    assert_eq!(runtime.control.items()[0].name, "Grace");
}

#[test]
fn runtime_ctrl_space_dropdown_commit_submits_immediately() {
    let mut runtime = ListRuntime::new();
    runtime.dispatch(add_key());
    assert_eq!(runtime.current_id(), Some("input"));

    runtime.dispatch(key(Key::Char(' '), KeyModifiers::CONTROL));

    assert!(!runtime.control.is_adding());
    assert_eq!(runtime.current_id(), Some("data-view"));
    assert_eq!(runtime.control.items()[0].name, "Ada");
}

#[test]
fn runtime_dropdown_cancel_keys_cancel_draft_once() {
    for cancel in [
        key(Key::Esc, KeyModifiers::NONE),
        key(Key::Char('['), KeyModifiers::CONTROL),
    ] {
        let mut runtime = ListRuntime::new();
        runtime.dispatch(add_key());
        assert_eq!(runtime.current_id(), Some("input"));

        runtime.dispatch(cancel);
        assert!(!runtime.control.is_adding());
        assert_eq!(runtime.current_id(), Some("data-view"));
        assert_eq!(
            runtime.control.take_events(),
            vec![ListControlEvent::AddCancelled]
        );
        runtime.validate_focus();
        assert!(runtime.control.take_events().is_empty());
    }
}

#[test]
fn runtime_focus_reconciliation_after_submit_emits_no_duplicate_event() {
    let mut runtime = ListRuntime::new();
    runtime.dispatch(add_key());
    runtime.dispatch(key(Key::Enter, KeyModifiers::NONE));
    assert!(!runtime.control.is_adding());
    assert_eq!(runtime.current_id(), Some("data-view"));
    assert_eq!(
        runtime.control.take_events(),
        vec![ListControlEvent::Added { row_id: 1 }]
    );
    runtime.validate_focus();
    assert!(runtime.control.take_events().is_empty());
}

#[test]
fn monotonic_creator_does_not_reuse_deleted_highest_id() {
    let mut next_id = 4;
    let mut control: ListControl<Row, usize> = ListControl::list(
        [
            Row {
                id: 1,
                name: "one".into(),
            },
            Row {
                id: 2,
                name: "two".into(),
            },
            Row {
                id: 3,
                name: "three".into(),
            },
        ],
        |row| row.id,
        |row| row.name.clone(),
        move |name, _| {
            let row = Row { id: next_id, name };
            next_id += 1;
            row
        },
    );
    control.data_view_mut().remove_row(&3);
    let mut ctx = EventCtx::default();
    control.dispatch_event(&data_route(), &add_key(), &mut ctx);
    control.dispatch_event(&input_route(), &TuiEvent::Paste("four".into()), &mut ctx);
    control.dispatch_event(
        &input_route(),
        &key(Key::Enter, KeyModifiers::NONE),
        &mut ctx,
    );

    assert_eq!(control.items().last().map(|row| row.id), Some(4));
}

#[test]
fn panel_keeps_caller_titles_and_renders_only_standard_hotkey_badge() {
    let area = Rect::new(0, 0, 50, 5);
    let panel_after_hotkey = control([]).panel(
        Panel::new()
            .bottom_left("Caller help")
            .bottom_right("Ready"),
    );
    let panel_before_hotkey =
        ListControl::new([], |row: &Row| row.id, |name, _| Row { id: 1, name })
            .column(Column::text(
                "name",
                "Name",
                Constraint::Percentage(100),
                |row: &Row| row.name.clone(),
            ))
            .panel(Panel::new().bottom_left("Caller help"))
            .hotkey("z");

    let render = |control: &ListControl<Row, usize>| {
        let mut terminal = Terminal::new(TestBackend::new(50, 5)).expect("terminal should build");
        terminal
            .draw(|frame| {
                let mut ctx = RenderCtx::new();
                control.render(frame, area, &mut ctx);
            })
            .expect("control should render");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    };

    let after = render(&panel_after_hotkey);
    assert!(after.contains("Caller help"));
    assert!(after.contains("Ready"));
    assert!(after.contains('l'));
    assert!(!after.contains("+ add"));
    let before = render(&panel_before_hotkey);
    assert!(before.contains("Caller help"));
    assert!(before.contains('z'));
    assert!(!before.contains("remove"));
}

#[test]
fn nested_hotkey_events_highlight_and_clear_multiletter_panel_badge() {
    let area = Rect::new(0, 0, 50, 5);
    let mut control = control([]).hotkey("le");
    control.layout(area, &mut LayoutCtx::new());
    let badge_l_is_underlined = |control: &ListControl<Row, usize>| {
        let mut terminal = Terminal::new(TestBackend::new(50, 5)).expect("terminal should build");
        terminal
            .draw(|frame| {
                let mut ctx = RenderCtx::new();
                control.render(frame, area, &mut ctx);
            })
            .expect("control should render");
        terminal
            .backend()
            .buffer()
            .cell((47, 4))
            .expect("hotkey prefix cell")
            .modifier
            .contains(Modifier::UNDERLINED)
    };

    let mut pending_ctx = EventCtx::default();
    control.dispatch_event(
        &data_route(),
        &TuiEvent::Hotkey(HotkeyEvent::Pending("l".into())),
        &mut pending_ctx,
    );
    assert!(pending_ctx.redraw_requested());
    assert!(badge_l_is_underlined(&control));

    let mut canceled_ctx = EventCtx::default();
    control.dispatch_event(
        &data_route(),
        &TuiEvent::Hotkey(HotkeyEvent::Canceled),
        &mut canceled_ctx,
    );
    assert!(canceled_ctx.redraw_requested());
    assert!(!badge_l_is_underlined(&control));

    let mut unchanged_ctx = EventCtx::default();
    control.dispatch_event(
        &data_route(),
        &TuiEvent::Hotkey(HotkeyEvent::Canceled),
        &mut unchanged_ctx,
    );
    assert!(!unchanged_ctx.redraw_requested());

    control.dispatch_event(
        &data_route(),
        &TuiEvent::Hotkey(HotkeyEvent::Pending("l".into())),
        &mut EventCtx::default(),
    );
    let mut commit_ctx = EventCtx::default();
    control.dispatch_event(
        &data_route(),
        &TuiEvent::Hotkey(HotkeyEvent::Commit("le".into())),
        &mut commit_ctx,
    );
    assert!(commit_ctx.redraw_requested());
    assert!(!badge_l_is_underlined(&control));
}

#[test]
fn data_view_yank_copies_exact_visible_column_json() {
    let mut view = DataView::new([(1, "Ada".to_string(), "Ready".to_string())], |row| row.0)
        .columns([
            Column::text(
                "name",
                "Name",
                Constraint::Fill(1),
                |row: &(usize, String, String)| row.1.clone(),
            ),
            Column::text(
                "state",
                "State",
                Constraint::Fill(1),
                |row: &(usize, String, String)| row.2.clone(),
            ),
        ]);
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.event(&TuiEvent::Yank, &mut ctx);

    assert_eq!(outcome, tuicore::EventOutcome::Handled);
    assert_eq!(
        ctx.clipboard_request(),
        Some(r#"{"name":"Ada","state":"Ready"}"#)
    );
}

#[test]
fn data_view_default_yank_excludes_hidden_columns() {
    let mut view = DataView::new([(1, "Ada", "secret")], |row| row.0).columns([
        Column::text(
            "name",
            "Name",
            Constraint::Fill(1),
            |row: &(usize, &str, &str)| row.1.to_string(),
        ),
        Column::text(
            "private",
            "Private",
            Constraint::Fill(1),
            |row: &(usize, &str, &str)| row.2.to_string(),
        )
        .hidden(),
    ]);
    let mut ctx = EventCtx::<()>::default();

    view.event(&TuiEvent::Yank, &mut ctx);

    assert_eq!(ctx.clipboard_request(), Some(r#"{"name":"Ada"}"#));
}

#[test]
fn empty_data_view_yank_makes_no_clipboard_request() {
    let mut view = DataView::<Row, usize>::new([], |row| row.id).column(Column::text(
        "name",
        "Name",
        Constraint::Fill(1),
        |row: &Row| row.name.clone(),
    ));
    let mut ctx = EventCtx::<()>::default();

    let outcome = view.event(&TuiEvent::Yank, &mut ctx);

    assert_eq!(outcome, tuicore::EventOutcome::Handled);
    assert_eq!(ctx.clipboard_request(), None);
}

#[test]
fn list_control_copy_formatter_selects_exact_row_value() {
    let mut control = control([Row {
        id: 1,
        name: "Ada".into(),
    }])
    .copy_with(|row| format!("person:{}", row.name));
    let mut ctx = EventCtx::default();

    control.dispatch_event(&data_route(), &TuiEvent::Yank, &mut ctx);

    assert_eq!(ctx.clipboard_request(), Some("person:Ada"));
}

#[test]
fn custom_copy_follows_highlighted_id_after_sorting_and_reordering() {
    let mut sorted: ListControl<Row, usize> = ListControl::new(
        [
            Row {
                id: 1,
                name: "Beta".into(),
            },
            Row {
                id: 2,
                name: "Alpha".into(),
            },
        ],
        |row| row.id,
        |_, _| unreachable!(),
    )
    .column(
        Column::text("name", "Name", Constraint::Fill(1), |row: &Row| {
            row.name.clone()
        })
        .sortable(|row| row.name.clone()),
    )
    .copy_with(|row| row.name.clone());
    sorted.data_view_mut().highlight_id(&1);
    sorted
        .data_view_mut()
        .sort_by("name", SortDirection::Ascending);
    let mut sorted_ctx = EventCtx::<()>::default();
    sorted.dispatch_event(&data_route(), &TuiEvent::Yank, &mut sorted_ctx);
    assert_eq!(sorted.data_view().highlighted_id(), Some(1));
    assert_eq!(sorted_ctx.clipboard_request(), Some("Beta"));

    let mut reordered = ranked_control(ranked_rows()).copy_with(|row| format!("row-{}", row.id));
    reordered.dispatch_event(&data_route(), &reorder_key(), &mut EventCtx::default());
    reordered.dispatch_event(
        &data_route(),
        &key(Key::Down, KeyModifiers::NONE),
        &mut EventCtx::default(),
    );
    let mut reordered_ctx = EventCtx::<()>::default();
    reordered.dispatch_event(&data_route(), &TuiEvent::Yank, &mut reordered_ctx);
    assert_eq!(reordered.data_view().highlighted_id(), Some(1));
    assert_eq!(reordered_ctx.clipboard_request(), Some("row-1"));
}

#[test]
fn active_list_editor_owns_yank() {
    let mut control = control([Row {
        id: 1,
        name: "Existing".into(),
    }])
    .copy_with(|row| format!("row:{}", row.name));
    control.dispatch_event(&data_route(), &add_key(), &mut EventCtx::default());
    control.dispatch_event(
        &input_route(),
        &TuiEvent::Paste("Draft".into()),
        &mut EventCtx::default(),
    );
    let mut ctx = EventCtx::default();

    control.dispatch_event(&input_route(), &TuiEvent::Yank, &mut ctx);

    assert_eq!(ctx.clipboard_request(), Some("Draft"));
}
