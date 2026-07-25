use tuicore::{
    CalendarKeyBindings, DateTimePicker, DateTimePickerDropdown, TimePicker, TimePrecision,
};

#[test]
fn date_time_configuration_is_available_from_public_api() {
    let _calendar_keys = CalendarKeyBindings::default();

    let mut time_picker = TimePicker::<()>::new()
        .precision(TimePrecision::HourMinuteSecond)
        .minute_step(15);
    time_picker.set_precision(TimePrecision::HourMinute);
    time_picker.set_minute_step(5);

    let mut date_time_picker = DateTimePicker::<()>::new()
        .precision(TimePrecision::HourMinuteSecond)
        .minute_step(15);
    date_time_picker.set_precision(TimePrecision::HourMinute);
    date_time_picker.set_minute_step(5);

    let mut dropdown = DateTimePickerDropdown::<()>::new()
        .precision(TimePrecision::HourMinuteSecond)
        .minute_step(15);
    dropdown.set_precision(TimePrecision::HourMinute);
    dropdown.set_minute_step(5);
}
