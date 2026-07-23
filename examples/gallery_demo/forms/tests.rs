use ratatui::layout::Rect;
use tuicore::{
    AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute, ExternalEditorResponse, FocusCtx,
    FocusId, FocusRequest, FocusTarget, Key, KeyEvent, KeyModifiers, LayoutCtx, LayoutProposal,
    RenderCtx, TreePath, TuiEvent, TuiNode,
};

use super::super::super::Msg;
use super::component::{FormControlId, ValidatedForm, demo_date};
use super::model::FormError;

fn focus_field(form: &mut ValidatedForm, control: FormControlId, id: &str, focused: bool) {
    let target = FocusTarget {
        id: FocusId::new(id),
        path: TreePath::from_keys([control.key(), ChildKey::body()]),
        area: Rect::default(),
        enabled: true,
        tab_stop: true,
        hotkey: None,
        hotkeys: Vec::new(),
        hotkey_sequences: Vec::new(),
        suppress_global_hotkeys: false,
        focused_events_before_global_hotkeys: false,
    };
    form.dispatch_focus(&target, focused, &mut FocusCtx::default());
}

fn dispatch_key(form: &mut ValidatedForm, control: FormControlId, key: KeyEvent) -> EventCtx<Msg> {
    dispatch_event(form, control, &TuiEvent::Key(key))
}

fn dispatch_event(
    form: &mut ValidatedForm,
    control: FormControlId,
    event: &TuiEvent,
) -> EventCtx<Msg> {
    let route = EventRoute::new(TreePath::from_keys([control.key()]));
    let mut ctx = EventCtx::default();
    form.dispatch_event(&route, event, &mut ctx);
    ctx
}

fn apply_messages(form: &mut ValidatedForm, ctx: &EventCtx<Msg>) {
    for message in ctx.messages() {
        assert!(form.apply_message(message));
    }
}

fn focus_id(control: FormControlId) -> &'static str {
    match control {
        FormControlId::Name => "input",
        FormControlId::Description => "textarea",
        FormControlId::Password => "password-input",
        FormControlId::Start | FormControlId::End => "date-picker-dropdown",
        FormControlId::Environment => "field",
        FormControlId::Tags => "tag-input",
    }
}

fn field_is_active(form: &ValidatedForm, control: FormControlId) -> bool {
    match control {
        FormControlId::Name => form.name.child().insert_mode(),
        FormControlId::Description => form.description.child().insert_mode(),
        FormControlId::Password => form.password.child().insert_mode(),
        FormControlId::Start => form.start.child().is_open(),
        FormControlId::End => form.end.child().is_open(),
        FormControlId::Environment => form.environment.child().is_open(),
        FormControlId::Tags => form.tags.child().is_active(),
    }
}

fn model_control_is_editing(form: &ValidatedForm, control: FormControlId) -> bool {
    let controls = form.model.controls();
    match control {
        FormControlId::Name => controls.name.editing(),
        FormControlId::Description => controls.description.editing(),
        FormControlId::Password => controls.password.editing(),
        FormControlId::Start => controls.start.editing(),
        FormControlId::End => controls.end.editing(),
        FormControlId::Environment => controls.environment.editing(),
        FormControlId::Tags => controls.tags.editing(),
    }
}

