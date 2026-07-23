use super::*;
use crate::{Key, KeyBindings, KeyModifiers, KeySpec, keybindings};
use ratatui::{Terminal, backend::TestBackend};

struct KeyBindingsGuard {
    previous: KeyBindings,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl KeyBindingsGuard {
    fn replace(next: KeyBindings) -> Self {
        let lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
        let previous = keybindings();
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
            assert!(plain_picker.on_key(Key::Char(character)).changed);
            assert_eq!(plain_picker.cursor(), arrow_picker.cursor());

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
        KeyBindings::new().with_date_time_picker_line_right([KeySpec::plain('d')]),
    );
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let mut picker = DatePicker::<()>::new().today(date);

    assert!(picker.on_key(Key::Char('d')).changed);
    assert_eq!(picker.cursor(), date + Duration::days(1));
    assert_eq!(picker.on_key(Key::Right), PickerOutcome::IGNORED);
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
