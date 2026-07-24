use std::cmp::Ordering;
use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
#[cfg(test)]
use ratatui::{style::Style, text::Span};
use time::{Date, Duration, Weekday};

pub(crate) mod date_math;
mod event_wrap;
mod model;
mod view;

#[cfg(test)]
use event_wrap::wrap_event_spans;
#[cfg(test)]
use view::EventSummaryKind;

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
    LayoutSizeHint, TickResult, TuiNode,
};

use super::Panel;

const CALENDAR_FOCUS: &str = "calendar";
const MONTH_EVENT_LINES: usize = 1;
const WEEK_EVENT_LINES: usize = 3;
const DAY_EVENT_LINES: usize = 5;

type IdFn<T, Id> = dyn Fn(&T) -> Id;
type SpanFn<T> = dyn Fn(&T) -> CalendarSpan;
type TitleFn<T> = dyn Fn(&T) -> String;
type RoleFn<T> = dyn Fn(&T) -> Option<CalendarEntryRole>;
type EventMarkerFn<T> = dyn Fn(&T) -> char;
type EntryRenderFn<T> = dyn Fn(&T) -> Line<'static>;
type DetailRenderFn<T> = dyn Fn(&T) -> Text<'static>;

pub struct Calendar<T, Id = String, M = ()> {
    entries: Vec<T>,
    id: Box<IdFn<T, Id>>,
    span: Box<SpanFn<T>>,
    title: Box<TitleFn<T>>,
    role: Box<RoleFn<T>>,
    event_marker: Option<Box<EventMarkerFn<T>>>,
    render_entry: Option<Box<EntryRenderFn<T>>>,
    render_detail: Option<Box<DetailRenderFn<T>>>,
    on_event: Option<Box<dyn Fn(CalendarTypedEvent<Id>) -> M>>,
    view: CalendarView,
    stack: Vec<CalendarView>,
    cursor: Date,
    today: Date,
    first_day_of_week: Weekday,
    show_weekends: bool,
    highlighted_entry: Option<usize>,
    focused: bool,
    hotkey: Option<String>,
    keybindings: CalendarKeyBindings,
    area: Rect,
    events: Vec<CalendarTypedEvent<Id>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarKeyBindings {
    pub month_view: Vec<KeySpec>,
    pub week_view: Vec<KeySpec>,
    pub day_view: Vec<KeySpec>,
    pub toggle_weekends: Vec<KeySpec>,
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
            toggle_weekends: vec![KeySpec::key_with_modifiers(
                Key::Char('w'),
                KeyModifiers::CONTROL,
            )],
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

    pub fn month_view_label(&self) -> String {
        key_specs_label(&self.month_view)
    }

    pub fn week_view_label(&self) -> String {
        key_specs_label(&self.week_view)
    }

    pub fn day_view_label(&self) -> String {
        key_specs_label(&self.day_view)
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
            event_marker: None,
            render_entry: None,
            render_detail: None,
            on_event: None,
            view: CalendarView::Month,
            stack: Vec::new(),
            cursor: today,
            today,
            first_day_of_week: Weekday::Monday,
            show_weekends: true,
            highlighted_entry: None,
            focused: false,
            hotkey: None,
            keybindings: CalendarKeyBindings::default(),
            area: Rect::default(),
            events: Vec::new(),
        }
    }