fn create_tag(form: &mut ValidatedForm, label: &str) {
    for value in label.chars() {
        dispatch_key(form, FormControlId::Tags, KeyEvent::from(Key::Char(value)));
    }
    form.tags.dispatch_event(
        &EventRoute::new(TreePath::default()),
        &TuiEvent::Key(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut EventCtx::default(),
    );
    form.tags.child_mut().take_events();
    form.sync_tags_from_component();
}

#[test]
fn focus_and_blur_leave_every_field_pristine_without_visible_errors() {
    let mut form = ValidatedForm::new();
    for (control, id) in [
        (FormControlId::Name, "input"),
        (FormControlId::Description, "textarea"),
        (FormControlId::Password, "password-input"),
        (FormControlId::Start, "date-picker-dropdown"),
        (FormControlId::End, "date-picker-dropdown"),
        (FormControlId::Environment, "field"),
        (FormControlId::Tags, "tag-input"),
    ] {
        focus_field(&mut form, control, id, true);
        dispatch_key(&mut form, control, KeyEvent::from(Key::Tab));
        focus_field(&mut form, control, id, false);
    }

    let controls = form.model.controls();
    assert!(controls.name.pristine() && !controls.name.touched() && !controls.name.editing());
    assert!(controls.description.pristine() && !controls.description.touched());
    assert!(controls.password.pristine() && !controls.password.touched());
    assert!(controls.start.pristine() && !controls.start.touched());
    assert!(controls.end.pristine() && !controls.end.touched());
    assert!(controls.environment.pristine() && !controls.environment.touched());
    assert!(controls.tags.pristine() && !controls.tags.touched());
    assert!(form.name.error().is_none());
    assert!(form.environment.error().is_none());
}

#[test]
fn group_error_stays_presented_until_end_date_trigger() {
    let mut form = ValidatedForm::new();
    let start = demo_date();
    form.begin_edit(FormControlId::Start);
    form.select_start(start);
    form.end_edit(FormControlId::Start);
    form.begin_edit(FormControlId::End);
    form.select_end(start);
    form.end_edit(FormControlId::End);
    assert_eq!(form.model.errors(), &[FormError::EndNotAfterStart]);
    assert!(form.model.controls().end.errors().is_empty());
    assert_eq!(form.end.error(), Some("End date must be after start date"));

    form.begin_edit(FormControlId::End);
    form.select_end(start.next_day().expect("next day should exist"));
    assert!(form.model.errors().is_empty());
    assert_eq!(
        form.model.presented_errors(),
        &[FormError::EndNotAfterStart]
    );
    assert_eq!(form.end.error(), Some("End date must be after start date"));
    form.end_edit(FormControlId::End);
    assert!(form.model.presented_errors().is_empty());
    assert_eq!(form.end.error(), None);
}

#[test]
fn inactive_enter_requests_one_edit_and_activates_every_field() {
    for control in FormControlId::ALL {
        let mut form = ValidatedForm::new();
        focus_field(&mut form, control, focus_id(control), true);

        let enter = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));

        assert_eq!(
            enter.messages(),
            &[Msg::FormSubmitRequested(control)],
            "{control:?}"
        );
        apply_messages(&mut form, &enter);
        assert!(field_is_active(&form, control), "{control:?}");
        assert!(model_control_is_editing(&form, control), "{control:?}");
        assert!(!form.model.submitted());
    }
}

#[test]
fn inactive_control_hjkl_navigates_only_between_controls_in_mixed_form() {
    for (index, control) in FormControlId::ALL.into_iter().enumerate() {
        let mut form = ValidatedForm::new();
        let forward = dispatch_key(
            &mut form,
            control,
            KeyEvent {
                code: Key::Char('j'),
                modifiers: KeyModifiers::CONTROL,
            },
        );
        assert_eq!(
            forward.focus_request(),
            (index + 1 < FormControlId::ALL.len()).then_some(&FocusRequest::Next),
            "forward {control:?}"
        );

        let backward = dispatch_key(
            &mut form,
            control,
            KeyEvent {
                code: Key::Char('h'),
                modifiers: KeyModifiers::CONTROL,
            },
        );
        assert_eq!(
            backward.focus_request(),
            (index > 0).then_some(&FocusRequest::Previous),
            "backward {control:?}"
        );
    }
}

