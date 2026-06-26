use super::*;
use crate::{FocusRequest, MouseButton, MouseEvent, MouseEventKind, Propagation};

#[test]
fn handled_key_stops_propagation() {
    let mut input = TextInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn tab_bubbles_for_focus_navigation_before_insert_mode() {
    let mut input = TextInput::<()>::new().value("search").focused(true);
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(input.current_value(), "search");
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn text_input_marks_focus_as_text_entry_while_typing() {
    let mut input = TextInput::<()>::new();
    input.insert_mode = true;
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
        Some(&FocusRequest::Target(FocusId::new("input")))
    );
}

#[test]
fn password_input_marks_focus_as_text_entry_while_typing() {
    let mut input = PasswordInput::<()>::new();
    input.input.insert_mode = true;
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 10, 1), &mut ctx);

    let target = ctx.focus_targets().first().unwrap();
    assert!(target.suppress_global_hotkeys);
    assert!(target.focused_events_before_global_hotkeys);
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
fn control_enter_does_not_submit_text_input() {
    let mut input = TextInput::<()>::new().value("ship");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let key = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(input.insert_mode);
    assert!(ctx.messages().is_empty());
}

#[test]
fn enter_submit_emits_message_blurs_and_stops_propagation() {
    let mut input = TextInput::new()
        .value("ship")
        .on_submit(|value| format!("submit:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode);
    assert_eq!(ctx.messages(), &["submit:ship".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
}

#[test]
fn control_enter_does_not_submit_password_input() {
    let mut input = PasswordInput::<()>::new().value("secret");
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(input.input.insert_mode);
    assert!(ctx.messages().is_empty());
}

#[test]
fn enter_password_submit_emits_message_blurs_and_stops_propagation() {
    let mut input = PasswordInput::new()
        .value("secret")
        .on_submit(|value| format!("submit:{value}"));
    input.input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.input.insert_mode);
    assert_eq!(ctx.messages(), &["submit:secret".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
}

#[test]
fn delete_removes_next_character_in_text_input() {
    let mut input = TextInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent::from(Key::Delete));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
    assert_eq!(input.cursor, 1);
}

#[test]
fn shifted_delete_removes_next_character_in_text_input() {
    let mut input = TextInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent {
        code: Key::Delete,
        modifiers: KeyModifiers::SHIFT,
    });

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
}

#[test]
fn del_character_removes_next_character_in_text_input() {
    let mut input = TextInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent::from(Key::Char('\u{7f}')));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
}

#[test]
fn modified_del_character_removes_next_character_in_text_input() {
    let mut input = TextInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent {
        code: Key::Char('\u{7f}'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
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
fn delete_removes_next_character_in_password_input() {
    let mut input = PasswordInput::<()>::new().value("abcd");
    input.input.insert_mode = true;
    input.input.cursor = 1;

    let outcome = input.on_key(KeyEvent::from(Key::Delete));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
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
fn tab_inserts_tab_character_and_stops_propagation() {
    let mut input = TextInput::<()>::new().value("left");
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.line(10)), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn control_i_inserts_tab_character_and_stops_propagation() {
    let mut input = TextInput::<()>::new().value("left");
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();
    let key = KeyEvent {
        code: Key::Char('i'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.line(10)), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
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
fn placeholder_hotkey_renders_at_end() {
    let input = TextInput::<()>::new().placeholder("Ask").hotkey("p");

    assert_eq!(line_text(&input.line(20)), "Ask |p|");
}

#[test]
fn unfocused_value_hotkey_renders_after_value() {
    let input = TextInput::<()>::new().value("Ask").hotkey("i");

    assert_eq!(line_text(&input.line(20)), "Ask |i|");
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
fn focused_value_hotkey_renders_before_insert_mode() {
    let input = TextInput::<()>::new()
        .value("Ask")
        .hotkey("i")
        .focused(true);

    assert_eq!(line_text(&input.line(20)), "Ask |i|");
}

#[test]
fn insert_mode_value_hotkey_is_hidden() {
    let mut input = TextInput::<()>::new()
        .value("Ask")
        .hotkey("i")
        .focused(true);
    input.insert_mode = true;

    assert_eq!(line_text(&input.line(20)), "Ask ");
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
fn hotkey_registers_as_focus_shortcut() {
    let mut input = TextInput::<()>::new().hotkey("p");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 1), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["p"]);
    assert!(!ctx.focus_targets()[0].suppress_global_hotkeys);
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
    // Start cursor is at the end (19)
    assert_eq!(input.cursor, 19);

    // Ctrl+Left jumps to the start of "example" (12)
    input.on_key(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(input.cursor, 12);

    // Ctrl+Left jumps to the start of "world" (6)
    input.on_key(KeyEvent {
        code: Key::Left,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(input.cursor, 6);

    // Ctrl+Right jumps to the start of "example" (12)
    input.on_key(KeyEvent {
        code: Key::Right,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(input.cursor, 12);

    // Ctrl+Right jumps to the end of input (19)
    input.on_key(KeyEvent {
        code: Key::Right,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(input.cursor, 19);

    // Move cursor back to "world" (6)
    input.cursor = 6;

    // Ctrl+Backspace deletes "hello " (before cursor)
    input.on_key(KeyEvent {
        code: Key::Backspace,
        modifiers: KeyModifiers::CONTROL,
    });
    assert_eq!(input.current_value(), "world example");
    assert_eq!(input.cursor, 0);

    // Reset text and delete next word (Ctrl+Delete)
    input.set_value("hello world example");
    input.cursor = 6; // start of "world"
    input.on_key(KeyEvent {
        code: Key::Delete,
        modifiers: KeyModifiers::CONTROL,
    });
    // Deletes "world " (from cursor to start of next word)
    assert_eq!(input.current_value(), "hello example");
    assert_eq!(input.cursor, 6);

    // Test Alt+b (word backward)
    input.set_value("hello world example");
    input.cursor = 19;
    input.on_key(KeyEvent {
        code: Key::Char('b'),
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(input.cursor, 12);

    // Test Alt+f (word forward)
    input.cursor = 6;
    input.on_key(KeyEvent {
        code: Key::Char('f'),
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(input.cursor, 12);

    // Test Alt+d (delete word forward)
    input.set_value("hello world example");
    input.cursor = 6;
    input.on_key(KeyEvent {
        code: Key::Char('d'),
        modifiers: KeyModifiers::ALT,
    });
    assert_eq!(input.current_value(), "hello example");
    assert_eq!(input.cursor, 6);
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
        })
    );
    assert!(ctx.redraw_requested());
    assert!(!ctx.clear_requested());
}

#[test]
fn external_editor_response_updates_value_and_clamps_cursor() {
    let mut input = TextInput::<()>::new().value("initial");
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
    assert!(ctx.redraw_requested());
    assert!(ctx.clear_requested());
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
fn on_blur_emits_message_when_focus_lost() {
    let mut input = TextInput::new()
        .value("hello")
        .on_blur(|value| format!("blur:{value}"));
    let mut ctx = FocusCtx::new(AnimationSettings::default());

    input.focus(None, false, &mut ctx);

    assert_eq!(
        ctx.drain_messages().collect::<Vec<_>>(),
        vec!["blur:hello".to_string()]
    );
}

#[test]
fn password_input_masks_value_without_changing_secret() {
    let mut input = PasswordInput::<()>::new().value("secret").mask_char('*');

    input.on_key(KeyEvent::from(Key::Char('!')));

    assert_eq!(input.current_value(), "secret!");
    assert_eq!(line_text(&input.line(20)), "*******");
}

#[test]
fn password_input_placeholder_hotkey_renders_at_end() {
    let input = PasswordInput::<()>::new().placeholder("Secret").hotkey("p");

    assert_eq!(line_text(&input.line(20)), "Secret |p|");
}

#[test]
fn password_input_unfocused_value_hotkey_renders_after_mask() {
    let input = PasswordInput::<()>::new().value("secret").hotkey("p");

    assert_eq!(line_text(&input.line(20)), "•••••• |p|");
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
fn password_input_focused_value_hotkey_renders_before_insert_mode() {
    let input = PasswordInput::<()>::new()
        .value("secret")
        .hotkey("p")
        .focused(true);

    assert_eq!(line_text(&input.line(20)), "•••••• |p|");
}

#[test]
fn password_input_insert_mode_value_hotkey_is_hidden() {
    let mut input = PasswordInput::<()>::new()
        .value("secret")
        .hotkey("p")
        .focused(true);
    input.input.insert_mode = true;

    assert_eq!(line_text(&input.line(20)), "•••••• ");
}

#[test]
fn password_input_hotkey_registers_as_focus_shortcut() {
    let mut input = PasswordInput::<()>::new().hotkey("p");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 1), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["p"]);
    assert!(!ctx.focus_targets()[0].suppress_global_hotkeys);
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
fn escape_leaves_insert_mode_without_bubbling() {
    let mut input = TextInput::<()>::new().value("abc").focused(true);
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode);
    assert_eq!(input.current_value(), "abc");
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn control_left_bracket_leaves_insert_mode_without_bubbling() {
    let mut input = TextInput::<()>::new().value("abc").focused(true);
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();
    let key = KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode);
    assert_eq!(input.current_value(), "abc");
    assert!(ctx.layout_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}
