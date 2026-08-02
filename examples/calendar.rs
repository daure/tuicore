use ratatui::text::{Line, Text};
use time::{Date, Duration, Month, Time};
use tuicore::{Calendar, CalendarEntryRole, CalendarSpan, CalendarTypedEvent, CalendarView, Panel};

#[derive(Clone)]
struct ScheduleEntry {
    id: u8,
    title: &'static str,
    span: CalendarSpan,
    role: CalendarEntryRole,
    detail: &'static str,
}

fn demo_date() -> Date {
    Date::from_calendar_date(2026, Month::August, 3).expect("valid demo date")
}

fn at(date: Date, hour: u8, minute: u8) -> time::PrimitiveDateTime {
    date.with_time(Time::from_hms(hour, minute, 0).expect("valid demo time"))
}

fn entries() -> Vec<ScheduleEntry> {
    let monday = demo_date();
    vec![
        ScheduleEntry {
            id: 1,
            title: "Sprint planning",
            span: CalendarSpan::timed(at(monday, 9, 30), at(monday, 10, 30)),
            role: CalendarEntryRole::Accent,
            detail: "Set sprint goal and confirm team capacity.",
        },
        ScheduleEntry {
            id: 2,
            title: "Design review",
            span: CalendarSpan::timed(
                at(monday + Duration::days(1), 14, 0),
                at(monday + Duration::days(1), 15, 0),
            ),
            role: CalendarEntryRole::Success,
            detail: "Review navigation and empty states.",
        },
        ScheduleEntry {
            id: 3,
            title: "Release freeze",
            span: CalendarSpan::all_day_range(
                monday + Duration::days(3),
                monday + Duration::days(5),
            ),
            role: CalendarEntryRole::Warning,
            detail: "Two-day freeze before release.",
        },
    ]
}

fn event_status(event: &CalendarTypedEvent<u8>) -> String {
    match event {
        CalendarTypedEvent::ViewChanged { view } => format!("view: {view:?}"),
        CalendarTypedEvent::EntryActivated { entry_id } => format!("activated #{entry_id}"),
        CalendarTypedEvent::DateActivated { date } => format!("opened {date}"),
        CalendarTypedEvent::CursorChanged { date } => format!("cursor: {date}"),
        CalendarTypedEvent::DrillDown { from, to } => format!("{from:?} → {to:?}"),
        CalendarTypedEvent::Back { from, to } => format!("{from:?} → {to:?}"),
        CalendarTypedEvent::EntryHighlighted { entry_id } => format!("entry: {entry_id:?}"),
        CalendarTypedEvent::RangeChanged { start, end } => format!("{start} – {end}"),
    }
}

fn main() -> tuicore::Result<()> {
    tuicore::init();

    let calendar = Calendar::new(
        entries(),
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .cursor(demo_date())
    .view(CalendarView::Week)
    .role(|entry| Some(entry.role))
    .render_entry(|entry| Line::from(entry.title))
    .render_detail(|entry| {
        Text::from(vec![
            Line::from(entry.title),
            Line::from(""),
            Line::from(entry.detail),
        ])
    })
    .event_detail_on_activate(true)
    .on_event(|event| event);
    let app = Panel::new()
        .top_left("Schedule")
        .top_right("m month · w week · d day · Enter drill down")
        .host(calendar);

    tuicore::TreeApp::new(app)
        .on_message(|root, event, ctx| {
            root.panel_mut().set_top_right(event_status(&event));
            ctx.request_redraw();
        })
        .run()
}
