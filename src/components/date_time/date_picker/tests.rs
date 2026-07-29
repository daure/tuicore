use super::super::KeyBindingsGuard;
use super::*;
use crate::{Key, KeyBindings, KeyModifiers, KeySpec};
use ratatui::style::Modifier;
use ratatui::{Terminal, backend::TestBackend};
use std::time::Duration as StdDuration;

fn rendered_rows(picker: &DatePicker<()>) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(23, 10)).expect("terminal should build");
    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");
    let buffer = terminal.backend().buffer();
    (0..10)
        .map(|y| {
            (1..22)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect()
        })
        .collect()
}

#[test]
fn date_picker_defaults_to_monday_first_with_dates_in_matching_columns() {
    let date = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let rows = rendered_rows(&DatePicker::new().today(date));

    assert_eq!(rows[2], "Mo Tu We Th Fr Sa Su ");
    assert_eq!(rows[3], " 1  2  3  4  5  6  7 ");
}

#[test]
fn date_picker_sunday_override_rotates_header_and_dates() {
    let date = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let rows = rendered_rows(
        &DatePicker::new()
            .today(date)
            .first_day_of_week(Weekday::Sunday),
    );

    assert_eq!(rows[2], "Su Mo Tu We Th Fr Sa ");
    assert_eq!(rows[3], "31  1  2  3  4  5  6 ");
}

#[test]
fn date_picker_setter_changes_existing_instance_rendering() {
    let date = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::new().today(date);
    assert!(rendered_rows(&picker)[2].starts_with("Mo "));

    picker.set_first_day_of_week(Weekday::Sunday);

    assert!(rendered_rows(&picker)[2].starts_with("Su "));
}

#[test]
fn date_picker_renders_hotkey_on_bottom_right_border() {
    let picker = DatePicker::<()>::new().hotkey("dp");
    let mut terminal = Terminal::new(TestBackend::new(23, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((20, 9)).unwrap().symbol(), "d");
    assert_eq!(buffer.cell((21, 9)).unwrap().symbol(), "p");
}

#[test]
fn date_picker_does_not_render_outside_a_narrow_area() {
    let picker = DatePicker::<()>::new();
    let mut terminal = Terminal::new(TestBackend::new(30, 12)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let background = vec!["x".repeat(30); 12].join("\n");
            frame.render_widget(Paragraph::new(background), frame.area());
            picker.render(frame, Rect::new(1, 1, 10, 10));
        })
        .expect("picker should render");

    let buffer = terminal.backend().buffer();
    for y in 1..11 {
        for x in 11..30 {
            assert_eq!(buffer.cell((x, y)).unwrap().symbol(), "x");
        }
    }
}

#[test]
fn month_navigation_clamps_invalid_days() {
    let jan_31 = Date::from_calendar_date(2024, Month::January, 31).unwrap();
    let feb_29 = Date::from_calendar_date(2024, Month::February, 29).unwrap();
    assert_eq!(add_months(jan_31, 1), feb_29);
}

#[test]
fn date_picker_selects_cursor() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);
    let outcome = picker.on_key(Key::Enter);
    assert!(outcome.selected);
    assert_eq!(picker.current_value(), Some(date));
}

#[test]
fn date_picker_quick_jump_waits_for_two_digit_days() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);

    let pending = picker.on_key(Key::Char('1'));
    assert!(pending.handled);
    assert_eq!(picker.cursor(), june_15);

    let jumped = picker.on_key(Key::Char('8'));
    assert!(jumped.changed);
    assert!(jumped.selected);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::June, 18).unwrap()
    );
    assert_eq!(picker.current_value(), Some(picker.cursor()));
}

#[test]
fn date_picker_quick_jump_handles_three_based_on_month_length() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut june = DatePicker::<()>::new().today(june_15);

    june.on_key(Key::Char('3'));
    assert_eq!(june.cursor(), june_15);
    june.on_key(Key::Char('0'));
    assert_eq!(
        june.cursor(),
        Date::from_calendar_date(2026, Month::June, 30).unwrap()
    );

    let february_15 = Date::from_calendar_date(2026, Month::February, 15).unwrap();
    let mut february = DatePicker::<()>::new().today(february_15);
    february.on_key(Key::Char('3'));
    assert_eq!(
        february.cursor(),
        Date::from_calendar_date(2026, Month::February, 3).unwrap()
    );
}

