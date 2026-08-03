use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use ratatui::{Frame, buffer::Buffer};
use time::{Date, Duration};

use crate::{line_width, theme};

use super::event_wrap::wrap_event_spans;
use super::*;

impl<T, Id, M> Calendar<T, Id, M>
where
    Id: Clone + Eq,
{
    pub(super) fn render_month(&self, frame: &mut Frame, area: Rect) {
        let title = format!("{} {}", self.cursor.month(), self.cursor.year());
        self.render_panel(frame, area, title);
        let inner = self.content_area(area);
        if inner.height < 2 {
            return;
        }
        let visible_offsets = self.visible_weekday_offsets();
        let content_width = calendar_content_width(inner.width, visible_offsets.len());
        let (mut scroll, geometry) = calendar_scroll(inner, content_width);
        let content_area = Rect::new(0, 0, content_width, geometry.layout.viewport.height);
        let mut buffer = Buffer::empty(content_area);
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
            .split(content_area);
        self.render_weekday_header(&mut buffer, rows[0]);
        self.render_month_grid_lines(&mut buffer, &rows);
        let start = week_range(first_of_month(self.cursor), self.first_day_of_week).0;
        for week in 0..6 {
            let cols = calendar_columns(rows[week + 1], visible_offsets.len());
            for (column, day) in visible_offsets.iter().copied().enumerate() {
                let date = start + Duration::days((week * 7 + day) as i64);
                self.render_month_cell_into(&mut buffer, cols[column], date, column > 0);
            }
        }
        let cols = calendar_columns(content_area, visible_offsets.len());
        let horizontal_offset = self.horizontal_offset(&cols, geometry.layout.viewport.width);
        scroll.scroll_to(
            ScrollOffset::new(horizontal_offset.into(), 0),
            geometry.viewport,
            geometry.content,
            disabled_animation_settings(),
        );
        blit_horizontal_viewport(
            frame,
            &buffer,
            geometry.layout.viewport,
            scroll.offset().x.min(u16::MAX as usize) as u16,
        );
        scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    #[cfg(test)]
    pub(super) fn render_month_cell(&self, frame: &mut Frame, area: Rect, date: Date) {
        self.render_month_cell_into(frame.buffer_mut(), area, date, false);
    }

    fn render_month_cell_into(
        &self,
        buffer: &mut Buffer,
        area: Rect,
        date: Date,
        reserve_left_line: bool,
    ) {
        if area.is_empty() {
            return;
        }
        let inner = grid_cell_inner(area, true, reserve_left_line);
        if inner.is_empty() {
            return;
        }
        let mut lines = vec![self.month_day_line(date)];
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
        Paragraph::new(lines)
            .style(self.date_cell_style(date))
            .render(inner, buffer);
    }

    pub(super) fn render_week(&self, frame: &mut Frame, area: Rect) {
        let (start, end) = week_range(self.cursor, self.first_day_of_week);
        self.render_panel(frame, area, format!("{start} — {end}"));
        let inner = self.content_area(area);
        if inner.height == 0 {
            return;
        }
        let visible_offsets = self.visible_weekday_offsets();
        let content_width = calendar_content_width(inner.width, visible_offsets.len());
        let (mut scroll, geometry) = calendar_scroll(inner, content_width);
        let content_area = Rect::new(0, 0, content_width, geometry.layout.viewport.height);
        let mut buffer = Buffer::empty(content_area);
        let cols = calendar_columns(content_area, visible_offsets.len());
        self.render_week_grid_lines(&mut buffer, &cols);
        for (column, offset) in visible_offsets.into_iter().enumerate() {
            let date = start + Duration::days(offset as i64);
            self.render_week_column_into(&mut buffer, cols[column], date, column > 0);
        }
        let horizontal_offset = self.horizontal_offset(&cols, geometry.layout.viewport.width);
        scroll.scroll_to(
            ScrollOffset::new(horizontal_offset.into(), 0),
            geometry.viewport,
            geometry.content,
            disabled_animation_settings(),
        );
        blit_horizontal_viewport(
            frame,
            &buffer,
            geometry.layout.viewport,
            scroll.offset().x.min(u16::MAX as usize) as u16,
        );
        scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    #[cfg(test)]
    pub(super) fn render_week_column(&self, frame: &mut Frame, area: Rect, date: Date) {
        self.render_week_column_into(frame.buffer_mut(), area, date, false);
    }

    fn render_week_column_into(
        &self,
        buffer: &mut Buffer,
        area: Rect,
        date: Date,
        reserve_left_line: bool,
    ) {
        if area.is_empty() {
            return;
        }
        let inner = grid_cell_inner(area, false, reserve_left_line);
        if inner.is_empty() {
            return;
        }
        let mut lines = vec![
            Line::from(Span::styled(
                weekday_short(date),
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
        Paragraph::new(lines)
            .style(self.date_cell_style(date))
            .render(inner, buffer);
    }

    fn render_month_grid_lines(&self, buffer: &mut Buffer, rows: &[Rect]) {
        if rows.len() < 2 {
            return;
        }
        let grid = rows[0].union(rows[rows.len() - 1]);
        let cols = calendar_columns(grid, self.visible_weekday_offsets().len());
        self.render_grid_vertical_lines(buffer, &cols);
        let join_xs = cols.iter().skip(1).map(|col| col.x).collect::<Vec<_>>();
        for row in rows.iter().skip(2) {
            self.render_horizontal_line(buffer, row.y, grid.x, grid.width, &join_xs);
        }
    }

    fn render_week_grid_lines(&self, buffer: &mut Buffer, cols: &[Rect]) {
        self.render_grid_vertical_lines(buffer, cols);
    }

    fn render_grid_vertical_lines(&self, buffer: &mut Buffer, cols: &[Rect]) {
        let Some(first) = cols.first() else {
            return;
        };
        for col in cols.iter().skip(1) {
            self.render_vertical_line(buffer, col.x, first.y, first.height);
        }
    }

    fn render_horizontal_line(
        &self,
        buffer: &mut Buffer,
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
        Paragraph::new(line)
            .style(Style::default().fg(theme().border_fg()))
            .render(Rect::new(x, y, width, 1), buffer);
    }

    fn render_vertical_line(&self, buffer: &mut Buffer, x: u16, y: u16, height: u16) {
        for offset in 0..height {
            Paragraph::new("│")
                .style(Style::default().fg(theme().border_fg()))
                .render(Rect::new(x, y + offset, 1, 1), buffer);
        }
    }

    pub(super) fn render_day(&self, frame: &mut Frame, area: Rect) {
        self.render_panel(
            frame,
            area,
            format!("{} · {}", self.cursor, weekday_short(self.cursor)),
        );
        let inner = self.content_area(area);
        self.day_entries.render(frame, inner);
    }

    pub(super) fn render_detail_view(&self, frame: &mut Frame, area: Rect) {
        self.render_panel(frame, area, String::new());
        let inner = self.content_area(area);
        let Some(index) = self.highlighted_entry else {
            frame.render_widget(Paragraph::new("No entry selected"), inner);
            return;
        };
        frame.render_widget(
            Paragraph::new(self.detail_text(index)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn render_weekday_header(&self, buffer: &mut Buffer, area: Rect) {
        let labels = weekday_labels(self.first_day_of_week)
            .into_iter()
            .filter(|(_, weekday)| self.show_weekends || !is_weekend_weekday(*weekday))
            .map(|(label, _)| label)
            .collect::<Vec<_>>();
        let cols = calendar_columns(area, labels.len());
        for (index, label) in labels.into_iter().enumerate() {
            Paragraph::new(label)
                .style(Style::default().fg(theme().muted_fg()))
                .render(grid_cell_inner(cols[index], false, index > 0), buffer);
        }
    }

    fn horizontal_offset(&self, cols: &[Rect], viewport_width: u16) -> u16 {
        let visible_offsets = self.visible_weekday_offsets();
        let selected_column = visible_offsets
            .iter()
            .position(|offset| {
                weekday_after(self.first_day_of_week, *offset) == self.cursor.weekday()
            })
            .unwrap_or_default();
        let Some(selected) = cols.get(selected_column) else {
            return 0;
        };
        if selected.width >= viewport_width {
            selected.x
        } else {
            selected.right().saturating_sub(viewport_width)
        }
    }

    fn render_panel(&self, frame: &mut Frame, area: Rect, title: impl Into<String>) {
        let title = title.into();
        let keys = &self.keybindings;
        let inactive_style = Style::default().fg(theme().muted_fg());
        let mode_span = |label: String, view| {
            if self.view == view
                || self.view == CalendarView::EventDetail && view == CalendarView::Day
            {
                Span::raw(label)
            } else {
                Span::styled(label, inactive_style)
            }
        };
        let legend = Line::from(vec![
            mode_span(
                format!("Day |{}|", keys.day_view_label()),
                CalendarView::Day,
            ),
            Span::raw(" · "),
            mode_span(
                format!("Week |{}|", keys.week_view_label()),
                CalendarView::Week,
            ),
            Span::raw(" · "),
            mode_span(
                format!("Month |{}|", keys.month_view_label()),
                CalendarView::Month,
            ),
        ]);
        let title_width = line_width(&Line::from(title.as_str()));
        let legend_width = line_width(&legend);
        let show_legend = title_width + legend_width + 4
            <= usize::from(area.width.saturating_sub(u16::from(self.bordered) * 4));
        if self.bordered {
            let mut panel = Panel::new().focused(self.focused);
            if !title.is_empty() {
                panel = panel.top_left(title);
            }
            if show_legend {
                panel = panel.top_right_line(legend);
            }
            panel.render(frame, area);
            return;
        }

        let title_style = Style::default().fg(if self.focused {
            theme().accent_fg()
        } else {
            theme().muted_fg()
        });
        frame.render_widget(Paragraph::new(title).style(title_style), area);
        if show_legend {
            frame.render_widget(Paragraph::new(legend).alignment(Alignment::Right), area);
        }
    }

    pub(super) fn content_area(&self, area: Rect) -> Rect {
        if self.bordered {
            Panel::inner_area(area)
        } else {
            Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            )
        }
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

    fn month_day_line(&self, date: Date) -> Line<'static> {
        let style = self.date_style(date, date.month() != self.cursor.month());
        let label = date.day().to_string();
        if !self.month_quick_jump_matches(date) {
            return Line::from(Span::styled(label, style));
        }
        Line::from(vec![
            Span::styled(
                label[..1].to_owned(),
                style.add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled(label[1..].to_owned(), style),
        ])
    }

    fn month_quick_jump_matches(&self, date: Date) -> bool {
        let Some(digit) = self.quick_jump_digit else {
            return false;
        };
        date.year() == self.cursor.year()
            && date.month() == self.cursor.month()
            && (date.day() == digit || date.day() / 10 == digit)
    }

    fn date_cell_style(&self, date: Date) -> Style {
        if self.focused && date == self.cursor {
            Style::default().bg(theme().highlight_bg())
        } else {
            Style::default()
        }
    }

    fn entry_style(&self, index: usize, selected: bool) -> Style {
        calendar_entry_style((self.role)(&self.entries[index]), selected && self.focused)
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
        };
        let entry = self.entry_line(index);
        let line_style = self.entry_summary_style(index, entry.style);
        let on_highlight_background = self.highlighted_entry == Some(index)
            || matches!(kind, EventSummaryKind::Month | EventSummaryKind::Week)
                && span.covers_date(self.cursor);
        let marker_style = if self.focused && on_highlight_background {
            Style::default().fg(theme().highlight_fg())
        } else {
            Style::default().fg(theme().accent_fg())
        };
        let mut body_spans = Vec::new();
        if week_timed {
            body_spans.push(Span::styled(
                format!("{} ", format_time(span.start.time())),
                marker_style,
            ));
        }
        body_spans.extend(entry.spans);
        let prefix_width = line_width(&Line::from(prefix.as_str()));
        let body_width = width.saturating_sub(prefix_width.min(u16::MAX as usize) as u16);
        if body_width == 0 {
            return (max_lines > 0)
                .then(|| Line::from(Span::styled(prefix, marker_style)).style(line_style))
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
                    marker_style,
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

fn calendar_scroll(area: Rect, content_width: u16) -> (ScrollState, crate::ScrollGeometry) {
    let scroll = ScrollState::from_preset(ScrollAxes::Horizontal, preset().scroll());
    let content = ScrollSize::new(content_width.into(), area.height.into());
    let geometry = scroll.geometry(area, content);
    (scroll, geometry)
}

fn disabled_animation_settings() -> crate::AnimationSettings {
    let mut settings = animation_settings();
    settings.enabled = false;
    settings
}

#[derive(Debug, Clone, Copy)]
pub(super) enum EventSummaryKind {
    Month,
    Week,
}
