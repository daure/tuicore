use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use time::{Date, Duration, Month, Weekday};

use crate::border_set;
use crate::components::calendar::date_math::week_range;
use crate::event::{ExternalEditorResponse, Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, HotkeyEvent, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, TickResult, TuiNode, hotkey_badge_width, hotkey_edge_spans,
    hotkey_underline_style, keybindings, preset, theme,
};

use super::{
    DATE_PICKER_FOCUS, DATE_PICKER_WIDTH, PickerOutcome, add_months, centered_grid, choice_style,
    date_in_month, finish_event, first_of_month, last_of_month, month_abbr, parse_editor_date,
    picker_size_hint, plain_digit, request_unfocus_if_canceled, today, year_page_start,
};

const QUICK_JUMP_TIMEOUT: StdDuration = StdDuration::from_secs(1);

pub struct DatePicker<M = ()> {
    value: Option<Date>,
    cursor: Date,
    display_month: Date,
    view: DatePickerView,
    year_page_start: i32,
    today: Date,
    min: Option<Date>,
    max: Option<Date>,
    first_day_of_week: Weekday,
    focused: bool,
    hotkey: Option<String>,
    pending_hotkey_prefix: Option<String>,
    pending_top_prefix: bool,
    quick_jump_digit: Option<u8>,
    quick_jump_text: String,
    quick_jump_elapsed: StdDuration,
    quick_jump_selected: bool,
    on_select: Option<Box<dyn Fn(Date) -> M>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatePickerView {
    Day,
    Month,
    Year,
}

impl<M> DatePicker<M> {
    pub fn new() -> Self {
        let today = today();
        Self {
            value: None,
            cursor: today,
            display_month: first_of_month(today),
            view: DatePickerView::Day,
            year_page_start: year_page_start(today.year()),
            today,
            min: None,
            max: None,
            first_day_of_week: Weekday::Monday,
            focused: false,
            hotkey: None,
            pending_hotkey_prefix: None,
            pending_top_prefix: false,
            quick_jump_digit: None,
            quick_jump_text: String::new(),
            quick_jump_elapsed: StdDuration::ZERO,
            quick_jump_selected: false,
            on_select: None,
        }
    }

    pub fn value(mut self, value: Option<Date>) -> Self {
        self.set_value(value);
        self
    }

    pub fn today(mut self, today: Date) -> Self {
        self.today = today;
        if self.value.is_none() {
            self.cursor = self.clamp(today);
            self.display_month = first_of_month(self.cursor);
        }
        self
    }

    pub fn min(mut self, min: Date) -> Self {
        self.min = Some(min);
        self.value = self.value.map(|date| self.clamp(date));
        self.cursor = self.clamp(self.cursor);
        self.display_month = first_of_month(self.cursor);
        self
    }

    pub fn max(mut self, max: Date) -> Self {
        self.max = Some(max);
        self.value = self.value.map(|date| self.clamp(date));
        self.cursor = self.clamp(self.cursor);
        self.display_month = first_of_month(self.cursor);
        self
    }

    pub fn on_select(mut self, handler: impl Fn(Date) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn first_day_of_week(mut self, weekday: Weekday) -> Self {
        self.set_first_day_of_week(weekday);
        self
    }

    pub fn set_first_day_of_week(&mut self, weekday: Weekday) {
        self.first_day_of_week = weekday;
    }

    #[cfg(test)]
    pub(super) fn configured_first_day_of_week(&self) -> Weekday {
        self.first_day_of_week
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
        self.pending_hotkey_prefix = None;
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.pending_hotkey_prefix = None;
    }

    pub fn current_value(&self) -> Option<Date> {
        self.value
    }

    pub fn cursor(&self) -> Date {
        self.cursor
    }

    pub fn set_value(&mut self, value: Option<Date>) {
        self.value = value.map(|date| self.clamp(date));
        self.cursor = self.value.unwrap_or_else(|| self.clamp(self.today));
        self.display_month = first_of_month(self.cursor);
        self.year_page_start = year_page_start(self.cursor.year());
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.clear_quick_jump();
        }
    }

