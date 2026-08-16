use super::*;
use crate::components::text_input::disabled_input_background;
use crate::{FocusRequest, MouseButton, MouseEvent, MouseEventKind, Propagation, TreePath};
use ratatui::style::Modifier;
use ratatui::{Terminal, backend::TestBackend};

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

fn finish_syntax(input: &mut TextareaInput<()>) -> TickResult {
    for _ in 0..2_000 {
        let result = Animated::tick(input, Duration::ZERO, AnimationSettings::default());
        if input
            .syntax_cache
            .as_ref()
            .is_some_and(|cache| cache.revision == input.syntax_revision)
        {
            return result;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("syntax highlighting should finish");
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

    let mut input = TextareaInput::<()>::new()
        .value("locked")
        .focused(true)
        .disabled(true);
    input.insert_mode = true;
    let mut ctx = EventCtx::<()>::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert_eq!(input.current_value(), "locked");
    assert_eq!(ctx.propagation(), Propagation::Continue);
    assert!(!input.insert_mode());
}

#[test]
fn disabled_textarea_does_not_enter_insert_mode() {
    let mut input = TextareaInput::<()>::new()
        .value("locked")
        .focused(true)
        .disabled(true);
    let mut ctx = EventCtx::default();

    let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    assert_eq!(outcome, EventOutcome::Ignored);
    assert!(!input.insert_mode());
    assert_eq!(ctx.propagation(), Propagation::Continue);
}

#[test]
fn textarea_marks_focus_as_text_entry_while_typing() {
    let mut input = TextareaInput::<()>::new().action_hotkey("ds", |_| ());
    input.insert_mode = true;
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 10, 1), &mut ctx);

    let target = ctx.focus_targets().first().unwrap();
    assert_eq!(target.hotkey_sequences, vec!["ds"]);
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
fn pending_prefix_underlines_native_action_hotkey_badge() {
    let mut input = TextareaInput::<()>::new()
        .hotkey("dd")
        .editor_hotkey("do")
        .action_hotkey("ds", |_| ())
        .panel("Notes");
    let mut ctx = EventCtx::<()>::default();

    input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("d".into())),
        &mut ctx,
    );
    let mut terminal = Terminal::new(TestBackend::new(32, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("textarea should render");

    let buffer = terminal.backend().buffer();
    let bottom = (0..32)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(bottom.contains("┤dd·do·ds│"));
    assert_eq!(
        (0..32)
            .filter(|x| {
                let cell = buffer.cell((*x, 2)).unwrap();
                cell.symbol() == "d" && cell.modifier.contains(Modifier::UNDERLINED)
            })
            .count(),
        3
    );
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

    assert_eq!(hint.min.height, 2);
    assert_eq!(hint.preferred.height, 3);
    assert_eq!(input.current_value(), "one\ntwo\nthree\nfour");
}

#[test]
fn min_rows_is_measured_minimum_with_panel_chrome() {
    let plain = TextareaInput::<()>::new()
        .min_rows(2)
        .max_rows(3)
        .value("one\ntwo\nthree\nfour");
    let panel = TextareaInput::<()>::new()
        .min_rows(2)
        .max_rows(3)
        .value("one\ntwo\nthree\nfour")
        .panel("Notes");

    let plain_hint = plain.measure(LayoutProposal::unbounded());
    let panel_hint = panel.measure(LayoutProposal::unbounded());

    assert_eq!(plain_hint.min.height, 2);
    assert_eq!(plain_hint.preferred.height, 3);
    assert_eq!(panel_hint.min.height, 4);
    assert_eq!(panel_hint.preferred.height, 5);
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
fn navigation_scroll_position_survives_focus_loss_and_relayout() {
    let mut input = TextareaInput::<()>::new()
        .value("one\ntwo\nthree\nfour")
        .focused(true);
    let area = Rect::new(0, 0, 20, 2);
    input.layout(area, &mut LayoutCtx::new());
    let mut event = EventCtx::default();
    input.event(&TuiEvent::Key(Key::Down.into()), &mut event);
    assert_eq!(input.scroll.target_offset().y, 1);

    input.set_focused(false);
    input.layout(area, &mut LayoutCtx::new());

    assert_eq!(input.scroll.target_offset().y, 1);
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
fn textarea_composes_and_registers_repeatable_action_hotkeys_in_badge_order() {
    let mut input = TextareaInput::<()>::new()
        .value("Draft")
        .hotkey("dd")
        .editor_hotkey("do")
        .action_hotkey("ds", |_| ())
        .action_hotkey("dp", |_| ());
    let mut ctx = LayoutCtx::new();

    input.layout(Rect::new(0, 0, 32, 2), &mut ctx);

    assert_eq!(
        line_text(&input.visible_lines(32, 1).lines[0]),
        "Draft |dd·do·ds·dp|"
    );
    assert_eq!(
        ctx.focus_targets()[0].hotkey_sequences,
        vec!["dd", "do", "ds", "dp"]
    );
}

#[test]
fn textarea_action_hotkey_emits_current_value_without_entering_insert_mode() {
    let mut input = TextareaInput::new()
        .value("current draft")
        .action_hotkey("ds", |value| format!("save:{value}"));
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("DS".into())),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(ctx.messages(), &["save:current draft".to_string()]);
    assert!(!input.insert_mode());
    assert_eq!(ctx.propagation(), Propagation::Stopped);
}

#[test]
fn disabled_textarea_hides_unregisters_and_ignores_action_hotkeys() {
    let mut input = TextareaInput::new()
        .value("locked")
        .hotkey("dd")
        .editor_hotkey("do")
        .action_hotkey("ds", |value| format!("save:{value}"))
        .disabled(true);
    let mut layout = LayoutCtx::new();
    input.layout(Rect::new(0, 0, 24, 2), &mut layout);
    let mut event = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Hotkey(HotkeyEvent::Commit("ds".into())),
        &mut event,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["dd"]);
    assert_eq!(
        line_text(&input.visible_lines(24, 1).lines[0]),
        "locked |dd|"
    );
    assert!(event.messages().is_empty());
    assert!(!input.insert_mode());
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
            file_extension: None,
        })
    );
}

