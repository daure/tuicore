use super::*;
use crate::{FocusRequest, MouseButton, MouseEvent, MouseEventKind, Propagation, TreePath};
use ratatui::style::Modifier;

#[test]
fn plain_character_bubbles_before_insert_mode() {
    let mut input = TextareaInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn control_enter_finishes_edit_without_submit_message() {
    let mut input = TextareaInput::new()
        .value("first\nsecond")
        .on_submit(|value| format!("submit:{value}"))
        .on_edit_end(|value| format!("end:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let outcome = input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!input.insert_mode);
    assert_eq!(ctx.messages(), &["end:first\nsecond".to_string()]);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
}

#[test]
fn textarea_emits_one_change_only_for_each_actual_mutation() {
    let mut input = TextareaInput::new()
        .value("a")
        .focused(true)
        .on_change(|value| format!("change:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Left)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);
    input.event(&TuiEvent::Paste("b\nc".into()), &mut ctx);

    assert_eq!(
        ctx.messages(),
        &[
            "change:a\n".to_string(),
            "change:a".to_string(),
            "change:ab\nc".to_string(),
        ]
    );
}

#[test]
fn focused_textarea_submit_emits_once_and_enters_insert_mode() {
    let mut input = TextareaInput::new()
        .value("draft")
        .focused(true)
        .on_submit(|value| format!("submit:{value}"));
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert!(input.insert_mode);
    assert_eq!(ctx.messages(), &["submit:draft".to_string()]);
}

#[test]
fn enter_inserts_newline() {
    let mut input = TextareaInput::<()>::new().value("first");
    input.insert_mode = true;

    let outcome = input.on_key(KeyEvent::from(Key::Enter));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "first\n");
}

#[test]
fn control_j_inserts_newline() {
    let mut input = TextareaInput::<()>::new().value("first");
    input.insert_mode = true;

    let outcome = input.on_key(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "first\n");
}

#[test]
fn enter_switches_focused_textarea_into_insert_mode() {
    let mut input = TextareaInput::<()>::new().value("abc").focused(true);
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
fn delete_removes_next_character_in_textarea() {
    let mut input = TextareaInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent::from(Key::Delete));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
    assert_eq!(input.cursor, 1);
}

#[test]
fn shifted_delete_removes_next_character_in_textarea() {
    let mut input = TextareaInput::<()>::new().value("abcd");
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
fn del_character_removes_next_character_in_textarea() {
    let mut input = TextareaInput::<()>::new().value("abcd");
    input.insert_mode = true;
    input.cursor = 1;

    let outcome = input.on_key(KeyEvent::from(Key::Char('\u{7f}')));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "acd");
}

#[test]
fn modified_del_character_removes_next_character_in_textarea() {
    let mut input = TextareaInput::<()>::new().value("abcd");
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
fn delete_removes_next_character_before_insert_mode_in_textarea() {
    let mut input = TextareaInput::<()>::new().value("abcd").focused(true);
    input.cursor = 1;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Delete)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "acd");
    assert!(input.insert_mode);
    assert!(ctx.layout_requested());
}

#[test]
fn control_d_does_not_finish_or_submit() {
    let mut input = TextareaInput::new()
        .value("draft")
        .on_submit(|value| format!("submit:{value}"));
    input.insert_mode = true;
    let mut ctx = EventCtx::default();
    let key = KeyEvent {
        code: Key::Char('d'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(input.insert_mode);
    assert!(ctx.messages().is_empty());
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn control_c_clears_value_and_stops_propagation() {
    let mut input = TextareaInput::<()>::new().value("first\nsecond");
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
    let mut input = TextareaInput::<()>::new().value("left");
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.visible_lines(10, 1).lines[0]), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn tab_bubbles_for_focus_navigation_before_insert_mode() {
    let mut input = TextareaInput::<()>::new().value("left").focused(true);
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(input.current_value(), "left");
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn textarea_marks_focus_as_text_entry_while_typing() {
    let mut input = TextareaInput::<()>::new();
    input.insert_mode = true;
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 10, 1), &mut ctx);

    let target = ctx.focus_targets().first().unwrap();
    assert!(target.suppress_global_hotkeys);
    assert!(target.focused_events_before_global_hotkeys);
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
    assert!(lines.lines[0].spans.iter().any(|span| {
        span.content.as_ref() == "t" && span.style.add_modifier.contains(Modifier::UNDERLINED)
    }));
}

#[test]
fn control_i_inserts_tab_character_and_stops_propagation() {
    let mut input = TextareaInput::<()>::new().value("left");
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();
    let key = KeyEvent {
        code: Key::Char('i'),
        modifiers: KeyModifiers::CONTROL,
    };

    let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "left    ");
    assert_eq!(line_text(&input.visible_lines(10, 1).lines[0]), "left    ");
    assert_eq!(ctx.propagation(), Propagation::Stopped);
    assert!(ctx.redraw_requested());
}

#[test]
fn visible_lines_clip_wide_unicode_by_terminal_width() {
    let input = TextareaInput::<()>::new().wrap(false).value("ab界d");

    let lines = input.visible_lines(4, 1);

    assert_eq!(line_text(&lines.lines[0]), "ab界");
    assert_eq!(cell_width(&line_text(&lines.lines[0])), 4);
}

#[test]
fn text_wraps_visually_by_default_without_changing_value() {
    let input = TextareaInput::<()>::new().value("abcdef");

    let lines = input.visible_lines(3, 2);

    assert_eq!(line_text(&lines.lines[0]), "abc");
    assert_eq!(line_text(&lines.lines[1]), "def");
    assert_eq!(input.current_value(), "abcdef");
}

#[test]
fn wrapping_moves_whole_word_instead_of_leaving_leading_space() {
    let input = TextareaInput::<()>::new().value("aaa we");

    let lines = input.visible_lines(5, 2);

    assert_eq!(line_text(&lines.lines[0]), "aaa ");
    assert_eq!(line_text(&lines.lines[1]), "we");
}

#[test]
fn wrapping_moves_word_as_soon_as_next_typed_char_overflows() {
    let input = TextareaInput::<()>::new().value("see whe");

    let lines = input.visible_lines(6, 2);

    assert_eq!(line_text(&lines.lines[0]), "see ");
    assert_eq!(line_text(&lines.lines[1]), "whe");
}

#[test]
fn insert_mode_wraps_when_cursor_would_overflow_full_row() {
    let mut input = TextareaInput::<()>::new().value("aaa beeeee").focused(true);
    input.insert_mode = true;

    let lines = input.visible_lines(10, 2);

    assert_eq!(line_text(&lines.lines[0]), "aaa ");
    assert_eq!(line_text(&lines.lines[1]), "beeeee ");
}

#[test]
fn disabled_wrap_preserves_horizontal_cursor_scrolling() {
    let mut input = TextareaInput::<()>::new()
        .wrap(false)
        .value("abcdef")
        .focused(true);
    input.insert_mode = true;

    let lines = input.visible_lines(3, 1);

    assert_eq!(line_text(&lines.lines[0]), "ef ");
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
    let mut input = TextareaInput::<()>::new()
        .placeholder("Write multiple lines...")
        .focused(true);
    input.insert_mode = true;

    let lines = input.visible_lines(8, 1);

    assert_eq!(lines.lines[0].spans[0].content.as_ref(), "W");
    assert_eq!(line_text(&lines.lines[0]), "Write mu");
}

#[test]
fn placeholder_hotkey_renders_at_end() {
    let input = TextareaInput::<()>::new().placeholder("Write").hotkey("p");

    let lines = input.visible_lines(20, 1);

    assert_eq!(line_text(&lines.lines[0]), "Write |p|");
}

#[test]
fn unfocused_value_hotkey_renders_after_last_line() {
    let input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t");

    let lines = input.visible_lines(20, 2);

    assert_eq!(line_text(&lines.lines[0]), "First");
    assert_eq!(line_text(&lines.lines[1]), "Second |t|");
}

#[test]
fn focused_value_hotkey_renders_before_insert_mode() {
    let input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t")
        .focused(true);

    let lines = input.visible_lines(20, 2);

    assert_eq!(line_text(&lines.lines[1]), "Second |t|");
}

#[test]
fn insert_mode_value_hotkey_is_hidden() {
    let mut input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t")
        .focused(true);
    input.insert_mode = true;

    let lines = input.visible_lines(20, 2);

    assert_eq!(line_text(&lines.lines[1]), "Second ");
}

#[test]
fn hotkey_registers_as_focus_shortcut() {
    let mut input = TextareaInput::<()>::new().hotkey("p");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 3), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["p"]);
    assert!(!ctx.focus_targets()[0].suppress_global_hotkeys);
}

#[test]
fn hotkey_commit_enters_insert_mode() {
    let mut input = TextareaInput::<()>::new().value("Draft").hotkey("t");
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Hotkey(HotkeyEvent::Commit("t".into())), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.insert_mode);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn measure_counts_trailing_blank_line() {
    let input = TextareaInput::<()>::new().value("first\n");

    let hint = <TextareaInput<()> as TuiNode<()>>::measure(&input, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 2);
}

#[test]
fn measure_counts_wrapped_rows_for_bounded_width() {
    let input = TextareaInput::<()>::new().value("abcdef");

    let hint = <TextareaInput<()> as TuiNode<()>>::measure(&input, LayoutProposal::at_most(3, 10));

    assert_eq!(hint.preferred.width, 3);
    assert_eq!(hint.preferred.height, 2);
}

#[test]
fn measure_respects_min_and_max_rows_without_clamping_value() {
    let input = TextareaInput::<()>::new()
        .min_rows(2)
        .max_rows(3)
        .value("one\ntwo\nthree\nfour");

    let hint = <TextareaInput<()> as TuiNode<()>>::measure(&input, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 3);
    assert_eq!(input.current_value(), "one\ntwo\nthree\nfour");
}

#[test]
fn min_rows_does_not_exceed_max_rows_regardless_of_builder_order() {
    let input = TextareaInput::<()>::new().max_rows(2).min_rows(4);

    let hint = <TextareaInput<()> as TuiNode<()>>::measure(&input, LayoutProposal::unbounded());

    assert_eq!(hint.preferred.height, 2);
}

#[test]
fn textarea_panel_style_adds_border_space_and_focuses_inner_area() {
    let mut input = TextareaInput::<()>::new()
        .min_rows(2)
        .max_rows(2)
        .panel("Notes");
    let hint = input.measure(LayoutProposal::unbounded());
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(2, 3, 20, 4), &mut ctx);

    assert_eq!(hint.preferred.height, 4);
    assert_eq!(ctx.focus_targets()[0].area, Rect::new(3, 4, 18, 2));
}

#[test]
fn textarea_panel_style_shrinks_to_min_rows_when_content_is_short() {
    let mut input = TextareaInput::<()>::new()
        .value("one\ntwo\nthree\nfour")
        .min_rows(2)
        .max_rows(4)
        .panel("Notes");
    input.set_value("");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(2, 3, 20, 6), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].area, Rect::new(3, 4, 18, 2));
}