    #[cfg(test)]
    pub(super) fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> PickerOutcome {
        let key = key.into();
        self.quick_jump_selected = false;
        let bindings = keybindings();
        let date_keys = bindings.date_time_picker();
        if let Some(outcome) = self.handle_quick_jump(key) {
            return outcome;
        }
        if date_keys.top_prefix_matches(key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                return self.set_cursor(self.view_start_date());
            }
            self.pending_top_prefix = true;
            return PickerOutcome::handled(false);
        }
        self.pending_top_prefix = false;
        if date_keys.bottom_matches(key) {
            return self.set_cursor(self.view_end_date());
        }
        if date_keys.day_view_matches(key) {
            self.view = DatePickerView::Day;
            return PickerOutcome::handled(true);
        }
        if bindings.date_time_picker().month_view_matches(key) {
            self.view = DatePickerView::Month;
            return PickerOutcome::handled(true);
        }
        if bindings.date_time_picker().year_view_matches(key) {
            self.view = DatePickerView::Year;
            self.year_page_start = year_page_start(self.cursor.year());
            return PickerOutcome::handled(true);
        }
        if bindings.date_time_picker().today_matches(key) {
            self.view = DatePickerView::Day;
            return self.set_cursor(self.today);
        }
        if date_keys.line_left_matches(key) {
            return self.move_left();
        }
        if date_keys.line_right_matches(key) {
            return self.move_right();
        }
        if date_keys.line_up_matches(key) {
            return self.move_up();
        }
        if date_keys.line_down_matches(key) {
            return self.move_down();
        }
        if bindings.page_up_matches(key) {
            return self.page(-1);
        }
        if bindings.page_down_matches(key) {
            return self.page(1);
        }
        if bindings.home_matches(key) {
            return self.set_cursor(self.view_start_date());
        }
        if bindings.end_matches(key) {
            return self.set_cursor(self.view_end_date());
        }
        if key.code == Key::Enter && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.value = Some(self.cursor);
            return PickerOutcome::selected(true);
        }
        if bindings.button().press_matches(key) {
            if self.view == DatePickerView::Year {
                self.view = DatePickerView::Month;
                return PickerOutcome::handled(true);
            }
            if self.view == DatePickerView::Month {
                self.view = DatePickerView::Day;
                return PickerOutcome::handled(true);
            }
            self.value = Some(self.cursor);
            return PickerOutcome::selected(true);
        }
        if bindings.focus().unfocus_matches(key) {
            let old_cursor = self.cursor;
            let old_view = self.view;
            self.view = DatePickerView::Day;
            self.cursor = self.clamp(self.value.unwrap_or(self.today));
            self.display_month = first_of_month(self.cursor);
            return PickerOutcome::canceled(old_cursor != self.cursor || old_view != self.view);
        }
        PickerOutcome::IGNORED
    }

    pub(super) fn apply_external_editor_response(
        &mut self,
        response: &ExternalEditorResponse,
    ) -> PickerOutcome {
        let Some(date) = parse_editor_date(&response.value) else {
            return PickerOutcome::handled(false);
        };
        let outcome = self.set_cursor(date);
        self.value = Some(self.cursor);
        self.view = DatePickerView::Day;
        PickerOutcome::selected(outcome.changed || self.value != Some(date))
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        match self.view {
            DatePickerView::Day => self.render_day(frame, area),
            DatePickerView::Month => self.render_month_picker(frame, area),
            DatePickerView::Year => self.render_year_picker(frame, area),
        }
    }

    fn render_day(&self, frame: &mut Frame, area: Rect) {
        let styles = DateStyles {
            cursor: self.cursor,
            selected: self.value,
            today: self.today,
            focused: self.focused,
            min: self.min,
            max: self.max,
            display_month: self.display_month,
            quick_jump_digit: self.quick_jump_digit,
        };
        let block = self.block("");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.height > 0 {
            frame.render_widget(
                Paragraph::new(format!(
                    "{} {}",
                    self.display_month.month(),
                    self.display_month.year()
                ))
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(theme().accent_fg())
                        .add_modifier(Modifier::BOLD),
                ),
                Rect::new(inner.x, inner.y, inner.width, 1),
            );
        }
        if inner.height > 1 {
            for (column, label) in narrow_weekday_labels(self.first_day_of_week)
                .into_iter()
                .enumerate()
            {
                let x = inner.x + column as u16 * 3;
                if x >= inner.right() {
                    continue;
                }
                frame.render_widget(
                    Paragraph::new(label).style(Style::default().fg(theme().muted_fg())),
                    Rect::new(x, inner.y + 1, 3.min(inner.right() - x), 1),
                );
            }
        }
        let start = week_range(self.display_month, self.first_day_of_week).0;
        for offset in 0..42 {
            let Some(date) = start.checked_add(Duration::days(offset)) else {
                continue;
            };
            let row = offset / 7;
            if row as u16 + 2 >= inner.height {
                break;
            }
            let column = offset % 7;
            let x = inner.x + column as u16 * 3;
            if x >= inner.right() {
                continue;
            }
            frame.render_widget(
                Paragraph::new(styles.day_line(date, date.month() != self.display_month.month())),
                Rect::new(x, inner.y + row as u16 + 2, 3.min(inner.right() - x), 1),
            );
        }
        self.render_hotkey_label(frame, area);
    }

    fn render_month_picker(&self, frame: &mut Frame, area: Rect) {
        let block = self.block(format!(" {} ▴ ", self.cursor.year()));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.render_hotkey_label(frame, area);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(centered_grid(inner, 3, 20));
        for row in 0..3 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ])
                .split(rows[row]);
            for col in 0..4 {
                let month_number = row * 4 + col + 1;
                let month = Month::try_from(month_number as u8).expect("month in grid");
                let selected = month == self.cursor.month();
                let style = choice_style(selected, self.focused);
                frame.render_widget(
                    Paragraph::new(self.quick_jump_line(month_abbr(month), style)),
                    cols[col],
                );
            }
        }
    }

    fn render_year_picker(&self, frame: &mut Frame, area: Rect) {
        let block = self.block(format!(
            " {} — {} ▴ ",
            self.year_page_start,
            self.year_page_start.saturating_add(23)
        ));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        self.render_hotkey_label(frame, area);
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(centered_grid(inner, 6, 24));
        for row in 0..6 {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(6),
                ])
                .split(rows[row]);
            for col in 0..4 {
                let year = self.year_page_start + (row * 4 + col) as i32;
                let style = choice_style(year == self.cursor.year(), self.focused);
                frame.render_widget(
                    Paragraph::new(self.quick_jump_line(&year.to_string(), style)),
                    cols[col],
                );
            }
        }
    }

    fn block(&self, title: impl Into<String>) -> Block<'static> {
        let t = theme();
        Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(preset().border()))
            .title(title.into())
            .border_style(Style::default().fg(if self.focused {
                t.highlight_bg()
            } else {
                t.border_fg()
            }))
    }

    fn render_hotkey_label(&self, frame: &mut Frame, area: Rect) {
        let Some(hotkey) = self.hotkey.as_deref() else {
            return;
        };
        if area.width <= 4 || area.height < 2 {
            return;
        }
        let border = preset().border();
        let border_style = Style::default().fg(if self.focused {
            theme().highlight_bg()
        } else {
            theme().border_fg()
        });
        let hotkey_style = Style::default().fg(theme().text_fg());
        let line = Line::from(hotkey_edge_spans(
            hotkey,
            self.pending_hotkey_prefix.as_deref(),
            border,
            border_style,
            hotkey_style,
            hotkey_underline_style(hotkey_style),
        ));
        let width = hotkey_badge_width(hotkey).min(u16::MAX as usize) as u16;
        frame.render_widget(
            Paragraph::new(line),
            Rect::new(
                area.x + area.width.saturating_sub(width),
                area.bottom().saturating_sub(1),
                width,
                1,
            ),
        );
    }

    fn move_left(&mut self) -> PickerOutcome {
        match self.view {
            DatePickerView::Day => self.move_days(-1),
            DatePickerView::Month => self.move_months(-1),
            DatePickerView::Year => self.move_years(-1),
        }
    }

    fn move_right(&mut self) -> PickerOutcome {
        match self.view {
            DatePickerView::Day => self.move_days(1),
            DatePickerView::Month => self.move_months(1),
            DatePickerView::Year => self.move_years(1),
        }
    }

    fn move_up(&mut self) -> PickerOutcome {
        match self.view {
            DatePickerView::Day => self.move_days(-7),
            DatePickerView::Month => self.move_months(-4),
            DatePickerView::Year => self.move_years(-4),
        }
    }

    fn move_down(&mut self) -> PickerOutcome {
        match self.view {
            DatePickerView::Day => self.move_days(7),
            DatePickerView::Month => self.move_months(4),
            DatePickerView::Year => self.move_years(4),
        }
    }

    fn page(&mut self, delta: i32) -> PickerOutcome {
        match self.view {
            DatePickerView::Day => self.move_months(delta),
            DatePickerView::Month => self.move_years(delta),
            DatePickerView::Year => self.move_years(delta.saturating_mul(24)),
        }
    }

    fn view_start_date(&self) -> Date {
        match self.view {
            DatePickerView::Day => first_of_month(self.cursor),
            DatePickerView::Month => {
                date_in_month(self.cursor.year(), Month::January, self.cursor.day())
            }
            DatePickerView::Year => {
                date_in_month(self.year_page_start, self.cursor.month(), self.cursor.day())
            }
        }
    }

    fn view_end_date(&self) -> Date {
        match self.view {
            DatePickerView::Day => last_of_month(self.cursor),
            DatePickerView::Month => {
                date_in_month(self.cursor.year(), Month::December, self.cursor.day())
            }
            DatePickerView::Year => date_in_month(
                self.year_page_start.saturating_add(23),
                self.cursor.month(),
                self.cursor.day(),
            ),
        }
    }

    fn move_days(&mut self, days: i64) -> PickerOutcome {
        let next = self
            .cursor
            .checked_add(Duration::days(days))
            .unwrap_or_else(|| {
                if days.is_negative() {
                    Date::MIN
                } else {
                    Date::MAX
                }
            });
        self.set_cursor(next)
    }

    fn move_months(&mut self, months: i32) -> PickerOutcome {
        self.set_cursor(add_months(self.cursor, months))
    }

    fn move_years(&mut self, years: i32) -> PickerOutcome {
        self.set_cursor(add_months(self.cursor, years.saturating_mul(12)))
    }

    fn set_cursor(&mut self, date: Date) -> PickerOutcome {
        let next = self.clamp(date);
        if next == self.cursor {
            return PickerOutcome::handled(false);
        }
        self.cursor = next;
        self.display_month = first_of_month(next);
        if next.year() < self.year_page_start
            || next.year() > self.year_page_start.saturating_add(23)
        {
            self.year_page_start = year_page_start(next.year());
        }
        PickerOutcome::handled(true)
    }

    fn handle_quick_jump(&mut self, key: KeyEvent) -> Option<PickerOutcome> {
        match self.view {
            DatePickerView::Day => self.handle_day_quick_jump(key),
            DatePickerView::Month => self.handle_month_quick_jump(key),
            DatePickerView::Year => self.handle_year_quick_jump(key),
        }
    }

    fn handle_day_quick_jump(&mut self, key: KeyEvent) -> Option<PickerOutcome> {
        if let Some(first) = self.quick_jump_digit {
            if quick_jump_accepts(key) {
                self.clear_quick_jump();
                return Some(self.select_quick_jump_day(first));
            }
            if let Some(second) = plain_digit(key) {
                self.clear_quick_jump();
                let day = first * 10 + second;
                return Some(if day <= self.days_in_display_month() {
                    self.select_quick_jump_day(day)
                } else {
                    PickerOutcome::handled(false)
                });
            }
            self.clear_quick_jump();
        }
        let digit = plain_digit(key)?;
        if digit == 0 {
            return Some(PickerOutcome::handled(false));
        }
        if digit <= 3 && digit * 10 <= self.days_in_display_month() {
            self.quick_jump_digit = Some(digit);
            self.quick_jump_elapsed = StdDuration::ZERO;
            return Some(PickerOutcome::handled(true));
        }
        Some(self.select_quick_jump_day(digit))
    }

    fn handle_month_quick_jump(&mut self, key: KeyEvent) -> Option<PickerOutcome> {
        self.quick_jump_digit = None;
        if !self.quick_jump_text.is_empty() && quick_jump_accepts(key) {
            self.clear_quick_jump();
            return Some(PickerOutcome::handled(true));
        }
        let Some(character) = plain_letter(key) else {
            if !self.quick_jump_text.is_empty() {
                self.clear_quick_jump();
            }
            return None;
        };
        let had_pending_prefix = !self.quick_jump_text.is_empty();
        let mut prefix = self.quick_jump_text.clone();
        prefix.push(character);
        let matches = months_matching_prefix(&prefix);
        if matches.is_empty() {
            self.clear_quick_jump();
            return had_pending_prefix.then(|| PickerOutcome::handled(true));
        }
        if matches.len() == 1 {
            self.clear_quick_jump();
            return Some(self.complete_month_quick_jump(matches[0]));
        }
        self.quick_jump_text = prefix;
        self.quick_jump_elapsed = StdDuration::ZERO;
        Some(PickerOutcome::handled(true))
    }

    fn handle_year_quick_jump(&mut self, key: KeyEvent) -> Option<PickerOutcome> {
        self.quick_jump_digit = None;
        if !self.quick_jump_text.is_empty() && quick_jump_accepts(key) {
            self.clear_quick_jump();
            return Some(PickerOutcome::handled(true));
        }
        let Some(digit) = plain_digit(key) else {
            if !self.quick_jump_text.is_empty() {
                self.clear_quick_jump();
            }
            return None;
        };
        self.quick_jump_text.push(char::from(b'0' + digit));
        self.quick_jump_elapsed = StdDuration::ZERO;
        if self.quick_jump_text.len() < 4 {
            return Some(PickerOutcome::handled(true));
        }
        let year = self
            .quick_jump_text
            .parse::<i32>()
            .expect("four ASCII digits form a valid year");
        self.clear_quick_jump();
        Some(self.complete_year_quick_jump(year))
    }

    fn select_quick_jump_day(&mut self, day: u8) -> PickerOutcome {
        let previous = self.value;
        let changed = self
            .set_cursor(date_in_month(
                self.display_month.year(),
                self.display_month.month(),
                day,
            ))
            .changed;
        self.value = Some(self.cursor);
        self.quick_jump_selected = true;
        PickerOutcome::selected(changed || previous != self.value)
    }

    fn complete_month_quick_jump(&mut self, month: Month) -> PickerOutcome {
        self.set_cursor(date_in_month(self.cursor.year(), month, self.cursor.day()));
        self.view = DatePickerView::Day;
        PickerOutcome::handled(true)
    }

    fn complete_year_quick_jump(&mut self, year: i32) -> PickerOutcome {
        self.set_cursor(date_in_month(year, self.cursor.month(), self.cursor.day()));
        self.view = DatePickerView::Month;
        PickerOutcome::handled(true)
    }

    pub(super) fn take_quick_jump_selection(&mut self) -> bool {
        std::mem::take(&mut self.quick_jump_selected)
    }

    fn days_in_display_month(&self) -> u8 {
        self.display_month.month().length(self.display_month.year())
    }

    fn clear_quick_jump(&mut self) {
        self.quick_jump_digit = None;
        self.quick_jump_text.clear();
        self.quick_jump_elapsed = StdDuration::ZERO;
    }

    fn quick_jump_line(&self, label: &str, style: Style) -> Line<'static> {
        let prefix_len = self.quick_jump_text.len();
        if prefix_len == 0
            || label.len() < prefix_len
            || !label[..prefix_len].eq_ignore_ascii_case(&self.quick_jump_text)
        {
            return Line::from(Span::styled(label.to_owned(), style));
        }
        Line::from(vec![
            Span::styled(
                label[..prefix_len].to_owned(),
                style.add_modifier(Modifier::UNDERLINED),
            ),
            Span::styled(label[prefix_len..].to_owned(), style),
        ])
    }

    fn clamp(&self, date: Date) -> Date {
        if let Some(min) = self.min
            && date < min
        {
            return min;
        }
        if let Some(max) = self.max
            && date > max
        {
            return max;
        }
        date
    }
}

