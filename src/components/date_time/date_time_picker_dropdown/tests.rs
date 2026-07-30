use super::*;
use ratatui::style::Modifier;
use ratatui::{Terminal, backend::TestBackend};
use std::time::Duration as StdDuration;

#[test]
fn focused_field_is_bold_and_unfocused_field_is_not() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    assert!(
        !dropdown.field_line(31).spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );

    dropdown.focused = true;

    let style = dropdown.field_line(31).spans[0].style;
    assert_eq!(style.fg, Some(theme().highlight_fg()));
    assert_eq!(style.bg, Some(theme().highlight_bg()));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn empty_date_time_picker_field_uses_muted_placeholder_foreground() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();

    for focused in [false, true] {
        dropdown.focused = focused;
        let line = dropdown.field_line(31);
        let placeholder = line
            .spans
            .iter()
            .find(|span| span.content.contains("Select date & time"))
            .expect("placeholder span should render");
        assert_eq!(placeholder.style.fg, Some(theme().muted_fg()));
    }
}

#[test]
fn date_time_picker_dropdown_switches_to_time_after_date_selection() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    dropdown.set_open(true);

    let mut ctx = EventCtx::new(crate::animation_settings());
    let outcome = dropdown.event(&TuiEvent::Key(crate::Key::Enter.into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.step, DateTimeDropdownStep::Time);
}

#[test]
fn control_enter_submits_highlighted_date_without_opening_time_picker() {
    let initial = Date::from_calendar_date(2026, time::Month::June, 25)
        .unwrap()
        .with_time(time::Time::from_hms(10, 20, 0).unwrap());
    let expected = Date::from_calendar_date(2026, time::Month::June, 26)
        .unwrap()
        .with_time(initial.time());
    let mut dropdown = DateTimePickerDropdown::new()
        .value(Some(initial))
        .on_select(|selected| selected);
    dropdown.set_open(true);
    let mut ctx = EventCtx::default();
    dropdown.event(&TuiEvent::Key(crate::Key::Right.into()), &mut ctx);

    let outcome = dropdown.event(
        &TuiEvent::Key(crate::KeyEvent {
            code: crate::Key::Enter,
            modifiers: crate::KeyModifiers::CONTROL,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.current_value(), Some(expected));
    assert_eq!(ctx.messages(), &[expected]);
}

#[test]
fn date_time_picker_dropdown_day_quick_match_advances_to_hour() {
    let value = Date::from_calendar_date(2026, time::Month::June, 22)
        .unwrap()
        .with_time(time::Time::from_hms(9, 30, 0).unwrap());
    let mut dropdown = DateTimePickerDropdown::<()>::new().value(Some(value));
    dropdown.time.on_key(crate::Key::Enter);
    dropdown.set_open(true);
    let mut ctx = EventCtx::default();

    let outcome = dropdown.event(&TuiEvent::Key(crate::Key::Char('4').into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert_eq!(dropdown.step, DateTimeDropdownStep::Time);
    assert_eq!(dropdown.time.active_field(), TimeField::Hour);
}

#[test]
fn date_time_picker_dropdown_closes_when_two_digits_complete_minutes() {
    let initial = Date::from_calendar_date(2026, time::Month::June, 18)
        .unwrap()
        .with_time(time::Time::from_hms(9, 5, 0).unwrap());
    let expected = initial.replace_time(time::Time::from_hms(9, 30, 0).unwrap());
    let mut dropdown = DateTimePickerDropdown::new()
        .value(Some(initial))
        .on_select(|selected| selected);
    dropdown.set_open(true);
    let mut ctx = EventCtx::default();
    dropdown.event(&TuiEvent::Key(crate::Key::Enter.into()), &mut ctx);
    dropdown.event(&TuiEvent::Key(crate::Key::Enter.into()), &mut ctx);

    dropdown.event(&TuiEvent::Key(crate::Key::Char('3').into()), &mut ctx);
    let outcome = dropdown.event(&TuiEvent::Key(crate::Key::Char('0').into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.current_value(), Some(expected));
    assert_eq!(ctx.messages(), &[expected]);
}

#[test]
fn date_time_picker_dropdown_expires_pending_quick_jump_digit() {
    let initial = Date::from_calendar_date(2026, time::Month::June, 15)
        .unwrap()
        .with_time(time::Time::from_hms(9, 30, 0).unwrap());
    let mut dropdown = DateTimePickerDropdown::<()>::new().value(Some(initial));
    dropdown.set_open(true);
    let mut ctx = EventCtx::default();
    dropdown.event(&TuiEvent::Key(crate::Key::Char('1').into()), &mut ctx);

    let tick = dropdown.tick(StdDuration::from_millis(1_001), crate::animation_settings());
    assert!(tick.changed);
    dropdown.event(&TuiEvent::Key(crate::Key::Char('8').into()), &mut ctx);

    assert_eq!(
        dropdown.date.cursor(),
        Date::from_calendar_date(2026, time::Month::June, 8).unwrap()
    );
}

#[test]
fn date_time_picker_dropdown_accepts_external_datetime() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    let mut ctx = EventCtx::new(crate::animation_settings());

    let outcome = dropdown.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: String::from("2026-07-22 09:30"),
            line: 1,
            col: 17,
        }),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(
        dropdown.current_value(),
        Some(
            Date::from_calendar_date(2026, time::Month::July, 22)
                .unwrap()
                .with_time(time::Time::from_hms(9, 30, 0).unwrap())
        )
    );
}

#[test]
fn second_precision_external_editor_round_trip_preserves_date_time() {
    let value = Date::from_calendar_date(2026, time::Month::July, 22)
        .unwrap()
        .with_time(time::Time::from_hms(9, 30, 42).unwrap());
    let mut dropdown = DateTimePickerDropdown::<()>::new()
        .value(Some(value))
        .precision(TimePrecision::HourMinuteSecond);
    let mut launch = EventCtx::default();

    dropdown.event(
        &TuiEvent::Key(crate::KeyEvent {
            code: crate::Key::Char('o'),
            modifiers: crate::KeyModifiers::CONTROL,
        }),
        &mut launch,
    );

    let request = launch
        .external_editor_request()
        .expect("external editor should be requested");
    assert_eq!(request.value, "2026-07-22 09:30:42");
    dropdown.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: request.value.clone(),
            line: request.line,
            col: request.col,
        }),
        &mut EventCtx::default(),
    );
    assert_eq!(dropdown.current_value(), Some(value));
}

