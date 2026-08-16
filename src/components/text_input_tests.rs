use super::*;
use crate::{FocusRequest, MouseButton, MouseEvent, MouseEventKind, Propagation, TreePath};
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn plain_character_bubbles_before_insert_mode_for_text_and_password() {
    let mut input = TextInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(input.current_value(), "");
    assert_eq!(ctx.propagation(), Propagation::Continue);

    let mut input = PasswordInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(input.current_value(), "");
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn text_and_password_mark_focus_as_text_entry_while_typing() {
    let mut input = TextInput::<()>::new();
    input.insert_mode = true;
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 10, 1), &mut ctx);

    let target = ctx.focus_targets().first().unwrap();
    assert!(target.suppress_global_hotkeys);
    assert!(target.focused_events_before_global_hotkeys);

    let mut input = PasswordInput::<()>::new();
    input.input.insert_mode = true;
    let mut ctx = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 10, 1), &mut ctx);

    let target = ctx.focus_targets().first().unwrap();
    assert!(target.suppress_global_hotkeys);
    assert!(target.focused_events_before_global_hotkeys);
}

#[test]
fn text_input_panel_style_adds_border_space_and_focuses_inner_area() {
    let mut input = TextInput::<()>::new().placeholder("Name").panel("Label");
    let hint = input.measure(LayoutProposal::unbounded());
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(2, 3, 12, 3), &mut ctx);

    assert_eq!(hint.preferred.height, 3);
    assert_eq!(ctx.focus_targets()[0].area, Rect::new(3, 4, 10, 1));
}

#[test]
fn text_input_panel_style_moves_hotkey_to_panel() {
    let mut input = TextInput::<()>::new()
        .placeholder("Name")
        .hotkey("n")
        .style(InputChrome::panel("Label").top_right("Required"));
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(2, 3, 12, 3), &mut ctx);

    assert_eq!(line_text(&input.line(20)), "Name");
    assert_eq!(ctx.focus_targets()[0].area, Rect::new(3, 4, 10, 1));
    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["n"]);
}

#[test]
fn text_input_registers_action_and_editor_hotkeys_separately() {
    let mut input = TextInput::<()>::new().hotkey("pa").editor_hotkey("pb");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 1), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["pa", "pb"]);
    assert_eq!(line_text(&input.line(20)), " |pa·pb|");
}

#[test]
fn text_input_action_and_editor_hotkeys_have_distinct_behavior() {
    let mut input = TextInput::<()>::new()
        .value("draft")
        .hotkey("pa")
        .editor_hotkey("pb");

    let mut action = EventCtx::default();
    assert_eq!(
        input.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("pa".into())),
            &mut action,
        ),
        EventOutcome::Handled
    );
    assert!(input.insert_mode());
    assert!(action.external_editor_request().is_none());

    input.set_insert_mode(false);
    let mut editor = EventCtx::default();
    assert_eq!(
        input.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("pb".into())),
            &mut editor,
        ),
        EventOutcome::Handled
    );
    assert_eq!(
        editor.external_editor_request(),
        Some(&crate::ExternalEditorRequest {
            value: "draft".into(),
            line: 1,
            col: 6,
            file_extension: None,
        })
    );
}

#[test]
fn text_input_dual_panel_badge_highlights_shared_pending_prefix() {
    let mut input = TextInput::<()>::new()
        .hotkey("pa")
        .editor_hotkey("pb")
        .panel("Label");
    input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
        &mut EventCtx::default(),
    );
    let mut terminal = Terminal::new(TestBackend::new(20, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("input should render");

    let buffer = terminal.backend().buffer();
    let bottom = (0..20)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(bottom.contains("┤pa·pb│"));
    assert_eq!(
        (0..20)
            .filter(|x| {
                let cell = buffer.cell((*x, 2)).unwrap();
                cell.symbol() == "p" && cell.modifier.contains(Modifier::UNDERLINED)
            })
            .count(),
        2
    );
}

#[test]
fn disabled_text_input_suppresses_editor_hotkey() {
    let mut input = TextInput::<()>::new()
        .value("locked")
        .hotkey("pa")
        .editor_hotkey("pb")
        .disabled(true);
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 1), &mut layout);
    let mut event = EventCtx::default();

    input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("pb".into())),
        &mut event,
    );

    assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["pa"]);
    assert_eq!(line_text(&input.line(20)), "locked |pa|");
    assert!(event.external_editor_request().is_none());
    assert!(!input.insert_mode());
}

