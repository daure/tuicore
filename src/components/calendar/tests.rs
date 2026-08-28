use super::*;
use crate::event::{Key, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::style::{Color, Modifier};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer};
use std::time::Duration as StdDuration;
use time::{Duration, Month, PrimitiveDateTime, Time};

#[derive(Clone)]
struct DemoEntry {
    id: &'static str,
    title: &'static str,
    span: CalendarSpan,
}

fn rendered_month_header(calendar: &Calendar<DemoEntry, &'static str>) -> Vec<String> {
    let area = Rect::new(0, 0, 100, 12);
    let inner = Panel::inner_area(area);
    let count = if calendar.is_showing_weekends() { 7 } else { 5 };
    let content_width = calendar_content_width(inner.width, count);
    let columns = calendar_columns(Rect::new(inner.x, inner.y, content_width, 1), count);
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    let buffer = terminal.backend().buffer();
    columns
        .into_iter()
        .enumerate()
        .map(|(index, column)| {
            let x = column.x + u16::from(index > 0);
            (x..x + 3)
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
    let area = Rect::new(0, 0, 100, 12);
    let inner = Panel::inner_area(area);
    let count = if calendar.is_showing_weekends() { 7 } else { 5 };
    let content_width = calendar_content_width(inner.width, count);
    let columns = calendar_columns(
        Rect::new(inner.x, inner.y, content_width, inner.height),
        count,
    );
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    let buffer = terminal.backend().buffer();
    columns
        .into_iter()
        .enumerate()
        .map(|(index, column)| {
            let x = column.x + u16::from(index > 0);
            (x..x + 3)
                .map(|x| buffer.cell((x, inner.y)).unwrap().symbol())
                .collect()
        })
        .collect()
}

fn rendered_row(calendar: &Calendar<DemoEntry, &'static str>, width: u16, y: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).expect("terminal should build");
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .expect("calendar should render");
    (0..width)
        .map(|x| terminal.backend().buffer().cell((x, y)).unwrap().symbol())
        .collect()
}

#[test]
fn drilldown_and_back_follow_stack() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Month)
        .event_detail_on_activate(true);

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
fn month_quick_jump_waits_for_a_second_digit_then_drills_to_week() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    assert_eq!(calendar.on_key(Key::Char('1')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 22));
    assert_eq!(calendar.current_view(), CalendarView::Month);

    assert_eq!(calendar.on_key(Key::Char('8')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 18));
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::DrillDown {
                from: CalendarView::Month,
                to: CalendarView::Week,
            })
    );

    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn month_quick_jump_waits_only_when_a_matching_two_digit_day_exists() {
    let mut june = demo_calendar().view(CalendarView::Month);
    assert_eq!(june.on_key(Key::Char('3')), CalendarOutcome::CHANGED);
    assert_eq!(june.cursor_date(), date(2026, Month::June, 22));
    assert_eq!(june.current_view(), CalendarView::Month);

    let february_date = date(2026, Month::February, 15);
    let mut february = demo_calendar()
        .today(february_date)
        .view(CalendarView::Month);
    assert_eq!(february.on_key(Key::Char('3')), CalendarOutcome::CHANGED);
    assert_eq!(february.cursor_date(), date(2026, Month::February, 3));
    assert_eq!(february.current_view(), CalendarView::Week);
}

#[test]
fn month_quick_jump_enter_and_space_accept_a_pending_single_digit() {
    for accept_key in [Key::Enter, Key::Char(' ')] {
        let mut calendar = demo_calendar().view(CalendarView::Month);
        calendar.on_key(Key::Char('1'));

        assert_eq!(calendar.on_key(accept_key), CalendarOutcome::CHANGED);
        assert_eq!(calendar.cursor_date(), date(2026, Month::June, 1));
        assert_eq!(calendar.current_view(), CalendarView::Week);
    }
}

#[test]
fn month_quick_jump_expires_after_one_second() {
    let mut calendar = demo_calendar().view(CalendarView::Month);
    calendar.on_key(Key::Char('1'));

    let tick = calendar.tick(StdDuration::from_millis(1_001), crate::animation_settings());
    assert!(tick.changed);

    calendar.on_key(Key::Char('8'));
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 8));
    assert_eq!(calendar.current_view(), CalendarView::Week);
}

#[test]
fn month_quick_jump_underlines_matching_days_in_current_month() {
    let mut calendar = demo_calendar().view(CalendarView::Month);
    calendar.on_key(Key::Char('1'));

    for day in 1..=19 {
        let mut terminal = Terminal::new(TestBackend::new(11, 3)).expect("terminal should build");
        terminal
            .draw(|frame| {
                calendar.render_month_cell(frame, frame.area(), date(2026, Month::June, day));
            })
            .expect("month cell should render");
        let underlined = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .filter(|cell| cell.modifier.contains(Modifier::UNDERLINED))
            .map(|cell| cell.symbol())
            .collect::<String>();
        let expected = if day == 1 || day >= 10 { "1" } else { "" };
        assert_eq!(underlined, expected, "day {day}");
    }

    let mut surrounding = Terminal::new(TestBackend::new(11, 3)).expect("terminal should build");
    surrounding
        .draw(|frame| {
            calendar.render_month_cell(frame, frame.area(), date(2026, Month::July, 1));
        })
        .expect("surrounding month cell should render");
    assert!(
        surrounding
            .backend()
            .buffer()
            .content()
            .iter()
            .all(|cell| !cell.modifier.contains(Modifier::UNDERLINED))
    );
}

#[test]
fn week_quick_jump_waits_for_a_second_digit_then_drills_to_day() {
    let mut calendar = demo_calendar().view(CalendarView::Week);

    assert_eq!(calendar.on_key(Key::Char('2')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 22));
    assert_eq!(calendar.current_view(), CalendarView::Week);

    assert_eq!(calendar.on_key(Key::Char('3')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
    assert_eq!(calendar.current_view(), CalendarView::Day);
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::DrillDown {
                from: CalendarView::Week,
                to: CalendarView::Day,
            })
    );
}

