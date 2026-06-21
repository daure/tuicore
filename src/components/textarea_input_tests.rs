use super::*;
use crate::Propagation;
use ratatui::style::Modifier;

#[test]
fn handled_key_stops_propagation() {
    let mut input = TextareaInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn control_enter_submit_emits_message_and_stops_propagation() {
    let mut input = TextareaInput::new()
        .value("first\nsecond")
        .on_submit(|value| format!("submit:{value}"));
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.messages(), &["submit:first\nsecond".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn control_d_submit_emits_message_and_stops_propagation() {
    let mut input = TextareaInput::new()
        .value("draft")
        .on_submit(|value| format!("submit:{value}"));
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Char('d'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.messages(), &["submit:draft".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn control_c_clears_value_and_stops_propagation() {
    let mut input = TextareaInput::<()>::new().value("first\nsecond");
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
    let mut input = TextareaInput::<()>::new().value("left");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.visible_lines(10, 1)[0]), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn pending_hotkey_underlines_textarea_hotkey() {
    let mut input = TextareaInput::<()>::new().value("Draft note").hotkey("t");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("t".into())),
        &mut ctx,
    );
    let lines = input.visible_lines(24, 1);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(ctx.redraw_requested());
    assert!(lines[0].spans.iter().any(|span| {
        span.content.as_ref() == "t" && span.style.add_modifier.contains(Modifier::UNDERLINED)
    }));
}

#[test]
fn control_i_inserts_tab_character_and_stops_propagation() {
    let mut input = TextareaInput::<()>::new().value("left");
    let mut ctx = EventCtx::<()>::default();
    let key = KeyEvent {
        code: Key::Char('i'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.visible_lines(10, 1)[0]), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn visible_lines_clip_wide_unicode_by_terminal_width() {
    let input = TextareaInput::<()>::new().value("ab界d");

    let lines = input.visible_lines(4, 1);

    assert_eq!(line_text(&lines[0]), "ab界");
    assert_eq!(cell_width(&line_text(&lines[0])), 4);
}

#[test]
fn custom_submit_key_replaces_default_control_enter() {
    let keys = TextareaInputKeyBindings {
        submit: vec![KeySpec::plain('s')],
        ..TextareaInputKeyBindings::default()
    };
    let mut input = TextareaInput::<()>::new().keybindings(keys);
    let control_enter = KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::CONTROL,
    };

    assert_eq!(input.on_key(control_enter), InputOutcome::IDLE);
    assert!(input.on_key(KeyEvent::from(Key::Char('s'))).submitted);
}

#[test]
fn focused_placeholder_draws_cursor_over_first_character() {
    let input = TextareaInput::<()>::new()
        .placeholder("Write multiple lines...")
        .focused(true);

    let lines = input.visible_lines(8, 1);

    assert_eq!(lines[0].spans[0].content.as_ref(), "W");
    assert_eq!(line_text(&lines[0]), "Write mu");
}

#[test]
fn placeholder_hotkey_renders_at_end() {
    let input = TextareaInput::<()>::new().placeholder("Write").hotkey("p");

    let lines = input.visible_lines(20, 1);

    assert_eq!(line_text(&lines[0]), "Write |p|");
}

#[test]
fn unfocused_value_hotkey_renders_after_last_line() {
    let input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t");

    let lines = input.visible_lines(20, 2);

    assert_eq!(line_text(&lines[0]), "First");
    assert_eq!(line_text(&lines[1]), "Second |t|");
}

#[test]
fn focused_value_hotkey_is_hidden() {
    let input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t")
        .focused(true);

    let lines = input.visible_lines(20, 2);

    assert_eq!(line_text(&lines[1]), "Second ");
}

#[test]
fn hotkey_registers_as_focus_shortcut() {
    let mut input = TextareaInput::<()>::new().hotkey("p");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 3), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["p"]);
    assert!(ctx.focus_targets()[0].suppress_global_hotkeys);
}

#[test]
fn measure_counts_trailing_blank_line() {
    let input = TextareaInput::<()>::new().value("first\n");

    let hint = <TextareaInput<()> as TuiNode<()>>::measure(&input, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 2);
}

#[test]
fn escape_bubbles_to_parent_policy() {
    let mut input = TextareaInput::<()>::new();
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
    let mut input = TextareaInput::<()>::new().value("hello world example");
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
}

#[test]
fn ctrl_o_requests_external_editor() {
    let mut input = TextareaInput::<()>::new().value("initial");
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
fn external_editor_response_clamps_column_to_selected_line() {
    let mut input = TextareaInput::<()>::new().value("initial");
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "edited\nlines\n".to_string(),
            line: 2,
            col: 99,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "edited\nlines\n");
    assert_eq!(input.cursor, "edited\nlines".chars().count());
    assert!(ctx.redraw_requested());
    assert!(ctx.clear_requested());
}

#[test]
fn paste_inserts_multiline_text() {
    let mut input = TextareaInput::<()>::new().value("hello");
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Paste(" world\nagain".into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "hello world\nagain");
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn on_blur_emits_message_when_focus_lost() {
    let mut input = TextareaInput::new()
        .value("hello")
        .on_blur(|value| format!("blur:{value}"));
    let mut ctx = FocusCtx::new(AnimationSettings::default());

    input.focus(None, false, &mut ctx);

    assert_eq!(
        ctx.drain_messages().collect::<Vec<_>>(),
        vec!["blur:hello".to_string()]
    );
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}