#[test]
fn text_input_panel_click_requests_input_focus() {
    let mut input = TextInput::<()>::new().panel("Label");
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(2, 3, 12, 3), &mut layout);
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: 3,
            modifiers: KeyModifiers::NONE,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.focus_request(),
        Some(&FocusRequest::TargetAt {
            path: TreePath::new(),
            id: FocusId::new("input"),
        })
    );
}

#[test]
fn focused_text_input_uses_strong_selection_highlight_before_insert_mode() {
    let input = TextInput::<()>::new().value("search").focused(true);
    let line = input.line(20);

    assert!(
        line.spans
            .iter()
            .all(|span| span.style.bg == Some(theme().highlight_bg()))
    );
    assert!(
        line.spans
            .iter()
            .all(|span| span.style.fg == Some(theme().highlight_fg()))
    );
}

#[test]
fn control_enter_only_finishes_active_text_and_password_edits() {
    let control_enter = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };
    let mut inactive = TextInput::<()>::new().value("ship").focused(true);
    let mut ctx = EventCtx::default();
    assert_eq!(
        inactive.event(&TuiEvent::Key(control_enter), &mut ctx),
        EventOutcome::Ignored
    );
    assert!(!inactive.insert_mode());
    assert_eq!(ctx.propagation(), Propagation::Continue);

    let mut input = TextInput::new()
        .value("ship")
        .on_edit_end(|value| format!("end:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let outcome = input.event(&TuiEvent::Key(control_enter), &mut ctx);
    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode());
    assert_eq!(ctx.messages(), &["end:ship".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);

    let mut input = PasswordInput::<()>::new().value("secret").focused(true);
    let mut ctx = EventCtx::default();
    let outcome = input.event(&TuiEvent::Key(control_enter), &mut ctx);
    assert!(!input.insert_mode());
    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(ctx.propagation(), Propagation::Continue);

    let mut input = PasswordInput::new()
        .value("secret")
        .on_edit_end(|value| format!("end:{value}"));
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let outcome = input.event(&TuiEvent::Key(control_enter), &mut ctx);
    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode());
    assert_eq!(ctx.messages(), &["end:secret".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn insert_mode_enter_finishes_text_edit_without_submit_message() {
    let mut input = TextInput::new()
        .value("ship")
        .on_submit(|value| format!("submit:{value}"))
        .on_edit_end(|value| format!("end:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode);
    assert_eq!(ctx.messages(), &["end:ship".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
}

#[test]
fn text_input_emits_one_change_only_for_each_actual_mutation() {
    let mut input = TextInput::new()
        .value("a")
        .focused(true)
        .on_change(|value| format!("change:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('b'))), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Left)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);
    input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('c'),
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );
    input.event(&TuiEvent::Paste("z".into()), &mut ctx);

    assert_eq!(
        ctx.messages(),
        &[
            "change:ab".to_string(),
            "change:a".to_string(),
            "change:".to_string(),
            "change:z".to_string(),
        ]
    );
}

#[test]
fn focused_text_input_submit_emits_once_and_enters_insert_mode() {
    let mut input = TextInput::new()
        .value("ship")
        .focused(true)
        .on_submit(|value| format!("submit:{value}"));
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert!(input.insert_mode);
    assert_eq!(ctx.messages(), &["submit:ship".to_string()]);
}

#[test]
fn insert_mode_enter_finishes_password_edit_without_submit_message() {
    let mut input = PasswordInput::new()
        .value("secret")
        .on_submit(|value| format!("submit:{value}"))
        .on_edit_end(|value| format!("end:{value}"));
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.input.insert_mode);
    assert_eq!(ctx.messages(), &["end:secret".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
}

#[test]
fn password_input_change_callback_receives_actual_value_once_per_mutation() {
    let mut input = PasswordInput::new()
        .value("secret")
        .focused(true)
        .on_change(|value| format!("change:{value}"));
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('!'))), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Left)), &mut ctx);
    input.event(&TuiEvent::Paste("?".into()), &mut ctx);

    assert_eq!(
        ctx.messages(),
        &["change:secret!".to_string(), "change:secret?!".to_string(),]
    );
    assert_eq!(line_text(&input.line(20)), "••••••••");
}

#[test]
fn password_enter_without_submit_callback_preserves_enter_to_edit() {
    let mut input = PasswordInput::<()>::new().value("secret").focused(true);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.input.insert_mode);
    assert!(ctx.messages().is_empty());
}

#[test]
fn delete_key_variants_remove_next_character() {
    for key in [
        KeyEvent::from(Key::Delete),
        KeyEvent {
            code: Key::Delete,
            modifiers: KeyModifiers::SHIFT,
        },
        KeyEvent::from(Key::Char('\u{7f}')),
        KeyEvent {
            code: Key::Char('\u{7f}'),
            modifiers: KeyModifiers::CONTROL,
        },
    ] {
        let mut input = TextInput::<()>::new().value("abcd");
        input.insert_mode = true;
        input.cursor = 1;

        assert_eq!(input.on_key(key), InputOutcome::CHANGED, "key: {key:?}");
        assert_eq!(input.current_value(), "acd", "key: {key:?}");
        assert_eq!(input.cursor, 1, "key: {key:?}");
    }
}

#[test]
fn delete_removes_next_character_before_insert_mode_in_text_input() {
    let mut input = TextInput::<()>::new().value("abcd").focused(true);
    input.cursor = 1;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "acd");
    assert!(input.insert_mode);
    assert!(ctx.layout_requested());
}

#[test]
fn delete_removes_next_character_before_insert_mode_in_password_input() {
    let mut input = PasswordInput::<()>::new().value("abcd").focused(true);
    input.input.cursor = 1;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "acd");
    assert!(input.input.insert_mode);
    assert!(ctx.layout_requested());
}

#[test]
fn control_c_clears_value_and_stops_propagation() {
    let mut input = TextInput::<()>::new().value("search");
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();
    let key = KeyEvent {
        code: Key::Char('c'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn tab_and_control_i_insert_tab_and_stop_propagation() {
    for key in [
        KeyEvent::from(Key::Tab),
        KeyEvent {
            code: Key::Char('i'),
            modifiers: KeyModifiers::CONTROL,
        },
    ] {
        let mut input = TextInput::<()>::new().value("left");
        input.insert_mode = true;
        let mut ctx = EventCtx::<()>::default();

        assert_eq!(
            input.event(&TuiEvent::Key(key), &mut ctx),
            EventOutcome::Handled,
            "key: {key:?}"
        );
        assert_eq!(line_text(&input.line(10)), "left    ", "key: {key:?}");
        assert_eq!(ctx.propagation(), Propagation::Stopped, "key: {key:?}");
        assert!(ctx.redraw_requested(), "key: {key:?}");
    }
}

#[test]
fn line_clips_wide_unicode_by_terminal_width() {
    let input = TextInput::<()>::new().value("ab界d");

    let line = input.line(4);

    assert_eq!(line_text(&line), "ab界");
    assert_eq!(cell_width(&line_text(&line)), 4);
}

#[test]
fn custom_submit_key_replaces_default_enter() {
    let keys = TextInputKeyBindings {
        submit: vec![KeySpec::plain('s')],
        ..TextInputKeyBindings::default()
    };
    let mut input = TextInput::<()>::new().keybindings(keys);

    assert_eq!(
        input.on_key(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::NONE,
        }),
        InputOutcome::IDLE
    );
    assert!(input.on_key(KeyEvent::from(Key::Char('s'))).submitted);
}

#[test]
fn focused_placeholder_draws_cursor_over_first_character() {
    let mut input = TextInput::<()>::new().placeholder("Ask").focused(true);
    input.insert_mode = true;

    let line = input.line(3);

    assert_eq!(line.spans[0].content.as_ref(), "A");
    assert_eq!(line_text(&line), "Ask");
}

#[test]
fn text_input_hotkey_rendering_tracks_content_and_insert_mode() {
    let cases = [
        (
            TextInput::<()>::new().placeholder("Ask").hotkey("p"),
            "Ask |p|",
        ),
        (TextInput::new().value("Ask").hotkey("i"), "Ask |i|"),
        (
            TextInput::new().value("Ask").hotkey("i").focused(true),
            "Ask |i|",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(line_text(&input.line(20)), expected);
    }

    let mut input = TextInput::<()>::new()
        .value("Ask")
        .hotkey("i")
        .focused(true);
    input.insert_mode = true;
    assert_eq!(line_text(&input.line(20)), "Ask ");
}

#[test]
fn pending_hotkey_underlines_text_input_hotkey() {
    let mut input = TextInput::<()>::new().value("Ask").hotkey("i");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("i".into())),
        &mut ctx,
    );
    let line = input.line(20);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(ctx.redraw_requested());
    assert!(line.spans.iter().any(|span| {
        span.content.as_ref() == "i" && span.style.add_modifier.contains(Modifier::UNDERLINED)
    }));
}

#[test]
fn hotkey_commit_enters_insert_mode() {
    let mut input = TextInput::<()>::new().value("Ask").hotkey("i");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Hotkey(HotkeyEvent::Commit("i".into())), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.insert_mode);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn text_input_suppresses_global_hotkeys_only_in_insert_mode() {
    let mut input = TextInput::<()>::new().hotkey("p");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 1), &mut ctx);
    assert!(!ctx.focus_targets()[0].suppress_global_hotkeys);

    input.insert_mode = true;
    let mut insert_ctx = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 1), &mut insert_ctx);

    assert!(insert_ctx.focus_targets()[0].suppress_global_hotkeys);
}