#[test]
fn active_mixed_form_control_keeps_hjkl_for_its_component() {
    let mut form = ValidatedForm::new();
    focus_field(&mut form, FormControlId::Name, "input", true);
    dispatch_key(&mut form, FormControlId::Name, KeyEvent::from(Key::Enter));

    let ctx = dispatch_key(
        &mut form,
        FormControlId::Name,
        KeyEvent::from(Key::Char('l')),
    );

    assert!(ctx.focus_request().is_none());
    assert_eq!(form.name.child().current_value(), "l");
}

#[test]
fn plain_hjkl_bypasses_form_navigation_and_reaches_date_picker() {
    let mut form = ValidatedForm::new();
    let control = FormControlId::Start;
    focus_field(&mut form, control, focus_id(control), true);
    let activation = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));
    apply_messages(&mut form, &activation);

    let navigation = dispatch_key(&mut form, control, KeyEvent::from(Key::Char('l')));
    assert!(navigation.focus_request().is_none());
    let selection = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));
    apply_messages(&mut form, &selection);

    assert_eq!(
        form.start.child().current_value(),
        Some(demo_date().next_day().expect("demo date should advance"))
    );
}

#[test]
fn active_enter_never_requests_submit() {
    for control in FormControlId::ALL {
        let mut form = ValidatedForm::new();
        focus_field(&mut form, control, focus_id(control), true);
        let activation = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));
        apply_messages(&mut form, &activation);

        let active_enter = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));

        assert!(
            !active_enter
                .messages()
                .contains(&Msg::FormSubmitRequested(control)),
            "{control:?}: {:?}",
            active_enter.messages()
        );
        apply_messages(&mut form, &active_enter);
    }
}

#[test]
fn ctrl_enter_submits_exactly_once_for_every_field_except_textarea() {
    let shortcut = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };
    for control in FormControlId::ALL {
        let mut form = ValidatedForm::new();
        focus_field(&mut form, control, focus_id(control), true);
        let expected_submit_count = usize::from(control != FormControlId::Description);

        let inactive = dispatch_key(&mut form, control, shortcut);
        assert_eq!(
            inactive
                .messages()
                .iter()
                .filter(|message| **message == Msg::FormSubmitAttempt)
                .count(),
            expected_submit_count,
            "inactive {control:?}: {:?}",
            inactive.messages()
        );
        apply_messages(&mut form, &inactive);

        let activation = dispatch_key(&mut form, control, KeyEvent::from(Key::Enter));
        apply_messages(&mut form, &activation);
        let active = dispatch_key(&mut form, control, shortcut);
        assert_eq!(
            active
                .messages()
                .iter()
                .filter(|message| **message == Msg::FormSubmitAttempt)
                .count(),
            expected_submit_count,
            "active {control:?}: {:?}",
            active.messages()
        );
        apply_messages(&mut form, &active);
    }
}