impl<M> Default for DatePicker<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> TuiNode<M> for DatePicker<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        picker_size_hint(DATE_PICKER_WIDTH, 10).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(DATE_PICKER_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(DATE_PICKER_FOCUS), area, true);
        }
        ctx.set_focus_control(FocusId::new(DATE_PICKER_FOCUS), true);
        ctx.set_focus_receives_events_before_global_hotkeys(FocusId::new(DATE_PICKER_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            match hotkey {
                HotkeyEvent::Pending(prefix) => {
                    self.pending_hotkey_prefix = Some(prefix.clone());
                    ctx.request_redraw();
                    return EventOutcome::Ignored;
                }
                HotkeyEvent::Canceled => {
                    if self.pending_hotkey_prefix.take().is_some() {
                        ctx.request_redraw();
                    }
                    return EventOutcome::Ignored;
                }
                HotkeyEvent::Commit(sequence) => {
                    self.pending_hotkey_prefix = None;
                    if self.hotkey.as_deref().is_some_and(|hotkey| {
                        crate::hotkey::normalize_hotkey(hotkey)
                            == crate::hotkey::normalize_hotkey(sequence)
                    }) {
                        ctx.request_redraw();
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    return EventOutcome::Ignored;
                }
            }
        }
        if let TuiEvent::ExternalEditor(response) = event {
            let outcome = self.apply_external_editor_response(response);
            if outcome.selected
                && let Some(on_select) = &self.on_select
            {
                ctx.emit(on_select(self.cursor));
            }
            ctx.request_clear();
            return finish_event(ctx, outcome);
        }
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.cursor.to_string());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if keybindings()
            .date_time_picker()
            .external_editor_matches(*key)
        {
            ctx.request_external_editor(
                self.cursor.to_string(),
                1,
                self.cursor.to_string().len() + 1,
            );
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        request_unfocus_if_canceled(ctx, outcome);
        if outcome.selected
            && let Some(on_select) = &self.on_select
        {
            ctx.emit(on_select(self.cursor));
        }
        finish_event(ctx, outcome)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: StdDuration, _settings: crate::AnimationSettings) -> TickResult {
        if self.quick_jump_digit.is_none() && self.quick_jump_text.is_empty() {
            return TickResult::IDLE;
        }
        self.quick_jump_elapsed = self.quick_jump_elapsed.saturating_add(dt);
        if self.quick_jump_elapsed >= QUICK_JUMP_TIMEOUT {
            self.clear_quick_jump();
            TickResult::CHANGED
        } else {
            TickResult::scheduled_after(QUICK_JUMP_TIMEOUT - self.quick_jump_elapsed)
        }
    }
}