#[test]
fn escape_bubbles_to_parent_policy() {
    let mut input = TextInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);
    let mut parent_observed = false;
    let bubbled = outcome.bubble(&mut ctx, |_ctx| {
        parent_observed = true;
        EventOutcome::Handled
    });

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(bubbled, EventOutcome::Handled);
    assert!(parent_observed);
    assert_eq!(ctx.propagation(), Propagation::Continue);
    assert!(ctx.redraw_requested());
}

#[test]
fn word_navigation_and_deletion() {
    let mut input = TextInput::<()>::new().value("hello world example");
    for (key, cursor) in [
        (modified_key(Key::Left, KeyModifiers::CONTROL), 12),
        (modified_key(Key::Left, KeyModifiers::CONTROL), 6),
        (modified_key(Key::Right, KeyModifiers::CONTROL), 12),
        (modified_key(Key::Right, KeyModifiers::CONTROL), 19),
    ] {
        input.on_key(key);
        assert_eq!(input.cursor, cursor, "key: {key:?}");
    }

    input.cursor = 6;
    input.on_key(modified_key(Key::Backspace, KeyModifiers::CONTROL));
    assert_eq!(input.current_value(), "world example");
    assert_eq!(input.cursor, 0);

    input.set_value("hello world example");
    input.cursor = 6;
    input.on_key(modified_key(Key::Delete, KeyModifiers::CONTROL));
    assert_eq!(input.current_value(), "hello example");
    assert_eq!(input.cursor, 6);

    input.set_value("hello world example");
    input.cursor = 19;
    input.on_key(modified_key(Key::Char('b'), KeyModifiers::ALT));
    assert_eq!(input.cursor, 12);

    input.cursor = 6;
    input.on_key(modified_key(Key::Char('f'), KeyModifiers::ALT));
    assert_eq!(input.cursor, 12);

    input.set_value("hello world example");
    input.cursor = 6;
    input.on_key(modified_key(Key::Char('d'), KeyModifiers::ALT));
    assert_eq!(input.current_value(), "hello example");
    assert_eq!(input.cursor, 6);
}