#[test]
fn date_picker_quick_jump_clears_invalid_day_without_moving() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);

    picker.on_key(Key::Char('3'));
    let invalid = picker.on_key(Key::Char('1'));

    assert!(invalid.handled);
    assert!(!invalid.changed);
    assert_eq!(picker.cursor(), june_15);
    picker.on_key(Key::Char('8'));
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::June, 8).unwrap()
    );
}

#[test]
fn date_picker_quick_jump_enter_and_space_submit_pending_single_digit() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();

    for accept_key in [Key::Enter, Key::Char(' ')] {
        let mut picker = DatePicker::<()>::new().today(june_15);
        picker.on_key(Key::Char('1'));

        let outcome = picker.on_key(accept_key);

        assert!(outcome.handled);
        assert!(outcome.selected);
        assert_eq!(
            picker.cursor(),
            Date::from_calendar_date(2026, Month::June, 1).unwrap()
        );
        assert_eq!(picker.current_value(), Some(picker.cursor()));
    }
}

#[test]
fn date_picker_quick_jump_expires_after_one_second() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.on_key(Key::Char('1'));

    let tick = picker.tick(StdDuration::from_millis(1_001), crate::animation_settings());
    assert!(tick.changed);
    assert_eq!(picker.cursor(), june_15);

    picker.on_key(Key::Char('8'));
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::June, 8).unwrap()
    );
}

#[test]
fn date_picker_quick_jump_underlines_only_matching_days_in_current_month() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.on_key(Key::Char('1'));
    let mut terminal = Terminal::new(TestBackend::new(23, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let buffer = terminal.backend().buffer();
    for day in 1..=19 {
        if day != 1 && day < 10 {
            continue;
        }
        let offset = day - 1;
        let cell_x = 1 + (offset % 7) * 3;
        let cell_y = 3 + offset / 7;
        let prefix_x = cell_x + u16::from(day < 10);
        assert!(
            buffer
                .cell((prefix_x, cell_y))
                .unwrap()
                .modifier
                .contains(Modifier::UNDERLINED),
            "day {day} prefix should be underlined"
        );
        for x in cell_x..cell_x + 3 {
            if x != prefix_x {
                assert!(
                    !buffer
                        .cell((x, cell_y))
                        .unwrap()
                        .modifier
                        .contains(Modifier::UNDERLINED),
                    "only day {day} prefix should be underlined"
                );
            }
        }
    }
    assert!(
        !buffer
            .cell((7, 7))
            .unwrap()
            .modifier
            .contains(Modifier::UNDERLINED)
    );
}

#[test]
fn date_picker_month_quick_jump_uses_shortest_unique_prefix() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let ambiguous = [
        ("ap", Month::April),
        ("au", Month::August),
        ("ja", Month::January),
        ("jun", Month::June),
        ("jul", Month::July),
        ("mar", Month::March),
        ("may", Month::May),
    ];
    for (input, expected) in ambiguous {
        let mut picker = DatePicker::<()>::new().today(june_15);
        picker.view = DatePickerView::Month;
        for character in input.chars() {
            picker.on_key(Key::Char(character));
        }
        assert_eq!(picker.cursor().month(), expected, "input {input}");
        assert_eq!(picker.view, DatePickerView::Day, "input {input}");
    }

    for (character, expected) in [
        ('f', Month::February),
        ('s', Month::September),
        ('o', Month::October),
        ('n', Month::November),
        ('d', Month::December),
    ] {
        let mut picker = DatePicker::<()>::new().today(june_15);
        picker.view = DatePickerView::Month;
        picker.on_key(Key::Char(character));
        assert_eq!(picker.cursor().month(), expected, "input {character}");
        assert_eq!(picker.view, DatePickerView::Day, "input {character}");
    }
}

#[test]
fn date_picker_month_quick_jump_underlines_only_typed_prefix() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.view = DatePickerView::Month;
    picker.on_key(Key::Char('a'));
    let mut terminal = Terminal::new(TestBackend::new(23, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let underlined = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(Modifier::UNDERLINED))
        .map(|cell| cell.symbol())
        .collect::<Vec<_>>();
    assert_eq!(underlined, ["A", "A"]);
}