#[test]
fn second_precision_field_renders_and_measures_seconds() {
    let value = Date::from_calendar_date(2026, time::Month::July, 22)
        .unwrap()
        .with_time(time::Time::from_hms(9, 30, 42).unwrap());
    let dropdown = DateTimePickerDropdown::<()>::new()
        .value(Some(value))
        .precision(TimePrecision::HourMinuteSecond);

    assert_eq!(dropdown.measure_size(), (34, 1));
    let text = dropdown
        .field_line(34)
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert!(text.contains("09:30:42"));
}

#[test]
fn focused_time_popup_border_uses_accent_chrome() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    dropdown.focused = true;
    dropdown.set_open(true);
    dropdown.step = DateTimeDropdownStep::Time;
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).expect("terminal should build");

    terminal
        .draw(|frame| dropdown.render_portal_popup(frame, frame.area()))
        .expect("popup should render");

    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().fg,
        theme().accent_fg()
    );
}

#[test]
fn date_time_picker_dropdown_forwards_first_day_of_week_builder_and_setter() {
    let mut dropdown = DateTimePickerDropdown::<()>::new().first_day_of_week(time::Weekday::Sunday);
    assert_eq!(
        dropdown.date.configured_first_day_of_week(),
        time::Weekday::Sunday
    );

    dropdown.set_first_day_of_week(time::Weekday::Monday);

    assert_eq!(
        dropdown.date.configured_first_day_of_week(),
        time::Weekday::Monday
    );
}

#[test]
fn closed_date_time_picker_dropdown_does_not_take_keys_before_global_hotkeys() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();

    dropdown.layout(Rect::new(0, 0, 31, 1), &mut ctx);

    assert!(!ctx.focus_targets()[0].focused_events_before_global_hotkeys);
}

#[test]
fn open_date_time_picker_dropdown_takes_picker_keys_before_global_hotkeys() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    dropdown.set_open(true);
    let mut ctx = LayoutCtx::new();

    dropdown.layout(Rect::new(0, 0, 31, 1), &mut ctx);

    assert!(ctx.focus_targets()[0].focused_events_before_global_hotkeys);
}

#[test]
fn date_time_picker_dropdown_clamps_vertical_popup_for_off_bound_fields() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    let bounds = Rect::new(10, 10, 40, 20);

    for (field, expected) in [
        (Rect::new(15, 8, 24, 4), Rect::new(15, 12, 24, 10)),
        (Rect::new(15, 2, 24, 3), Rect::new(15, 10, 24, 10)),
        (Rect::new(15, 28, 24, 4), Rect::new(15, 18, 24, 10)),
        (Rect::new(15, 35, 24, 3), Rect::new(15, 20, 24, 10)),
    ] {
        dropdown.field_area = field;
        assert_eq!(dropdown.popup_area(bounds), expected);
    }
}

#[test]
fn date_time_picker_dropdown_keeps_calendar_width_when_field_shrinks() {
    let mut dropdown = DateTimePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();
    let bounds = Rect::new(0, 0, 40, 20);

    dropdown.layout(Rect::new(30, 2, 10, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(17, 3, 23, 10));
}

#[test]
fn open_date_time_picker_dropdown_cancel_keys_close_without_requesting_unfocus() {
    for key in [
        crate::KeyEvent::from(crate::Key::Esc),
        crate::KeyEvent {
            code: crate::Key::Char('['),
            modifiers: crate::KeyModifiers::CONTROL,
        },
    ] {
        let mut dropdown = DateTimePickerDropdown::<()>::new();
        dropdown.focused = true;
        dropdown.set_open(true);
        let mut ctx = EventCtx::default();

        let outcome = dropdown.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(ctx.focus_request(), None);
    }
}
