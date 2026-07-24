use super::*;
use crate::event::{Key, KeyModifiers};
use ratatui::style::Color;
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
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
    let count = if calendar.is_showing_weekends() { 7 } else { 5 };
    let columns = calendar_columns(Rect::new(inner.x, inner.y, inner.width, 1), count);
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

fn rendered_top_border(calendar: &Calendar<DemoEntry, &'static str>, width: u16) -> String {
    let area = Rect::new(0, 0, width, 12);
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    let buffer = terminal.backend().buffer();
    (0..area.width)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect()
}

fn rendered_week_headers(calendar: &Calendar<DemoEntry, &'static str>) -> Vec<String> {
    let area = Rect::new(0, 0, 70, 12);
    let inner = Panel::inner_area(area);
    let count = if calendar.is_showing_weekends() { 7 } else { 5 };
    let columns = calendar_columns(inner, count);
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    let buffer = terminal.backend().buffer();
    columns
        .into_iter()
        .map(|column| {
            (column.x + 1..column.x + 4)
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
    assert_eq!(calendar.on_key(Key::Char('d')), CalendarOutcome::IDLE);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.on_key(Key::Char('b')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Day);
}

#[test]
fn weekends_are_visible_by_default_and_public_controls_update_visibility() {
    let mut calendar = demo_calendar();
    assert!(calendar.is_showing_weekends());

    calendar.set_show_weekends(false);
    assert!(!calendar.is_showing_weekends());

    calendar.toggle_weekends();
    assert!(calendar.is_showing_weekends());

    let calendar = demo_calendar().show_weekends(false);
    assert!(!calendar.is_showing_weekends());
}

#[test]
fn ctrl_w_toggles_weekends_while_plain_w_switches_to_week_view() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('w'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::CHANGED
    );
    assert!(!calendar.is_showing_weekends());
    assert_eq!(calendar.current_view(), CalendarView::Month);

    assert_eq!(calendar.on_key(Key::Char('w')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Week);
}

#[test]
fn custom_binding_overrides_weekend_toggle() {
    let keys = CalendarKeyBindings {
        toggle_weekends: vec![KeySpec::plain('x')],
        ..CalendarKeyBindings::default()
    };
    let mut calendar = demo_calendar().keybindings(keys);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('w'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::IDLE
    );
    assert!(calendar.is_showing_weekends());

    assert_eq!(calendar.on_key(Key::Char('x')), CalendarOutcome::CHANGED);
    assert!(!calendar.is_showing_weekends());
}

#[test]
fn month_header_removes_weekend_columns() {
    let visible = demo_calendar().first_day_of_week(Weekday::Monday);
    assert_eq!(
        rendered_month_header(&visible),
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    );

    let hidden = visible.show_weekends(false);
    assert_eq!(
        rendered_month_header(&hidden),
        ["Mon", "Tue", "Wed", "Thu", "Fri"]
    );
}

#[test]
fn week_view_removes_weekend_columns() {
    let visible = demo_calendar()
        .view(CalendarView::Week)
        .first_day_of_week(Weekday::Monday);
    assert_eq!(
        rendered_week_headers(&visible),
        ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
    );

    let hidden = visible.show_weekends(false);
    assert_eq!(
        rendered_week_headers(&hidden),
        ["MON", "TUE", "WED", "THU", "FRI"]
    );
}

#[test]
fn hidden_weekend_header_filters_identity_after_arbitrary_first_weekday() {
    let calendar = demo_calendar()
        .first_day_of_week(Weekday::Friday)
        .show_weekends(false);

    assert_eq!(
        rendered_month_header(&calendar),
        ["Fri", "Mon", "Tue", "Wed", "Thu"]
    );
}

#[test]
fn hiding_weekends_normalizes_month_and_week_cursor_to_friday() {
    for view in [CalendarView::Month, CalendarView::Week] {
        let mut calendar = demo_calendar()
            .view(view)
            .cursor(date(2026, Month::June, 28));

        calendar.set_show_weekends(false);

        assert_eq!(calendar.cursor_date(), date(2026, Month::June, 26));
    }
}

#[test]
fn keyboard_weekend_toggle_emits_cursor_then_changed_range() {
    let mut calendar =
        demo_calendar()
            .view(CalendarView::Month)
            .cursor(date(2026, Month::August, 1));

    calendar.on_key(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(calendar.cursor_date(), date(2026, Month::July, 31));
    assert_eq!(
        calendar.take_events(),
        vec![
            CalendarTypedEvent::CursorChanged {
                date: date(2026, Month::July, 31),
            },
            CalendarTypedEvent::RangeChanged {
                start: date(2026, Month::July, 1),
                end: date(2026, Month::July, 31),
            },
        ]
    );
}

#[test]
fn keyboard_weekend_toggle_emits_one_new_cursor_highlight() {
    let friday = date(2026, Month::June, 26);
    let saturday = date(2026, Month::June, 27);
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "friday",
                title: "Friday",
                span: CalendarSpan::all_day(friday),
            },
            DemoEntry {
                id: "saturday",
                title: "Saturday",
                span: CalendarSpan::all_day(saturday),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(saturday)
    .view(CalendarView::Month);

    calendar.on_key(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(
        calendar.take_events(),
        vec![
            CalendarTypedEvent::CursorChanged { date: friday },
            CalendarTypedEvent::EntryHighlighted {
                entry_id: Some("friday"),
            },
        ]
    );
}

#[test]
fn weekend_configuration_normalizes_without_interaction_events() {
    let mut calendar =
        demo_calendar()
            .view(CalendarView::Month)
            .cursor(date(2026, Month::August, 1));

    calendar.set_show_weekends(false);

    assert_eq!(calendar.cursor_date(), date(2026, Month::July, 31));
    assert!(calendar.take_events().is_empty());
}

#[test]
fn month_and_week_navigation_skip_hidden_weekends() {
    for view in [CalendarView::Month, CalendarView::Week] {
        let mut calendar = demo_calendar()
            .view(view)
            .cursor(date(2026, Month::June, 26))
            .show_weekends(false);

        calendar.on_key(Key::Right);
        assert_eq!(calendar.cursor_date(), date(2026, Month::June, 29));

        calendar.on_key(Key::Left);
        assert_eq!(calendar.cursor_date(), date(2026, Month::June, 26));
    }
}

#[test]
fn month_boundaries_include_weekends_when_visible() {
    let mut calendar =
        demo_calendar()
            .view(CalendarView::Month)
            .cursor(date(2026, Month::August, 12));

    calendar.on_key(Key::Home);
    assert_eq!(calendar.cursor_date(), date(2026, Month::August, 1));

    calendar.on_key(Key::End);
    assert_eq!(calendar.cursor_date(), date(2026, Month::August, 31));
}

#[test]
fn weekend_today_normalizes_when_hidden_in_multi_day_view() {
    let calendar = demo_calendar()
        .view(CalendarView::Week)
        .show_weekends(false)
        .today(date(2026, Month::June, 28));

    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 26));
}

#[test]
fn hiding_weekends_does_not_change_day_cursor_or_navigation() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Day)
        .cursor(date(2026, Month::June, 27));

    calendar.set_show_weekends(false);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 27));

    calendar.on_key(Key::Right);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 28));
}