    pub fn today(mut self, today: Date) -> Self {
        self.today = today;
        self.cursor = today;
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn cursor(mut self, cursor: Date) -> Self {
        self.cursor = cursor;
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn first_day_of_week(mut self, weekday: Weekday) -> Self {
        self.set_first_day_of_week(weekday);
        self
    }

    pub fn first_weekday(self, weekday: Weekday) -> Self {
        self.first_day_of_week(weekday)
    }

    pub fn show_weekends(mut self, show: bool) -> Self {
        self.set_show_weekends(show);
        self
    }

    pub fn is_showing_weekends(&self) -> bool {
        self.show_weekends
    }

    pub fn set_show_weekends(&mut self, show: bool) {
        if self.show_weekends == show {
            return;
        }
        self.show_weekends = show;
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
    }

    pub fn toggle_weekends(&mut self) {
        self.set_show_weekends(!self.show_weekends);
    }

    pub fn set_first_day_of_week(&mut self, weekday: Weekday) {
        self.first_day_of_week = weekday;
    }

    pub fn view(mut self, view: CalendarView) -> Self {
        self.view = view;
        self.stack.clear();
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
        self
    }

    pub fn role(mut self, role: impl Fn(&T) -> Option<CalendarEntryRole> + 'static) -> Self {
        self.role = Box::new(role);
        self
    }

    pub fn event_marker(mut self, marker: impl Fn(&T) -> char + 'static) -> Self {
        self.set_event_marker(marker);
        self
    }

    pub fn set_event_marker(&mut self, marker: impl Fn(&T) -> char + 'static) {
        self.event_marker = Some(Box::new(marker));
    }

    pub fn clear_event_marker(&mut self) {
        self.event_marker = None;
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
        self.keybindings = keybindings;
        self
    }

    pub fn set_keybindings(&mut self, keybindings: CalendarKeyBindings) {
        self.keybindings = keybindings;
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
            CalendarView::Week => week_range(self.cursor, self.first_day_of_week),
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
        if let Some(action) = self.key_action(key) {
            return self.apply_key_action(action);
        }
        CalendarOutcome::IDLE
    }

    fn key_action(&self, key: KeyEvent) -> Option<CalendarKeyAction> {
        let keys = &self.keybindings;
        if matches_key_specs(&keys.month_view, key) {
            Some(CalendarKeyAction::Month)
        } else if matches_key_specs(&keys.week_view, key) {
            Some(CalendarKeyAction::Week)
        } else if matches_key_specs(&keys.day_view, key) {
            Some(CalendarKeyAction::Day)
        } else if matches_key_specs(&keys.toggle_weekends, key) {
            Some(CalendarKeyAction::ToggleWeekends)
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
            CalendarKeyAction::ToggleWeekends => self.toggle_weekends_action(),
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
        self.normalize_hidden_weekend_cursor();
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

    fn set_cursor(&mut self, mut date: Date) -> CalendarOutcome {
        if !self.show_weekends && matches!(self.view, CalendarView::Month | CalendarView::Week) {
            date = previous_friday_if_weekend(date);
        }
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
            CalendarView::Month => {
                let mut date = first_of_month(self.cursor);
                while !self.show_weekends && is_weekend(date) {
                    date += Duration::days(1);
                }
                self.set_cursor(date)
            }
            CalendarView::Week => {
                let start = week_range(self.cursor, self.first_day_of_week).0;
                let offset = self.visible_weekday_offsets().first().copied().unwrap_or(0);
                self.set_cursor(start + Duration::days(offset as i64))
            }
            CalendarView::Day => self.highlight_entry_boundary(false),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn end(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Month => {
                let mut date = last_of_month(self.cursor);
                while !self.show_weekends && is_weekend(date) {
                    date -= Duration::days(1);
                }
                self.set_cursor(date)
            }
            CalendarView::Week => {
                let start = week_range(self.cursor, self.first_day_of_week).0;
                let offset = self.visible_weekday_offsets().last().copied().unwrap_or(6);
                self.set_cursor(start + Duration::days(offset as i64))
            }
            CalendarView::Day => self.highlight_entry_boundary(true),
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
        }
    }

    fn move_days(&mut self, days: i64) -> CalendarOutcome {
        let mut date = self
            .cursor
            .checked_add(Duration::days(days))
            .unwrap_or_else(|| {
                if days.is_negative() {
                    Date::MIN
                } else {
                    Date::MAX
                }
            });
        if !self.show_weekends && matches!(self.view, CalendarView::Month | CalendarView::Week) {
            let direction = if days.is_negative() { -1 } else { 1 };
            while is_weekend(date) {
                let Some(next) = date.checked_add(Duration::days(direction)) else {
                    break;
                };
                date = next;
            }
        }
        self.set_cursor(date)
    }

    fn toggle_weekends_action(&mut self) -> CalendarOutcome {
        let before_cursor = self.cursor;
        let before_range = self.current_range();
        self.show_weekends = !self.show_weekends;
        self.normalize_hidden_weekend_cursor();
        if self.cursor != before_cursor {
            self.push_event(CalendarTypedEvent::CursorChanged { date: self.cursor });
            if self.current_range() != before_range {
                self.emit_range_changed();
            }
            self.highlight_first_entry_on_cursor();
        }
        CalendarOutcome::CHANGED
    }

    fn normalize_hidden_weekend_cursor(&mut self) {
        if self.show_weekends
            || !matches!(self.view, CalendarView::Month | CalendarView::Week)
            || !is_weekend(self.cursor)
        {
            return;
        }
        self.cursor = previous_friday_if_weekend(self.cursor);
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
        LayoutSizeHint::content(72, 12).normalized(proposal)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarKeyAction {
    Month,
    Week,
    Day,
    ToggleWeekends,
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

fn key_specs_label(keys: &[KeySpec]) -> String {
    keys.iter()
        .map(|key| key.label())
        .collect::<Vec<_>>()
        .join("/")
}

fn calendar_columns(area: Rect, count: usize) -> Vec<Rect> {
    let count = count.max(1) as u16;
    let width = area.width / count;
    let remainder = area.width % count;
    (0..usize::from(count))
        .map(|index| {
            let extra = u16::from(index < remainder as usize);
            let x = area.x + width * index as u16 + remainder.min(index as u16);
            Rect::new(x, area.y, width + extra, area.height)
        })
        .collect()
}

fn weekday_after(mut weekday: Weekday, offset: usize) -> Weekday {
    for _ in 0..offset {
        weekday = weekday.next();
    }
    weekday
}

fn is_weekend(date: Date) -> bool {
    is_weekend_weekday(date.weekday())
}

fn is_weekend_weekday(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Saturday | Weekday::Sunday)
}

fn previous_friday_if_weekend(date: Date) -> Date {
    match date.weekday() {
        Weekday::Saturday => date - Duration::days(1),
        Weekday::Sunday => date - Duration::days(2),
        _ => date,
    }
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