#[test]
fn week_quick_jump_enter_accepts_a_pending_single_digit() {
    let mut calendar = demo_calendar().view(CalendarView::Week);

    assert_eq!(calendar.on_key(Key::Char('1')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 1));
    assert_eq!(calendar.current_view(), CalendarView::Day);
}

#[test]
fn week_quick_jump_commits_a_digit_that_matches_one_day() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Week)
        .cursor(date(2026, Month::June, 1));

    assert_eq!(calendar.on_key(Key::Char('2')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 2));
    assert_eq!(calendar.current_view(), CalendarView::Day);
}

#[test]
fn week_quick_jump_underlines_matching_day_prefixes() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Week)
        .cursor(date(2026, Month::June, 21));
    calendar.on_key(Key::Char('2'));
    let mut terminal = Terminal::new(TestBackend::new(11, 3)).expect("terminal should build");

    terminal
        .draw(|frame| {
            calendar.render_week_column(frame, frame.area(), date(2026, Month::June, 21));
        })
        .expect("week column should render");

    let prefix = terminal.backend().buffer().cell((0, 1)).unwrap();
    assert_eq!(prefix.symbol(), "2");
    assert!(prefix.modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn clicking_a_month_cell_sets_the_date_and_drills_to_week() {
    let area = Rect::new(0, 0, 100, 20);
    let mut calendar = demo_calendar()
        .view(CalendarView::Month)
        .first_day_of_week(Weekday::Sunday);
    calendar.layout(area, &mut LayoutCtx::new());
    calendar.take_events();

    let inner = calendar.content_area(area);
    let visible_offsets = calendar.visible_weekday_offsets();
    let content_width = calendar_content_width(inner.width, visible_offsets.len());
    let (_, geometry) = view::calendar_scroll(inner, content_width);
    let content_area = Rect::new(0, 0, content_width, geometry.layout.viewport.height);
    let columns = calendar_columns(content_area, visible_offsets.len());
    let rows = view::calendar_month_rows(content_area);
    let target = date(2026, Month::June, 23);
    let start = week_range(first_of_month(target), Weekday::Sunday).0;
    let days_from_start = (target - start).whole_days() as usize;
    let source_column = columns[days_from_start % 7].x + 1;
    let source_row = rows[days_from_start / 7 + 1].y + 1;
    let mut ctx = EventCtx::default();

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: geometry.layout.viewport.x + source_column,
                row: geometry.layout.viewport.y,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Ignored
    );
    assert_eq!(calendar.current_view(), CalendarView::Month);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 22));

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: geometry.layout.viewport.x + source_column,
                row: geometry.layout.viewport.y + source_row,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Handled
    );
    assert_eq!(calendar.cursor_date(), target);
    assert_eq!(calendar.current_view(), CalendarView::Week);
    let events = calendar.take_events();
    assert!(events.contains(&CalendarTypedEvent::CursorChanged { date: target }));
    assert!(events.contains(&CalendarTypedEvent::DrillDown {
        from: CalendarView::Month,
        to: CalendarView::Week,
    }));
}

#[test]
fn clicking_a_scrolled_week_cell_sets_the_date_and_drills_to_day() {
    let area = Rect::new(0, 0, 24, 12);
    let mut calendar = demo_calendar()
        .view(CalendarView::Week)
        .cursor(date(2026, Month::June, 26));
    calendar.layout(area, &mut LayoutCtx::new());
    calendar.take_events();

    let inner = calendar.content_area(area);
    let visible_offsets = calendar.visible_weekday_offsets();
    let content_width = calendar_content_width(inner.width, visible_offsets.len());
    let (_, geometry) = view::calendar_scroll(inner, content_width);
    let content_area = Rect::new(0, 0, content_width, geometry.layout.viewport.height);
    let columns = calendar_columns(content_area, visible_offsets.len());
    let horizontal_offset = calendar.horizontal_offset(&columns, geometry.layout.viewport.width);
    assert!(horizontal_offset > 0);
    let target = date(2026, Month::June, 25);
    let source_column = columns[3].x + 1;
    let mut ctx = EventCtx::default();

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: geometry.layout.viewport.x + source_column - horizontal_offset,
                row: geometry.layout.viewport.y,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Ignored
    );
    assert_eq!(calendar.current_view(), CalendarView::Week);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 26));

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: geometry.layout.viewport.x + source_column - horizontal_offset,
                row: geometry.layout.viewport.y + 1,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Handled
    );
    assert_eq!(calendar.cursor_date(), target);
    assert_eq!(calendar.current_view(), CalendarView::Day);
    let events = calendar.take_events();
    assert!(events.contains(&CalendarTypedEvent::CursorChanged { date: target }));
    assert!(events.contains(&CalendarTypedEvent::DrillDown {
        from: CalendarView::Week,
        to: CalendarView::Day,
    }));
}

#[test]
fn narrow_month_and_week_views_scroll_horizontally_to_the_cursor() {
    let week = demo_calendar()
        .view(CalendarView::Week)
        .cursor(date(2026, Month::June, 26));
    let month = demo_calendar()
        .view(CalendarView::Month)
        .cursor(date(2026, Month::June, 26));

    let week_header = rendered_row(&week, 24, 1);
    let month_header = rendered_row(&month, 24, 1);

    assert!(week_header.contains("Fri"), "{week_header}");
    assert!(!week_header.contains("Mon"), "{week_header}");
    assert!(month_header.contains("Fri"), "{month_header}");
    assert!(!month_header.contains("Mon"), "{month_header}");

    let scrollbar = rendered_row(&week, 24, 10);
    assert!(scrollbar.contains('━'), "{scrollbar}");
    assert!(scrollbar.contains('─'), "{scrollbar}");
}

#[test]
fn month_and_week_cells_have_eleven_character_minimum_width() {
    assert_eq!(calendar_content_width(0, 7), 77);
    assert_eq!(calendar_content_width(120, 7), 120);
}