#[test]
fn panel_legend_uses_default_view_binding_labels() {
    let border = rendered_top_border(&demo_calendar(), 100);

    assert!(border.contains(" Day |d| · Week |w| · Month |m| "));
}

#[test]
fn panel_legend_uses_custom_view_binding_labels() {
    let keys = CalendarKeyBindings {
        month_view: vec![KeySpec::plain('1')],
        week_view: vec![KeySpec::plain('2')],
        day_view: vec![KeySpec::plain('3')],
        ..CalendarKeyBindings::default()
    };
    let border = rendered_top_border(&demo_calendar().keybindings(keys), 100);

    assert!(border.contains(" Day |3| · Week |2| · Month |1| "));
}

#[test]
fn preferred_width_fits_week_title_and_exact_legend() {
    let calendar = demo_calendar().view(CalendarView::Week);
    let preferred = calendar
        .measure(LayoutProposal::unbounded())
        .preferred
        .width;
    let border = rendered_top_border(&calendar, preferred);

    assert_eq!(preferred, 72);
    assert!(
        border.contains(" Week • 2026-06-22 — 2026-06-28 "),
        "{border}"
    );
    assert!(border.contains(" Day |d| · Week |w| · Month |m| "));
}

#[test]
fn constrained_width_preserves_title_instead_of_overwriting_it() {
    let calendar = demo_calendar().view(CalendarView::Week);
    let border = rendered_top_border(&calendar, 40);

    assert!(border.contains(" Week • 2026-06-22 — 2026-06-28 "));
    assert!(!border.contains(" Day |d|"));
}

fn buffer_row(buffer: &Buffer, y: u16, width: u16) -> String {
    (0..width)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn month_event_summary_uses_one_line_and_visible_ellipsis() {
    let day = date(2026, Month::June, 22);
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "long",
            title: "abcdefghijk",
            span: CalendarSpan::all_day(day),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    );
    let mut terminal = Terminal::new(TestBackend::new(10, 3)).unwrap();

    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();

    assert_eq!(buffer_row(terminal.backend().buffer(), 2, 10), " ■ abcd...");
}

