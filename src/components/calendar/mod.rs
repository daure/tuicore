use std::cmp::Ordering;
use std::time::Duration as StdDuration;

use ratatui::layout::Rect;
use ratatui::style::Style;
#[cfg(test)]
use ratatui::text::Span;
use ratatui::text::{Line, Text};
use ratatui::{Frame, buffer::Buffer};
use time::{Date, Duration, Weekday};

pub(crate) mod date_math;
mod day;
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
use day::{CalendarDayEvent, CalendarDayList, CalendarDayRow, DAY_ENTRIES_SLOT};

use crate::event::{
    Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TuiEvent,
};
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, KeySpec, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, ScrollAxes, ScrollOffset, ScrollSize,
    ScrollState, TickResult, TuiNode, animation_settings, preset, theme,
};

use super::Panel;

const CALENDAR_FOCUS: &str = "calendar";
const MONTH_EVENT_LINES: usize = 2;
const WEEK_EVENT_LINES: usize = 3;
const MIN_CALENDAR_CELL_WIDTH: u16 = 11;
const QUICK_JUMP_TIMEOUT: StdDuration = StdDuration::from_secs(1);

type IdFn<T, Id> = dyn Fn(&T) -> Id;
type SpanFn<T> = dyn Fn(&T) -> CalendarSpan;
type TitleFn<T> = dyn Fn(&T) -> String;
type RoleFn<T> = dyn Fn(&T) -> Option<CalendarEntryRole>;
type EventMarkerFn<T> = dyn Fn(&T) -> char;
type EntryRenderFn<T> = dyn Fn(&T) -> Line<'static>;
type DayEntryContinuationIndentFn<T> = dyn Fn(&T) -> usize;
type DetailRenderFn<T> = dyn Fn(&T) -> Text<'static>;
type EntryOrderFn<T> = dyn Fn(&T, &T) -> Ordering;
type ReorderGroupFn<T> = dyn Fn(&T, &T) -> bool;

pub(super) fn calendar_entry_style(role: Option<CalendarEntryRole>, selected: bool) -> Style {
    let t = theme();
    if selected {
        return Style::default()
            .fg(t.highlight_fg())
            .bg(t.highlight_bg())
            .add_modifier(ratatui::style::Modifier::BOLD);
    }
    match role {
        Some(CalendarEntryRole::Accent) => Style::default().fg(t.accent_fg()),
        Some(CalendarEntryRole::Success) => Style::default().fg(t.success_fg()),
        Some(CalendarEntryRole::Warning) => Style::default().fg(t.warning_fg()),
        Some(CalendarEntryRole::Error) => Style::default().fg(t.error_fg()),
        Some(CalendarEntryRole::Muted) => Style::default().fg(t.muted_fg()),
        None => Style::default().fg(t.text_fg()),
    }
}

pub struct Calendar<T, Id = String, M = ()> {
    entries: Vec<T>,
    id: Box<IdFn<T, Id>>,
    span: Box<SpanFn<T>>,
    title: Box<TitleFn<T>>,
    compact_summary_title: Option<(u16, Box<TitleFn<T>>)>,
    role: Box<RoleFn<T>>,
    event_marker: Option<Box<EventMarkerFn<T>>>,
    render_entry: Option<Box<EntryRenderFn<T>>>,
    day_entry_continuation_indent: Option<Box<DayEntryContinuationIndentFn<T>>>,
    render_detail: Option<Box<DetailRenderFn<T>>>,
    entry_order: Option<Box<EntryOrderFn<T>>>,
    reorder_group: Option<Box<ReorderGroupFn<T>>>,
    event_detail_on_activate: bool,
    on_event: Option<Box<dyn Fn(CalendarTypedEvent<Id>) -> M>>,
    view: CalendarView,
    stack: Vec<CalendarView>,
    cursor: Date,
    today: Date,
    first_day_of_week: Weekday,
    show_weekends: bool,
    bordered: bool,
    highlighted_entry: Option<usize>,
    day_entries: CalendarDayList<Id, M>,
    focused: bool,
    hotkey: Option<String>,
    keybindings: CalendarKeyBindings,
    pending_top_prefix: bool,
    quick_jump_digit: Option<u8>,
    quick_jump_elapsed: StdDuration,
    area: Rect,
    path: crate::TreePath,
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
    pub top_prefix: Vec<KeySpec>,
    pub bottom: Vec<KeySpec>,
    pub reorder: Vec<KeySpec>,
}