#[test]
fn first_month_and_week_cells_have_no_left_padding() {
    let month = demo_calendar()
        .view(CalendarView::Month)
        .cursor(date(2026, Month::June, 1));
    let week = demo_calendar()
        .view(CalendarView::Week)
        .cursor(date(2026, Month::June, 1));

    let month_date_row = rendered_row(&month, 100, 3);
    let week_header = rendered_row(&week, 100, 1);

    assert_eq!(month_date_row.chars().nth(1), Some('1'));
    assert_eq!(
        week_header.chars().skip(1).take(3).collect::<String>(),
        "Mon"
    );
}

#[test]
fn month_column_dividers_extend_through_weekday_header() {
    let calendar = demo_calendar().view(CalendarView::Month);
    let area = Rect::new(0, 0, 100, 12);
    let inner = Panel::inner_area(area);
    let content_width = calendar_content_width(inner.width, 7);
    let columns = calendar_columns(Rect::new(0, 0, content_width, inner.height), 7);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();

    for column in columns.iter().skip(1) {
        assert_eq!(
            terminal
                .backend()
                .buffer()
                .cell((inner.x + column.x, inner.y))
                .unwrap()
                .symbol(),
            "│"
        );
    }
}

#[test]
fn direct_view_switch_does_not_push_history() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    calendar.on_key(Key::Enter);
    calendar.on_key(Key::Enter);
    assert_eq!(calendar.current_view(), CalendarView::Day);

    assert_eq!(calendar.on_key(Key::Char('M')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.current_view(), CalendarView::Month);
    assert_eq!(calendar.on_key(Key::Esc), CalendarOutcome::IDLE);
    assert_eq!(calendar.current_view(), CalendarView::Month);
}

#[test]
fn default_view_switches_use_uppercase_shortcuts() {
    let mut calendar = demo_calendar().view(CalendarView::Week);

    for key in ['m', 'w', 'd'] {
        assert_eq!(calendar.on_key(Key::Char(key)), CalendarOutcome::IDLE);
        assert_eq!(calendar.current_view(), CalendarView::Week);
    }

    for (key, view) in [
        ('M', CalendarView::Month),
        ('W', CalendarView::Week),
        ('D', CalendarView::Day),
    ] {
        assert_eq!(calendar.on_key(Key::Char(key)), CalendarOutcome::CHANGED);
        assert_eq!(calendar.current_view(), view);
    }
}

#[test]
fn default_today_binding_uses_uppercase_shortcut() {
    let today = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(today)
        .cursor(today + Duration::days(1));

    assert_eq!(calendar.on_key(Key::Char('t')), CalendarOutcome::IDLE);
    assert_eq!(calendar.cursor_date(), today + Duration::days(1));
    assert_eq!(calendar.on_key(Key::Char('T')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), today);
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
fn day_wheel_navigation_moves_between_months_and_emits_range_changes() {
    let mut calendar = demo_calendar().view(CalendarView::Day);
    let mut ctx = EventCtx::default();

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Handled
    );
    assert_eq!(calendar.current_view(), CalendarView::Day);
    assert_eq!(calendar.cursor_date(), date(2026, Month::May, 22));
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::RangeChanged {
                start: date(2026, Month::May, 22),
                end: date(2026, Month::May, 22),
            })
    );

    assert_eq!(
        calendar.event(
            &TuiEvent::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            }),
            &mut ctx,
        ),
        EventOutcome::Handled
    );
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 22));
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
fn day_page_keys_page_the_entry_data_view_without_changing_date() {
    let day = date(2026, Month::June, 22);
    let entries = [
        ("one", "One", 8),
        ("two", "Two", 9),
        ("three", "Three", 10),
        ("four", "Four", 11),
        ("five", "Five", 12),
        ("six", "Six", 13),
    ]
    .map(|(id, title, hour)| DemoEntry {
        id,
        title,
        span: CalendarSpan::timed(
            PrimitiveDateTime::new(day, Time::from_hms(hour, 0, 0).unwrap()),
            PrimitiveDateTime::new(day, Time::from_hms(hour, 30, 0).unwrap()),
        ),
    });
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        entries,
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day);
    calendar.area = Rect::new(0, 0, 40, 6);

    assert_eq!(calendar.highlighted_entry_id(), Some("one"));
    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::CHANGED
    );
    assert_eq!(calendar.highlighted_entry_id(), Some("four"));
    assert_eq!(calendar.cursor_date(), day);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('u'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::CHANGED
    );
    assert_eq!(calendar.highlighted_entry_id(), Some("one"));
    assert_eq!(calendar.cursor_date(), day);

    calendar.set_keybindings(CalendarKeyBindings {
        page_up: vec![KeySpec::plain('z')],
        page_down: vec![KeySpec::plain('x')],
        ..CalendarKeyBindings::default()
    });
    assert_eq!(calendar.on_key(Key::Char('x')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.highlighted_entry_id(), Some("four"));
    assert_eq!(calendar.on_key(Key::Char('z')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.highlighted_entry_id(), Some("one"));
    assert_eq!(calendar.cursor_date(), day);
}

#[test]
fn selected_day_entry_highlight_fills_the_view_width() {
    let mut calendar = demo_calendar().view(CalendarView::Day);
    calendar.set_focused(true);
    let area = Rect::new(0, 0, 30, 6);
    let inner = Panel::inner_area(area);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    for x in inner.x..inner.right() {
        assert_eq!(
            buffer.cell((x, inner.y)).unwrap().bg,
            crate::theme().highlight_bg(),
            "highlight should fill cell at x={x}"
        );
    }
}

#[test]
fn selected_day_entry_remains_visually_selected_when_unfocused() {
    let mut calendar = demo_calendar().view(CalendarView::Day);
    calendar.set_focused(false);
    let area = Rect::new(0, 0, 30, 6);
    let inner = Panel::inner_area(area);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();

    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    for x in inner.x..inner.right() {
        assert_eq!(
            buffer.cell((x, inner.y)).unwrap().bg,
            crate::theme().selected_bg(),
            "selected style should fill cell at x={x}"
        );
    }
}

#[test]
fn month_selection_persists_through_blur_and_refocus() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(KeyEvent {
        code: Key::Char('m'),
        modifiers: KeyModifiers::SHIFT,
    });

    calendar.set_focused(false);
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    for y in 1..4 {
        assert_eq!(
            terminal.backend().buffer().cell((0, y)).unwrap().bg,
            crate::theme().selected_bg(),
            "month row {y} should retain selection"
        );
    }
    assert_eq!(
        terminal.backend().buffer().cell((0, 2)).unwrap().fg,
        crate::theme().selected_fg()
    );

    calendar.set_focused(true);
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().bg,
        crate::theme().highlight_bg()
    );
}