#[test]
fn week_event_summary_wraps_to_three_lines_then_shows_more_without_overlap() {
    let day = date(2026, Month::June, 22);
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "long",
                title: "one two three four five six seven eight",
                span: CalendarSpan::all_day(day),
            },
            DemoEntry {
                id: "next",
                title: "Next",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 10, 0),
                    datetime(2026, Month::June, 22, 11, 0),
                ),
            },
            DemoEntry {
                id: "later",
                title: "Later",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 12, 0),
                    datetime(2026, Month::June, 22, 13, 0),
                ),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    );
    let mut terminal = Terminal::new(TestBackend::new(14, 6)).unwrap();

    terminal
        .draw(|frame| calendar.render_week_column(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer_row(buffer, 2, 14), " ■ one two");
    assert_eq!(buffer_row(buffer, 3, 14), "   three four");
    assert!(buffer_row(buffer, 4, 14).ends_with("..."));
    assert_eq!(buffer_row(buffer, 5, 14), " +2 more");
}

#[test]
fn week_timed_summary_wraps_time_and_title_as_one_body() {
    let day = date(2026, Month::June, 22);
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "timed",
            title: "Change request needs careful review today",
            span: CalendarSpan::timed(
                day.with_time(Time::from_hms(9, 0, 0).expect("valid time")),
                day.with_time(Time::from_hms(10, 0, 0).expect("valid time")),
            ),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    );
    let content = |line: &Line<'static>| {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
            .trim_end()
            .to_string()
    };

    let capped = calendar.event_summary_lines(0, EventSummaryKind::Week, 16, 3);
    let capped = capped.iter().map(content).collect::<Vec<_>>();

    assert_eq!(capped[0], "• 09:00 Change");
    assert_eq!(capped[1], "  request needs");
    assert_eq!(capped[2], "  careful rev...");
    assert_eq!(capped.join("").matches("09:00").count(), 1);
    assert!(capped.iter().skip(1).all(|line| line.starts_with("  ")));
    assert!(
        capped
            .iter()
            .skip(1)
            .all(|line| !line.starts_with("        "))
    );

    let uncapped = calendar.event_summary_lines(0, EventSummaryKind::Week, 16, 4);
    let uncapped = uncapped.iter().map(content).collect::<Vec<_>>();
    assert_eq!(uncapped.len(), 4);
    assert!(!uncapped.iter().any(|line| line.contains("...")));
}

#[test]
fn day_event_summary_wraps_to_five_lines_and_short_text_stays_plain() {
    let day = date(2026, Month::June, 22);
    let long: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "long",
            title: "one two three four five six seven eight nine ten eleven twelve",
            span: CalendarSpan::all_day(day),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .view(CalendarView::Day)
    .cursor(day);
    let mut terminal = Terminal::new(TestBackend::new(18, 8)).unwrap();
    terminal
        .draw(|frame| long.render(frame, frame.area()))
        .unwrap();
    let long_rows = (1..6)
        .map(|y| buffer_row(terminal.backend().buffer(), y, 18))
        .collect::<Vec<_>>();

    assert_eq!(long_rows.len(), 5);
    assert!(long_rows[4].contains("..."), "{long_rows:?}");

    let short = demo_calendar().view(CalendarView::Day);
    terminal
        .draw(|frame| short.render(frame, frame.area()))
        .unwrap();
    let rendered = buffer_row(terminal.backend().buffer(), 1, 18);
    assert!(rendered.contains("Standup"), "{rendered}");
    assert!(!rendered.contains("..."), "{rendered}");
}

#[test]
fn event_body_wrapping_preserves_span_styles_and_whitespace() {
    let red = Style::default().fg(Color::Red);
    let blue = Style::default().fg(Color::Blue);
    let spans = vec![Span::styled("  ab", red), Span::styled("  cd", blue)];

    let lines = wrap_event_spans(&spans, 4, 3, Style::default());
    let content = lines
        .iter()
        .flat_map(|line| line.iter())
        .map(|span| span.content.as_ref())
        .collect::<String>();

    assert_eq!(content, "  ab  cd");
    assert!(lines[0].iter().all(|span| span.style == red));
    assert!(lines[1].iter().any(|span| span.style == blue));
}