#[test]
fn date_picker_month_quick_jump_expires_after_one_second() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.view = DatePickerView::Month;
    picker.on_key(Key::Char('a'));

    assert!(
        picker
            .tick(StdDuration::from_millis(1_001), crate::animation_settings())
            .changed
    );
    picker.on_key(Key::Char('p'));

    assert_eq!(picker.cursor(), june_15);
}

#[test]
fn date_picker_year_quick_jump_accepts_any_four_digit_year() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.view = DatePickerView::Year;

    for character in "1984".chars() {
        picker.on_key(Key::Char(character));
    }

    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(1984, Month::June, 15).unwrap()
    );
    assert_eq!(picker.view, DatePickerView::Month);
    assert!((picker.year_page_start..=picker.year_page_start + 23).contains(&1984));
}

#[test]
fn date_picker_year_quick_jump_underlines_typed_prefix_on_visible_years() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.view = DatePickerView::Year;
    picker.on_key(Key::Char('2'));
    picker.on_key(Key::Char('0'));
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let underlined = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .filter(|cell| cell.modifier.contains(Modifier::UNDERLINED))
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(underlined, "20".repeat(24));
}

#[test]
fn date_picker_year_quick_jump_expires_and_partial_confirm_does_not_move() {
    let june_15 = Date::from_calendar_date(2026, Month::June, 15).unwrap();
    for accept_key in [Key::Enter, Key::Char(' ')] {
        let mut picker = DatePicker::<()>::new().today(june_15);
        picker.view = DatePickerView::Year;
        for character in "202".chars() {
            picker.on_key(Key::Char(character));
        }
        let outcome = picker.on_key(accept_key);
        assert!(outcome.handled);
        assert!(!outcome.selected);
        assert_eq!(picker.cursor(), june_15);
    }

    let mut picker = DatePicker::<()>::new().today(june_15);
    picker.view = DatePickerView::Year;
    picker.on_key(Key::Char('1'));
    picker.tick(StdDuration::from_millis(1_001), crate::animation_settings());
    for character in "984".chars() {
        picker.on_key(Key::Char(character));
    }
    assert_eq!(picker.cursor(), june_15);
}

#[test]
fn date_picker_switches_month_and_year_views() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    picker.on_key(Key::Char('m'));
    assert_eq!(picker.view, DatePickerView::Month);
    picker.on_key(Key::Char('y'));
    assert_eq!(picker.view, DatePickerView::Year);
    picker.on_key(Key::Enter);
    assert_eq!(picker.view, DatePickerView::Month);
    picker.on_key(Key::Enter);
    assert_eq!(picker.view, DatePickerView::Day);
}

#[test]
fn date_picker_d_switches_day_and_year_views_to_day_without_changing_selection() {
    let _guard = KeyBindingsGuard::replace(KeyBindings::default());
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();

    for view in [DatePickerView::Day, DatePickerView::Year] {
        let mut picker = DatePicker::<()>::new().today(date).value(Some(date));
        picker.view = view;
        picker.on_key(Key::Right);
        let cursor = picker.cursor();

        let outcome = picker.on_key(Key::Char('d'));

        assert!(outcome.handled);
        assert_eq!(picker.view, DatePickerView::Day);
        assert_eq!(picker.cursor(), cursor);
        assert_eq!(picker.current_value(), Some(date));
    }
}

#[test]
fn date_picker_day_view_binding_requires_exact_modifiers() {
    let _guard = KeyBindingsGuard::replace(KeyBindings::default());
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);
    picker.view = DatePickerView::Year;

    assert!(
        picker
            .on_key(KeyEvent {
                code: Key::Char('d'),
                modifiers: KeyModifiers::CONTROL,
            })
            .handled
    );
    assert_eq!(picker.view, DatePickerView::Year);
    assert_eq!(
        picker.on_key(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::ALT,
        }),
        PickerOutcome::IGNORED
    );
    assert_eq!(picker.view, DatePickerView::Year);
}