#[test]
fn week_selection_persists_through_blur_and_refocus() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::SHIFT,
    });

    calendar.set_focused(false);
    let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();
    terminal
        .draw(|frame| calendar.render_week_column(frame, frame.area(), day))
        .unwrap();
    for y in 1..4 {
        assert_eq!(
            terminal.backend().buffer().cell((0, y)).unwrap().bg,
            crate::theme().selected_bg(),
            "week row {y} should retain selection"
        );
    }
    assert_eq!(
        terminal.backend().buffer().cell((0, 2)).unwrap().fg,
        crate::theme().selected_fg()
    );

    calendar.set_focused(true);
    terminal
        .draw(|frame| calendar.render_week_column(frame, frame.area(), day))
        .unwrap();
    assert_eq!(
        terminal.backend().buffer().cell((0, 1)).unwrap().bg,
        crate::theme().highlight_bg()
    );
}

#[test]
fn day_selection_survives_focus_loss_and_gain() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });

    calendar.set_focused(false);
    calendar.set_focused(true);

    assert_eq!(
        calendar
            .day_selection
            .as_ref()
            .expect("day selection should remain active")
            .selected_entries,
        vec![0, 1]
    );
    assert!(calendar.day_entries.selection_overlay_active_for_test());
    assert!(!calendar.is_reordering());
}

#[test]
fn transient_selected_ids_returns_shift_range_in_display_order() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "third",
                title: "Third",
                span,
            },
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .entry_order(|left, right| left.title.cmp(right.title))
    .reorderable(|left, right| left.span.start == right.span.start);

    assert!(calendar.transient_selected_ids().is_empty());
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });

    assert_eq!(
        calendar.transient_selected_ids(),
        vec!["first", "second", "third"]
    );
    calendar.set_focused(false);
    assert_eq!(
        calendar.transient_selected_ids(),
        vec!["first", "second", "third"]
    );
}

#[test]
fn transient_selected_ids_returns_sparse_ctrl_selection_in_display_order() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "third",
                title: "Third",
                span,
            },
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .entry_order(|left, right| left.title.cmp(right.title))
    .reorderable(|left, right| left.span.start == right.span.start);

    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::CONTROL,
    });
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::CONTROL,
    });
    calendar.on_key(KeyEvent {
        code: Key::Char(' '),
        modifiers: KeyModifiers::CONTROL,
    });

    assert_eq!(calendar.transient_selected_ids(), vec!["first", "third"]);
}

#[test]
fn set_entries_preserves_transient_selection_across_unrelated_refresh() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
            DemoEntry {
                id: "unrelated",
                title: "Unrelated",
                span: CalendarSpan::all_day(day + Duration::days(1)),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });

    calendar.set_entries([
        DemoEntry {
            id: "unrelated",
            title: "Updated unrelated",
            span: CalendarSpan::all_day(day + Duration::days(1)),
        },
        DemoEntry {
            id: "first",
            title: "First",
            span,
        },
        DemoEntry {
            id: "second",
            title: "Second",
            span,
        },
    ]);

    assert_eq!(calendar.transient_selected_ids(), vec!["first", "second"]);
    assert_eq!(calendar.highlighted_entry_id(), Some("second"));
    assert!(calendar.day_entries.selection_overlay_active_for_test());
}

#[test]
fn set_entries_prunes_removed_transient_selection_and_repairs_anchor() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });

    calendar.set_entries([DemoEntry {
        id: "second",
        title: "Second",
        span,
    }]);

    assert_eq!(calendar.transient_selected_ids(), vec!["second"]);
    let selection = calendar
        .day_selection
        .as_ref()
        .expect("surviving selection should remain active");
    assert_eq!(calendar.highlighted_entry_id(), Some("second"));
    assert_eq!(calendar.entries[selection.anchor].id, "second");
    assert!(calendar.day_entries.selection_overlay_active_for_test());
}

#[test]
fn clearing_transient_selection_retains_highlight() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);

    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(calendar.highlighted_entry_id(), Some("second"));

    calendar.clear_transient_selection();

    assert!(calendar.transient_selected_ids().is_empty());
    assert_eq!(calendar.highlighted_entry_id(), Some("second"));
    assert!(!calendar.day_entries.selection_overlay_active_for_test());
}

#[test]
fn highlighting_entry_id_replaces_only_a_valid_highlight() {
    let mut calendar = demo_calendar().view(CalendarView::Day);

    assert_eq!(
        calendar.highlight_entry_id(&"planning"),
        CalendarOutcome::CHANGED
    );
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));

    assert_eq!(
        calendar.highlight_entry_id(&"missing"),
        CalendarOutcome::IDLE
    );
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));
}

#[test]
fn month_navigation_clears_day_selection() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(Key::Char('m'));
    calendar.on_key(Key::Right);

    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
    assert!(calendar.day_selection.is_none());
}

#[test]
fn week_navigation_clears_day_selection() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(Key::Char('w'));
    calendar.on_key(Key::Right);

    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
    assert!(calendar.day_selection.is_none());
}

#[test]
fn today_rollover_clears_day_selection_when_cursor_follows_today() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.set_focused(false);

    let tomorrow = day + Duration::days(1);
    calendar.set_today(tomorrow);

    assert_eq!(calendar.cursor_date(), tomorrow);
    assert!(calendar.day_selection.is_none());
    assert!(!calendar.day_entries.selection_overlay_active_for_test());
}