#[test]
fn event_body_wrapping_retains_single_space_at_exact_boundary() {
    let spans = [Span::raw("ab cd")];

    let lines = wrap_event_spans(&spans, 4, 2, Style::default());
    let content = lines
        .iter()
        .map(|line| {
            line.iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    assert_eq!(content, ["ab ", "cd"]);
    assert_eq!(content.concat(), "ab cd");
}

#[test]
fn render_entry_preserves_styles_spaces_and_graphemes_when_wrapping() {
    let day = date(2026, Month::June, 22);
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "styled",
            title: "unused",
            span: CalendarSpan::all_day(day),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .render_entry(|_| {
        Line::from(vec![
            Span::styled("ab ", Style::default().fg(Color::Red)),
            Span::styled("cd🇺🇸e\u{301}", Style::default().fg(Color::Blue)),
        ])
    })
    .view(CalendarView::Day)
    .cursor(day);
    let mut terminal = Terminal::new(TestBackend::new(16, 5)).unwrap();

    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer.cell((11, 1)).unwrap().symbol(), "a");
    assert_eq!(buffer.cell((12, 1)).unwrap().symbol(), "b");
    assert_eq!(buffer.cell((13, 1)).unwrap().symbol(), " ");
    assert_eq!(buffer.cell((13, 1)).unwrap().fg, Color::Red);
    assert_eq!(buffer.cell((11, 2)).unwrap().symbol(), "c");
    assert_eq!(buffer.cell((12, 2)).unwrap().symbol(), "d");
    assert_eq!(buffer.cell((13, 2)).unwrap().symbol(), "🇺🇸");
    assert_eq!(buffer.cell((13, 2)).unwrap().fg, Color::Blue);
    assert_eq!(buffer.cell((11, 3)).unwrap().symbol(), "e\u{301}");
    assert_eq!(buffer.cell((11, 3)).unwrap().fg, Color::Blue);
}

#[test]
fn event_body_wrapping_keeps_graphemes_intact_at_wrap_and_ellipsis_boundaries() {
    let content = "A🇺🇸e\u{301}👩\u{200d}💻Z";
    let spans = [Span::raw(content)];

    let wrapped = wrap_event_spans(&spans, 2, 8, Style::default());
    let wrapped_spans = wrapped
        .iter()
        .flat_map(|line| line.iter())
        .collect::<Vec<_>>();
    assert_eq!(
        wrapped_spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        content
    );
    assert!(wrapped_spans.iter().any(|span| span.content == "🇺🇸"));
    assert!(wrapped_spans.iter().any(|span| span.content == "e\u{301}"));
    assert!(
        wrapped_spans
            .iter()
            .any(|span| span.content == "👩\u{200d}💻")
    );

    let ellipsized = wrap_event_spans(&spans, 4, 1, Style::default());
    let ellipsized_content = ellipsized[0]
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert_eq!(ellipsized_content, "A...");
}

#[test]
fn event_body_ellipsizes_only_when_content_overflows() {
    let exact = [Span::raw("four")];
    let overflow = [Span::raw("fives")];

    let exact = wrap_event_spans(&exact, 4, 1, Style::default());
    let overflow = wrap_event_spans(&overflow, 4, 1, Style::default());

    assert_eq!(exact[0][0].content, "f");
    assert!(!exact[0].iter().any(|span| span.content == "..."));
    assert_eq!(
        overflow[0]
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>(),
        "f..."
    );
}

#[test]
fn event_markers_default_by_span_and_callback_supports_per_event_unicode() {
    let day = date(2026, Month::June, 22);
    let entries = [
        DemoEntry {
            id: "all-day",
            title: "Holiday",
            span: CalendarSpan::all_day(day),
        },
        DemoEntry {
            id: "timed",
            title: "Call",
            span: CalendarSpan::timed(
                datetime(2026, Month::June, 22, 10, 0),
                datetime(2026, Month::June, 22, 11, 0),
            ),
        },
    ];
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        entries.clone(),
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    );
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer_row(buffer, 2, 20).starts_with(" ■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with(" • Call"));

    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        entries,
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .event_marker(|entry| if entry.id == "all-day" { '界' } else { '✓' });
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let all_day = buffer_row(buffer, 2, 20);
    let timed = buffer_row(buffer, 3, 20);
    assert!(
        all_day.contains('界') && all_day.contains("Holiday"),
        "{all_day}"
    );
    assert!(timed.contains("✓ Call"), "{timed}");

    calendar.set_event_marker(|entry| if entry.id == "all-day" { '◆' } else { '→' });
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer_row(buffer, 2, 20).contains("◆ Holiday"));
    assert!(buffer_row(buffer, 3, 20).contains("→ Call"));

    calendar.set_event_marker(|_| '\n');
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer_row(buffer, 2, 20).starts_with(" ■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with(" • Call"));

    calendar.set_event_marker(|_| '◆');
    calendar.clear_event_marker();
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer_row(buffer, 2, 20).starts_with(" ■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with(" • Call"));
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