#[test]
fn date_picker_uses_arrows_and_plain_hjkl_in_every_view() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let cases = [
        (Key::Left, 'h'),
        (Key::Down, 'j'),
        (Key::Up, 'k'),
        (Key::Right, 'l'),
    ];

    for view in [
        DatePickerView::Day,
        DatePickerView::Month,
        DatePickerView::Year,
    ] {
        for (arrow, character) in cases {
            let mut arrow_picker = DatePicker::<()>::new().today(date);
            arrow_picker.view = view;
            assert!(arrow_picker.on_key(arrow).changed);

            let mut plain_picker = DatePicker::<()>::new().today(date);
            plain_picker.view = view;
            let plain_outcome = plain_picker.on_key(Key::Char(character));
            if view == DatePickerView::Month && character == 'j' {
                assert!(plain_outcome.handled);
                assert_eq!(plain_picker.cursor(), date);
            } else {
                assert!(plain_outcome.changed);
                assert_eq!(plain_picker.cursor(), arrow_picker.cursor());
            }

            let mut controlled_picker = DatePicker::<()>::new().today(date);
            controlled_picker.view = view;
            assert_eq!(
                controlled_picker.on_key(KeyEvent {
                    code: Key::Char(character),
                    modifiers: KeyModifiers::CONTROL,
                }),
                PickerOutcome::IGNORED
            );
            assert_eq!(controlled_picker.cursor(), date);
        }
    }
}

#[test]
fn date_picker_directional_bindings_can_be_overridden_with_builder() {
    let _guard = KeyBindingsGuard::replace(
        KeyBindings::new()
            .with_date_time_picker_day_view([])
            .with_date_time_picker_line_right([KeySpec::plain('d')]),
    );
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    assert!(picker.on_key(Key::Char('d')).changed);
    assert_eq!(picker.cursor(), date + Duration::days(1));
    assert_eq!(picker.on_key(Key::Right), PickerOutcome::IGNORED);
}

#[test]
fn date_picker_day_view_binding_can_be_overridden_with_builder() {
    let _guard = KeyBindingsGuard::replace(
        KeyBindings::new().with_date_time_picker_day_view([KeySpec::plain('v')]),
    );
    let mut picker = DatePicker::<()>::new();
    picker.view = DatePickerView::Year;

    assert!(picker.on_key(Key::Char('v')).handled);
    assert_eq!(picker.view, DatePickerView::Day);
    picker.view = DatePickerView::Year;
    assert_eq!(picker.on_key(Key::Char('d')), PickerOutcome::IGNORED);
}

#[test]
fn date_picker_directional_bindings_can_be_overridden_with_toml() {
    let bindings = KeyBindings::from_toml_str(
        r#"
        [date_time_picker]
        line_up = "w"
        "#,
    );
    let _guard = KeyBindingsGuard::replace(bindings);
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    assert!(picker.on_key(Key::Char('w')).changed);
    assert_eq!(picker.cursor(), date - Duration::days(7));
    assert_eq!(picker.on_key(Key::Up), PickerOutcome::IGNORED);
}

#[test]
fn date_picker_day_view_binding_can_be_overridden_with_toml() {
    let bindings = KeyBindings::from_toml_str(
        r#"
        [date_time_picker]
        day_view = "v"
        "#,
    );
    let _guard = KeyBindingsGuard::replace(bindings);
    let mut picker = DatePicker::<()>::new();
    picker.view = DatePickerView::Month;

    assert!(picker.on_key(Key::Char('v')).handled);
    assert_eq!(picker.view, DatePickerView::Day);
    picker.view = DatePickerView::Year;
    assert_eq!(picker.on_key(Key::Char('d')), PickerOutcome::IGNORED);
}

#[test]
fn date_picker_today_shortcut_moves_cursor_to_today() {
    let today = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(today);

    picker.on_key(Key::Right);
    assert_ne!(picker.cursor(), today);
    let outcome = picker.on_key(Key::Char('t'));

    assert!(outcome.handled);
    assert_eq!(picker.cursor(), today);
}

#[test]
fn date_picker_gg_and_shift_g_match_home_and_end() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    let first_g = picker.on_key(Key::Char('g'));
    let second_g = picker.on_key(Key::Char('g'));
    assert!(first_g.handled);
    assert!(second_g.handled);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::June, 1).unwrap()
    );

    let shift_g = picker.on_key(KeyEvent {
        code: Key::Char('g'),
        modifiers: crate::KeyModifiers::SHIFT,
    });
    assert!(shift_g.handled);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::June, 30).unwrap()
    );
}

