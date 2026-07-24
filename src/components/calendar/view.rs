use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use time::{Date, Duration};

use crate::{line_width, theme};

use super::event_wrap::wrap_event_spans;
use super::*;

impl<T, Id, M> Calendar<T, Id, M>
where
    Id: Clone + Eq,
{
    pub(super) fn render_month(&self, frame: &mut Frame, area: Rect) {
        let title = format!(" Month • {} {} ", self.cursor.month(), self.cursor.year());
        self.render_panel(frame, area, title);
        let inner = Panel::inner_area(area);
        if inner.height < 2 {
            return;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
                Constraint::Ratio(1, 6),
            ])
            .split(inner);
        self.render_weekday_header(frame, rows[0]);
        self.render_month_grid_lines(frame, &rows[1..]);
        let start = week_range(first_of_month(self.cursor), self.first_day_of_week).0;
        let visible_offsets = self.visible_weekday_offsets();
        for week in 0..6 {
            let cols = calendar_columns(rows[week + 1], visible_offsets.len());
            for (column, day) in visible_offsets.iter().copied().enumerate() {
                let date = start + Duration::days((week * 7 + day) as i64);
                self.render_month_cell(frame, cols[column], date);
            }
        }
    }

    pub(super) fn render_month_cell(&self, frame: &mut Frame, area: Rect, date: Date) {
        if area.is_empty() {
            return;
        }
        let inner = grid_cell_inner(area, true);
        if inner.is_empty() {
            return;
        }
        let mut lines = vec![Line::from(Span::styled(
            format!("{:>2}", date.day()),
            self.date_style(date, date.month() != self.cursor.month()),
        ))];
        let event_capacity = usize::from(inner.height.saturating_sub(1));
        let entries = self.entries_on(date);
        self.append_event_lines(
            &mut lines,
            &entries,
            event_capacity,
            inner.width,
            MONTH_EVENT_LINES,
            EventSummaryKind::Month,
        );
        frame.render_widget(
            Paragraph::new(lines).style(self.date_cell_style(date)),
            inner,
        );
    }

    pub(super) fn render_week(&self, frame: &mut Frame, area: Rect) {
        let (start, end) = week_range(self.cursor, self.first_day_of_week);
        self.render_panel(frame, area, format!(" Week • {start} — {end} "));
        let inner = Panel::inner_area(area);
        if inner.height == 0 {
            return;
        }
        let visible_offsets = self.visible_weekday_offsets();
        let cols = calendar_columns(inner, visible_offsets.len());
        self.render_week_grid_lines(frame, &cols);
        for (column, offset) in visible_offsets.into_iter().enumerate() {
            let date = start + Duration::days(offset as i64);
            self.render_week_column(frame, cols[column], date);
        }
    }

    pub(super) fn render_week_column(&self, frame: &mut Frame, area: Rect, date: Date) {
        if area.is_empty() {
            return;
        }
        let inner = grid_cell_inner(area, false);
        if inner.is_empty() {
            return;
        }
        let mut lines = vec![
            Line::from(Span::styled(
                weekday_short(date).to_uppercase(),
                Style::default().fg(theme().muted_fg()),
            )),
            Line::from(Span::styled(
                format!("{}", date.day()),
                self.date_style(date, false),
            )),
        ];
        let event_capacity = usize::from(inner.height.saturating_sub(2));
        let entries = self.entries_on(date);
        self.append_event_lines(
            &mut lines,
            &entries,
            event_capacity,
            inner.width,
            WEEK_EVENT_LINES,
            EventSummaryKind::Week,
        );
        frame.render_widget(
            Paragraph::new(lines).style(self.date_cell_style(date)),
            inner,
        );
    }

    fn render_month_grid_lines(&self, frame: &mut Frame, rows: &[Rect]) {
        if rows.is_empty() {
            return;
        }
        let grid = rows[0].union(rows[rows.len() - 1]);
        let cols = calendar_columns(grid, self.visible_weekday_offsets().len());
        self.render_grid_vertical_lines(frame, &cols);
        let join_xs = cols.iter().skip(1).map(|col| col.x).collect::<Vec<_>>();
        for row in rows.iter().skip(1) {
            self.render_horizontal_line(frame, row.y, grid.x, grid.width, &join_xs);
        }
    }

    fn render_week_grid_lines(&self, frame: &mut Frame, cols: &[Rect]) {
        self.render_grid_vertical_lines(frame, cols);
    }

    fn render_grid_vertical_lines(&self, frame: &mut Frame, cols: &[Rect]) {
        let Some(first) = cols.first() else {
            return;
        };
        for col in cols.iter().skip(1) {
            self.render_vertical_line(frame, col.x, first.y, first.height);
        }
    }

    fn render_horizontal_line(
        &self,
        frame: &mut Frame,
        y: u16,
        x: u16,
        width: u16,
        join_xs: &[u16],
    ) {
        if width == 0 {
            return;
        }
        let line = (0..width)
            .map(|offset| {
                if join_xs.contains(&(x + offset)) {
                    '┼'
                } else {
                    '─'
                }
            })
            .collect::<String>();
        frame.render_widget(
            Paragraph::new(line).style(Style::default().fg(theme().border_fg())),
            Rect::new(x, y, width, 1),
        );
    }

    fn render_vertical_line(&self, frame: &mut Frame, x: u16, y: u16, height: u16) {
        for offset in 0..height {
            frame.render_widget(
                Paragraph::new("│").style(Style::default().fg(theme().border_fg())),
                Rect::new(x, y + offset, 1, 1),
            );
        }
    }

    pub(super) fn render_day(&self, frame: &mut Frame, area: Rect) {
        self.render_panel(frame, area, format!(" Day • {} ", self.cursor));
        let inner = Panel::inner_area(area);
        let entries = self.entries_on(self.cursor);
        let mut lines = Vec::new();
        self.append_event_lines(
            &mut lines,
            &entries,
            usize::from(inner.height),
            inner.width,
            DAY_EVENT_LINES,
            EventSummaryKind::Day,
        );
        let text = if lines.is_empty() {
            Text::from("No entries")
        } else {
            Text::from(lines)
        };
        frame.render_widget(Paragraph::new(text), inner);
    }

    pub(super) fn render_detail_view(&self, frame: &mut Frame, area: Rect) {
        self.render_panel(frame, area, String::from(" Detail "));
        let inner = Panel::inner_area(area);
        let Some(index) = self.highlighted_entry else {
            frame.render_widget(Paragraph::new("No entry selected"), inner);
            return;
        };
        frame.render_widget(
            Paragraph::new(self.detail_text(index)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn render_weekday_header(&self, frame: &mut Frame, area: Rect) {
        let labels = weekday_labels(self.first_day_of_week)
            .into_iter()
            .filter(|(_, weekday)| self.show_weekends || !is_weekend_weekday(*weekday))
            .map(|(label, _)| label)
            .collect::<Vec<_>>();
        let cols = calendar_columns(area, labels.len());
        for (index, label) in labels.into_iter().enumerate() {
            frame.render_widget(
                Paragraph::new(label).style(Style::default().fg(theme().muted_fg())),
                cols[index],
            );
        }
    }

    fn render_panel(&self, frame: &mut Frame, area: Rect, title: impl Into<String>) {
        let title = title.into();
        let keys = &self.keybindings;
        let legend = format!(
            " Day |{}| · Week |{}| · Month |{}| ",
            keys.day_view_label(),
            keys.week_view_label(),
            keys.month_view_label()
        );
        let title_width = line_width(&Line::from(title.as_str()));
        let legend_width = line_width(&Line::from(legend.as_str()));
        let mut panel = Panel::new().top_left(title).focused(self.focused);
        if title_width + legend_width + 4 <= usize::from(area.width.saturating_sub(4)) {
            panel = panel.top_right(legend);
        }
        panel.render(frame, area);
    }

    pub(super) fn visible_weekday_offsets(&self) -> Vec<usize> {
        (0..7)
            .filter(|offset| {
                self.show_weekends
                    || !is_weekend_weekday(weekday_after(self.first_day_of_week, *offset))
            })
            .collect()
    }

    fn date_style(&self, date: Date, muted: bool) -> Style {
        let t = theme();
        if self.focused && date == self.cursor {
            return Style::default()
                .fg(t.highlight_fg())
                .bg(t.highlight_bg())
                .add_modifier(Modifier::BOLD);
        }
        if date == self.today {
            return Style::default()
                .fg(t.accent_fg())
                .add_modifier(Modifier::BOLD);
        }
        if muted {
            Style::default().fg(t.subtle_fg())
        } else {
            Style::default().fg(t.text_fg())
        }
    }

    fn date_cell_style(&self, date: Date) -> Style {
        if self.focused && date == self.cursor {
            Style::default().bg(theme().highlight_bg())
        } else {
            Style::default()
        }
    }

    fn entry_style(&self, index: usize, selected: bool) -> Style {
        let t = theme();
        if selected && self.focused {
            return Style::default()
                .fg(t.highlight_fg())
                .bg(t.highlight_bg())
                .add_modifier(Modifier::BOLD);
        }
        match (self.role)(&self.entries[index]) {
            Some(CalendarEntryRole::Accent) => Style::default().fg(t.accent_fg()),
            Some(CalendarEntryRole::Success) => Style::default().fg(t.success_fg()),
            Some(CalendarEntryRole::Warning) => Style::default().fg(t.warning_fg()),
            Some(CalendarEntryRole::Error) => Style::default().fg(t.error_fg()),
            Some(CalendarEntryRole::Muted) => Style::default().fg(t.muted_fg()),
            None => Style::default().fg(t.text_fg()),
        }
    }

    fn append_event_lines(
        &self,
        lines: &mut Vec<Line<'static>>,
        entries: &[usize],
        capacity: usize,
        width: u16,
        per_event_cap: usize,
        kind: EventSummaryKind,
    ) {
        let mut used = 0;
        let mut visible_events = 0;
        for (position, index) in entries.iter().copied().enumerate() {
            let remaining = capacity.saturating_sub(used);
            let more_entries_follow = position + 1 < entries.len();
            let event_capacity = remaining.saturating_sub(usize::from(more_entries_follow));
            if event_capacity == 0 {
                break;
            }
            let event_lines =
                self.event_summary_lines(index, kind, width, per_event_cap.min(event_capacity));
            if event_lines.is_empty() {
                break;
            }
            used += event_lines.len();
            visible_events += 1;
            lines.extend(event_lines);
        }
        if visible_events < entries.len() && used < capacity {
            lines.push(Line::from(Span::styled(
                format!("+{} more", entries.len() - visible_events),
                Style::default().fg(theme().muted_fg()),
            )));
        }
    }

    pub(super) fn event_summary_lines(
        &self,
        index: usize,
        kind: EventSummaryKind,
        width: u16,
        max_lines: usize,
    ) -> Vec<Line<'static>> {
        let span = (self.span)(&self.entries[index]);
        let marker = self
            .event_marker
            .as_ref()
            .map(|marker| marker(&self.entries[index]))
            .filter(|marker| !marker.is_control())
            .unwrap_or(if span.all_day { '■' } else { '•' });
        let week_timed = matches!(kind, EventSummaryKind::Week) && !span.all_day;
        let prefix = match kind {
            EventSummaryKind::Month => format!("{marker} "),
            EventSummaryKind::Week if span.all_day => format!("{marker} "),
            EventSummaryKind::Week => format!("{marker} "),
            EventSummaryKind::Day if span.all_day => format!("{marker} all-day "),
            EventSummaryKind::Day => format!("{marker} {} ", format_time(span.start.time())),
        };
        let entry = self.entry_line(index);
        let line_style = self.entry_summary_style(index, entry.style);
        let mut body_spans = Vec::new();
        if week_timed {
            body_spans.push(Span::styled(
                format!("{} ", format_time(span.start.time())),
                Style::default().fg(theme().accent_fg()),
            ));
        }
        body_spans.extend(entry.spans);
        let prefix_width = line_width(&Line::from(prefix.as_str()));
        let body_width = width.saturating_sub(prefix_width.min(u16::MAX as usize) as u16);
        if body_width == 0 {
            return (max_lines > 0)
                .then(|| {
                    Line::from(Span::styled(
                        prefix,
                        Style::default().fg(theme().accent_fg()),
                    ))
                    .style(line_style)
                })
                .into_iter()
                .collect();
        }
        wrap_event_spans(&body_spans, body_width as usize, max_lines, line_style)
            .into_iter()
            .enumerate()
            .map(|(line_index, body_spans)| {
                let mut spans = vec![Span::styled(
                    if line_index == 0 {
                        prefix.clone()
                    } else {
                        " ".repeat(prefix_width)
                    },
                    Style::default().fg(theme().accent_fg()),
                )];
                spans.extend(body_spans);
                Line::from(spans).style(line_style)
            })
            .collect()
    }

    fn entry_summary_style(&self, index: usize, line_style: Style) -> Style {
        line_style.patch(self.entry_style(index, self.highlighted_entry == Some(index)))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum EventSummaryKind {
    Month,
    Week,
    Day,
}