fn quick_jump_accepts(key: KeyEvent) -> bool {
    matches!(key.code, Key::Enter | Key::Char(' ')) && key.modifiers.is_empty()
}

fn plain_letter(key: KeyEvent) -> Option<char> {
    if !key.modifiers.is_empty() && key.modifiers != KeyModifiers::SHIFT {
        return None;
    }
    let Key::Char(character) = key.code else {
        return None;
    };
    character
        .is_ascii_alphabetic()
        .then(|| character.to_ascii_lowercase())
}

fn months_matching_prefix(prefix: &str) -> Vec<Month> {
    (1_u8..=12)
        .filter_map(|number| Month::try_from(number).ok())
        .filter(|month| month_abbr(*month).to_ascii_lowercase().starts_with(prefix))
        .collect()
}

fn narrow_weekday_labels(first_day_of_week: Weekday) -> [&'static str; 7] {
    const LABELS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let offset = first_day_of_week.number_days_from_monday() as usize;
    std::array::from_fn(|index| LABELS[(index + offset) % LABELS.len()])
}

#[derive(Clone, Copy)]
struct DateStyles {
    cursor: Date,
    selected: Option<Date>,
    today: Date,
    focused: bool,
    min: Option<Date>,
    max: Option<Date>,
    display_month: Date,
    quick_jump_digit: Option<u8>,
}

