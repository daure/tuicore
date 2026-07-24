use super::*;
use ratatui::style::Modifier;

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