#[test]
fn deleting_previous_word_only_preserves_separator_when_text_follows_cursor() {
    for (value, cursor, expected, expected_cursor) in [
        ("hello world", 9, "hello ld", 6),
        ("hello world", 11, "hello", 5),
        ("ab cd ef", 6, "ab ef", 3),
        ("ab cd ef", 5, "ab ef", 2),
    ] {
        let mut text = TextInput::<()>::new().value(value);
        let mut password = PasswordInput::<()>::new().value(value);
        text.cursor = cursor;
        password.input.cursor = cursor;
        let key = modified_key(Key::Backspace, KeyModifiers::CONTROL);

        assert_eq!(text.on_key(key), InputOutcome::CHANGED, "value: {value}");
        assert_eq!(
            password.on_key(key),
            InputOutcome::CHANGED,
            "value: {value}"
        );
        assert_eq!(text.current_value(), expected, "value: {value}");
        assert_eq!(text.cursor, expected_cursor, "value: {value}");
        assert_eq!(password.current_value(), expected, "value: {value}");
        assert_eq!(password.input.cursor, expected_cursor, "value: {value}");
    }
}

#[test]
fn ctrl_o_requests_external_editor() {
    let mut input = TextInput::<()>::new().value("initial");
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Char('o'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "initial");
    assert_eq!(
        ctx.external_editor_request(),
        Some(&crate::ExternalEditorRequest {
            value: "initial".to_string(),
            line: 1,
            col: 8,
            file_extension: None,
        })
    );
    assert!(ctx.redraw_requested());
    assert!(!ctx.clear_requested());
}