impl Default for CalendarKeyBindings {
    fn default() -> Self {
        Self {
            month_view: vec![KeySpec::shifted('m')],
            week_view: vec![KeySpec::shifted('w')],
            day_view: vec![KeySpec::shifted('d')],
            toggle_weekends: vec![KeySpec::key_with_modifiers(
                Key::Char('w'),
                KeyModifiers::CONTROL,
            )],
            today: vec![KeySpec::shifted('t')],
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
            top_prefix: vec![KeySpec::plain('g')],
            bottom: vec![KeySpec::shifted('g')],
            reorder: vec![KeySpec::key_with_modifiers(
                Key::Char('m'),
                KeyModifiers::CONTROL,
            )],
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

    pub fn with_top_prefix(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.top_prefix = keys.into_iter().collect();
        self
    }

    pub fn with_bottom(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.bottom = keys.into_iter().collect();
        self
    }

    pub fn reorder(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.reorder = keys.into_iter().collect();
        self
    }
}

impl<T, Id, M: 'static> Calendar<T, Id, M>
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
        let entries = entries.into_iter().collect::<Vec<_>>();
        let keybindings = CalendarKeyBindings::default();
        let mut calendar = Self {
            entries,
            id: Box::new(id),
            span: Box::new(span),
            title: Box::new(title),
            compact_summary_title: None,
            role: Box::new(|_| None),
            event_marker: None,
            render_entry: None,
            day_entry_continuation_indent: None,
            render_detail: None,
            entry_order: None,
            reorder_group: None,
            event_detail_on_activate: false,
            on_event: None,
            view: CalendarView::Month,
            stack: Vec::new(),
            cursor: today,
            today,
            first_day_of_week: Weekday::Monday,
            show_weekends: true,
            bordered: true,
            highlighted_entry: None,
            day_entries: CalendarDayList::new(keybindings.clone()),
            focused: false,
            hotkey: None,
            keybindings,
            pending_top_prefix: false,
            quick_jump_digit: None,
            quick_jump_elapsed: StdDuration::ZERO,
            area: Rect::default(),
            path: crate::TreePath::new(),
            events: Vec::new(),
        };
        calendar.reconcile_day_row_ids();
        calendar.refresh_day_entries();
        calendar
    }

    pub fn today(mut self, today: Date) -> Self {
        self.today = today;
        self.cursor = today;
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
        self.refresh_day_entries();
        self
    }

    pub fn set_today(&mut self, today: Date) {
        let cursor_followed_today = self.cursor == self.today;
        self.today = today;
        if cursor_followed_today {
            self.clear_transient_selection();
            self.cursor = today;
            self.normalize_hidden_weekend_cursor();
            self.highlighted_entry = self.first_entry_on_cursor();
            self.refresh_day_entries();
        }
    }