#[test]
fn selected_nonhighlighted_month_entry_uses_selected_style() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::all_day(day);
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
            DemoEntry {
                id: "third",
                title: "Third",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(Key::Char('m'));
    let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();

    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();

    let selected = terminal.backend().buffer().cell((0, 2)).unwrap();
    assert_eq!(selected.fg, crate::theme().selected_fg());
    assert_eq!(selected.bg, crate::theme().selected_bg());
    let highlighted = terminal.backend().buffer().cell((0, 3)).unwrap();
    assert_eq!(highlighted.fg, crate::theme().highlight_fg());
    assert_eq!(highlighted.bg, crate::theme().highlight_bg());
}

#[test]
fn selected_nonhighlighted_week_entry_uses_selected_style() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
            DemoEntry {
                id: "third",
                title: "Third",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);
    calendar.set_focused(true);
    calendar.on_key(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(Key::Char('w'));
    let mut terminal = Terminal::new(TestBackend::new(20, 5)).unwrap();

    terminal
        .draw(|frame| calendar.render_week_column(frame, frame.area(), day))
        .unwrap();

    let selected = terminal.backend().buffer().cell((3, 2)).unwrap();
    assert_eq!(selected.fg, crate::theme().selected_fg());
    assert_eq!(selected.bg, crate::theme().selected_bg());
    let highlighted = terminal.backend().buffer().cell((3, 3)).unwrap();
    assert_eq!(highlighted.fg, crate::theme().highlight_fg());
    assert_eq!(highlighted.bg, crate::theme().highlight_bg());
}

#[test]
fn day_reordering_uses_semantic_highlight_and_emits_scoped_order() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span,
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span,
            },
            DemoEntry {
                id: "later",
                title: "Later",
                span: CalendarSpan::timed(
                    datetime(2026, Month::June, 22, 10, 0),
                    datetime(2026, Month::June, 22, 10, 1),
                ),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);
    let area = Rect::new(0, 0, 30, 6);
    let inner = Panel::inner_area(area);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::HANDLED
    );
    assert!(calendar.is_reordering());
    let moving_entry = calendar
        .highlighted_entry
        .expect("first entry is highlighted");
    assert!(
        calendar
            .day_entries
            .row_has_reorder_highlight(&moving_entry)
    );

    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();
    assert_eq!(
        terminal
            .backend()
            .buffer()
            .cell((inner.x, inner.y))
            .unwrap()
            .bg,
        crate::theme().highlight_bg()
    );

    calendar.on_key(Key::Down);
    calendar.on_key(Key::Enter);

    assert!(!calendar.is_reordering());
    assert_eq!(calendar.day_entries.rows()[1].entry_index, moving_entry);
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::EntriesReordered {
                entry_ids: vec!["second", "first"],
            })
    );

    calendar.set_entries([
        DemoEntry {
            id: "second",
            title: "Second",
            span,
        },
        DemoEntry {
            id: "first",
            title: "First",
            span,
        },
        DemoEntry {
            id: "later",
            title: "Later",
            span: CalendarSpan::timed(
                datetime(2026, Month::June, 22, 10, 0),
                datetime(2026, Month::June, 22, 10, 1),
            ),
        },
    ]);
    assert!(calendar.day_entries.rows().iter().all(|row| {
        !calendar
            .day_entries
            .row_has_reorder_highlight(&row.entry_index)
    }));
}

#[test]
fn day_reordering_moves_a_shift_selected_block() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "a",
                title: "A",
                span,
            },
            DemoEntry {
                id: "b",
                title: "B",
                span,
            },
            DemoEntry {
                id: "c",
                title: "C",
                span,
            },
            DemoEntry {
                id: "d",
                title: "D",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);

    calendar.on_key(Key::Down);
    calendar.on_key(KeyEvent {
        code: Key::Char('j'),
        modifiers: KeyModifiers::SHIFT,
    });
    calendar.on_key(KeyEvent {
        code: Key::Char('m'),
        modifiers: KeyModifiers::CONTROL,
    });
    assert!(calendar.day_entries.selected_ids().is_empty());
    assert!(calendar.day_entries.rows().iter().all(|row| {
        !calendar
            .day_entries
            .row_has_reorder_highlight(&row.entry_index)
    }));
    let source_rows = calendar
        .day_entries
        .rows()
        .iter()
        .map(|row| row.entry_index)
        .collect::<Vec<_>>();
    let area = Rect::new(0, 0, 30, 8);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    let position = |text: &str| rendered.find(text).expect("calendar text should render");
    assert!(
        position("09:00 B") < position("09:00 C")
            && position("09:00 C") < position("Moving 2 tasks")
            && position("Moving 2 tasks") < position("09:00 D"),
        "calendar render: {rendered:?}"
    );
    calendar.on_key(Key::Down);
    assert_eq!(
        calendar
            .day_entries
            .rows()
            .iter()
            .map(|row| row.entry_index)
            .collect::<Vec<_>>(),
        source_rows
    );
    calendar.on_key(Key::Enter);

    let events = calendar.take_events();
    assert!(
        events.contains(&CalendarTypedEvent::EntriesReordered {
            entry_ids: vec!["a", "d", "b", "c"],
        }),
        "calendar events: {events:?}"
    );
}