#[test]
fn text_input_passes_file_extension_to_external_editor_request() {
    let mut input = TextInput::<()>::new()
        .value("query")
        .external_editor_file_extension("sql");
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('o'),
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.external_editor_request(),
        Some(&crate::ExternalEditorRequest {
            value: "query".into(),
            line: 1,
            col: 6,
            file_extension: Some("sql".into()),
        })
    );
}

#[test]
fn inactive_external_editor_session_emits_one_start_and_one_end() {
    let mut input = TextInput::new()
        .value("initial")
        .on_submit(|value| format!("start:{value}"))
        .on_change(|value| format!("change:{value}"))
        .on_edit_end(|value| format!("end:{value}"));
    let mut launch = EventCtx::default();

    input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('o'),
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut launch,
    );

    assert!(input.insert_mode());
    assert_eq!(launch.messages(), &["start:initial".to_string()]);

    let mut response = EventCtx::default();
    input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "edited".to_string(),
            line: 1,
            col: 1,
        }),
        &mut response,
    );
    assert!(!input.insert_mode());
    assert_eq!(
        response.messages(),
        &["change:edited".to_string(), "end:edited".to_string()]
    );

    let mut focus = FocusCtx::default();
    input.focus(None, false, &mut focus);
    assert_eq!(focus.drain_messages().count(), 0);
}

#[test]
fn external_editor_response_updates_value_and_clamps_cursor() {
    let mut input = TextInput::<()>::new().value("initial");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "edited\nvalue".to_string(),
            line: 2,
            col: 99,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "edited value");
    assert_eq!(input.cursor, input.len_chars());
    assert!(!input.insert_mode);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert!(ctx.clear_requested());
}

#[test]
fn external_editor_emits_change_only_when_accepted_value_differs() {
    let mut input = TextInput::new()
        .value("initial")
        .on_change(|value| format!("change:{value}"));
    let mut ctx = EventCtx::default();

    input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "initial".to_string(),
            line: 1,
            col: 1,
        }),
        &mut ctx,
    );
    input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "edited".to_string(),
            line: 1,
            col: 1,
        }),
        &mut ctx,
    );

    assert_eq!(ctx.messages(), &["change:edited".to_string()]);
}

#[test]
fn numbers_only_input_rejects_non_digits_from_keys_and_paste() {
    let mut input = TextInput::<()>::new().numbers_only(true).value("12");
    input.insert_mode = true;

    assert_eq!(
        input.on_key(KeyEvent::from(Key::Char('a'))),
        InputOutcome::HANDLED
    );
    assert_eq!(input.on_paste("3x"), InputOutcome::HANDLED);
    assert_eq!(input.current_value(), "12");

    assert_eq!(
        input.on_key(KeyEvent::from(Key::Char('3'))),
        InputOutcome::CHANGED
    );
    assert_eq!(input.on_paste("45"), InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "12345");
}

#[test]
fn numbers_only_input_filters_programmatic_values() {
    let mut input = TextInput::<()>::new().value("room 101").numbers_only(true);
    assert_eq!(input.current_value(), "101");

    input.set_value("floor 2, room 03");
    assert_eq!(input.current_value(), "203");
}

#[test]
fn numbers_only_input_discards_invalid_external_editor_value_with_warning() {
    let mut input = TextInput::<String>::new()
        .numbers_only(true)
        .value("123")
        .on_change(|value| format!("change:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "12x\n".into(),
            line: 1,
            col: 4,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "123");
    assert!(ctx.messages().is_empty());
    assert_eq!(ctx.notifications().len(), 1);
    assert_eq!(
        ctx.notifications()[0].kind(),
        crate::NotificationKind::Warning
    );
    assert_eq!(ctx.notifications()[0].title(), "Invalid number");
    assert!(!input.insert_mode());
}

#[test]
fn numbers_only_input_accepts_editor_terminal_newline() {
    let mut input = TextInput::<()>::new().numbers_only(true).value("123");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "456\n".into(),
            line: 1,
            col: 4,
        }),
        &mut ctx,
    );

    assert_eq!(input.current_value(), "456");
    assert!(ctx.notifications().is_empty());
}