    pub fn cursor(mut self, cursor: Date) -> Self {
        self.cursor = cursor;
        self.normalize_hidden_weekend_cursor();
        self.highlighted_entry = self.first_entry_on_cursor();
        self.refresh_day_entries();
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

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn set_bordered(&mut self, bordered: bool) {
        self.bordered = bordered;
    }

    pub fn is_bordered(&self) -> bool {
        self.bordered
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
        self.refresh_day_entries();
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
        self.refresh_day_entries();
        self
    }

    /// Wraps Day-view entry text to the available width and grows rows to keep it visible.
    pub fn wrap_day_entries(mut self) -> Self {
        self.set_wrap_day_entries(true);
        self
    }

    pub fn set_wrap_day_entries(&mut self, wrap_cells: bool) {
        self.day_entries.set_wrap_cells(wrap_cells);
    }

    pub fn role(mut self, role: impl Fn(&T) -> Option<CalendarEntryRole> + 'static) -> Self {
        self.role = Box::new(role);
        self.refresh_day_entries();
        self
    }

    pub fn compact_summary_title(
        mut self,
        breakpoint: u16,
        title: impl Fn(&T) -> String + 'static,
    ) -> Self {
        self.compact_summary_title = Some((breakpoint, Box::new(title)));
        self
    }

    pub fn event_marker(mut self, marker: impl Fn(&T) -> char + 'static) -> Self {
        self.set_event_marker(marker);
        self
    }

    pub fn set_event_marker(&mut self, marker: impl Fn(&T) -> char + 'static) {
        self.event_marker = Some(Box::new(marker));
        self.refresh_day_entries();
    }

    pub fn clear_event_marker(&mut self) {
        self.event_marker = None;
        self.refresh_day_entries();
    }

    pub fn render_entry(mut self, render: impl Fn(&T) -> Line<'static> + 'static) -> Self {
        self.render_entry = Some(Box::new(render));
        self.refresh_day_entries();
        self
    }

    /// Indents wrapped Day-view entry text after the rendered entry metadata.
    pub fn day_entry_wrap_continuation_indent_by(
        mut self,
        indent: impl Fn(&T) -> usize + 'static,
    ) -> Self {
        self.day_entry_continuation_indent = Some(Box::new(indent));
        self.refresh_day_entries();
        self
    }

    pub fn render_detail(mut self, render: impl Fn(&T) -> Text<'static> + 'static) -> Self {
        self.render_detail = Some(Box::new(render));
        self
    }

    pub fn entry_order(mut self, compare: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        self.entry_order = Some(Box::new(compare));
        self.refresh_day_entries();
        self
    }

    pub fn reorderable(mut self, group: impl Fn(&T, &T) -> bool + 'static) -> Self {
        self.reorder_group = Some(Box::new(group));
        self.day_entries.set_reorderable(true);
        self.refresh_day_entries();
        self
    }

    pub fn event_detail_on_activate(mut self, enabled: bool) -> Self {
        self.set_event_detail_on_activate(enabled);
        self
    }

    pub fn set_event_detail_on_activate(&mut self, enabled: bool) {
        self.event_detail_on_activate = enabled;
    }

    pub fn is_event_detail_on_activate(&self) -> bool {
        self.event_detail_on_activate
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
        self.set_keybindings(keybindings);
        self
    }

    pub fn set_keybindings(&mut self, keybindings: CalendarKeyBindings) {
        self.day_entries.set_keybindings(keybindings.clone());
        self.keybindings = keybindings;
        self.pending_top_prefix = false;
    }

    pub fn set_entries(&mut self, entries: impl IntoIterator<Item = T>) {
        let highlighted_id = self.highlighted_entry_id();
        self.entries = entries.into_iter().collect();
        self.reconcile_day_row_ids();
        self.highlighted_entry = highlighted_id
            .and_then(|id| {
                self.entries.iter().position(|entry| {
                    (self.id)(entry) == id && (self.span)(entry).covers_date(self.cursor)
                })
            })
            .or_else(|| self.first_entry_on_cursor());
        self.refresh_day_entries();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.day_entries.set_display_focused(focused);
        if !focused {
            self.pending_top_prefix = false;
            self.clear_quick_jump();
        }
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

    pub fn highlight_entry_id(&mut self, entry_id: &Id) -> CalendarOutcome {
        let Some(index) = self
            .entries_on(self.cursor)
            .into_iter()
            .find(|index| (self.id)(&self.entries[*index]) == *entry_id)
        else {
            return CalendarOutcome::IDLE;
        };
        self.highlight_entry(index)
    }

    /// Returns transient Day-view selection IDs in current reorder scope order.
    pub fn transient_selected_ids(&self) -> Vec<Id> {
        self.day_entries.transient_selected_ids()
    }

    pub fn clear_transient_selection(&mut self) {
        self.day_entries.clear_transient_selection();
    }

    pub fn is_reordering(&self) -> bool {
        self.day_entries.is_reordering()
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
        self.clear_day_display_prefix_for_key(key);
        if self.view == CalendarView::Day && !self.day_bubbles_key(key) {
            if matches_key_specs(&self.keybindings.reorder, key)
                && !self.day_entries.is_reordering()
                && !self.day_entries.can_start_reorder()
            {
                return CalendarOutcome::HANDLED;
            }
            let content = self.content_area(self.area);
            self.day_entries.layout(content, &mut LayoutCtx::new());
            let mut ctx = EventCtx::default();
            let (outcome, events) = self.day_entries.event(&TuiEvent::Key(key), &mut ctx);
            let translated = self.translate_day_events(events);
            if outcome.handled() {
                return translated;
            }
        }
        if let Some(outcome) = self.handle_date_quick_jump(key) {
            return outcome;
        }
        if matches_key_specs(&self.keybindings.top_prefix, key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                return self.apply_key_action(CalendarKeyAction::Home);
            }
            self.pending_top_prefix = true;
            return CalendarOutcome::HANDLED;
        }
        self.pending_top_prefix = false;
        if let Some(action) = self.key_action(key) {
            return self.apply_key_action(action);
        }
        CalendarOutcome::IDLE
    }

    fn clear_day_display_prefix_for_key(&mut self, key: KeyEvent) {
        if self.view == CalendarView::Day && !matches_key_specs(&self.keybindings.top_prefix, key) {
            self.day_entries.clear_display_pending_top_prefix();
        }
    }

    fn clear_day_display_prefix_for_event(&mut self, event: &TuiEvent) {
        if !matches!(event, TuiEvent::Key(key) if matches_key_specs(&self.keybindings.top_prefix, *key))
        {
            self.day_entries.clear_display_pending_top_prefix();
        }
    }

    fn day_bubbles_key(&self, key: KeyEvent) -> bool {
        matches!(
            self.key_action(key),
            Some(
                CalendarKeyAction::Month
                    | CalendarKeyAction::Week
                    | CalendarKeyAction::Day
                    | CalendarKeyAction::ToggleWeekends
                    | CalendarKeyAction::Today
                    | CalendarKeyAction::Left
                    | CalendarKeyAction::Right
            )
        )
    }

    fn translate_day_events(&mut self, events: Vec<CalendarDayEvent<Id>>) -> CalendarOutcome {
        let mut outcome = CalendarOutcome::HANDLED;
        for event in events {
            match event {
                CalendarDayEvent::Highlighted(entry_id) => {
                    self.highlighted_entry = entry_id.as_ref().and_then(|entry_id| {
                        self.entries
                            .iter()
                            .position(|entry| (self.id)(entry) == *entry_id)
                    });
                    self.push_event(CalendarTypedEvent::EntryHighlighted { entry_id });
                    outcome = CalendarOutcome::CHANGED;
                }
                CalendarDayEvent::Activated(entry_id) => {
                    self.highlighted_entry = self
                        .entries
                        .iter()
                        .position(|entry| (self.id)(entry) == entry_id);
                    self.push_event(CalendarTypedEvent::EntryActivated { entry_id });
                    if self.event_detail_on_activate {
                        self.drill_to(CalendarView::EventDetail);
                    }
                    outcome = CalendarOutcome::ACTIVATED;
                }
                CalendarDayEvent::Reordered(entry_ids) => {
                    self.push_event(CalendarTypedEvent::EntriesReordered { entry_ids });
                    outcome = CalendarOutcome::CHANGED;
                }
            }
        }
        let highlighted = self.day_entries.highlighted_entry_index();
        if self.highlighted_entry != highlighted {
            self.highlighted_entry = highlighted;
            self.push_event(CalendarTypedEvent::EntryHighlighted {
                entry_id: highlighted.map(|index| (self.id)(&self.entries[index])),
            });
            if outcome == CalendarOutcome::HANDLED {
                outcome = CalendarOutcome::CHANGED;
            }
        }
        outcome
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
        } else if matches_key_specs(&keys.bottom, key) {
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

    fn handle_date_quick_jump(&mut self, key: KeyEvent) -> Option<CalendarOutcome> {
        if !matches!(self.view, CalendarView::Month | CalendarView::Week) {
            return None;
        }
        if let Some(first) = self.quick_jump_digit {
            if quick_jump_accepts(key) {
                self.clear_quick_jump();
                return Some(self.complete_date_quick_jump(first));
            }
            if let Some(second) = plain_digit(key) {
                self.clear_quick_jump();
                let day = first * 10 + second;
                return Some(if day <= self.cursor.month().length(self.cursor.year()) {
                    self.complete_date_quick_jump(day)
                } else {
                    CalendarOutcome::CHANGED
                });
            }
            self.clear_quick_jump();
        }
        let digit = plain_digit(key)?;
        if digit == 0 {
            return Some(CalendarOutcome::HANDLED);
        }
        if self.view == CalendarView::Week
            && let Some(date) = self.unique_week_quick_jump_date(digit)
        {
            return Some(self.complete_quick_jump_date(date));
        }
        if digit <= 3 && digit * 10 <= self.cursor.month().length(self.cursor.year()) {
            self.quick_jump_digit = Some(digit);
            self.quick_jump_elapsed = StdDuration::ZERO;
            return Some(CalendarOutcome::CHANGED);
        }
        Some(self.complete_date_quick_jump(digit))
    }

    fn complete_date_quick_jump(&mut self, day: u8) -> CalendarOutcome {
        let date = self
            .cursor
            .replace_day(day)
            .expect("quick-jump day is valid in cursor month");
        self.complete_quick_jump_date(date)
    }

    fn complete_quick_jump_date(&mut self, date: Date) -> CalendarOutcome {
        self.set_cursor(date);
        self.drill_to(match self.view {
            CalendarView::Month => CalendarView::Week,
            CalendarView::Week => CalendarView::Day,
            CalendarView::Day | CalendarView::EventDetail => {
                unreachable!("quick jumps only apply to month and week views")
            }
        })
    }

    fn click_date(&mut self, mouse: MouseEvent) -> CalendarOutcome {
        let Some(date) = self.date_at_mouse_position(mouse.column, mouse.row) else {
            return CalendarOutcome::IDLE;
        };
        self.set_cursor(date);
        self.drill_to(match self.view {
            CalendarView::Month => CalendarView::Week,
            CalendarView::Week => CalendarView::Day,
            CalendarView::Day | CalendarView::EventDetail => {
                unreachable!("only month and week cells have clickable dates")
            }
        })
    }

    fn unique_week_quick_jump_date(&self, digit: u8) -> Option<Date> {
        let (start, _) = week_range(self.cursor, self.first_day_of_week);
        let mut matches = (0..7)
            .map(|offset| start + Duration::days(offset))
            .filter(|date| self.show_weekends || !is_weekend(*date))
            .filter(|date| date.day() == digit || date.day() / 10 == digit);
        let date = matches.next()?;
        matches.next().is_none().then_some(date)
    }

    fn clear_quick_jump(&mut self) {
        self.quick_jump_digit = None;
        self.quick_jump_elapsed = StdDuration::ZERO;
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
                if self.event_detail_on_activate {
                    self.drill_to(CalendarView::EventDetail).with_activated()
                } else {
                    CalendarOutcome::ACTIVATED
                }
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
        self.clear_quick_jump();
        if self.view == view {
            return CalendarOutcome::HANDLED;
        }
        if self.view == CalendarView::Day {
            self.day_entries.clear_display_pending_top_prefix();
        }
        self.view = view;
        self.normalize_hidden_weekend_cursor();
        if view != CalendarView::EventDetail {
            self.highlight_first_entry_on_cursor();
        }
        self.refresh_day_entries();
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
        self.clear_transient_selection();
        self.cursor = date;
        self.push_event(CalendarTypedEvent::CursorChanged { date });
        if before_range != self.current_range() {
            self.emit_range_changed();
        }
        self.highlight_first_entry_on_cursor();
        self.refresh_day_entries();
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
            CalendarView::Day => CalendarOutcome::HANDLED,
            CalendarView::EventDetail => CalendarOutcome::HANDLED,
            CalendarView::Month | CalendarView::Week => self.move_days(-7),
        }
    }

    fn move_down(&mut self) -> CalendarOutcome {
        match self.view {
            CalendarView::Day => CalendarOutcome::HANDLED,
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

    fn scroll_month(&mut self, delta: i32) -> CalendarOutcome {
        self.set_cursor(add_months(self.cursor, delta))
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
            CalendarView::Day => CalendarOutcome::HANDLED,
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
            CalendarView::Day => CalendarOutcome::HANDLED,
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
        self.refresh_day_entries();
        CalendarOutcome::CHANGED
    }

    fn normalize_hidden_weekend_cursor(&mut self) {
        if self.show_weekends
            || !matches!(self.view, CalendarView::Month | CalendarView::Week)
            || !is_weekend(self.cursor)
        {
            return;
        }
        self.clear_transient_selection();
        self.cursor = previous_friday_if_weekend(self.cursor);
    }

    fn highlight_first_entry_on_cursor(&mut self) {
        let next = self.first_entry_on_cursor();
        self.set_highlighted_entry(next);
    }

    fn first_entry_on_cursor(&self) -> Option<usize> {
        self.entries_on(self.cursor).first().copied()
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
        let entry_id = index.map(|index| (self.id)(&self.entries[index]));
        self.day_entries.set_highlighted(entry_id.as_ref());
        self.push_event(CalendarTypedEvent::EntryHighlighted { entry_id });
    }

    fn emit_range_changed(&mut self) {
        let (start, end) = self.current_range();
        self.push_event(CalendarTypedEvent::RangeChanged { start, end });
    }

    fn push_event(&mut self, event: CalendarTypedEvent<Id>) {
        self.events.push(event);
    }

    fn entries_on(&self, date: Date) -> Vec<usize> {
        self.sorted_entries_on(date)
    }

    fn sorted_entries_on(&self, date: Date) -> Vec<usize> {
        let mut entries = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| (self.span)(entry).covers_date(date).then_some(index))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| self.compare_entries(*left, *right));
        entries
    }

    fn refresh_day_entries(&mut self) {
        let entries = self.entries_on(self.cursor);
        let rows = entries
            .iter()
            .copied()
            .enumerate()
            .map(|(rank, entry_index)| self.day_entry_row(entry_index, rank, &entries))
            .collect::<Vec<_>>();
        self.day_entries.replace_rows(rows);
        self.day_entries.set_display_focused(self.focused);
        if let Some(index) = self.highlighted_entry {
            let entry_id = (self.id)(&self.entries[index]);
            if self.day_entries.highlighted_entry_index() != Some(index) {
                self.day_entries.set_highlighted(Some(&entry_id));
            }
        }
    }

    fn day_entry_row(&self, entry_index: usize, rank: usize, entries: &[usize]) -> CalendarDayRow {
        let entry = &self.entries[entry_index];
        let span = (self.span)(entry);
        let marker = self
            .event_marker
            .as_ref()
            .map(|marker| marker(entry))
            .filter(|marker| !marker.is_control())
            .unwrap_or(if span.all_day { '■' } else { '•' });
        let (time_prefix, marker_prefix) = if span.all_day {
            (None, format!("all-day {marker} "))
        } else {
            (
                Some(format!("{} ", format_time(span.start.time()))),
                format!("{marker} "),
            )
        };
        let prefix = format!(
            "{}{}",
            time_prefix.as_deref().unwrap_or_default(),
            marker_prefix
        );
        let key = self.day_entries.key_for(&(self.id)(entry));
        let continuation_indent = self
            .day_entry_continuation_indent
            .as_ref()
            .map_or(0, |indent| {
                crate::line_width(&Line::from(prefix.clone())) + indent(entry)
            });
        let scope = self.reorder_group.as_ref().map_or_else(Vec::new, |group| {
            entries
                .iter()
                .copied()
                .filter(|candidate| group(entry, &self.entries[*candidate]))
                .map(|candidate| {
                    self.day_entries
                        .key_for(&(self.id)(&self.entries[candidate]))
                })
                .collect()
        });
        CalendarDayRow::new(
            key,
            entry_index,
            rank,
            time_prefix,
            marker_prefix,
            self.entry_line(entry_index),
            continuation_indent,
            (self.role)(entry),
            scope,
        )
    }

    fn reconcile_day_row_ids(&mut self) {
        self.day_entries
            .reconcile_ids(self.entries.iter().map(|entry| (self.id)(entry)));
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
                self.entry_order.as_ref().map_or_else(
                    || (self.title)(&self.entries[left]).cmp(&(self.title)(&self.entries[right])),
                    |compare| compare(&self.entries[left], &self.entries[right]),
                )
            })
    }

    fn entry_line(&self, index: usize) -> Line<'static> {
        if let Some(render_entry) = &self.render_entry {
            return render_entry(&self.entries[index]);
        }
        Line::from((self.title)(&self.entries[index]))
    }

    fn summary_entry_line(&self, index: usize) -> Line<'static> {
        if let Some((breakpoint, title)) = &self.compact_summary_title
            && self.area.width < *breakpoint
        {
            return Line::from(title(&self.entries[index]));
        }
        self.entry_line(index)
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
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(72, 12).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        self.path = ctx.current_path();
        ctx.register_hit_region(crate::HitRegion::new(ctx.current_path(), area));
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
        if self.view == CalendarView::Day {
            let content = self.content_area(area);
            ctx.push_slot(ChildKey::new(DAY_ENTRIES_SLOT), content, |ctx| {
                self.day_entries.layout(content, ctx);
            });
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if self.view == CalendarView::Day {
            self.render_day_node(frame, area, ctx);
        } else {
            Self::render(self, frame, area);
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.cursor_date().to_string());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if matches!(event, TuiEvent::Mouse(_)) {
            self.pending_top_prefix = false;
            self.day_entries.clear_display_pending_top_prefix();
        }
        let event_start = self.events.len();
        let had_day_entries_child = self.view == CalendarView::Day;
        let outcome = match event {
            TuiEvent::Mouse(mouse)
                if self.view != CalendarView::Day
                    && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) =>
            {
                self.click_date(*mouse)
            }
            TuiEvent::Mouse(mouse) if self.view == CalendarView::Day => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_month(-1),
                MouseEventKind::ScrollDown => self.scroll_month(1),
                _ => {
                    self.clear_day_display_prefix_for_event(event);
                    let (outcome, events) = self.day_entries.event(event, ctx);
                    let translated = self.translate_day_events(events);
                    if outcome.handled() {
                        translated
                    } else {
                        return EventOutcome::Ignored;
                    }
                }
            },
            TuiEvent::Key(key) if self.view == CalendarView::Day => {
                if self.day_bubbles_key(*key)
                    || matches_key_specs(&self.keybindings.reorder, *key)
                        && !self.day_entries.is_reordering()
                        && !self.day_entries.can_start_reorder()
                {
                    self.on_key(*key)
                } else {
                    self.clear_day_display_prefix_for_event(event);
                    let (outcome, events) = self.day_entries.event(event, ctx);
                    let translated = self.translate_day_events(events);
                    if outcome.handled() {
                        translated
                    } else {
                        self.on_key(*key)
                    }
                }
            }
            TuiEvent::Key(key) => self.on_key(*key),
            _ => return EventOutcome::Ignored,
        };
        if let Some(on_event) = &self.on_event {
            let events = self.events.drain(event_start..).collect::<Vec<_>>();
            for event in events {
                ctx.emit(on_event(event));
            }
        }
        if had_day_entries_child != (self.view == CalendarView::Day) {
            ctx.request_layout();
        }
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled {
            if matches!(event, TuiEvent::Mouse(_)) {
                self.focus_self(ctx);
            }
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

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if let Some(path) = route
            .path
            .without_first_if(&ChildKey::new(DAY_ENTRIES_SLOT))
        {
            if self.view != CalendarView::Day {
                return EventOutcome::Ignored;
            }
            if matches!(
                event,
                TuiEvent::Mouse(MouseEvent {
                    kind: MouseEventKind::ScrollUp | MouseEventKind::ScrollDown,
                    ..
                })
            ) {
                return self.event(event, ctx);
            }
            let event_start = self.events.len();
            self.clear_day_display_prefix_for_event(event);
            let (outcome, events) = if path.is_empty() {
                self.day_entries.event(event, ctx)
            } else {
                self.day_entries
                    .dispatch_event(&EventRoute::new(path), event, ctx)
            };
            let translated = self.translate_day_events(events);
            if outcome.handled() {
                if let Some(on_event) = &self.on_event {
                    let events = self.events.drain(event_start..).collect::<Vec<_>>();
                    for event in events {
                        ctx.emit(on_event(event));
                    }
                }
                if translated.needs_redraw() {
                    ctx.request_redraw();
                }
                if self.view != CalendarView::Day {
                    ctx.request_layout();
                }
                if matches!(event, TuiEvent::Mouse(_)) {
                    self.focus_self(ctx);
                }
                return EventOutcome::Handled;
            }
            if let TuiEvent::Key(key) = event {
                return self.event(&TuiEvent::Key(*key), ctx);
            }
            return EventOutcome::Ignored;
        }
        if route.path.is_empty() {
            self.event(event, ctx)
        } else {
            EventOutcome::Ignored
        }
    }

    fn tick(&mut self, dt: StdDuration, settings: crate::AnimationSettings) -> TickResult {
        let mut result = if self.quick_jump_digit.is_none() {
            TickResult::IDLE
        } else {
            self.quick_jump_elapsed = self.quick_jump_elapsed.saturating_add(dt);
            if self.quick_jump_elapsed >= QUICK_JUMP_TIMEOUT {
                self.clear_quick_jump();
                TickResult::CHANGED
            } else {
                TickResult::scheduled_after(QUICK_JUMP_TIMEOUT - self.quick_jump_elapsed)
            }
        };
        if self.view == CalendarView::Day {
            result = result.merge(self.day_entries.tick(dt, settings));
        }
        result
    }

    fn init(&mut self, ctx: &mut crate::LifecycleCtx<M>) {
        self.day_entries.init(ctx);
    }

    fn mount(&mut self, ctx: &mut crate::LifecycleCtx<M>) {
        self.day_entries.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut crate::LifecycleCtx<M>) {
        self.day_entries.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut crate::LifecycleCtx<M>) {
        self.day_entries.destroy(ctx);
    }
}