#[test]
fn external_editor_routes_one_edit_session_for_text_textarea_and_date() {
    for control in [
        FormControlId::Name,
        FormControlId::Description,
        FormControlId::Start,
    ] {
        let mut form = ValidatedForm::new();
        focus_field(&mut form, control, focus_id(control), true);
        let launch = dispatch_key(
            &mut form,
            control,
            KeyEvent {
                code: Key::Char('o'),
                modifiers: KeyModifiers::CONTROL,
            },
        );
        assert_eq!(
            launch.messages(),
            &[Msg::FormSubmitRequested(control)],
            "{control:?}"
        );
        apply_messages(&mut form, &launch);
        assert!(model_control_is_editing(&form, control));

        let value = match control {
            FormControlId::Name => "release",
            FormControlId::Description => "release notes",
            FormControlId::Start => "2026-07-20",
            _ => unreachable!(),
        };
        let response = dispatch_event(
            &mut form,
            control,
            &TuiEvent::ExternalEditor(ExternalEditorResponse {
                value: value.to_string(),
                line: 1,
                col: 1,
            }),
        );
        assert_eq!(
            response
                .messages()
                .iter()
                .filter(|message| { **message == Msg::FormControlEditEnded(control) })
                .count(),
            1,
            "{control:?}: {:?}",
            response.messages()
        );
        apply_messages(&mut form, &response);
        assert!(!model_control_is_editing(&form, control));

        let target = FocusTarget {
            id: FocusId::new(focus_id(control)),
            path: TreePath::from_keys([control.key(), ChildKey::body()]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        };
        let mut focus = FocusCtx::default();
        form.dispatch_focus(&target, false, &mut focus);
        assert_eq!(focus.drain_messages().count(), 0, "{control:?}");
    }
}

#[test]
fn submit_attempt_reveals_current_errors_without_changing_edit_state() {
    let mut form = ValidatedForm::new();
    form.begin_edit(FormControlId::Name);
    form.submit_attempt();

    assert!(form.model.submitted());
    assert_eq!(form.status, "Submitted • invalid");
    assert_eq!(form.name.error(), Some("Name is required"));
    assert_eq!(form.description.error(), Some("Description is required"));
    assert_eq!(form.password.error(), Some("Password is required"));
    assert_eq!(form.start.error(), Some("Start date is required"));
    assert_eq!(form.end.error(), Some("End date is required"));
    assert_eq!(form.environment.error(), Some("Environment is required"));
    assert_eq!(form.tags.error(), Some("Select at least 2 tags"));
    assert!(form.model.controls().name.editing());
    assert!(form.model.controls().name.pristine());
}

#[test]
fn ctrl_enter_in_active_textarea_only_ends_edit() {
    let mut form = ValidatedForm::new();
    focus_field(&mut form, FormControlId::Description, "textarea", true);
    form.description.child_mut().set_insert_mode(true);
    form.begin_edit(FormControlId::Description);
    let exit = dispatch_key(
        &mut form,
        FormControlId::Description,
        KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        },
    );
    assert_eq!(
        exit.messages(),
        &[Msg::FormControlEditEnded(FormControlId::Description)]
    );
    assert!(!form.description.child().insert_mode());
    assert!(!form.model.submitted());
}

#[test]
fn on_input_exit_retains_old_error_during_valid_and_invalid_edits() {
    let mut form = ValidatedForm::new();
    form.begin_edit(FormControlId::Name);
    form.end_edit(FormControlId::Name);
    assert_eq!(form.name.error(), Some("Name is required"));

    form.begin_edit(FormControlId::Name);
    form.change_name("x".to_string());
    assert_eq!(
        form.model.controls().name.errors(),
        &[FormError::NameTooShort]
    );
    assert_eq!(form.name.error(), Some("Name is required"));
    form.end_edit(FormControlId::Name);
    assert_eq!(
        form.name.error(),
        Some("Name must be at least 3 characters")
    );

    form.begin_edit(FormControlId::Name);
    form.change_name("valid".to_string());
    assert!(form.model.controls().name.errors().is_empty());
    assert_eq!(
        form.name.error(),
        Some("Name must be at least 3 characters")
    );
    form.end_edit(FormControlId::Name);
    assert_eq!(form.name.error(), None);
}

#[test]
fn on_input_error_refreshes_immediately() {
    let mut form = ValidatedForm::new();
    form.begin_edit(FormControlId::Password);
    form.change_password("short".to_string());
    assert_eq!(
        form.password.error(),
        Some("Password must be at least 8 characters")
    );
    form.end_edit(FormControlId::Password);
    form.begin_edit(FormControlId::Password);
    form.change_password("long enough".to_string());
    assert_eq!(form.password.error(), None);
}

