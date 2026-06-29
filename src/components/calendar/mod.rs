use std::cmp::Ordering;
use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use time::{Date, Duration, Weekday};

mod date_math;
mod model;

pub use model::{
    CalendarEntryRole, CalendarOutcome, CalendarSpan, CalendarTypedEvent, CalendarView,
};

use date_math::{
    add_months, first_of_month, format_time, last_of_month, today, week_range, weekday_labels,
    weekday_short,
};

use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, KeySpec, LayoutCtx, LayoutProposal, LayoutResult,
    LayoutSizeHint, TickResult, TuiNode, keybindings, theme,
};

use super::Panel;

const CALENDAR_FOCUS: &str = "calendar";

type IdFn<T, Id> = dyn Fn(&T) -> Id;
type SpanFn<T> = dyn Fn(&T) -> CalendarSpan;
type TitleFn<T> = dyn Fn(&T) -> String;
type RoleFn<T> = dyn Fn(&T) -> Option<CalendarEntryRole>;
type EntryRenderFn<T> = dyn Fn(&T) -> Line<'static>;
type DetailRenderFn<T> = dyn Fn(&T) -> Text<'static>;

pub struct Calendar<T, Id = String, M = ()> {
    entries: Vec<T>,
    id: Box<IdFn<T, Id>>,
    span: Box<SpanFn<T>>,
    title: Box<TitleFn<T>>,
    role: Box<RoleFn<T>>,
    render_entry: Option<Box<EntryRenderFn<T>>>,
    render_detail: Option<Box<DetailRenderFn<T>>>,
    on_event: Option<Box<dyn Fn(CalendarTypedEvent<Id>) -> M>>,
    view: CalendarView,
    stack: Vec<CalendarView>,
    cursor: Date,
    today: Date,
    first_weekday: Weekday,
    highlighted_entry: Option<usize>,
    focused: bool,
    hotkey: Option<String>,
    keybindings: Option<CalendarKeyBindings>,
    area: Rect,
    events: Vec<CalendarTypedEvent<Id>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarKeyBindings {
    pub month_view: Vec<KeySpec>,
    pub week_view: Vec<KeySpec>,
    pub day_view: Vec<KeySpec>,
    pub today: Vec<KeySpec>,
    pub activate: Vec<KeySpec>,
    pub back: Vec<KeySpec>,
    pub left: Vec<KeySpec>,
    pub right: Vec<KeySpec>,
    pub up: Vec<KeySpec>,
    pub down: Vec<KeySpec>,
    pub page_up: Vec<KeySpec>,
    pub page_down: Vec<KeySpec>,
    pub home: Vec<KeySpec>,
    pub end: Vec<KeySpec>,
}

impl Default for CalendarKeyBindings {
    fn default() -> Self {
        Self {
            month_view: vec![KeySpec::plain('m')],
            week_view: vec![KeySpec::plain('w')],
            day_view: vec![KeySpec::plain('d')],
            today: vec![KeySpec::plain('t')],
            activate: vec![KeySpec::key(Key::Enter), KeySpec::plain(' ')],
            back: vec![
                KeySpec::key(Key::Esc),
                KeySpec::key_with_modifiers(Key::Char('['), KeyModifiers::CONTROL),
            ],
            left: vec![KeySpec::key(Key::Left), KeySpec::plain('h')],
            right: vec![KeySpec::key(Key::Right), KeySpec::plain('l')],
            up: vec![KeySpec::key(Key::Up), KeySpec::plain('k')],
            down: vec![KeySpec::key(Key::Down), KeySpec::plain('j')],
            page_up: vec![
                KeySpec::key(Key::PageUp),
                KeySpec::key_with_modifiers(Key::Char('u'), KeyModifiers::CONTROL),
            ],
            page_down: vec![
                KeySpec::key(Key::PageDown),
                KeySpec::key_with_modifiers(Key::Char('d'), KeyModifiers::CONTROL),
            ],
            home: vec![KeySpec::key(Key::Home)],
            end: vec![KeySpec::key(Key::End)],
        }
    }
}

impl CalendarKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<T, Id, M> Calendar<T, Id, M>
where
    Id: Clone + Eq,
{
    pub fn new(
        entries: impl IntoIterator<Item = T>,
        id: impl Fn(&T) -> Id + 'static,
        span: impl Fn(&T) -> CalendarSpan + 'static,
        title: impl Fn(&T) -> String + 'static,
    ) -> Self {
        let today = today();
        Self {
            entries: entries.into_iter().collect(),
            id: Box::new(id),
            span: Box::new(span),
            title: Box::new(title),
            role: Box::new(|_| None),
            render_entry: None,
            render_detail: None,
            on_event: None,
            view: CalendarView::Month,
            stack: Vec::new(),
            cursor: today,
            today,
            first_weekday: Weekday::Monday,
            highlighted_entry: None,
            focused: false,
            hotkey: None,
            keybindings: None,
            area: Rect::default(),
            events: Vec::new(),
        }
    }

    pub fn today(mut self, today: Date) -> Self {
        self.today = today;
        self.cursor = today;
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn cursor(mut self, cursor: Date) -> Self {
        self.cursor = cursor;
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn first_weekday(mut self, weekday: Weekday) -> Self {
        self.first_weekday = weekday;
        self
    }

    pub fn view(mut self, view: CalendarView) -> Self {
        self.view = view;
        self.stack.clear();
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn role(mut self, role: impl Fn(&T) -> Option<CalendarEntryRole> + 'static) -> Self {
        self.role = Box::new(role);
        self
    }

    pub fn render_entry(mut self, render: impl Fn(&T) -> Line<'static> + 'static) -> Self {
        self.render_entry = Some(Box::new(render));
        self
    }

    pub fn render_detail(mut self, render: impl Fn(&T) -> Text<'static> + 'static) -> Self {
        self.render_detail = Some(Box::new(render));
        self
    }

    pub fn on_event(mut self, handler: impl Fn(CalendarTypedEvent<Id>) -> M + 'static) -> Self {
        self.on_event = Some(Box::new(handler));
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn keybindings(mut self, keybindings: CalendarKeyBindings) -> Self {
        self.keybindings = Some(keybindings);
        self
    }

    pub fn set_keybindings(&mut self, keybindings: CalendarKeyBindings) {
        self.keybindings = Some(keybindings);
    }

    pub fn set_entries(&mut self, entries: impl IntoIterator<Item = T>) {
        self.entries = entries.into_iter().collect();
        self.highlight_first_entry_on_cursor();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn current_view(&self) -> CalendarView {
        self.view
    }

    pub fn cursor_date(&self) -> Date {
        self.cursor
    }

    pub fn highlighted_entry_id(&self) -> Option<Id> {
        self.highlighted_entry
            .map(|index| (self.id)(&self.entries[index]))
    }

    pub fn current_range(&self) -> (Date, Date) {
        match self.view {
            CalendarView::Month => (first_of_month(self.cursor), last_of_month(self.cursor)),
            CalendarView::Week => week_range(self.cursor, self.first_weekday),
            CalendarView::Day | CalendarView::EventDetail => (self.cursor, self.cursor),
        }
    }

    pub fn take_events(&mut self) -> Vec<CalendarTypedEvent<Id>> {
        self.events.drain(..).collect()
    }

    pub fn drain_events(&mut self) -> Vec<CalendarTypedEvent<Id>> {
        self.take_events()
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> CalendarOutcome {
        let key = key.into();
        if let Some(action) = self.custom_key_action(key) {
            return self.apply_key_action(action);
        }
        let bindings = keybindings();
        if bindings.date_time_picker().month_view_matches(key) {
            return self.direct_view(CalendarView::Month);
        }
        if plain_char(key, 'w') {
            return self.direct_view(CalendarView::Week);
        }
        if plain_char(key, 'd') {
            return self.direct_view(CalendarView::Day);
        }
        if bindings.date_time_picker().today_matches(key) {
            return self.set_cursor(self.today);
        }
        if bindings.button().press_matches(key) {
            return self.activate();
        }
        if bindings.focus().unfocus_matches(key) {
            return self.back();
        }
        if bindings.line_left_matches(key) {
            return self.move_left();
        }
        if bindings.line_right_matches(key) {
            return self.move_right();
        }
        if bindings.line_up_matches(key) {
            return self.move_up();
        }
        if bindings.line_down_matches(key) {
            return self.move_down();
        }
        if bindings.page_up_matches(key) {
            return self.page(-1);
        }
        if bindings.page_down_matches(key) {
            return self.page(1);
        }
        if bindings.home_matches(key) {
            return self.home();
        }
        if bindings.end_matches(key) {
            return self.end();
        }
        CalendarOutcome::IDLE
    }

    fn custom_key_action(&self, key: KeyEvent) -> Option<CalendarKeyAction> {
        let keys = self.keybindings.as_ref()?;
        if matches_key_specs(&keys.month_view, key) {
            Some(CalendarKeyAction::Month)
        } else if matches_key_specs(&keys.week_view, key) {
            Some(CalendarKeyAction::Week)
        } else if matches_key_specs(&keys.day_view, key) {
            Some(CalendarKeyAction::Day)
        } else if matches_key_specs(&keys.today, key) {
            Some(CalendarKeyAction::Today)
        } else if matches_key_specs(&keys.activate, key) {
            Some(CalendarKeyAction::Activate)
        } else if matches_key_specs(&keys.back, key) {
            Some(CalendarKeyAction::Back)
        } else if matches_key_specs(&keys.left, key) {
            Some(CalendarKeyAction::Left)
        } else if matches_key_specs(&keys.right, key) {
            Some(CalendarKeyAction::Right)
        } else if matches_key_specs(&keys.up, key) {
            Some(CalendarKeyAction::Up)
        } else if matches_key_specs(&keys.down, key) {
            Some(CalendarKeyAction::Down)
        } else if matches_key_specs(&keys.page_up, key) {
            Some(CalendarKeyAction::PageUp)
        } else if matches_key_specs(&keys.page_down, key) {
            Some(CalendarKeyAction::PageDown)
        } else if matches_key_specs(&keys.home, key) {
            Some(CalendarKeyAction::Home)
        } else if matches_key_specs(&keys.end, key) {
            Some(CalendarKeyAction::End)
        } else {
            None
        }
    }

    fn apply_key_action(&mut self, action: CalendarKeyAction) -> CalendarOutcome {
        match action {
            CalendarKeyAction::Month => self.direct_view(CalendarView::Month),
            CalendarKeyAction::Week => self.direct_view(CalendarView::Week),
            CalendarKeyAction::Day => self.direct_view(CalendarView::Day),
            CalendarKeyAction::Today => self.set_cursor(self.today),
            CalendarKeyAction::Activate => self.activate(),
            CalendarKeyAction::Back => self.back(),
            CalendarKeyAction::Left => self.move_left(),
            CalendarKeyAction::Right => self.move_right(),
            CalendarKeyAction::Up => self.move_up(),
            CalendarKeyAction::Down => self.move_down(),
            CalendarKeyAction::PageUp => self.page(-1),
            CalendarKeyAction::PageDown => self.page(1),
            CalendarKeyAction::Home => self.home(),
            CalendarKeyAction::End => self.end(),
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        match self.view {
            CalendarView::Month => self.render_month(frame, area),
            CalendarView::Week => self.render_week(frame, area),
            CalendarView::Day => self.render_day(frame, area),
            CalendarView::EventDetail => self.render_detail_view(frame, area),
        }
    }

    fn direct_view(&mut self, view: CalendarView) -> CalendarOutcome {
        self.stack.clear();
        self.set_view(view, None)
    }

    fn activate(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month => self.drill_to(CalendarView::Week),
            CalendarView::Week => self.drill_to(CalendarView::Day),
            CalendarView::Day => {
                let Some(index) = self.highlighted_entry else {
                    self.push_event(CalendarTypedEvent::DateActivated { date: self.cursor });
                    return CalendarOutcome::ACTIVATED;
                };
                let id = (self.id)(&self.entries[index]);
                self.push_event(CalendarTypedEvent::EntryActivated { entry_id: id });
                self.drill_to(CalendarView::EventDetail).with_activated()
            }
            CalendarView::EventDetail => {
                let Some(index) = self.highlighted_entry else {
                    return CalendarOutcome::HANDLED;
                };
                let id = (self.id)(&self.entries[index]);
                self.push_event(CalendarTypedEvent::EntryActivated { entry_id: id });
                CalendarOutcome::ACTIVATED
            }
        }
    }

    fn drill_to(&mut self, view: CalendarView) -> CalendarOutcome {
        let from = self.view;
        self.stack.push(from);
        self.set_view(view, Some(CalendarTypedEvent::DrillDown { from, to: view }))
    }

    fn back(&mut self) -> CalendarOutcome {
        let from = self.view;
        let view = match self.view {
            CalendarView::EventDetail => CalendarView::Day,
            CalendarView::Day => CalendarView::Week,
            CalendarView::Week => CalendarView::Month,
            CalendarView::Month => return CalendarOutcome::IDLE,
        };
        if let Some(position) = self.stack.iter().rposition(|stacked| *stacked == view) {
            self.stack.truncate(position);
        } else {
            self.stack.clear();
        }
        self.set_view(view, Some(CalendarTypedEvent::Back { from, to: view }))
    }

    fn set_view(
        &mut self,
        view: CalendarView,
        transition: Option<CalendarTypedEvent<Id>>,
    ) -> CalendarOutcome {
        if self.view == view {
            return CalendarOutcome::HANDLED;
        }
        self.view = view;
        if view != CalendarView::EventDetail {
            self.highlight_first_entry_on_cursor();
        }
        if let Some(event) = transition {
            self.push_event(event);
        }
        self.push_event(CalendarTypedEvent::ViewChanged { view });
        self.emit_range_changed();
        CalendarOutcome::CHANGED
    }

    fn set_cursor(&mut self, date: Date) -> CalendarOutcome {
        if self.cursor == date {
            return CalendarOutcome::HANDLED;
        }
        let before_range = self.current_range();
        self.cursor = date;
        self.push_event(CalendarTypedEvent::CursorChanged { date });
        if before_range != self.current_range() {
            self.emit_range_changed();
        }
        self.highlight_first_entry_on_cursor();
        CalendarOutcome::CHANGED
    }

    fn move_left(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month | CalendarView::Week | CalendarView::Day => self.move_days(-1),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn move_right(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month | CalendarView::Week | CalendarView::Day => self.move_days(1),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn move_up(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Day => self.highlight_previous_entry(),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
            CalendarView::Month | CalendarView::Week => self.move_days(-7),
        }
    }

    fn move_down(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Day => self.highlight_next_entry(),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
            CalendarView::Month | CalendarView::Week => self.move_days(7),
        }
    }

    fn page(&mut self, delta: i32) -> CalendarOutcome {
        match self.view {
            CalendarView::Month => self.set_cursor(add_months(self.cursor, delta)),
            CalendarView::Week | CalendarView::Day => self.move_days(i64::from(delta) * 7),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn home(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month => self.set_cursor(first_of_month(self.cursor)),
            CalendarView::Week => self.set_cursor(week_range(self.cursor, self.first_weekday).0),
            CalendarView::Day => self.highlight_entry_boundary(false),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn end(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month => self.set_cursor(last_of_month(self.cursor)),
            CalendarView::Week => self.set_cursor(week_range(self.cursor, self.first_weekday).1),
            CalendarView::Day => self.highlight_entry_boundary(true),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn move_days(&mut self, days: i64) -> CalendarOutcome {
        let date = self
            .cursor
            .checked_add(Duration::days(days))
            .unwrap_or_else(|| {
                if days.is_negative() {
                    Date::MIN
                } else {
                    Date::MAX
                }
            });
        self.set_cursor(date)
    }

    fn highlight_first_entry_on_cursor(&mut self) {
        let next = self.first_entry_on_cursor();
        self.set_highlighted_entry(next);
    }

    fn first_entry_on_cursor(&self) -> Option<usize> {
        self.entries_on(self.cursor).first().copied()
    }

    fn highlight_next_entry(&mut self) -> CalendarOutcome {
        let entries = self.entries_on(self.cursor);
        if entries.is_empty() {
            return CalendarOutcome::HANDLED;
        }
        let current = self
            .highlighted_entry
            .and_then(|index| entries.iter().position(|entry| *entry == index))
            .unwrap_or(0);
        let next = entries[current
            .saturating_add(1)
            .min(entries.len().saturating_sub(1))];
        self.highlight_entry(next)
    }

    fn highlight_previous_entry(&mut self) -> CalendarOutcome {
        let entries = self.entries_on(self.cursor);
        if entries.is_empty() {
            return CalendarOutcome::HANDLED;
        }
        let current = self
            .highlighted_entry
            .and_then(|index| entries.iter().position(|entry| *entry == index))
            .unwrap_or(0);
        let next = entries[current.saturating_sub(1)];
        self.highlight_entry(next)
    }

    fn highlight_entry_boundary(&mut self, last: bool) -> CalendarOutcome {
        let entries = self.entries_on(self.cursor);
        let next = if last {
            entries.last().copied()
        } else {
            entries.first().copied()
        };
        let Some(next) = next else {
            return CalendarOutcome::HANDLED;
        };
        self.highlight_entry(next)
    }

    fn highlight_entry(&mut self, index: usize) -> CalendarOutcome {
        if self.highlighted_entry == Some(index) {
            return CalendarOutcome::HANDLED;
        }
        self.set_highlighted_entry(Some(index));
        CalendarOutcome::CHANGED
    }

    fn set_highlighted_entry(&mut self, index: Option<usize>) {
        if self.highlighted_entry == index {
            return;
        }
        self.highlighted_entry = index;
        self.push_event(CalendarTypedEvent::EntryHighlighted {
            entry_id: index.map(|index| (self.id)(&self.entries[index])),
        });
    }

    fn emit_range_changed(&mut self) {
        let (start, end) = self.current_range();
        self.push_event(CalendarTypedEvent::RangeChanged { start, end });
    }

    fn push_event(&mut self, event: CalendarTypedEvent<Id>) {
        self.events.push(event);
    }

    fn entries_on(&self, date: Date) -> Vec<usize> {
        let mut entries = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (self.span)(entry).covers_date(date).then_some(index))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| self.compare_entries(*left, *right));
        entries
    }

    fn compare_entries(&self, left: usize, right: usize) -> Ordering {
        let left_span = (self.span)(&self.entries[left]);
        let right_span = (self.span)(&self.entries[right]);
        left_span
            .all_day
            .cmp(&right_span.all_day)
            .reverse()
            .then_with(|| left_span.start.cmp(&right_span.start))
            .then_with(|| {
                (self.title)(&self.entries[left]).cmp(&(self.title)(&self.entries[right]))
            })
    }

    fn entry_line(&self, index: usize) -> Line<'static> {
        if let Some(render_entry) = &self.render_entry {
            return render_entry(&self.entries[index]);
        }
        Line::from((self.title)(&self.entries[index]))
    }

    fn detail_text(&self, index: usize) -> Text<'static> {
        if let Some(render_detail) = &self.render_detail {
            return render_detail(&self.entries[index]);
        }
        let span = (self.span)(&self.entries[index]);
        let when = if span.all_day {
            format!("{} all day", span.start.date())
        } else {
            format!(
                "{} {}–{}",
                span.start.date(),
                format_time(span.start.time()),
                format_time(span.end.time())
            )
        };
        Text::from(vec![
            Line::from((self.title)(&self.entries[index])),
            Line::from(when),
        ])
    }
}

impl<T, Id, M> TuiNode<M> for Calendar<T, Id, M>
where
    Id: Clone + Eq + 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(34, 12).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        if let Some(hotkey) = &self.hotkey {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(CALENDAR_FOCUS),
                area,
                true,
                vec![hotkey.clone()],
            );
        } else {
            ctx.register_focusable(FocusId::new(CALENDAR_FOCUS), area, true);
        }
        ctx.set_focus_receives_events_before_global_hotkeys(FocusId::new(CALENDAR_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let event_start = self.events.len();
        let outcome = self.on_key(*key);
        if let Some(on_event) = &self.on_event {
            let events = self.events.drain(event_start..).collect::<Vec<_>>();
            for event in events {
                ctx.emit(on_event(event));
            }
        }
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, _dt: StdDuration, _settings: crate::AnimationSettings) -> TickResult {
        TickResult::IDLE
    }
}

impl<T, Id, M> Calendar<T, Id, M>
where
    Id: Clone + Eq,
{
    fn render_month(&self, frame: &mut Frame, area: Rect) {
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
        let start = week_range(first_of_month(self.cursor), self.first_weekday).0;
        for week in 0..6 {
            let cols = week_columns(rows[week + 1]);
            for day in 0..7 {
                let date = start + Duration::days((week * 7 + day) as i64);
                self.render_month_cell(frame, cols[day], date);
            }
        }
    }

    fn render_month_cell(&self, frame: &mut Frame, area: Rect, date: Date) {
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
        let visible_events = if entries.len() > event_capacity {
            event_capacity.saturating_sub(1)
        } else {
            event_capacity
        };
        for index in entries.iter().take(visible_events).copied() {
            lines.push(self.event_summary_line(index, false));
        }
        if entries.len() > visible_events && event_capacity > 0 {
            lines.push(Line::from(Span::styled(
                format!("+{} more", entries.len() - visible_events),
                Style::default().fg(theme().muted_fg()),
            )));
        }
        frame.render_widget(
            Paragraph::new(lines).style(self.date_cell_style(date)),
            inner,
        );
    }

    fn render_week(&self, frame: &mut Frame, area: Rect) {
        let (start, end) = week_range(self.cursor, self.first_weekday);
        self.render_panel(frame, area, format!(" Week • {start} — {end} "));
        let inner = Panel::inner_area(area);
        if inner.height == 0 {
            return;
        }
        let cols = week_columns(inner);
        self.render_week_grid_lines(frame, &cols);
        for offset in 0..7 {
            let date = start + Duration::days(offset);
            self.render_week_column(frame, cols[offset as usize], date);
        }
    }

    fn render_week_column(&self, frame: &mut Frame, area: Rect, date: Date) {
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
        let visible_events = if entries.len() > event_capacity {
            event_capacity.saturating_sub(1)
        } else {
            event_capacity
        };
        for index in entries.iter().take(visible_events).copied() {
            lines.push(self.event_summary_line(index, true));
        }
        if entries.len() > visible_events && event_capacity > 0 {
            lines.push(Line::from(Span::styled(
                format!("+{} more", entries.len() - visible_events),
                Style::default().fg(theme().muted_fg()),
            )));
        }
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
        let cols = week_columns(grid);
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

    fn render_day(&self, frame: &mut Frame, area: Rect) {
        self.render_panel(frame, area, format!(" Day • {} ", self.cursor));
        let inner = Panel::inner_area(area);
        let entries = self.entries_on(self.cursor);
        let lines = entries
            .into_iter()
            .map(|index| {
                let span = (self.span)(&self.entries[index]);
                let selected = self.highlighted_entry == Some(index);
                let prefix = if span.all_day {
                    String::from("all-day ")
                } else {
                    format!("{} ", format_time(span.start.time()))
                };
                let mut line = self.entry_line(index);
                line.spans.insert(
                    0,
                    Span::styled(prefix, Style::default().fg(theme().muted_fg())),
                );
                line.style = self.entry_style(index, selected);
                line
            })
            .collect::<Vec<_>>();
        let text = if lines.is_empty() {
            Text::from("No entries")
        } else {
            Text::from(lines)
        };
        frame.render_widget(Paragraph::new(text), inner);
    }

    fn render_detail_view(&self, frame: &mut Frame, area: Rect) {
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
        let cols = week_columns(area);
        for (index, label) in weekday_labels(self.first_weekday).into_iter().enumerate() {
            frame.render_widget(
                Paragraph::new(label).style(Style::default().fg(theme().muted_fg())),
                cols[index],
            );
        }
    }

    fn render_panel(&self, frame: &mut Frame, area: Rect, title: impl Into<String>) {
        Panel::new()
            .top_left(title)
            .focused(self.focused)
            .render(frame, area);
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

    fn event_summary_line(&self, index: usize, show_time: bool) -> Line<'static> {
        let span = (self.span)(&self.entries[index]);
        let prefix = if span.all_day {
            String::from("■ ")
        } else if show_time {
            format!("• {} ", format_time(span.start.time()))
        } else {
            String::from("• ")
        };
        let mut spans = vec![Span::styled(
            prefix,
            Style::default().fg(theme().accent_fg()),
        )];
        spans.extend(self.entry_line(index).spans);
        Line::from(spans).style(self.entry_style(index, self.highlighted_entry == Some(index)))
    }
}

fn plain_char(key: KeyEvent, value: char) -> bool {
    key.modifiers.is_empty() && matches!(key.code, Key::Char(ch) if ch == value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarKeyAction {
    Month,
    Week,
    Day,
    Today,
    Activate,
    Back,
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

fn matches_key_specs(keys: &[KeySpec], key: KeyEvent) -> bool {
    keys.iter().copied().any(|spec| spec.matches(key))
}

fn week_columns(area: Rect) -> Vec<Rect> {
    let width = area.width / 7;
    let remainder = area.width % 7;
    (0..7)
        .map(|index| {
            let extra = u16::from(index < remainder as usize);
            let x = area.x + width * index as u16 + remainder.min(index as u16);
            Rect::new(x, area.y, width + extra, area.height)
        })
        .collect()
}

fn grid_cell_inner(area: Rect, reserve_top_line: bool) -> Rect {
    let x = area.x.saturating_add(1);
    let y = area.y.saturating_add(u16::from(reserve_top_line));
    let width = area.width.saturating_sub(1);
    let height = area.height.saturating_sub(u16::from(reserve_top_line));
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests;