impl<T, Id, M: 'static> Calendar<T, Id, M>
where
    Id: Clone + Eq,
{
    fn focus_self(&self, ctx: &mut EventCtx<M>) {
        ctx.focus(crate::FocusRequest::TargetAt {
            path: self.path.clone(),
            id: FocusId::new(CALENDAR_FOCUS),
        });
    }
}

fn quick_jump_accepts(key: KeyEvent) -> bool {
    matches!(key.code, Key::Enter | Key::Char(' ')) && key.modifiers.is_empty()
}

fn plain_digit(key: KeyEvent) -> Option<u8> {
    if !key.modifiers.is_empty() {
        return None;
    }
    let Key::Char(character) = key.code else {
        return None;
    };
    character.to_digit(10).map(|digit| digit as u8)
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

fn calendar_content_width(viewport_width: u16, column_count: usize) -> u16 {
    viewport_width
        .max(MIN_CALENDAR_CELL_WIDTH.saturating_mul(column_count.min(u16::MAX as usize) as u16))
}

fn blit_horizontal_viewport(
    frame: &mut Frame,
    source: &Buffer,
    viewport: Rect,
    horizontal_offset: u16,
) {
    for y_offset in 0..viewport.height {
        for x_offset in 0..viewport.width {
            let source_position = (horizontal_offset + x_offset, y_offset);
            let target_position = (viewport.x + x_offset, viewport.y + y_offset);
            if let (Some(source_cell), Some(target_cell)) = (
                source.cell(source_position),
                frame.buffer_mut().cell_mut(target_position),
            ) {
                *target_cell = source_cell.clone();
            }
        }
    }
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

fn grid_cell_inner(area: Rect, reserve_top_line: bool, reserve_left_line: bool) -> Rect {
    let left = u16::from(reserve_left_line);
    let x = area.x.saturating_add(left);
    let y = area.y.saturating_add(u16::from(reserve_top_line));
    let width = area.width.saturating_sub(left);
    let height = area.height.saturating_sub(u16::from(reserve_top_line));
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests;