#[test]
fn textarea_passes_file_extension_to_external_editor_request() {
    let mut input = TextareaInput::<()>::new()
        .value("# Draft")
        .external_editor_file_extension("md");
    let mut ctx = EventCtx::default();

    let outcome = input.event(
        &TuiEvent::Key(modified_key(Key::Char('o'), KeyModifiers::CONTROL)),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        ctx.external_editor_request(),
        Some(&crate::ExternalEditorRequest {
            value: "# Draft".into(),
            line: 1,
            col: 8,
            file_extension: Some("md".into()),
        })
    );
}

#[test]
fn textarea_language_api_sets_and_clears_language() {
    let mut input = TextareaInput::<()>::new().language(Language::Rust);

    assert_eq!(input.current_language(), Some(Language::Rust));

    input.set_language(Language::Markdown);
    assert_eq!(input.current_language(), Some(Language::Markdown));

    input.clear_language();
    assert_eq!(input.current_language(), None);
}

#[test]
fn textarea_language_derives_editor_extension_without_overriding_explicit_extension() {
    let mut input = TextareaInput::<()>::new().language(Language::Markdown);
    assert_eq!(input.external_editor_extension().as_deref(), Some("md"));

    input.set_external_editor_file_extension("txt");
    input.set_language(Language::Rust);
    assert_eq!(input.external_editor_extension().as_deref(), Some("txt"));

    input.clear_external_editor_file_extension();
    assert_eq!(input.external_editor_extension().as_deref(), Some("rs"));
}

#[test]
fn textarea_language_extension_is_sent_to_external_editor() {
    let mut input = TextareaInput::<()>::new()
        .value("# Draft")
        .language(Language::Markdown);
    let mut ctx = EventCtx::default();

    input.event(
        &TuiEvent::Key(modified_key(Key::Char('o'), KeyModifiers::CONTROL)),
        &mut ctx,
    );

    assert_eq!(
        ctx.external_editor_request()
            .and_then(|request| request.file_extension.as_deref()),
        Some("md")
    );
}

#[test]
fn textarea_highlights_initial_content_without_blocking_layout() {
    let mut input = TextareaInput::<()>::new()
        .value("# Heading\n\n**Bold** text")
        .language(Language::Markdown);
    assert!(input.syntax_cache.is_none());
    assert!(input.syntax_job.is_none());
    let mut lifecycle = LifecycleCtx::default();
    <TextareaInput<()> as TuiNode<()>>::init(&mut input, &mut lifecycle);
    assert!(lifecycle.tick_requested());

    input.layout(Rect::new(0, 0, 40, 4), &mut LayoutCtx::new());

    assert!(input.syntax_cache.is_none());
    assert!(finish_syntax(&mut input).changed);
}

#[test]
fn pending_syntax_highlighting_requests_runtime_ticks() {
    let mut input = TextareaInput::<()>::new()
        .value("# Heading")
        .language(Language::Markdown);
    let mut lifecycle = LifecycleCtx::default();

    <TextareaInput<()> as TuiNode<()>>::init(&mut input, &mut lifecycle);

    assert!(lifecycle.tick_requested());
}

