use super::*;
use crate::{FocusRequest, MouseButton, MouseEvent, MouseEventKind, Propagation, TreePath};
use ratatui::style::Modifier;
use ratatui::{backend::TestBackend, Terminal};

struct KeyBindingsGuard {
    previous: crate::KeyBindings,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl KeyBindingsGuard {
    fn replace(next: crate::KeyBindings) -> Self {
        let lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
        let previous = crate::keybindings();
        crate::set_keybindings(next);
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for KeyBindingsGuard {
    fn drop(&mut self) {
        crate::set_keybindings(self.previous.clone());
    }
}

#[test]
fn plain_character_bubbles_before_insert_mode() {
    let mut input = TextareaInput::<()>::new();
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn control_navigation_keys_bubble_before_insert_mode() {
    let custom_key = KeySpec::key_with_modifiers(Key::Char('n'), KeyModifiers::CONTROL);
    let bindings = crate::KeyBindings::default()
        .with_focus_next_control([
            KeySpec::key_with_modifiers(Key::Char('j'), KeyModifiers::CONTROL),
            custom_key.clone(),
        ])
        .with_focus_previous_control([KeySpec::key_with_modifiers(
            Key::Char('p'),
            KeyModifiers::CONTROL,
        )]);
    let _guard = KeyBindingsGuard::replace(bindings);
    let mut keys = TextareaInputKeyBindings::default();
    keys.insert_newline.push(custom_key);
    let mut input = TextareaInput::<()>::new().focused(true).keybindings(keys);

    for key in [
        KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        },
        KeyEvent {
            code: Key::Char('n'),
            modifiers: KeyModifiers::CONTROL,
        },
        KeyEvent {
            code: Key::Char('p'),
            modifiers: KeyModifiers::CONTROL,
        },
    ] {
        let mut ctx = EventCtx::<()>::default();
        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert!(!input.insert_mode);
        assert_eq!(ctx.propagation(), Propagation::Continue);
    }
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
    input.cursor = input.len_chars();
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
fn enter_inserts_newline() {
    let mut input = TextareaInput::<()>::new().value("first");
    input.cursor = input.len_chars();
    input.insert_mode = true;

    let outcome = input.on_key(KeyEvent::from(Key::Enter));

    assert_eq!(outcome, InputOutcome::CHANGED);
    assert_eq!(input.current_value(), "first\n");
}

#[test]
fn control_j_inserts_newline() {
    let mut input = TextareaInput::<()>::new().value("first");
    input.cursor = input.len_chars();
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "first\n");
    assert!(input.insert_mode);
    assert_eq!(ctx.propagation(), Propagation::Stopped);
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
fn entering_insert_mode_moves_cursor_after_existing_text() {
    let mut input = TextareaInput::<()>::new().value("abcd").focused(true);
    input.cursor = 3;
    let mut ctx = EventCtx::<()>::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Backspace)), &mut ctx);

    assert_eq!(input.current_value(), "abc");
}

#[test]
fn delete_key_variants_remove_next_character() {
    for key in [
        KeyEvent::from(Key::Delete),
        modified_key(Key::Delete, KeyModifiers::SHIFT),
        KeyEvent::from(Key::Char('\u{7f}')),
        modified_key(Key::Char('\u{7f}'), KeyModifiers::CONTROL),
    ] {
        let mut input = TextareaInput::<()>::new().value("abcd");
        input.insert_mode = true;
        input.cursor = 1;

        assert_eq!(input.on_key(key), InputOutcome::CHANGED, "key: {key:?}");
        assert_eq!(input.current_value(), "acd", "key: {key:?}");
        assert_eq!(input.cursor, 1, "key: {key:?}");
    }
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
fn tab_and_control_i_insert_tab_and_stop_propagation() {
    for key in [
        KeyEvent::from(Key::Tab),
        modified_key(Key::Char('i'), KeyModifiers::CONTROL),
    ] {
        let mut input = TextareaInput::<()>::new().value("left");
        input.cursor = input.len_chars();
        input.insert_mode = true;
        let mut ctx = EventCtx::<()>::default();

        assert_eq!(
            input.event(&TuiEvent::Key(key), &mut ctx),
            EventOutcome::Handled,
            "key: {key:?}"
        );
        assert_eq!(input.current_value(), "left    ", "key: {key:?}");
        assert_eq!(ctx.propagation(), Propagation::Stopped, "key: {key:?}");
        assert!(ctx.redraw_requested(), "key: {key:?}");
    }
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
    input.cursor = input.len_chars();
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
    input.cursor = input.len_chars();
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
fn hotkey_rendering_tracks_content_and_insert_mode() {
    let cases = [
        (
            TextareaInput::<()>::new().placeholder("Write").hotkey("p"),
            1,
            0,
            "Write |p|",
        ),
        (
            TextareaInput::new().value("First\nSecond").hotkey("t"),
            2,
            1,
            "Second |t|",
        ),
        (
            TextareaInput::new()
                .value("First\nSecond")
                .hotkey("t")
                .focused(true),
            2,
            1,
            "Second |t|",
        ),
    ];
    for (input, height, line, expected) in cases {
        assert_eq!(
            line_text(&input.visible_lines(20, height).lines[line]),
            expected
        );
    }

    let mut input = TextareaInput::<()>::new()
        .value("First\nSecond")
        .hotkey("t")
        .focused(true);
    input.cursor = input.len_chars();
    input.insert_mode = true;

    assert_eq!(line_text(&input.visible_lines(20, 2).lines[1]), "Second ");
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
fn overflowing_builder_value_starts_at_top() {
    let input = TextareaInput::<()>::new().value("one\ntwo\nthree\nfour");

    let visible = input.visible_lines(20, 2);

    assert_eq!(input.cursor, 0);
    assert_eq!(visible.first_line, 0);
    assert_eq!(line_text(&visible.lines[0]), "one");
    assert_eq!(line_text(&visible.lines[1]), "two");
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
fn focused_navigation_mode_scrolls_down_one_line_for_j_and_down() {
    for key in [Key::Char('j'), Key::Down] {
        let mut input = TextareaInput::<()>::new()
            .value("one\ntwo\nthree\nfour")
            .focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(key)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.scroll.target_offset().y, 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }
}

#[test]
fn focused_navigation_mode_scrolls_up_one_line_for_k_and_up() {
    for key in [Key::Char('k'), Key::Up] {
        let mut input = TextareaInput::<()>::new()
            .value("one\ntwo\nthree\nfour")
            .focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let geometry = input.scroll_geometry(input.area);
        input.scroll.scroll_to(
            ScrollOffset::new(0, 1),
            geometry.viewport,
            geometry.content,
            disabled_animation_settings(),
        );
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(key)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.scroll.target_offset().y, 0);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }
}

#[test]
fn focused_navigation_mode_scrolls_to_top_for_gg_and_home() {
    for keys in [vec![Key::Char('g'), Key::Char('g')], vec![Key::Home]] {
        let mut input = TextareaInput::<()>::new()
            .value("one\ntwo\nthree\nfour")
            .focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let geometry = input.scroll_geometry(input.area);
        input.scroll.scroll_to(
            ScrollOffset::new(0, 2),
            geometry.viewport,
            geometry.content,
            disabled_animation_settings(),
        );

        for (index, key) in keys.into_iter().enumerate() {
            let mut ctx = EventCtx::default();
            let outcome = input.event(&TuiEvent::Key(KeyEvent::from(key)), &mut ctx);

            assert_eq!(outcome, EventOutcome::Handled);
            assert_eq!(ctx.propagation(), Propagation::Stopped);
            if index == 0 && key == Key::Char('g') {
                assert_eq!(input.scroll.target_offset().y, 2);
            }
        }

        assert_eq!(input.scroll.target_offset().y, 0);
    }
}

#[test]
fn focused_navigation_mode_scrolls_to_bottom_for_shift_g_and_end() {
    for key in [
        KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::SHIFT,
        },
        KeyEvent::from(Key::End),
    ] {
        let mut input = TextareaInput::<()>::new()
            .value("one\ntwo\nthree\nfour")
            .focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.scroll.target_offset().y, 2);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }
}

#[test]
fn navigation_line_keys_bubble_without_vertical_overflow() {
    for key in [Key::Char('j'), Key::Down, Key::Char('k'), Key::Up] {
        let mut input = TextareaInput::<()>::new().value("one\ntwo").focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(key)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(input.scroll.target_offset().y, 0);
        assert_eq!(ctx.propagation(), Propagation::Continue);
    }
}

#[test]
fn navigation_jump_keys_bubble_without_vertical_overflow() {
    for key in [
        KeyEvent::from(Key::Char('g')),
        KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::SHIFT,
        },
        KeyEvent::from(Key::Home),
        KeyEvent::from(Key::End),
    ] {
        let mut input = TextareaInput::<()>::new().value("one\ntwo").focused(true);
        let mut layout = LayoutCtx::new();
        input.layout(Rect::new(0, 0, 20, 2), &mut layout);
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(input.scroll.target_offset().y, 0);
        assert_eq!(ctx.propagation(), Propagation::Continue);
    }
}

#[test]
fn insert_mode_enters_j_and_k_without_scrolling() {
    let mut input = TextareaInput::<()>::new()
        .value("one\ntwo\nthree\nfour")
        .focused(true);
    input.cursor = input.len_chars();
    input.insert_mode = true;
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let initial_offset = input.scroll.target_offset().y;
    let mut ctx = EventCtx::default();

    let j_outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('j'))), &mut ctx);
    let k_outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('k'))), &mut ctx);

    assert_eq!(j_outcome, EventOutcome::Handled);
    assert_eq!(k_outcome, EventOutcome::Handled);
    assert_eq!(input.current_value(), "one\ntwo\nthree\nfourjk");
    assert_eq!(input.scroll.target_offset().y, initial_offset);
}

