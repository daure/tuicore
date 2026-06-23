use super::*;

#[test]
fn date_time_picker_tab_moves_forward_once_then_allows_focus_to_leave() {
    let mut picker = DateTimePicker::<()>::new();

    let first_tab = picker.on_key(Key::Tab.into());
    let second_tab = picker.on_key(Key::Tab.into());

    assert!(first_tab.handled);
    assert_eq!(picker.active, DateTimePart::Time);
    assert!(!second_tab.handled);
    assert_eq!(picker.active, DateTimePart::Time);
}

#[test]
fn date_time_picker_backtab_moves_backward_once_then_allows_focus_to_leave() {
    let mut picker = DateTimePicker::<()>::new();
    picker.active = DateTimePart::Time;

    let first_backtab = picker.on_key(Key::BackTab.into());
    let second_backtab = picker.on_key(Key::BackTab.into());

    assert!(first_backtab.handled);
    assert_eq!(picker.active, DateTimePart::Date);
    assert!(!second_backtab.handled);
    assert_eq!(picker.active, DateTimePart::Date);
}

#[test]
fn date_time_picker_focus_restarts_at_date_part() {
    let mut picker = DateTimePicker::<()>::new();
    picker.active = DateTimePart::Time;
    let mut ctx = FocusCtx::<()>::default();

    picker.focus(None, true, &mut ctx);

    assert_eq!(picker.active, DateTimePart::Date);
    assert!(picker.date.is_focused());
    assert!(!picker.time.is_focused());
}

#[test]
fn date_time_picker_value_none_clears_date_selection() {
    let date = Date::from_calendar_date(2026, Month::June, 22).unwrap();
    let time = Time::from_hms(10, 30, 0).unwrap();
    let value = PrimitiveDateTime::new(date, time);

    let picker = DateTimePicker::<()>::new().value(Some(value)).value(None);

    assert_eq!(picker.current_value(), None);
    assert_eq!(picker.time.current_value(), time);
}