#[test]
fn pending_syntax_poll_does_not_mark_whole_runtime_active() {
    let mut input = TextareaInput::<()>::new()
        .value("# Heading")
        .language(Language::Markdown);
    let (_sender, receiver) = mpsc::channel();
    input.syntax_job = Some(SyntaxJob { receiver });

    let result = Animated::tick(&mut input, Duration::ZERO, AnimationSettings::default());

    assert!(!result.changed);
    assert!(!result.active);
    assert_eq!(result.next_tick, Some(SYNTAX_POLL_INTERVAL));
}

#[test]
fn syntax_invalidation_keeps_one_in_flight_job() {
    let mut input = TextareaInput::<()>::new()
        .value("# First")
        .language(Language::Markdown);
    let (sender, receiver) = mpsc::channel();
    let in_flight_revision = input.syntax_revision;
    input.syntax_job = Some(SyntaxJob { receiver });

    input.set_value("# Latest");

    assert!(
        sender
            .send(SyntaxCache {
                revision: in_flight_revision,
                language: Language::Markdown,
                theme_name: theme().name(),
                source: Vec::new(),
                styles: Vec::new(),
            })
            .is_ok()
    );
    assert!(input.syntax_job.is_some());
    assert_ne!(input.syntax_revision, in_flight_revision);
    assert!(input.syntax_cache.is_none());
}

#[test]
fn rapid_syntax_invalidations_eventually_apply_only_latest_revision() {
    let mut input = TextareaInput::<()>::new()
        .value("# Initial")
        .language(Language::Markdown);
    for revision in 0..25 {
        input.set_value(format!("# Revision {revision}"));
    }
    let expected_revision = input.syntax_revision;

    finish_syntax(&mut input);

    assert_eq!(
        input.syntax_cache.as_ref().unwrap().revision,
        expected_revision
    );
    assert_eq!(input.current_value(), "# Revision 24");
}

#[test]
fn textarea_rebuilds_invalidated_highlighting_after_edits_and_set_value() {
    let mut input = TextareaInput::<()>::new()
        .value("fn main")
        .language(Language::Rust);
    let plain = Style::default().fg(theme().subtle_fg());

    assert!(finish_syntax(&mut input).changed);
    assert_ne!(input.visible_lines(20, 1).lines[0].spans[0].style, plain);

    input.cursor = input.len_chars();
    input.on_key(Key::Char('x'));
    assert_ne!(
        input.syntax_cache.as_ref().unwrap().revision,
        input.syntax_revision
    );
    finish_syntax(&mut input);
    assert_ne!(input.visible_lines(20, 1).lines[0].spans[0].style, plain);

    input.set_value("let value = 1;");
    assert_ne!(
        input.syntax_cache.as_ref().unwrap().revision,
        input.syntax_revision
    );
    finish_syntax(&mut input);
    assert_ne!(input.visible_lines(20, 1).lines[0].spans[0].style, plain);
}