#[test]
fn insert_mode_keeps_g_shift_g_home_and_end_as_editor_keys() {
    let mut input = TextareaInput::<()>::new()
        .value("one\ntwo\nthree\nfour")
        .focused(true);
    input.cursor = input.len_chars();
    input.insert_mode = true;
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let mut ctx = EventCtx::default();

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('g'))), &mut ctx);
    input.event(
        &TuiEvent::Key(KeyEvent {
            code: Key::Char('G'),
            modifiers: KeyModifiers::SHIFT,
        }),
        &mut ctx,
    );
    assert_eq!(input.current_value(), "one\ntwo\nthree\nfourgG");

    input.event(&TuiEvent::Key(KeyEvent::from(Key::Home)), &mut ctx);
    assert_eq!(input.cursor, 14);
    input.event(&TuiEvent::Key(KeyEvent::from(Key::End)), &mut ctx);
    assert_eq!(input.cursor, input.len_chars());
}

#[test]
fn wrapped_cursor_row_scrolls_into_view_after_layout() {
    let mut input = TextareaInput::<()>::new().value("abcdefghi");
    input.cursor = input.len_chars();
    input.insert_mode = true;
    let mut layout = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 3, 2), &mut layout);

    assert_eq!(input.scroll.target_offset().y, 3);
}

