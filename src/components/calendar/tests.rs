use super::*;
use crate::event::{Key, KeyModifiers};
use time::{Duration, Month, PrimitiveDateTime, Time};

#[derive(Clone)]
struct DemoEntry {
    id: &'static str,
    title: &'static str,
    span: CalendarSpan,
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