#[test]
fn textarea_edit_keeps_previous_highlighting_until_cache_rebuilds() {
    let mut input = TextareaInput::<()>::new()
        .value("fn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut input);
    let highlighted_style = input.visible_lines(20, 1).lines[0].spans[0].style;

    input.cursor = input.len_chars();
    input.on_key(Key::Char('x'));
    let line = &input.visible_lines(20, 1).lines[0];

    assert_eq!(line.spans[0].style, highlighted_style);
}

#[test]
fn textarea_edit_uses_stale_highlighting_only_for_unchanged_prefix() {
    let mut input = TextareaInput::<()>::new()
        .value("fn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut input);

    input.cursor = 0;
    input.on_key(Key::Char('x'));
    let line = &input.visible_lines(20, 1).lines[0];

    assert_eq!(line.spans[0].content, "x");
    assert_eq!(
        line.spans[0].style,
        Style::default().fg(theme().subtle_fg())
    );
}

#[test]
fn textarea_highlight_styles_stay_aligned_after_wrapping() {
    let mut input = TextareaInput::<()>::new()
        .value("    fn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut input);
    let plain = Style::default().fg(theme().subtle_fg());

    let lines = input.visible_lines(4, 4);
    let keyword = lines
        .lines
        .iter()
        .find(|line| line.spans.first().is_some_and(|span| span.content == "f"))
        .unwrap();

    assert_eq!(keyword.spans[1].content, "n");
    assert_ne!(keyword.spans[0].style, plain);
    assert_eq!(keyword.spans[0].style, keyword.spans[1].style);
}

#[test]
fn textarea_renders_rust_keyword_with_highlighter_style() {
    let mut input = TextareaInput::<()>::new()
        .value("fn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut input);

    let line = &input.visible_lines(20, 1).lines[0];
    let plain = Style::default().fg(theme().subtle_fg());

    assert_eq!(line.spans[0].content, "f");
    assert_ne!(line.spans[0].style, plain);
    assert_eq!(line.spans[0].style, line.spans[1].style);
}

#[test]
fn syntax_navigation_focus_keeps_keyword_foreground_and_adds_focus_background() {
    let mut unfocused = TextareaInput::<()>::new()
        .value("fn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut unfocused);
    let keyword_fg = unfocused.visible_lines(20, 1).lines[0].spans[0].style.fg;
    let focused = unfocused.focused(true);

    let line = &focused.visible_lines(20, 1).lines[0];

    assert_eq!(line.spans[0].style.fg, keyword_fg);
    assert_eq!(line.spans[0].style.bg, Some(theme().highlight_bg()));
    assert!(!line.spans[0].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn syntax_navigation_focus_styles_empty_placeholder_background_only() {
    let input = TextareaInput::<()>::new()
        .placeholder("Write Rust")
        .language(Language::Rust)
        .focused(true);

    let style = input.visible_lines(20, 1).lines[0].spans[0].style;

    assert_eq!(style.fg, Some(theme().muted_fg()));
    assert_eq!(style.bg, Some(theme().highlight_bg()));
    assert!(!style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn syntax_insert_mode_keeps_highlighting_without_navigation_background_and_shows_cursor() {
    let mut input = TextareaInput::<()>::new()
        .value("fn main() {}")
        .language(Language::Rust)
        .focused(true);
    finish_syntax(&mut input);
    let keyword_fg = input.visible_lines(20, 1).lines[0].spans[0].style.fg;
    let mut ctx = EventCtx::default();
    input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

    let line = &input.visible_lines(20, 1).lines[0];

    assert_eq!(line.spans[0].style.fg, keyword_fg);
    assert_ne!(line.spans[0].style.bg, Some(theme().highlight_bg()));
    assert_eq!(line.spans.last().unwrap().content.as_ref(), " ");
    assert_eq!(
        line.spans.last().unwrap().style.bg,
        Some(theme().highlight_bg())
    );
}

#[test]
fn borderless_syntax_textarea_fills_content_area_with_focus_background() {
    let mut input = TextareaInput::<()>::new()
        .value("fn")
        .min_rows(2)
        .language(Language::Rust)
        .focused(true);
    finish_syntax(&mut input);
    let mut terminal = Terminal::new(TestBackend::new(8, 2)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("textarea should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().highlight_bg());
    assert_eq!(buffer.cell((7, 0)).unwrap().bg, theme().highlight_bg());
    assert_eq!(buffer.cell((7, 1)).unwrap().bg, theme().highlight_bg());
}

#[test]
fn plain_textarea_navigation_focus_keeps_selected_style() {
    let input = TextareaInput::<()>::new().value("plain").focused(true);

    let style = input.visible_lines(10, 1).lines[0].spans[0].style;

    assert_eq!(style.fg, Some(theme().highlight_fg()));
    assert_eq!(style.bg, Some(theme().highlight_bg()));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn textarea_highlight_styles_align_across_unicode_and_lines() {
    let mut input = TextareaInput::<()>::new()
        .value("é🙂\nfn main() {}")
        .language(Language::Rust);
    finish_syntax(&mut input);

    let cache = input.syntax_cache.as_ref().unwrap();
    assert_eq!(cache.styles.len(), input.current_value().chars().count());
    let lines = input.visible_lines(20, 2);
    assert_eq!(lines.lines[0].spans[0].content, "é");
    assert_eq!(lines.lines[0].spans[1].content, "🙂");
    assert_eq!(lines.lines[1].spans[0].content, "f");
    assert_eq!(lines.lines[1].spans[0].style, cache.styles[3]);
    assert_eq!(lines.lines[1].spans[1].style, cache.styles[4]);
}

#[test]
fn textarea_without_language_keeps_plain_rendering() {
    let mut input = TextareaInput::<()>::new().value("fn main() {}");
    Animated::tick(&mut input, Duration::ZERO, AnimationSettings::default());

    assert!(input.syntax_cache.is_none());
    let line = &input.visible_lines(20, 1).lines[0];
    let plain = Style::default().fg(theme().subtle_fg());
    assert!(line.spans.iter().all(|span| span.style == plain));
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
fn disabled_textarea_uses_dashed_panel_border() {
    let input = TextareaInput::<()>::new()
        .value("locked")
        .panel("Notes")
        .disabled(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

    terminal
        .draw(|frame| input.render(frame, frame.area()))
        .expect("textarea should render");

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
fn focused_disabled_textarea_uses_dimmed_highlight_without_cursor() {
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
    assert_eq!(buffer.cell((0, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((3, 0)).unwrap().fg, theme().accent_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().fg, theme().highlight_fg());
    assert_eq!(buffer.cell((1, 1)).unwrap().bg, disabled_input_background());
    assert_eq!(buffer.cell((7, 1)).unwrap().bg, disabled_input_background());
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