#[test]
fn textarea_panel_style_moves_hotkey_to_panel() {
    let mut input = TextareaInput::<()>::new()
        .placeholder("Notes")
        .hotkey("n")
        .style(InputChrome::panel("Label").top_right("Required"));
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(2, 3, 20, 3), &mut ctx);

    assert_eq!(line_text(&input.visible_lines(20, 1).lines[0]), "Notes");
    assert_eq!(ctx.focus_targets()[0].area, Rect::new(3, 4, 18, 1));
    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["n"]);
}

#[test]
fn textarea_panel_click_requests_input_focus() {
    let mut input = TextareaInput::<()>::new().panel("Label");
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(2, 3, 20, 3), &mut layout);
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
            id: FocusId::new("textarea"),
        })
    );
}

#[test]
fn visible_lines_scroll_to_cursor_when_content_exceeds_viewport() {
    let input = TextareaInput::<()>::new().value("one\ntwo\nthree\nfour");

    let visible = input.visible_lines(20, 2);

    assert_eq!(visible.first_line, 2);
    assert_eq!(line_text(&visible.lines[0]), "three");
    assert_eq!(line_text(&visible.lines[1]), "four");
}

#[test]
fn page_down_uses_scroll_state_when_content_overflows() {
    let mut input = TextareaInput::<()>::new().value("one\ntwo\nthree\nfour");
    input.cursor = 0;
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::PageDown)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.scroll.target_offset().y, 1);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn wrapped_cursor_row_scrolls_into_view_after_layout() {
    let mut input = TextareaInput::<()>::new().value("abcdefghi");
    input.insert_mode = true;
    let mut layout = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 3, 2), &mut layout);

    assert_eq!(input.scroll.target_offset().y, 1);
}