#[test]
fn day_reordering_moves_a_ctrl_selected_block() {
    let day = date(2026, Month::June, 22);
    let span = CalendarSpan::timed(
        datetime(2026, Month::June, 22, 9, 0),
        datetime(2026, Month::June, 22, 9, 1),
    );
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "1",
                title: "1",
                span,
            },
            DemoEntry {
                id: "2",
                title: "2",
                span,
            },
            DemoEntry {
                id: "3",
                title: "3",
                span,
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .view(CalendarView::Day)
    .reorderable(|left, right| left.span.start == right.span.start);

    for key in [
        KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        },
        KeyEvent {
            code: Key::Char('j'),
            modifiers: KeyModifiers::CONTROL,
        },
        KeyEvent {
            code: Key::Char(' '),
            modifiers: KeyModifiers::CONTROL,
        },
        KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::CONTROL,
        },
    ] {
        calendar.on_key(key);
    }

    assert!(calendar.day_entries.selected_ids().is_empty());
    assert!(calendar.day_entries.rows().iter().all(|row| {
        !calendar
            .day_entries
            .row_has_reorder_highlight(&row.entry_index)
    }));
    let area = Rect::new(0, 0, 30, 8);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();
    let rendered = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(
        rendered.find("09:00 3").unwrap() < rendered.find("Moving 2 tasks").unwrap(),
        "calendar render: {rendered:?}"
    );
    calendar.on_key(Key::Down);
    calendar.on_key(Key::Enter);

    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::EntriesReordered {
                entry_ids: vec!["2", "1", "3"],
            })
    );
}

#[test]
fn day_reordering_ignores_a_scope_with_one_entry() {
    let day = date(2026, Month::June, 22);
    let mut calendar = demo_calendar()
        .today(day)
        .view(CalendarView::Day)
        .reorderable(|left, right| left.span.start == right.span.start);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('m'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::HANDLED
    );
    assert!(!calendar.is_reordering());
    assert!(
        !calendar
            .take_events()
            .iter()
            .any(|event| matches!(event, CalendarTypedEvent::EntriesReordered { .. }))
    );
}

#[test]
fn event_detail_preserves_highlighted_entry() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Day)
        .event_detail_on_activate(true);

    calendar.on_key(Key::Down);
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));

    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::ACTIVATED);

    assert_eq!(calendar.current_view(), CalendarView::EventDetail);
    assert_eq!(calendar.highlighted_entry_id(), Some("planning"));
}

#[test]
fn day_entry_activation_stays_in_day_view_by_default() {
    let mut calendar = demo_calendar().view(CalendarView::Day);

    assert!(!calendar.is_event_detail_on_activate());
    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::ACTIVATED);

    assert_eq!(calendar.current_view(), CalendarView::Day);
    assert!(
        calendar
            .take_events()
            .contains(&CalendarTypedEvent::EntryActivated {
                entry_id: "standup"
            })
    );
}

#[test]
fn event_detail_on_activate_setter_enables_detail_drilldown() {
    let mut calendar = demo_calendar().view(CalendarView::Day);
    calendar.set_event_detail_on_activate(true);

    assert!(calendar.is_event_detail_on_activate());
    assert_eq!(calendar.on_key(Key::Enter), CalendarOutcome::ACTIVATED);
    assert_eq!(calendar.current_view(), CalendarView::EventDetail);
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
fn yank_copies_cursor_date_in_every_view_and_stops_propagation() {
    for view in [
        CalendarView::Month,
        CalendarView::Week,
        CalendarView::Day,
        CalendarView::EventDetail,
    ] {
        let mut calendar = demo_calendar().view(view);
        let mut ctx = EventCtx::default();

        let outcome = calendar.event(&TuiEvent::Yank, &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled, "{view:?}");
        assert_eq!(ctx.clipboard_request(), Some("2026-06-22"), "{view:?}");
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped, "{view:?}");
    }
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
fn gg_matches_home_in_every_view() {
    for view in [
        CalendarView::Month,
        CalendarView::Week,
        CalendarView::Day,
        CalendarView::EventDetail,
    ] {
        let mut expected = demo_calendar().view(view);
        let mut actual = demo_calendar().view(view);

        let expected_outcome = expected.on_key(Key::Home);
        assert_eq!(actual.on_key(Key::Char('g')), CalendarOutcome::HANDLED);
        let actual_outcome = actual.on_key(Key::Char('g'));

        assert_eq!(actual_outcome, expected_outcome);
        assert_eq!(actual.cursor_date(), expected.cursor_date());
        assert_eq!(
            actual.highlighted_entry_id(),
            expected.highlighted_entry_id()
        );
    }
}

#[test]
fn shift_g_matches_end_in_every_view() {
    for view in [
        CalendarView::Month,
        CalendarView::Week,
        CalendarView::Day,
        CalendarView::EventDetail,
    ] {
        let mut expected = demo_calendar().view(view);
        let mut actual = demo_calendar().view(view);

        let expected_outcome = expected.on_key(Key::End);
        let actual_outcome = actual.on_key(KeyEvent {
            code: Key::Char('G'),
            modifiers: KeyModifiers::SHIFT,
        });

        assert_eq!(actual_outcome, expected_outcome);
        assert_eq!(actual.cursor_date(), expected.cursor_date());
        assert_eq!(
            actual.highlighted_entry_id(),
            expected.highlighted_entry_id()
        );
    }
}

#[test]
fn calendar_g_aliases_require_exact_modifiers() {
    let date = date(2026, Month::June, 22);
    let mut calendar = demo_calendar().view(CalendarView::Month);

    assert_eq!(
        calendar.on_key(KeyEvent {
            code: Key::Char('g'),
            modifiers: KeyModifiers::CONTROL,
        }),
        CalendarOutcome::IDLE
    );
    assert_eq!(calendar.cursor_date(), date);
    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::HANDLED);
    assert_eq!(calendar.cursor_date(), date);
}

#[test]
fn non_g_after_prefix_clears_prefix_and_is_processed_normally() {
    let mut calendar = demo_calendar().view(CalendarView::Month);

    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::HANDLED);
    assert_eq!(calendar.on_key(Key::Right), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::HANDLED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 23));
}

