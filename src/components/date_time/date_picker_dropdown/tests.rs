use super::*;
use ratatui::style::Modifier;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn focused_field_is_bold_and_unfocused_field_is_not() {
    let date = Date::from_calendar_date(2026, time::Month::August, 1).unwrap();
    let mut dropdown = DatePickerDropdown::<()>::new().value(Some(date));
    assert!(
        !dropdown.field_line(24).spans[0]
            .style
            .add_modifier
            .contains(Modifier::BOLD)
    );

    dropdown.focused = true;

    let style = dropdown.field_line(24).spans[0].style;
    assert_eq!(style.fg, Some(theme().highlight_fg()));
    assert_eq!(style.bg, Some(theme().highlight_bg()));
    assert!(style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn empty_date_picker_field_uses_muted_placeholder_foreground() {
    let mut dropdown = DatePickerDropdown::<()>::new();

    for focused in [false, true] {
        dropdown.focused = focused;
        let line = dropdown.field_line(24);
        let placeholder = line
            .spans
            .iter()
            .find(|span| span.content.contains("Select date"))
            .expect("placeholder span should render");
        assert_eq!(placeholder.style.fg, Some(theme().muted_fg()));
        assert!(line.spans.iter().all(|span| span.style.bg.is_none()));
        assert!(
            line.spans
                .iter()
                .all(|span| !span.style.add_modifier.contains(Modifier::BOLD))
        );
    }
}

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
fn date_picker_dropdown_clamps_vertical_popup_for_off_bound_fields() {
    let mut dropdown = DatePickerDropdown::<()>::new();
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
fn date_picker_dropdown_keeps_calendar_width_when_field_shrinks() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    let mut ctx = LayoutCtx::new();
    let bounds = Rect::new(0, 0, 40, 20);

    dropdown.layout(Rect::new(30, 2, 10, 1), &mut ctx);

    assert_eq!(dropdown.popup_area(bounds), Rect::new(17, 3, 23, 10));
}

#[test]
fn date_picker_dropdown_renders_inside_overlay_narrower_than_calendar() {
    let mut dropdown = DatePickerDropdown::<()>::new();
    dropdown.set_open(true);
    dropdown.layout(Rect::new(12, 1, 8, 1), &mut LayoutCtx::new());
    let bounds = Rect::new(2, 1, 18, 10);
    let popup = dropdown.popup_area(bounds);
    let mut terminal = Terminal::new(TestBackend::new(20, 12)).expect("terminal should build");

    terminal
        .draw(|frame| {
            let background = vec![Line::raw(".".repeat(20)); 12];
            frame.render_widget(Paragraph::new(background), frame.area());
            dropdown.render_portal_popup(frame, bounds);
        })
        .expect("dropdown should render");

    assert!(popup.x >= bounds.x);
    assert!(popup.right() <= bounds.right());
    assert!(popup.y >= bounds.y);
    assert!(popup.bottom() <= bounds.bottom());
    for y in 0..12 {
        for x in 0..20 {
            if x < popup.x || x >= popup.right() || y < popup.y || y >= popup.bottom() {
                assert_eq!(
                    terminal.backend().buffer().cell((x, y)).unwrap().symbol(),
                    "."
                );
            }
        }
    }
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
fn open_date_picker_dropdown_escape_closes_without_requesting_unfocus() {
    for key in [
        crate::KeyEvent::from(crate::Key::Esc),
        crate::KeyEvent {
            code: crate::Key::Char('['),
            modifiers: crate::KeyModifiers::CONTROL,
        },
    ] {
        let mut dropdown = DatePickerDropdown::<()>::new();
        dropdown.focused = true;
        dropdown.set_open(true);
        let mut ctx = EventCtx::default();

        let outcome = dropdown.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(ctx.focus_request(), None);
    }
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