#[test]
fn date_picker_home_end_apply_to_month_and_year_views() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    picker.on_key(Key::Char('m'));
    picker.on_key(Key::Home);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::January, 22).unwrap()
    );
    picker.on_key(Key::End);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2026, Month::December, 22).unwrap()
    );

    picker.on_key(Key::Char('y'));
    picker.on_key(Key::Home);
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2016, Month::December, 22).unwrap()
    );
    picker.on_key(KeyEvent {
        code: Key::Char('g'),
        modifiers: crate::KeyModifiers::SHIFT,
    });
    assert_eq!(
        picker.cursor(),
        Date::from_calendar_date(2039, Month::December, 22).unwrap()
    );
}

#[test]
fn date_picker_applies_external_editor_date() {
    let mut picker = DatePicker::<()>::new();
    let response = ExternalEditorResponse {
        value: String::from("2027-03-04\n"),
        line: 1,
        col: 1,
    };

    let outcome = picker.apply_external_editor_response(&response);

    assert!(outcome.selected);
    assert_eq!(
        picker.current_value(),
        Some(Date::from_calendar_date(2027, Month::March, 4).unwrap())
    );
}

#[test]
fn date_picker_registers_and_tracks_pending_hotkey() {
    let mut picker = DatePicker::<()>::new().hotkey("dt");
    let mut layout = LayoutCtx::new();
    picker.layout(Rect::new(0, 0, 24, 10), &mut layout);
    assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["dt"]);

    let mut ctx = EventCtx::<()>::new(crate::animation_settings());
    let pending = picker.event(
        &TuiEvent::Hotkey(HotkeyEvent::Pending("d".into())),
        &mut ctx,
    );
    assert_eq!(pending, EventOutcome::Ignored);
    assert_eq!(picker.pending_hotkey_prefix.as_deref(), Some("d"));
}

#[test]
fn date_picker_min_and_max_clamp_selected_value() {
    let min = Date::from_calendar_date(2026, Month::June, 1).unwrap();
    let before_min = Date::from_calendar_date(2026, Month::May, 1).unwrap();
    let picker = DatePicker::<()>::new().value(Some(before_min)).min(min);
    assert_eq!(picker.current_value(), Some(min));
    assert_eq!(picker.cursor(), min);

    let max = Date::from_calendar_date(2026, Month::July, 1).unwrap();
    let after_max = Date::from_calendar_date(2026, Month::August, 1).unwrap();
    let picker = DatePicker::<()>::new().value(Some(after_max)).max(max);
    assert_eq!(picker.current_value(), Some(max));
    assert_eq!(picker.cursor(), max);
}

#[test]
fn date_picker_cancel_restores_clamped_today_when_value_is_empty() {
    let today = Date::from_calendar_date(2026, Month::May, 1).unwrap();
    let min = Date::from_calendar_date(2026, Month::June, 1).unwrap();
    let mut picker = DatePicker::<()>::new().today(today).min(min);

    picker.on_key(Key::Right);
    let outcome = picker.on_key(Key::Esc);

    assert!(outcome.canceled);
    assert_eq!(picker.cursor(), min);
}

#[test]
fn direct_date_picker_cancel_keys_request_unfocus_and_remain_handled() {
    let selected = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let cancel_keys = [
        KeyEvent::from(Key::Esc),
        KeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::CONTROL,
        },
    ];

    for key in cancel_keys {
        let mut picker = DatePicker::<()>::new()
            .today(selected)
            .value(Some(selected));
        picker.on_key(Key::Right);
        let mut ctx = EventCtx::default();

        let outcome = picker.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.focus_request(), Some(&crate::FocusRequest::Unfocus));
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(picker.current_value(), Some(selected));
    }
}

#[test]
fn date_picker_navigation_clamps_at_supported_date_bounds() {
    let mut min_picker = DatePicker::<()>::new().today(Date::MIN);
    min_picker.on_key(Key::Left);
    min_picker.on_key(Key::PageUp);
    assert_eq!(min_picker.cursor(), Date::MIN);

    let mut max_picker = DatePicker::<()>::new().today(Date::MAX);
    max_picker.on_key(Key::Right);
    max_picker.on_key(Key::PageDown);
    assert_eq!(max_picker.cursor(), Date::MAX);
}
