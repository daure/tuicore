use super::*;
use crate::event::{Key, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};
use time::{Duration, Month, PrimitiveDateTime, Time};

#[derive(Clone)]
struct DemoEntry {
    id: &'static str,
    title: &'static str,
    span: CalendarSpan,
}

fn rendered_month_header(calendar: &Calendar<DemoEntry, &'static str>) -> Vec<String> {
    let area = Rect::new(0, 0, 70, 12);
    let inner = Panel::inner_area(area);
    let columns = week_columns(Rect::new(inner.x, inner.y, inner.width, 1));
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    let buffer = terminal.backend().buffer();
    columns
        .into_iter()
        .map(|column| {
            (column.x..column.x + 3)
                .map(|x| buffer.cell((x, inner.y)).unwrap().symbol())
                .collect()
        })
        .collect()
}

#[test]
fn drilldown_and_back_follow_stack() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Day);
    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::ACTIVATED);
    assert_eq!(calendar.current_view(), CalendarView::EventDetail);

    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Day);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn direct_view_switch_does_not_push_history() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    calendar.on_key(Key::Enter);
    calendar.on_key(Key::Enter);
    assert_eq!(calendar.current_view(), CalendarView::Day);

    assert_eq!(calendar.on_key(Key::Char('m')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::IDLE);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn navigation_emits_cursor_and_range_events() {
    let mut calendar = demo_calendar().view(CalendarView::Week);

    calendar.on_key(Key::Right);

    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::CursorChanged {
                date: date(2026, Month::June, 23)
            })
    );
}

#[test]
fn day_navigation_highlights_chronological_entries() {
    let mut calendar = demo_calendar().view(CalendarView::Day);

    assert_eq!(calendar.highlighted_entry_id(), Some("standup"));
    assert_eq!(calendar.on_key(Key::Down), CalendarOutcome::CHANGED);
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));
    assert_eq!(calendar.on_key(Key::Up), CalendarOutcome::CHANGED);
    assert_eq!(calendar.highlighted_entry_id(), Some("standup"));
}

#[test]
fn event_detail_preserves_highlighted_entry() {
    let mut calendar = demo_calendar().view(CalendarView::Day);

    calendar.on_key(Key::Down);
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));

    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::ACTIVATED);

    assert_eq!(calendar.current_view(), CalendarView::EventDetail);
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));
}

#[test]
fn ctrl_bracket_goes_back() {
    let mut calendar = demo_calendar().view(CalendarView::Month);
    calendar.on_key(Key::Enter);

    let outcome = calendar.on_key(KeyEvent {
        code: Key::Char('['),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(outcome, CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn escape_climbs_views_before_blur() {
    let mut calendar = demo_calendar().view(CalendarView::Day);

    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::IDLE);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn callback_events_are_drained_after_dispatch() {
    let mut calendar = demo_calendar_with_event_messages()
        .view(CalendarView::Week)
        .on_event(|event| event);
    let mut ctx = EventCtx::default();

    let outcome = calendar.event(&TuiEvent::Key(KeyEvent::from(Key::Right)), &mut ctx);

    assert!(outcome.handled());
    assert!(ctx.messages().contains(&CalendarTypedEvent::CursorChanged {
        date: date(2026, Month::June, 23)
    }));
    assert!(calendar.take_events().is_empty());
}

#[test]
fn all_day_ranges_are_end_exclusive() {
    let start = date(2026, Month::June, 22);
    let span = CalendarSpan::all_day_range(start, start + Duration::days(2));

    assert!(span.covers_date(start));
    assert!(span.covers_date(start + Duration::days(1)));
    assert!(!span.covers_date(start + Duration::days(2)));
}

#[test]
fn week_range_respects_first_weekday() {
    let calendar = demo_calendar()
        .view(CalendarView::Week)
        .first_weekday(Weekday::Sunday);

    assert_eq!(
        calendar.current_range(),
        (date(2026, Month::June, 21), date(2026, Month::June, 27))
    );
}

#[test]
fn week_range_defaults_to_monday() {
    let calendar = demo_calendar().view(CalendarView::Week);

    assert_eq!(
        calendar.current_range(),
        (date(2026, Month::June, 22), date(2026, Month::June, 28))
    );
}

#[test]
fn first_day_of_week_builder_and_setter_change_week_range() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Week)
        .first_day_of_week(Weekday::Sunday);
    assert_eq!(
        calendar.current_range(),
        (date(2026, Month::June, 21), date(2026, Month::June, 27))
    );

    calendar.set_first_day_of_week(Weekday::Tuesday);

    assert_eq!(
        calendar.current_range(),
        (date(2026, Month::June, 16), date(2026, Month::June, 22))
    );
}

#[test]
fn first_day_of_week_builder_and_setter_change_month_header() {
    let mut calendar = demo_calendar().first_day_of_week(Weekday::Sunday);
    assert_eq!(
        rendered_month_header(&calendar),
        ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
    );

    calendar.set_first_day_of_week(Weekday::Monday);

    assert_eq!(
        rendered_month_header(&calendar),
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    );
}

#[test]
fn custom_keybindings_switch_views() {
    let keys = CalendarKeyBindings {
        week_view: vec![KeySpec::plain('v')],
        day_view: vec![KeySpec::plain('b')],
        ..CalendarKeyBindings::default()
    };
    let mut calendar = demo_calendar().view(CalendarView::Month).keybindings(keys);

    assert_eq!(calendar.on_key(Key::Char('v')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.on_key(Key::Char('b')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Day);
}

fn demo_calendar() -> Calendar<DemoEntry, &'static str> {
    Calendar::new(
        [
            DemoEntry {
                id: "planning",
                title: "Planning",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 13, 0),
                    datetime(2026, Month::June, 22, 14, 0),
                ),
            },
            DemoEntry {
                id: "standup",
                title: "Standup",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 9, 30),
                    datetime(2026, Month::June, 22, 10, 0),
                ),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(date(2026, Month::June, 22))
}

fn demo_calendar_with_event_messages()
-> Calendar<DemoEntry, &'static str, CalendarTypedEvent<&'static str>> {
    Calendar::new(
        [
            DemoEntry {
                id: "planning",
                title: "Planning",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 13, 0),
                    datetime(2026, Month::June, 22, 14, 0),
                ),
            },
            DemoEntry {
                id: "standup",
                title: "Standup",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 9, 30),
                    datetime(2026, Month::June, 22, 10, 0),
                ),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(date(2026, Month::June, 22))
}

fn date(year: i32, month: Month, day: u8) -> Date {
    Date::from_calendar_date(year, month, day).expect("valid date")
}

fn datetime(year: i32, month: Month, day: u8, hour: u8, minute: u8) -> PrimitiveDateTime {
    date(year, month, day).with_time(Time::from_hms(hour, minute, 0).expect("valid time"))
}