#[test]
fn wrapped_content_height_uses_viewport_width_after_scrollbar_gutter() {
    let input = TextareaInput::<()>::new().value("one\ntwo\nthree\nfour five!");

    let geometry = input.scroll_geometry(Rect::new(0, 0, 10, 4));

    assert_eq!(geometry.layout.viewport.width, 9);
    assert_eq!(geometry.content.height, 5);
}

#[test]
fn escape_and_control_left_bracket_leave_insert_mode_without_bubbling() {
    for key in [
        KeyEvent::from(Key::Esc),
        modified_key(Key::Char('['), KeyModifiers::CONTROL),
    ] {
        let mut input = TextareaInput::<()>::new().value("abc").focused(true);
        input.insert_mode = true;
        let mut ctx = EventCtx::<()>::default();

        assert_eq!(
            input.event(&TuiEvent::Key(key), &mut ctx),
            EventOutcome::Handled,
            "key: {key:?}"
        );
        assert!(!input.insert_mode, "key: {key:?}");
        assert_eq!(input.current_value(), "abc", "key: {key:?}");
        assert!(ctx.layout_requested(), "key: {key:?}");
        assert_eq!(ctx.propagation(), Propagation::Stopped, "key: {key:?}");
    }
}

#[test]
fn word_navigation_and_deletion() {
    let mut input = TextareaInput::<()>::new().value("hello world example");
    input.cursor = input.len_chars();

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
}

#[test]
fn deleting_previous_word_only_preserves_separator_when_text_follows_textarea_cursor() {
    for (value, cursor, expected, expected_cursor) in [
        ("hello world", 9, "hello ld", 6),
        ("hello world\n\n", 13, "hello", 5),
        ("ab cd ef", 6, "ab ef", 3),
        ("ab cd ef", 5, "ab ef", 2),
    ] {
        let mut input = TextareaInput::<()>::new().value(value);
        input.cursor = cursor;
        assert_eq!(
            input.on_key(modified_key(Key::Backspace, KeyModifiers::CONTROL)),
            InputOutcome::CHANGED,
            "value: {value}"
        );
        assert_eq!(input.current_value(), expected, "value: {value}");
        assert_eq!(input.cursor, expected_cursor, "value: {value}");
    }
}