#[test]
fn date_and_dropdown_reveal_errors_when_closed() {
    let mut form = ValidatedForm::new();
    focus_field(
        &mut form,
        FormControlId::Start,
        "date-picker-dropdown",
        true,
    );
    dispatch_key(&mut form, FormControlId::Start, KeyEvent::from(Key::Enter));
    form.begin_edit(FormControlId::Start);
    let close = dispatch_key(&mut form, FormControlId::Start, KeyEvent::from(Key::Esc));
    assert_eq!(
        close.messages(),
        &[Msg::FormControlEditEnded(FormControlId::Start)]
    );
    form.end_edit(FormControlId::Start);
    assert_eq!(form.start.error(), Some("Start date is required"));

    focus_field(&mut form, FormControlId::Environment, "field", true);
    dispatch_key(
        &mut form,
        FormControlId::Environment,
        KeyEvent::from(Key::Enter),
    );
    form.begin_edit(FormControlId::Environment);
    let close = dispatch_key(
        &mut form,
        FormControlId::Environment,
        KeyEvent::from(Key::Esc),
    );
    assert_eq!(
        close.messages(),
        &[Msg::FormControlEditEnded(FormControlId::Environment)]
    );
    form.end_edit(FormControlId::Environment);
    assert_eq!(form.environment.error(), Some("Environment is required"));
}

#[test]
fn required_dropdown_starts_empty_and_clears_after_selection() {
    let mut form = ValidatedForm::new();
    assert_eq!(form.environment.child().selected_id(), None);
    focus_field(&mut form, FormControlId::Environment, "field", true);
    dispatch_key(
        &mut form,
        FormControlId::Environment,
        KeyEvent::from(Key::Enter),
    );
    form.begin_edit(FormControlId::Environment);
    dispatch_key(
        &mut form,
        FormControlId::Environment,
        KeyEvent::from(Key::Enter),
    );
    assert_eq!(form.environment.child().selected_id(), Some("dev"));
    assert_eq!(form.model.controls().environment.value(), &Some("dev"));
    assert!(form.model.controls().environment.errors().is_empty());
    assert!(form.environment.error().is_none());
}

#[test]
fn tag_bounds_refresh_only_after_input_exit() {
    let mut form = ValidatedForm::new();
    focus_field(&mut form, FormControlId::Tags, "tag-input", true);
    dispatch_key(&mut form, FormControlId::Tags, KeyEvent::from(Key::Enter));
    form.begin_edit(FormControlId::Tags);
    create_tag(&mut form, "one");
    assert!(form.tags.error().is_none());
    form.end_edit(FormControlId::Tags);
    assert_eq!(form.tags.error(), Some("Select at least 2 tags"));
    for tag in ["two", "three", "four", "five"] {
        create_tag(&mut form, tag);
    }
    assert_eq!(form.tags.error(), Some("Select at least 2 tags"));
    form.end_edit(FormControlId::Tags);
    assert_eq!(form.tags.error(), Some("Select no more than 4 tags"));
}

#[test]
fn form_field_measurement_grows_only_when_error_becomes_visible() {
    let mut form = ValidatedForm::new();
    let proposal = LayoutProposal {
        width: AxisProposal::Exact(80),
        height: AxisProposal::Unbounded,
    };
    let pristine_height = form.name.measure(proposal).preferred.height;
    form.begin_edit(FormControlId::Name);
    form.change_name("x".to_string());
    form.end_edit(FormControlId::Name);
    assert_eq!(
        form.name.measure(proposal).preferred.height,
        pristine_height + 1
    );
}

#[test]
fn environment_uses_one_semantic_error_border_title_and_message() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let mut form = ValidatedForm::new();
    form.begin_edit(FormControlId::Environment);
    form.end_edit(FormControlId::Environment);
    let area = Rect::new(0, 0, 30, 4);
    form.environment.layout(area, &mut LayoutCtx::new());
    let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
    terminal
        .draw(|frame| form.environment.render(frame, area, &mut RenderCtx::new()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    let error = tuicore::theme().error_fg();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "╭");
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, error);
    assert_eq!(buffer.cell((3, 0)).unwrap().symbol(), "E");
    assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "E");
    assert_eq!(
        (0..4)
            .flat_map(|y| (0..30).map(move |x| (x, y)))
            .filter(|position| buffer.cell(*position).unwrap().symbol() == "╭")
            .count(),
        1
    );
}