#[test]
fn wrapped_content_height_uses_viewport_width_after_scrollbar_gutter() {
    let input = TextareaInput::<()>::new().value("one\ntwo\nthree\nfour five");

    let geometry = input.scroll_geometry(Rect::new(0, 0, 10, 4));

    assert_eq!(geometry.layout.viewport.width, 9);
    assert_eq!(geometry.content.height, 5);
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
fn escape_leaves_insert_mode_without_bubbling() {
    let mut input = TextareaInput::<()>::new().value("abc").focused(true);
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
    let mut input = TextareaInput::<()>::new().value("abc").focused(true);
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
fn inactive_external_editor_session_emits_one_start_and_one_end() {
    let mut input = TextareaInput::new()
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
            value: "edited\nvalue".to_string(),
            line: 2,
            col: 1,
        }),
        &mut response,
    );
    assert!(!input.insert_mode());
    assert_eq!(
        response.messages(),
        &[
            "change:edited\nvalue".to_string(),
            "end:edited\nvalue".to_string(),
        ]
    );

    let mut focus = FocusCtx::default();
    input.focus(None, false, &mut focus);
    assert_eq!(focus.drain_messages().count(), 0);
}

#[test]
fn external_editor_response_clamps_column_to_selected_line() {
    let mut input = TextareaInput::<()>::new().value("initial");
    input.insert_mode = true;
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
    assert!(!input.insert_mode);
    assert!(ctx.layout_requested());
    assert!(ctx.redraw_requested());
    assert!(ctx.clear_requested());
}