#[test]
fn textarea_registers_action_and_editor_hotkeys_separately() {
    let mut input = TextareaInput::<()>::new().hotkey("pa").editor_hotkey("pb");
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 20, 2), &mut ctx);

    assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["pa", "pb"]);
}

#[test]
fn textarea_editor_hotkey_requests_editor_directly() {
    let mut input = TextareaInput::<()>::new()
        .value("first\nsecond")
        .hotkey("pa")
        .editor_hotkey("pb");
    input.cursor = input.len_chars();
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("pb".into())),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.external_editor_request(),
        Some(&crate::ExternalEditorRequest {
            value: "first\nsecond".into(),
            line: 2,
            col: 7,
        })
    );
}

#[test]
fn disabled_textarea_suppresses_editor_hotkey() {
    let mut input = TextareaInput::<()>::new()
        .value("locked")
        .hotkey("pa")
        .editor_hotkey("pb")
        .disabled(true);
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let mut event = EventCtx::default();

    input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("pb".into())),
        &mut event,
    );

    assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["pa"]);
    assert_eq!(
        line_text(&input.visible_lines(20, 1).lines[0]),
        "locked |pa|"
    );
    assert!(event.external_editor_request().is_none());
    assert!(!input.insert_mode());
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
fn paste_inserts_multiline_text() {
    let mut input = TextareaInput::<()>::new().value("hello");
    input.cursor = input.len_chars();
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
    input.cursor = input.len_chars();
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 20, 2), &mut layout);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(input.insert_mode);
    assert_eq!(input.scroll.target_offset().y, 2);
}

#[test]
fn disabled_textarea_blocks_all_text_mutation() {
    let mut input = TextareaInput::<()>::new().value("one\ntwo").disabled(true);

    assert_eq!(input.on_key(Key::Char('x')), InputOutcome::HANDLED);
    assert_eq!(input.on_key(Key::Enter), InputOutcome::SUBMITTED);
    assert_eq!(input.on_key(Key::Backspace), InputOutcome::HANDLED);
    assert_eq!(input.on_key(Key::Delete), InputOutcome::HANDLED);
    assert_eq!(input.on_paste("pasted"), InputOutcome::HANDLED);
    assert_eq!(input.current_value(), "one\ntwo");
}

#[test]
fn disabled_textarea_allows_horizontal_vertical_and_shortcut_navigation() {
    let mut input = TextareaInput::<()>::new().value("one\ntwo").disabled(true);
    input.cursor = input.len_chars();

    assert_eq!(input.on_key(Key::Left), InputOutcome::HANDLED);
    assert_eq!(input.cursor, 6);
    assert_eq!(input.on_key(Key::Up), InputOutcome::HANDLED);
    assert_eq!(input.cursor, 2);
    assert_eq!(
        input.on_key(KeyEvent {
            code: Key::Char('a'),
            modifiers: KeyModifiers::CONTROL,
        }),
        InputOutcome::HANDLED
    );
    assert_eq!(input.cursor, 0);
    assert_eq!(input.current_value(), "one\ntwo");
}

#[test]
fn disabled_textarea_dims_content_and_panel_border() {
    let input = TextareaInput::<()>::new()
        .value("locked")
        .panel("Notes")
        .disabled(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("textarea should render");

    let buffer = terminal.backend().buffer();
    assert!(buffer
        .cell((0, 0))
        .unwrap()
        .modifier
        .contains(Modifier::DIM));
    assert!(buffer
        .cell((1, 1))
        .unwrap()
        .modifier
        .contains(Modifier::DIM));
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().subtle_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().subtle_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().muted_fg());
    assert!(!buffer
        .cell((3, 0))
        .unwrap()
        .modifier
        .contains(Modifier::DIM));
    assert_ne!(buffer.cell((7, 1)).unwrap().bg, theme().highlight_bg());
}

#[test]
fn focused_disabled_textarea_uses_local_cursor_focus() {
    let mut input = TextareaInput::<()>::new()
        .value("locked")
        .panel("Notes")
        .focused(true)
        .disabled(true);
    input.cursor = input.len_chars();
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("textarea should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().subtle_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().muted_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().subtle_fg());
    assert_ne!(buffer.cell((0, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((7, 1)).unwrap().bg, theme().highlight_bg());
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
