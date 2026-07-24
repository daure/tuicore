use super::*;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn date_picker_dropdown_normalizes_committed_hotkey() {
    let mut dropdown = DatePickerDropdown::<()>::new().hotkey(" D ");
    let mut ctx = EventCtx::new(crate::animation_settings());

    let outcome = dropdown.handle_hotkey(&HotkeyEvent::Commit("d".into()), &mut ctx);

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
}

#[test]
fn date_picker_dropdown_forwards_first_day_of_week_builder_and_setter() {
    let date = Date::from_calendar_date(2026, time::Month::June, 15).unwrap();
    let mut dropdown = DatePickerDropdown::<()>::new()
        .today(date)
        .first_day_of_week(time::Weekday::Sunday);
    let mut terminal = Terminal::new(TestBackend::new(23, 10)).expect("terminal should build");
    terminal
        .draw(|frame| dropdown.picker.render(frame, frame.area()))
        .expect("picker should render");
    assert_eq!(
        terminal.backend().buffer().cell((1, 2)).unwrap().symbol(),
        "S"
    );

    dropdown.set_first_day_of_week(time::Weekday::Monday);
    terminal
        .draw(|frame| dropdown.picker.render(frame, frame.area()))
        .expect("picker should render");
    assert_eq!(
        terminal.backend().buffer().cell((1, 2)).unwrap().symbol(),
        "M"
    );
}

#[test]
fn date_picker_dropdown_measure_stays_field_height_when_open() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let proposal = LayoutProposal::unbounded();

    assert_eq!(dropdown.measure(proposal).preferred.height, 1);

    dropdown.set_open(true);

    assert_eq!(dropdown.measure(proposal).preferred.height, 1);
}

#[test]
fn closed_date_picker_dropdown_does_not_take_keys_before_global_hotkeys() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();

    dropdown.layout(Rect::new(0, 0, 24, 1), &mut ctx);

    assert!(!ctx.focus_targets()[0].focused_events_before_global_hotkeys);
}

#[test]
fn open_date_picker_dropdown_takes_picker_keys_before_global_hotkeys() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    dropdown.set_open(true);
    let mut ctx = LayoutCtx::new();

    dropdown.layout(Rect::new(0, 0, 24, 1), &mut ctx);

    assert!(ctx.focus_targets()[0].focused_events_before_global_hotkeys);
}

#[test]
fn date_picker_dropdown_places_popup_inside_overlay_bounds() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();
    let bounds = Rect::new(0, 0, 80, 24);

    dropdown.layout(Rect::new(5, 2, 30, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(5, 3, 24, 10));

    dropdown.layout(Rect::new(5, 20, 30, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(5, 10, 24, 10));
}

#[test]
fn focused_closed_enter_requests_submit_once_and_opens() {
    let mut dropdown = DatePickerDropdown::new().on_submit(|| "submit");
    dropdown.focused = true;
    let mut ctx = EventCtx::default();

    let outcome = dropdown.event(
        &TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter)),
        &mut ctx,
    );

    assert_eq!(outcome, EventOutcome::Handled);
    assert!(dropdown.is_open());
    assert_eq!(ctx.messages(), &["submit"]);
}

#[test]
fn open_enter_selects_without_submit_request() {
    let mut dropdown = DatePickerDropdown::new()
        .today(Date::from_calendar_date(2026, time::Month::July, 16).unwrap())
        .on_submit(|| "submit");
    dropdown.focused = true;
    dropdown.set_open(true);
    let mut ctx = EventCtx::default();

    dropdown.event(
        &TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter)),
        &mut ctx,
    );

    assert!(ctx.messages().is_empty());
}

#[test]
fn inactive_external_editor_session_requests_submit_once_and_closes_on_response() {
    let mut dropdown = DatePickerDropdown::new()
        .today(Date::from_calendar_date(2026, time::Month::July, 16).unwrap())
        .on_submit(|| "start")
        .on_select(|_| "select");
    let mut launch = EventCtx::default();

    dropdown.event(
        &TuiEvent::Key(crate::KeyEvent {
            code: crate::Key::Char('o'),
            modifiers: crate::KeyModifiers::CONTROL,
        }),
        &mut launch,
    );

    assert!(dropdown.is_open());
    assert_eq!(launch.messages(), &["start"]);
    assert!(launch.external_editor_request().is_some());

    let mut response = EventCtx::default();
    dropdown.event(
        &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
            value: "2026-07-20".to_string(),
            line: 1,
            col: 1,
        }),
        &mut response,
    );
    assert!(!dropdown.is_open());
    assert_eq!(response.messages(), &["select"]);
}