#[test]
fn replacing_keybindings_requires_a_fresh_full_prefix() {
    let replacement = CalendarKeyBindings::new().with_top_prefix([KeySpec::plain('z')]);
    let mut builder_calendar = demo_calendar().view(CalendarView::Month);
    assert_eq!(
        builder_calendar.on_key(Key::Char('g')),
        CalendarOutcome::HANDLED
    );

    builder_calendar = builder_calendar.keybindings(replacement.clone());

    assert_eq!(
        builder_calendar.on_key(Key::Char('z')),
        CalendarOutcome::HANDLED
    );
    assert_eq!(
        builder_calendar.on_key(Key::Char('z')),
        CalendarOutcome::CHANGED
    );

    let mut setter_calendar = demo_calendar().view(CalendarView::Month);
    assert_eq!(
        setter_calendar.on_key(Key::Char('g')),
        CalendarOutcome::HANDLED
    );

    setter_calendar.set_keybindings(replacement);

    assert_eq!(
        setter_calendar.on_key(Key::Char('z')),
        CalendarOutcome::HANDLED
    );
    assert_eq!(
        setter_calendar.on_key(Key::Char('z')),
        CalendarOutcome::CHANGED
    );
}

#[test]
fn blur_and_refocus_require_a_fresh_full_prefix() {
    let mut calendar = demo_calendar().view(CalendarView::Month);
    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::HANDLED);

    calendar.set_focused(false);
    calendar.set_focused(true);

    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::HANDLED);
    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::CHANGED);
}

#[test]
fn calendar_alias_bindings_are_configurable_per_instance() {
    let keys = CalendarKeyBindings::new()
        .with_top_prefix([KeySpec::plain('z')])
        .with_bottom([KeySpec::plain('x')]);
    let mut calendar = demo_calendar().view(CalendarView::Month).keybindings(keys);

    assert_eq!(calendar.on_key(Key::Char('g')), CalendarOutcome::IDLE);
    assert_eq!(calendar.on_key(Key::Char('z')), CalendarOutcome::HANDLED);
    assert_eq!(calendar.on_key(Key::Char('z')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 1));
    assert_eq!(calendar.on_key(Key::Char('x')), CalendarOutcome::CHANGED);
    assert_eq!(calendar.cursor_date(), date(2026, Month::June, 30));
}