impl DateStyles {
    fn style(&self, date: Date, surrounding: bool) -> Style {
        let t = theme();
        let style =
            if self.min.is_some_and(|min| date < min) || self.max.is_some_and(|max| date > max) {
                Style::default().fg(t.subtle_fg())
            } else if self.focused && date == self.cursor {
                Style::default()
                    .fg(t.highlight_fg())
                    .bg(t.highlight_bg())
                    .add_modifier(Modifier::BOLD)
            } else if Some(date) == self.selected {
                Style::default().fg(t.selected_fg()).bg(t.selected_bg())
            } else if date == self.today {
                Style::default()
                    .fg(t.accent_fg())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(if surrounding {
                    t.subtle_fg()
                } else {
                    t.text_fg()
                })
            };
        style
    }

    fn day_line(&self, date: Date, surrounding: bool) -> Line<'static> {
        let style = self.style(date, surrounding);
        if !self.quick_jump_matches(date) {
            return Line::from(Span::styled(format!("{:>2} ", date.day()), style));
        }
        let day = date.day();
        let leading = if day < 10 { " " } else { "" };
        let prefix = if day < 10 { day } else { day / 10 };
        let suffix = if day < 10 {
            String::new()
        } else {
            (day % 10).to_string()
        };
        Line::from(vec![
            Span::styled(leading.to_owned(), style),
            Span::styled(prefix.to_string(), style.add_modifier(Modifier::UNDERLINED)),
            Span::styled(suffix, style),
            Span::styled(" ", style),
        ])
    }

    fn quick_jump_matches(&self, date: Date) -> bool {
        let Some(digit) = self.quick_jump_digit else {
            return false;
        };
        date.year() == self.display_month.year()
            && date.month() == self.display_month.month()
            && (date.day() == digit || date.day() / 10 == digit)
    }
}

#[cfg(test)]
mod tests;
