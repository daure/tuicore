use super::*;
use crate::{Key, LayoutSize};
use ratatui::{Terminal, backend::TestBackend};
use time::{Date, Month, Time};

#[test]
fn stepped_date_time_picker_initially_renders_only_date() {
    let value = Date::from_calendar_date(2026, Month::June, 22)
        .unwrap()
        .with_time(Time::from_hms(9, 30, 0).unwrap());
    let picker = DateTimePicker::<()>::new()
        .value(Some(value))
        .layout(DateTimePickerLayout::Stepped);
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let rendered =
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .fold(String::new(), |mut rendered, cell| {
                rendered.push_str(cell.symbol());
                rendered
            });
    assert!(rendered.contains("June 2026"));
    assert!(!rendered.contains("09:30"));
}

#[test]
fn stepped_date_time_picker_switches_to_time_after_date_selection() {
    let mut picker = DateTimePicker::<()>::new().layout(DateTimePickerLayout::Stepped);
    let mut ctx = EventCtx::default();

    let outcome = picker.event(&TuiEvent::Key(Key::Enter.into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(picker.active, DateTimePart::Time);
    assert!(ctx.messages().is_empty());
}

#[test]
fn stepped_date_time_picker_renders_time_centered_inside_date_sized_border() {
    let value = Date::from_calendar_date(2026, Month::June, 22)
        .unwrap()
        .with_time(Time::from_hms(9, 30, 0).unwrap());
    let mut picker = DateTimePicker::<()>::new()
        .value(Some(value))
        .layout(DateTimePickerLayout::Stepped);
    picker.on_key(Key::Enter.into());
    let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");

    terminal
        .draw(|frame| picker.render(frame, frame.area()))
        .expect("picker should render");

    let buffer = terminal.backend().buffer();
    assert_ne!(buffer.cell((0, 0)).unwrap().symbol(), " ");
    assert_ne!(buffer.cell((23, 9)).unwrap().symbol(), " ");
    let time = (10..15)
        .map(|x| buffer.cell((x, 4)).unwrap().symbol())
        .collect::<String>();
    assert_eq!(time, "09:30");
}

#[test]
fn stepped_date_time_picker_measure_is_intrinsic_date_surface_size() {
    let picker = DateTimePicker::<()>::new().layout(DateTimePickerLayout::Stepped);

    let hint = picker.measure(LayoutProposal::unbounded());

    assert_eq!(hint.min, LayoutSize::new(24, 10));
    assert_eq!(hint.preferred, LayoutSize::new(24, 10));
    assert!(!hint.expand.width);
    assert!(!hint.expand.height);
}

#[test]
fn stepped_date_time_picker_emits_combined_selection_then_returns_to_date() {
    let value = Date::from_calendar_date(2026, Month::June, 22)
        .unwrap()
        .with_time(Time::from_hms(9, 30, 0).unwrap());
    let mut picker = DateTimePicker::new()
        .value(Some(value))
        .layout(DateTimePickerLayout::Stepped)
        .on_select(|selected| selected);
    let mut date_ctx = EventCtx::default();
    picker.event(&TuiEvent::Key(Key::Enter.into()), &mut date_ctx);
    let mut time_ctx = EventCtx::default();

    let outcome = picker.event(&TuiEvent::Key(Key::Enter.into()), &mut time_ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(picker.active, DateTimePart::Date);
    assert_eq!(picker.current_value(), Some(value));
    assert_eq!(time_ctx.messages(), &[value]);
}

#[test]
fn stepped_date_time_picker_escape_cancels_time_and_returns_to_date() {
    let value = Date::from_calendar_date(2026, Month::June, 22)
        .unwrap()
        .with_time(Time::from_hms(9, 30, 0).unwrap());
    let mut picker = DateTimePicker::new()
        .value(Some(value))
        .layout(DateTimePickerLayout::Stepped)
        .on_select(|selected| selected);
    let mut ctx = EventCtx::default();
    picker.event(&TuiEvent::Key(Key::Enter.into()), &mut ctx);
    picker.event(&TuiEvent::Key(Key::Up.into()), &mut ctx);

    let outcome = picker.event(&TuiEvent::Key(Key::Esc.into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert_eq!(picker.active, DateTimePart::Date);
    assert_eq!(picker.current_value(), Some(value));
    assert!(ctx.messages().is_empty());
}

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

#[test]
fn date_time_picker_forwards_first_day_of_week_builder_and_setter() {
    let mut picker = DateTimePicker::<()>::new().first_day_of_week(time::Weekday::Sunday);
    assert_eq!(
        picker.date.configured_first_day_of_week(),
        time::Weekday::Sunday
    );

    picker.set_first_day_of_week(time::Weekday::Monday);

    assert_eq!(
        picker.date.configured_first_day_of_week(),
        time::Weekday::Monday
    );
}