#[test]
fn numbers_only_input_trims_editor_spaces_before_validation() {
    let mut input = TextInput::<()>::new().numbers_only(true).value("123");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "  456  \n".into(),
            line: 1,
            col: 8,
        }),
        &mut ctx,
    );

    assert_eq!(input.current_value(), "456");
    assert!(ctx.notifications().is_empty());
}

#[test]
fn numbers_only_input_trims_spaces_when_enter_finishes_editing() {
    let mut input = TextInput::new()
        .numbers_only(true)
        .on_change(|value| format!("change:{value}"))
        .on_edit_end(|value| format!("end:{value}"));
    input.value = "  456  ".into();
    input.cursor = input.len_chars();
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "456");
    assert_eq!(
        ctx.messages(),
        &["change:456".to_string(), "end:456".to_string()]
    );
    assert!(!input.insert_mode());
}

#[test]
fn paste_inserts_text_and_collapses_newlines() {
    let mut input = TextInput::<()>::new().value("hello");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Paste(" world\nagain".into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "hello world again");
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn focus_loss_emits_edit_end_once_only_for_active_input() {
    let mut input = TextInput::new()
        .value("hello")
        .on_edit_end(|value| format!("end:{value}"));
    input.insert_mode = true;
    let mut ctx = FocusCtx::new(AnimationSettings::default());

    input.focus(None, false, &mut ctx);
    input.focus(None, false, &mut ctx);

    assert_eq!(
        ctx.drain_messages().collect::<Vec<_>>(),
        vec!["end:hello".to_string()]
    );

    let mut input = TextInput::new().on_edit_end(|value| format!("end:{value}"));
    let mut ctx = FocusCtx::new(AnimationSettings::default());

    input.focus(None, false, &mut ctx);

    assert!(ctx.drain_messages().next().is_none());
}

#[test]
fn password_input_masks_value_without_changing_secret() {
    let mut input = PasswordInput::<()>::new().value("secret").mask_char('*');

    input.on_key(KeyEvent::from(Key::Char('!')));

    assert_eq!(input.current_value(), "secret!");
    assert_eq!(line_text(&input.line(20)), "*******");
}

#[test]
fn password_hotkey_rendering_tracks_content_and_insert_mode() {
    let cases = [
        (
            PasswordInput::<()>::new().placeholder("Secret").hotkey("p"),
            "Secret |p|",
        ),
        (
            PasswordInput::new().value("secret").hotkey("p"),
            "•••••• |p|",
        ),
        (
            PasswordInput::new()
                .value("secret")
                .hotkey("p")
                .focused(true),
            "•••••• |p|",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(line_text(&input.line(20)), expected);
    }

    let mut input = PasswordInput::<()>::new()
        .value("secret")
        .hotkey("p")
        .focused(true);
    input.input.insert_mode = true;
    assert_eq!(line_text(&input.line(20)), "•••••• ");
}

#[test]
fn pending_hotkey_underlines_password_input_hotkey() {
    let mut input = PasswordInput::<()>::new().value("secret").hotkey("p");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
        &mut ctx,
    );
    let line = input.line(20);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(ctx.redraw_requested());
    assert!(line.spans.iter().any(|span| {
        span.content.as_ref() == "p" && span.style.add_modifier.contains(Modifier::UNDERLINED)
    }));
}

#[test]
fn password_input_can_clear_hotkey() {
    let mut input = PasswordInput::<()>::new().hotkey("p");

    input.clear_hotkey();

    assert_eq!(line_text(&input.line(20)), "");
}

#[test]
fn password_input_ignores_external_editor_shortcut() {
    let mut input = PasswordInput::<()>::new().value("secret");
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Char('o'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(ctx.external_editor_request().is_none());
}

#[test]
fn enter_switches_focused_text_input_into_insert_mode() {
    let mut input = TextInput::<()>::new().value("abc").focused(true);
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.insert_mode);
    assert_eq!(input.current_value(), "abc");
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn escape_and_control_left_bracket_leave_text_and_password_insert_mode() {
    for key in [
        KeyEvent::from(Key::Esc),
        modified_key(Key::Char('['), KeyModifiers::CONTROL),
    ] {
        let mut input = TextInput::<()>::new().value("abc").focused(true);
        input.insert_mode = true;
        let mut ctx = EventCtx::<()>::default();
        assert_eq!(
            input.event(&TuiEvent::Key(key), &mut ctx),
            EventOutcome::Handled,
            "text key: {key:?}"
        );
        assert!(!input.insert_mode, "text key: {key:?}");
        assert_eq!(input.current_value(), "abc", "text key: {key:?}");
        assert!(ctx.layout_requested(), "text key: {key:?}");
        assert_eq!(ctx.propagation(), Propagation::Stopped, "text key: {key:?}");

        let mut input = PasswordInput::<()>::new().value("abc").focused(true);
        input.input.insert_mode = true;
        let mut ctx = EventCtx::<()>::default();
        assert_eq!(
            input.event(&TuiEvent::Key(key), &mut ctx),
            EventOutcome::Handled,
            "password key: {key:?}"
        );
        assert!(!input.input.insert_mode, "password key: {key:?}");
        assert_eq!(input.current_value(), "abc", "password key: {key:?}");
        assert!(ctx.layout_requested(), "password key: {key:?}");
        assert_eq!(
            ctx.propagation(),
            Propagation::Stopped,
            "password key: {key:?}"
        );
    }
}

#[test]
fn disabled_text_input_blocks_all_text_mutation() {
    let mut input = TextInput::<()>::new().value("locked").disabled(true);

    assert_eq!(input.on_key(Key::Char('x')), InputOutcome::HANDLED);
    assert_eq!(input.on_key(Key::Backspace), InputOutcome::HANDLED);
    assert_eq!(input.on_key(Key::Delete), InputOutcome::HANDLED);
    assert_eq!(input.on_paste("pasted"), InputOutcome::HANDLED);
    assert_eq!(input.current_value(), "locked");
}

#[test]
fn disabled_text_input_allows_cursor_navigation() {
    let mut input = TextInput::<()>::new().value("one two").disabled(true);

    assert_eq!(input.on_key(Key::Left), InputOutcome::HANDLED);
    assert_eq!(input.cursor, 6);
    assert_eq!(
        input.on_key(KeyEvent {
            code: Key::Char('b'),
            modifiers: KeyModifiers::ALT,
        }),
        InputOutcome::HANDLED
    );
    assert_eq!(input.cursor, 4);
    assert_eq!(input.current_value(), "one two");
}

#[test]
fn disabled_text_input_bubbles_tab_for_focus_navigation() {
    let mut input = TextInput::<()>::new()
        .value("locked")
        .focused(true)
        .disabled(true);
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(ctx.propagation(), Propagation::Continue);
    assert!(!input.insert_mode());
}

#[test]
fn disabled_text_input_does_not_enter_insert_mode_or_submit() {
    let mut input = TextInput::new()
        .value("locked")
        .focused(true)
        .disabled(true)
        .on_submit(|value| format!("submit:{value}"));
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(ctx.messages().is_empty());
    assert!(!input.insert_mode());
    assert_eq!(input.current_value(), "locked");
}

#[test]
fn disabled_text_input_uses_dashed_panel_border() {
    let input = TextInput::<()>::new()
        .value("locked")
        .panel("Name")
        .disabled(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("input should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "┌");
    assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "╎");
    assert_eq!(buffer.cell((1, 2)).unwrap().symbol(), "-");
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().border_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().subtle_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().muted_fg());
    assert_ne!(buffer.cell((7, 1)).unwrap().bg, theme().highlight_bg());
}

#[test]
fn focused_disabled_text_input_uses_dimmed_highlight_without_cursor() {
    let input = TextInput::<()>::new()
        .value("locked")
        .panel("Name")
        .focused(true)
        .disabled(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("input should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().highlight_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().bg, disabled_input_background());
    assert_eq!(buffer.cell((7, 1)).unwrap().bg, disabled_input_background());
    assert_ne!(buffer.cell((7, 1)).unwrap().modifier, Modifier::REVERSED);
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn modified_key(code: Key, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent { code, modifiers }
}
