use super::*;

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