#[test]
fn calendar_aliases_preserve_hidden_weekend_boundaries() {
    let mut calendar = demo_calendar()
        .view(CalendarView::Month)
        .cursor(date(2026, Month::August, 12))
        .show_weekends(false);

    calendar.on_key(Key::Char('g'));
    calendar.on_key(Key::Char('g'));
    assert_eq!(calendar.cursor_date(), date(2026, Month::August, 3));

    calendar.on_key(KeyEvent {
        code: Key::Char('G'),
        modifiers: KeyModifiers::SHIFT,
    });
    assert_eq!(calendar.cursor_date(), date(2026, Month::August, 31));
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
fn ctrl_w_toggles_weekends_while_plain_w_is_idle() {
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

    assert_eq!(calendar.on_key(Key::Char('w')), CalendarOutcome::IDLE);
    assert_eq!(calendar.current_view(), CalendarView::Month);
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
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    );

    let hidden = visible.show_weekends(false);
    assert_eq!(
        rendered_week_headers(&hidden),
        ["Mon", "Tue", "Wed", "Thu", "Fri"]
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
fn day_view_title_includes_short_weekday() {
    let calendar = demo_calendar()
        .view(CalendarView::Day)
        .cursor(date(2026, Month::June, 22));

    let border = rendered_top_border(&calendar, 100);

    assert!(border.contains(" 2026-06-22 · Mon "), "{border}");
}

#[test]
fn panel_legend_uses_default_view_binding_labels() {
    let border = rendered_top_border(&demo_calendar(), 100);

    assert!(border.contains(" Day |D| · Week |W| · Month |M| "));
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
fn panel_legend_mutes_inactive_views_without_highlighting_the_active_view() {
    for (view, label) in [
        (CalendarView::Day, "Day"),
        (CalendarView::Week, "Week"),
        (CalendarView::Month, "Month"),
    ] {
        let mut calendar = demo_calendar().view(view);
        calendar.set_focused(true);
        let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();
        terminal
            .draw(|frame| calendar.render(frame, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let symbols = (0..100)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<Vec<_>>();
        let expected = label
            .chars()
            .map(|char| char.to_string())
            .collect::<Vec<_>>();
        let x = symbols
            .windows(expected.len())
            .position(|window| window == expected)
            .expect("active view label should render") as u16;
        let active_cell = buffer.cell((x, 0)).unwrap();
        let inactive_label = if label == "Day" { "Week" } else { "Day" };
        let inactive_expected = inactive_label
            .chars()
            .map(|char| char.to_string())
            .collect::<Vec<_>>();
        let inactive_x = symbols
            .windows(inactive_expected.len())
            .position(|window| window == inactive_expected)
            .expect("inactive view label should render") as u16;
        let inactive_cell = buffer.cell((inactive_x, 0)).unwrap();

        assert_eq!(active_cell.fg, crate::theme().accent_fg(), "{view:?}");
        assert_eq!(active_cell.bg, Color::Reset, "{view:?}");
        assert_eq!(inactive_cell.fg, crate::theme().muted_fg(), "{view:?}");
        assert_eq!(inactive_cell.bg, Color::Reset, "{view:?}");
    }
}

#[test]
fn panel_top_left_omits_view_names() {
    for (view, omitted) in [
        (CalendarView::Day, "Day •"),
        (CalendarView::Week, "Week •"),
        (CalendarView::Month, "Month •"),
        (CalendarView::EventDetail, "Detail"),
    ] {
        let border = rendered_top_border(&demo_calendar().view(view), 100);
        assert!(!border.contains(omitted), "{view:?}: {border}");
    }
}

#[test]
fn month_title_has_one_space_on_each_side() {
    let border = rendered_top_border(&demo_calendar().view(CalendarView::Month), 100);

    assert!(border.starts_with("╭─ June 2026 ─"), "{border}");
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
    assert!(border.contains(" 2026-06-22 — 2026-06-28 "), "{border}");
    assert!(!border.contains(" Week •"), "{border}");
    assert!(border.contains(" Day |D| · Week |W| · Month |M| "));
}

#[test]
fn constrained_width_preserves_title_instead_of_overwriting_it() {
    let calendar = demo_calendar().view(CalendarView::Week);
    let border = rendered_top_border(&calendar, 40);

    assert!(border.contains(" 2026-06-22 — 2026-06-28 "));
    assert!(!border.contains(" Week •"));
    assert!(!border.contains(" Day |D|"));
}

#[test]
fn borderless_calendar_removes_only_outer_border() {
    let calendar = demo_calendar().bordered(false);
    let mut terminal = Terminal::new(TestBackend::new(100, 12)).unwrap();

    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();

    let buffer = terminal.backend().buffer();
    assert!(!calendar.is_bordered());
    assert!(buffer_row(buffer, 0, 100).starts_with("June 2026"));
    assert!(buffer_row(buffer, 0, 100).contains("Day |D| · Week |W| · Month |M|"));
    assert!(buffer_row(buffer, 1, 100).starts_with("Mon"));
    assert_ne!(buffer.cell((0, 0)).unwrap().symbol(), "┌");
}

fn buffer_row(buffer: &Buffer, y: u16, width: u16) -> String {
    (0..width)
        .map(|x| buffer.cell((x, y)).unwrap().symbol())
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn month_event_summary_uses_up_to_two_lines() {
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
    let mut terminal = Terminal::new(TestBackend::new(10, 4)).unwrap();

    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();

    assert_eq!(buffer_row(terminal.backend().buffer(), 2, 10), "■ abcdefgh");
    assert_eq!(buffer_row(terminal.backend().buffer(), 3, 10), "  ijk");
}

#[test]
fn compact_summary_title_overrides_custom_entry_renderer() {
    let day = date(2026, Month::June, 22);
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "ID-1",
            title: "Full title",
            span: CalendarSpan::all_day(day),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .render_entry(|entry| Line::from(format!("{} {}", entry.id, entry.title)))
    .compact_summary_title(100, |entry| entry.title.to_string());
    calendar.layout(Rect::new(0, 0, 80, 12), &mut LayoutCtx::new());
    let mut terminal = Terminal::new(TestBackend::new(10, 4)).unwrap();

    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let rendered = (0..4)
        .map(|y| buffer_row(terminal.backend().buffer(), y, 10))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(rendered.contains("Full"), "{rendered}");
    assert!(!rendered.contains("ID-1"), "{rendered}");
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

    assert_eq!(buffer_row(buffer, 2, 14), "■ one two");
    assert_eq!(buffer_row(buffer, 3, 14), "  three four");
    assert!(buffer_row(buffer, 4, 14).ends_with("..."));
    assert_eq!(buffer_row(buffer, 5, 14), "+2 more");
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
fn day_data_view_renders_one_row_per_entry() {
    let day = date(2026, Month::June, 22);
    let calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "first",
                title: "First",
                span: CalendarSpan::all_day(day),
            },
            DemoEntry {
                id: "second",
                title: "Second",
                span: CalendarSpan::all_day(day),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .view(CalendarView::Day)
    .cursor(day);
    let mut terminal = Terminal::new(TestBackend::new(18, 8)).unwrap();
    terminal
        .draw(|frame| calendar.render(frame, frame.area()))
        .unwrap();
    let first = buffer_row(terminal.backend().buffer(), 1, 18);
    let second = buffer_row(terminal.backend().buffer(), 2, 18);

    assert!(first.contains("First"), "{first}");
    assert!(second.contains("Second"), "{second}");
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
fn day_data_view_preserves_render_entry_styles_and_graphemes() {
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
    assert_eq!(buffer.cell((14, 1)).unwrap().symbol(), "c");
    assert_eq!(buffer.cell((14, 1)).unwrap().fg, Color::Blue);
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
    assert!(buffer_row(buffer, 2, 20).starts_with("■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with("• Call"));

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
    assert!(buffer_row(buffer, 2, 20).starts_with("■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with("• Call"));

    calendar.set_event_marker(|_| '◆');
    calendar.clear_event_marker();
    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();
    let buffer = terminal.backend().buffer();
    assert!(buffer_row(buffer, 2, 20).starts_with("■ Holiday"));
    assert!(buffer_row(buffer, 3, 20).starts_with("• Call"));
}

#[test]
fn focused_date_event_markers_use_selection_foreground() {
    let day = date(2026, Month::June, 22);
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [
            DemoEntry {
                id: "highlighted",
                title: "Highlighted",
                span: CalendarSpan::all_day(day),
            },
            DemoEntry {
                id: "other",
                title: "Other",
                span: CalendarSpan::all_day(day),
            },
        ],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day)
    .event_marker(|_| '◆');
    calendar.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();

    terminal
        .draw(|frame| calendar.render_month_cell(frame, frame.area(), day))
        .unwrap();

    for y in [2, 3] {
        let marker = terminal.backend().buffer().cell((0, y)).unwrap();
        assert_eq!(marker.symbol(), "◆");
        assert_eq!(marker.fg, crate::theme().highlight_fg());
        assert_eq!(marker.bg, crate::theme().highlight_bg());
    }
}

#[test]
fn focused_week_timed_entry_uses_selection_foreground_for_time() {
    let day = date(2026, Month::June, 22);
    let mut calendar: Calendar<DemoEntry, &'static str> = Calendar::new(
        [DemoEntry {
            id: "timed",
            title: "Call",
            span: CalendarSpan::timed(
                datetime(2026, Month::June, 22, 10, 0),
                datetime(2026, Month::June, 22, 11, 0),
            ),
        }],
        |entry| entry.id,
        |entry| entry.span,
        |entry| entry.title.to_string(),
    )
    .today(day);
    calendar.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(20, 4)).unwrap();

    terminal
        .draw(|frame| calendar.render_week_column(frame, frame.area(), day))
        .unwrap();

    for x in 3..8 {
        let time = terminal.backend().buffer().cell((x, 2)).unwrap();
        assert_eq!(time.fg, crate::theme().highlight_fg());
        assert_eq!(time.bg, crate::theme().highlight_bg());
    }
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