#[test]
fn textarea_external_editor_emits_change_only_when_value_differs() {
    let mut input = TextareaInput::new()
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
            value: "edited\nvalue".to_string(),
            line: 2,
            col: 1,
        }),
        &mut ctx,
    );

    assert_eq!(ctx.messages(), &["change:edited\nvalue".to_string()]);
}

#[test]
fn paste_inserts_multiline_text() {
    let mut input = TextareaInput::<()>::new().value("hello");
    input.insert_mode = true;
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Paste(" world\nagain".into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "hello world\nagain");
    assert!(ctx.redraw_requested());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn growing_panel_textarea_keeps_new_cursor_row_visible_after_layout() {
    let mut input = TextareaInput::<()>::new()
        .value("asdf\nasdf")
        .min_rows(2)
        .max_rows(4)
        .panel("Notes");
    input.insert_mode = true;
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 4), &mut layout);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);
    input.layout(Rect::new(0, 0, 20, 6), &mut layout);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.scroll.target_offset().y, 0);
}

#[test]
fn entering_insert_mode_scrolls_to_cursor() {
    let mut input = TextareaInput::<()>::new()
        .value("one\ntwo\nthree\nfour")
        .max_rows(2);
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.insert_mode);
    assert_eq!(input.scroll.target_offset().y, 2);
}

#[test]
fn edit_end_emits_once_when_active_textarea_loses_focus() {
    let mut input = TextareaInput::new()
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
}

#[test]
fn focus_loss_without_active_edit_emits_nothing() {
    let mut input = TextareaInput::new().on_edit_end(|value| format!("end:{value}"));
    let mut ctx = FocusCtx::new(AnimationSettings::default());

    input.focus(None, false, &mut ctx);

    assert!(ctx.drain_messages().next().is_none());
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}
