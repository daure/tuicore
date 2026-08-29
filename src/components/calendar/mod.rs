use std::cmp::Ordering;
use std::time::Duration as StdDuration;

use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::{Frame, buffer::Buffer};
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

use crate::event::{
    Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TuiEvent,
};
use crate::{
    Animated, EventCtx, EventOutcome, FocusCtx, FocusId, KeySpec, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, ScrollAxes, ScrollOffset, ScrollSize, ScrollState, TickResult,
    TuiNode, animation_settings, preset, theme,
};

use super::{Column, DataView, Panel, SelectionMode};
use crate::components::data_view::SelectionOverlayPosition;
use crate::components::ordered_selection::OrderedSelection;

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
type DetailRenderFn<T> = dyn Fn(&T) -> Text<'static>;
type EntryOrderFn<T> = dyn Fn(&T, &T) -> Ordering;
type ReorderGroupFn<T> = dyn Fn(&T, &T) -> bool;

struct CalendarReorderState {
    selected_entries: Vec<usize>,
    source: Vec<usize>,
    staged: Vec<usize>,
    target_index: usize,
    pending_top_prefix: bool,
}

#[derive(Clone)]
struct CalendarDayRow {
    entry_index: usize,
    prefix: String,
    entry: Line<'static>,
    role: Option<CalendarEntryRole>,
}

fn day_entry_data_view() -> DataView<CalendarDayRow, usize> {
    DataView::new([], |row: &CalendarDayRow| row.entry_index)
        .selection_mode(SelectionMode::Single)
        .column(Column::rich(
            "entry",
            "",
            Constraint::Percentage(100),
            |row: &CalendarDayRow, _| {
                let body_style = calendar_entry_style(row.role, false);
                let marker_style = Style::default().fg(theme().accent_fg());
                let mut spans = vec![Span::styled(row.prefix.clone(), marker_style)];
                spans.extend(row.entry.spans.clone());
                Line {
                    spans,
                    style: row.entry.style.patch(body_style),
                    alignment: row.entry.alignment,
                }
            },
        ))
        .headers(false)
        .filter_controls(false)
        .empty_message("No entries")
}

fn calendar_entry_style(role: Option<CalendarEntryRole>, selected: bool) -> Style {
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
    render_detail: Option<Box<DetailRenderFn<T>>>,
    entry_order: Option<Box<EntryOrderFn<T>>>,
    reorder_group: Option<Box<ReorderGroupFn<T>>>,
    reordering: Option<CalendarReorderState>,
    day_selection: Option<OrderedSelection<Id>>,
    committed_reorder: Option<Vec<usize>>,
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
    day_entries: DataView<CalendarDayRow, usize>,
    focused: bool,
    hotkey: Option<String>,
    keybindings: CalendarKeyBindings,
    pending_top_prefix: bool,
    quick_jump_digit: Option<u8>,
    quick_jump_elapsed: StdDuration,
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
        let mut calendar = Self {
            entries: entries.into_iter().collect(),
            id: Box::new(id),
            span: Box::new(span),
            title: Box::new(title),
            compact_summary_title: None,
            role: Box::new(|_| None),
            event_marker: None,
            render_entry: None,
            render_detail: None,
            entry_order: None,
            reorder_group: None,
            reordering: None,
            day_selection: None,
            committed_reorder: None,
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
            day_entries: day_entry_data_view(),
            focused: false,
            hotkey: None,
            keybindings: CalendarKeyBindings::default(),
            pending_top_prefix: false,
            quick_jump_digit: None,
            quick_jump_elapsed: StdDuration::ZERO,
            area: Rect::default(),
            events: Vec::new(),
        };
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
            self.clear_day_selection();
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
        self.keybindings = keybindings;
        self.pending_top_prefix = false;
    }

    pub fn set_entries(&mut self, entries: impl IntoIterator<Item = T>) {
        self.committed_reorder = None;
        self.day_entries.clear_reorder_highlight_immediately();
        self.cancel_reorder_immediately();
        let highlighted_id = self.highlighted_entry_id();
        self.entries = entries.into_iter().collect();
        self.highlighted_entry = highlighted_id
            .and_then(|id| {
                self.entries.iter().position(|entry| {
                    (self.id)(entry) == id && (self.span)(entry).covers_date(self.cursor)
                })
            })
            .or_else(|| self.first_entry_on_cursor());
        self.reconcile_day_selection();
        self.refresh_day_entries();
        if self.day_selection.is_some() {
            self.day_entries
                .set_selection_overlay(self.selected_day_entries(), None, 0, false);
        } else {
            self.day_entries.clear_selection_overlay();
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.day_entries.set_focused(focused);
        if !focused {
            self.cancel_reorder_immediately();
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
        self.day_selection
            .as_ref()
            .map(|selection| selection.selected.clone())
            .unwrap_or_default()
    }

    pub fn clear_transient_selection(&mut self) {
        self.clear_day_selection();
    }

    pub fn is_reordering(&self) -> bool {
        self.reordering.is_some()
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
        if let Some(outcome) = self.handle_reorder_key(key) {
            return outcome;
        }
        if let Some(outcome) = self.handle_day_selection_key(key) {
            return outcome;
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
            if self.view == CalendarView::Day
                && let Some(outcome) = self.apply_day_data_view_action(action)
            {
                return outcome;
            }
            return self.apply_key_action(action);
        }
        CalendarOutcome::IDLE
    }

    fn handle_reorder_key(&mut self, key: KeyEvent) -> Option<CalendarOutcome> {
        if self.reordering.is_none() {
            if !matches_key_specs(&self.keybindings.reorder, key)
                || self.view != CalendarView::Day
                || self.reorder_group.is_none()
            {
                return None;
            }
            self.begin_reorder();
            return Some(CalendarOutcome::HANDLED);
        }

        let top_prefix = matches_key_specs(&self.keybindings.top_prefix, key);
        if !top_prefix && let Some(state) = &mut self.reordering {
            state.pending_top_prefix = false;
        }
        let outcome = if matches_key_specs(&self.keybindings.reorder, key)
            || matches!(key.code, Key::Enter | Key::Char(' '))
                && key.modifiers == KeyModifiers::NONE
        {
            self.commit_reorder()
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_reorder()
        } else if matches_key_specs(&self.keybindings.up, key) {
            self.move_reorder(-1)
        } else if matches_key_specs(&self.keybindings.down, key) {
            self.move_reorder(1)
        } else if matches_key_specs(&self.keybindings.page_up, key) {
            let page = self
                .day_entries
                .visible_page_step(self.content_area(self.area));
            self.move_reorder(-(page as isize))
        } else if matches_key_specs(&self.keybindings.page_down, key) {
            let page = self
                .day_entries
                .visible_page_step(self.content_area(self.area));
            self.move_reorder(page as isize)
        } else if matches_key_specs(&self.keybindings.home, key) {
            self.move_reorder_to(0)
        } else if matches_key_specs(&self.keybindings.end, key)
            || matches_key_specs(&self.keybindings.bottom, key)
        {
            self.move_reorder_to(usize::MAX)
        } else if top_prefix {
            self.handle_reorder_top_prefix()
        } else {
            CalendarOutcome::HANDLED
        };
        Some(outcome)
    }

    fn begin_reorder(&mut self) {
        let Some(moving_entry) = self.highlighted_entry else {
            return;
        };
        let Some(group) = &self.reorder_group else {
            return;
        };
        let source = self
            .entries_on(self.cursor)
            .into_iter()
            .filter(|entry| group(&self.entries[moving_entry], &self.entries[*entry]))
            .collect::<Vec<_>>();
        if source.len() < 2 {
            return;
        }
        let selected_entries = self
            .day_selection
            .as_ref()
            .map(|selection| {
                source
                    .iter()
                    .filter(|entry| {
                        selection
                            .selected
                            .contains(&(self.id)(&self.entries[**entry]))
                    })
                    .copied()
                    .collect::<Vec<_>>()
            })
            .filter(|selected| selected.len() >= 2)
            .unwrap_or_else(|| vec![moving_entry]);
        let target_index = source
            .iter()
            .position(|entry| entry == &selected_entries[0])
            .map(|index| {
                source[..index]
                    .iter()
                    .filter(|entry| !selected_entries.contains(entry))
                    .count()
            })
            .expect("selected calendar entry belongs to reorder scope");
        let block_move = selected_entries.len() >= 2;
        self.committed_reorder = None;
        if !block_move {
            self.day_entries
                .start_reorder_highlight(moving_entry, animation_settings());
        }
        self.reordering = Some(CalendarReorderState {
            selected_entries,
            staged: source.clone(),
            source,
            target_index,
            pending_top_prefix: false,
        });
        self.day_selection = None;
        if block_move {
            self.day_entries.clear_selection();
            self.day_entries.set_selection_overlay(
                self.reordering
                    .as_ref()
                    .expect("calendar reorder state is active")
                    .selected_entries
                    .clone(),
                Some(SelectionOverlayPosition::After(
                    *self
                        .reordering
                        .as_ref()
                        .expect("calendar reorder state is active")
                        .selected_entries
                        .last()
                        .expect("calendar block selection is not empty"),
                )),
                0,
                true,
            );
        }
    }

    fn move_reorder(&mut self, delta: isize) -> CalendarOutcome {
        let Some(state) = &self.reordering else {
            return CalendarOutcome::HANDLED;
        };
        let target = state.target_index.saturating_add_signed(delta);
        self.move_reorder_to(target)
    }

    fn move_reorder_to(&mut self, target: usize) -> CalendarOutcome {
        let Some(state) = &mut self.reordering else {
            return CalendarOutcome::HANDLED;
        };
        let remaining = state
            .source
            .iter()
            .filter(|entry| !state.selected_entries.contains(entry))
            .copied()
            .collect::<Vec<_>>();
        let target = target.min(remaining.len());
        if target == state.target_index {
            return CalendarOutcome::HANDLED;
        }
        let mut staged = remaining;
        staged.splice(target..target, state.selected_entries.iter().copied());
        state.target_index = target;
        state.staged = staged;
        if state.selected_entries.len() >= 2 {
            self.update_reorder_overlay();
            self.position_reorder_placeholder();
        } else {
            self.refresh_day_entries();
        }
        CalendarOutcome::CHANGED
    }

    fn handle_reorder_top_prefix(&mut self) -> CalendarOutcome {
        let Some(state) = &mut self.reordering else {
            return CalendarOutcome::HANDLED;
        };
        if !state.pending_top_prefix {
            state.pending_top_prefix = true;
            return CalendarOutcome::HANDLED;
        }
        state.pending_top_prefix = false;
        self.move_reorder_to(0)
    }

    fn commit_reorder(&mut self) -> CalendarOutcome {
        let Some(state) = self.reordering.take() else {
            return CalendarOutcome::HANDLED;
        };
        if state.selected_entries.len() == 1 {
            self.day_entries
                .clear_reorder_highlight(animation_settings());
        }
        self.day_entries.clear_selection_overlay();
        let changed = state.source != state.staged;
        self.committed_reorder = changed.then(|| state.staged.clone());
        self.refresh_day_entries();
        if changed {
            self.push_event(CalendarTypedEvent::EntriesReordered {
                entry_ids: state
                    .staged
                    .into_iter()
                    .map(|entry| (self.id)(&self.entries[entry]))
                    .collect(),
            });
        }
        CalendarOutcome::CHANGED
    }

    fn cancel_reorder(&mut self) -> CalendarOutcome {
        let Some(state) = self.reordering.take() else {
            return CalendarOutcome::HANDLED;
        };
        if state.selected_entries.len() == 1 {
            self.day_entries
                .clear_reorder_highlight(animation_settings());
        }
        self.day_entries.clear_selection_overlay();
        self.refresh_day_entries();
        CalendarOutcome::CHANGED
    }

    fn cancel_reorder_immediately(&mut self) {
        if let Some(state) = self.reordering.take() {
            if state.selected_entries.len() == 1 {
                self.day_entries.clear_reorder_highlight_immediately();
            }
            self.day_entries.clear_selection_overlay();
            self.refresh_day_entries();
        }
    }

    fn handle_day_selection_key(&mut self, key: KeyEvent) -> Option<CalendarOutcome> {
        if self.view != CalendarView::Day || self.reorder_group.is_none() {
            return None;
        }
        if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            if self.day_selection.is_some() {
                self.clear_day_selection();
                return Some(CalendarOutcome::CHANGED);
            }
            return None;
        }
        let shift = key.modifiers == KeyModifiers::SHIFT;
        let control = key.modifiers == KeyModifiers::CONTROL;
        if key.code == Key::Char(' ')
            && (control
                || key.modifiers == KeyModifiers::NONE
                    && self
                        .day_selection
                        .as_ref()
                        .is_some_and(|selection| !selection.range_mode))
        {
            self.toggle_day_selection_at_highlight();
            return Some(CalendarOutcome::CHANGED);
        }
        let direction = self.day_selection_direction(key, shift || control);
        let extends_range = shift && direction.is_some();
        if self
            .day_selection
            .as_ref()
            .is_some_and(|selection| selection.range_mode)
            && direction.is_some()
            && !control
            && !extends_range
        {
            self.clear_day_selection();
            return None;
        }
        if !shift && !control {
            return None;
        }
        let Some(delta) = direction else {
            return None;
        };
        let Some(current) = self.highlighted_entry else {
            return None;
        };
        let scope = self.default_reorder_scope(&current);
        let current_index = scope.iter().position(|entry| *entry == current)?;
        let destination_index = current_index
            .saturating_add_signed(delta)
            .min(scope.len().saturating_sub(1));
        let destination = scope[destination_index];
        let scope_ids = self.entry_ids(&scope);
        let current_id = (self.id)(&self.entries[current]);
        let destination_id = (self.id)(&self.entries[destination]);
        let selection = self.day_selection.get_or_insert_with(|| OrderedSelection {
            selected: Vec::new(),
            anchor: current_id.clone(),
            range_mode: shift,
        });
        if shift {
            selection.extend_range(&scope_ids, &current_id, &destination_id);
        } else {
            selection.move_with_control(current_id);
        }
        self.day_entries
            .set_selection_overlay(self.selected_day_entries(), None, 0, false);
        self.set_highlighted_entry(Some(destination));
        Some(CalendarOutcome::CHANGED)
    }

    fn day_selection_direction(&self, mut key: KeyEvent, modified: bool) -> Option<isize> {
        if modified {
            key.modifiers = KeyModifiers::NONE;
            if let Key::Char(character) = key.code {
                key.code = Key::Char(character.to_ascii_lowercase());
            }
        }
        if matches_key_specs(&self.keybindings.up, key) {
            Some(-1)
        } else if matches_key_specs(&self.keybindings.down, key) {
            Some(1)
        } else {
            None
        }
    }

    fn clear_day_selection(&mut self) {
        self.day_selection = None;
        self.day_entries.clear_selection_overlay();
    }

    fn reconcile_day_selection(&mut self) {
        let Some(selection) = self.day_selection.as_ref() else {
            return;
        };
        let entries_on_cursor = self.entries_on(self.cursor);
        let Some(anchor) = entries_on_cursor
            .iter()
            .copied()
            .find(|entry| (self.id)(&self.entries[*entry]) == selection.anchor)
            .or_else(|| {
                entries_on_cursor.iter().copied().find(|entry| {
                    selection
                        .selected
                        .contains(&(self.id)(&self.entries[*entry]))
                })
            })
        else {
            self.day_selection = None;
            return;
        };
        let scope_ids = self.entry_ids(&self.default_reorder_scope(&anchor));
        if !self
            .day_selection
            .as_mut()
            .expect("day selection remains active")
            .reconcile(&scope_ids)
        {
            self.day_selection = None;
        }
    }

    fn toggle_day_selection_at_highlight(&mut self) {
        let Some(current) = self.highlighted_entry else {
            return;
        };
        let scope = self.default_reorder_scope(&current);
        let scope_ids = self.entry_ids(&scope);
        let current_id = (self.id)(&self.entries[current]);
        let selected = {
            let selection = self.day_selection.get_or_insert_with(|| OrderedSelection {
                selected: Vec::new(),
                anchor: current_id.clone(),
                range_mode: false,
            });
            selection.toggle(&scope_ids, current_id)
        };
        if selected.is_empty() {
            self.clear_day_selection();
        } else {
            self.day_entries
                .set_selection_overlay(self.selected_day_entries(), None, 0, false);
        }
    }

    fn update_reorder_overlay(&mut self) {
        let Some(state) = self.reordering.as_ref() else {
            return;
        };
        let remaining = state
            .source
            .iter()
            .filter(|entry| !state.selected_entries.contains(entry))
            .copied()
            .collect::<Vec<_>>();
        let position = state
            .target_index
            .checked_sub(1)
            .and_then(|index| remaining.get(index))
            .copied()
            .map(SelectionOverlayPosition::After)
            .or_else(|| {
                remaining
                    .first()
                    .copied()
                    .map(SelectionOverlayPosition::Before)
            })
            .unwrap_or_else(|| {
                SelectionOverlayPosition::After(
                    *state
                        .selected_entries
                        .last()
                        .expect("calendar reorder selection is not empty"),
                )
            });
        self.day_entries.set_selection_overlay(
            state.selected_entries.clone(),
            Some(position),
            0,
            true,
        );
    }

    fn position_reorder_placeholder(&mut self) {
        let mut settings = animation_settings();
        settings.enabled = false;
        self.day_entries
            .ensure_selection_placeholder_visible(self.content_area(self.area), settings);
    }

    fn default_reorder_scope(&self, moving_entry: &usize) -> Vec<usize> {
        let Some(group) = &self.reorder_group else {
            return Vec::new();
        };
        self.sorted_entries_on(self.cursor)
            .into_iter()
            .filter(|entry| group(&self.entries[*moving_entry], &self.entries[*entry]))
            .collect()
    }

    fn entry_ids(&self, entries: &[usize]) -> Vec<Id> {
        entries
            .iter()
            .map(|entry| (self.id)(&self.entries[*entry]))
            .collect()
    }

    fn selected_day_entries(&self) -> Vec<usize> {
        let Some(selection) = self.day_selection.as_ref() else {
            return Vec::new();
        };
        self.entries_on(self.cursor)
            .into_iter()
            .filter(|entry| {
                selection
                    .selected
                    .contains(&(self.id)(&self.entries[*entry]))
            })
            .collect()
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

    fn apply_day_data_view_action(&mut self, action: CalendarKeyAction) -> Option<CalendarOutcome> {
        if !matches!(
            action,
            CalendarKeyAction::Up
                | CalendarKeyAction::Down
                | CalendarKeyAction::PageUp
                | CalendarKeyAction::PageDown
                | CalendarKeyAction::Home
                | CalendarKeyAction::End
                | CalendarKeyAction::Activate
        ) {
            return None;
        }
        if action == CalendarKeyAction::Activate {
            return Some(self.activate());
        }
        let rows = self.day_entries.rows();
        if rows.is_empty() {
            return Some(CalendarOutcome::HANDLED);
        }
        let current = self
            .highlighted_entry
            .and_then(|highlighted| rows.iter().position(|row| row.entry_index == highlighted))
            .unwrap_or(0);
        let viewport = self.content_area(self.area);
        let page = self.day_entries.visible_page_step(viewport);
        let target = match action {
            CalendarKeyAction::Up => current.saturating_sub(1),
            CalendarKeyAction::Down => current.saturating_add(1),
            CalendarKeyAction::PageUp => current.saturating_sub(page),
            CalendarKeyAction::PageDown => current.saturating_add(page),
            CalendarKeyAction::Home => 0,
            CalendarKeyAction::End => rows.len().saturating_sub(1),
            _ => unreachable!("filtered to day DataView actions"),
        };
        let outcome = if matches!(action, CalendarKeyAction::Up | CalendarKeyAction::Down) {
            self.day_entries
                .highlight_line_with_settings(target, viewport, animation_settings())
        } else {
            self.day_entries.highlight_centered_with_settings(
                target,
                viewport,
                animation_settings(),
            )
        };
        self.day_entries.take_events();
        let before = self.highlighted_entry;
        self.set_highlighted_entry(self.day_entries.highlighted_id());
        Some(if outcome.changed || self.highlighted_entry != before {
            CalendarOutcome::CHANGED
        } else {
            CalendarOutcome::HANDLED
        })
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
        self.clear_day_selection();
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
            CalendarView::Day => self
                .apply_day_data_view_action(CalendarKeyAction::Home)
                .expect("home is a day DataView action"),
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
            CalendarView::Day => self
                .apply_day_data_view_action(CalendarKeyAction::End)
                .expect("end is a day DataView action"),
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
        self.clear_day_selection();
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
        if let Some(index) = index {
            self.day_entries.highlight_id(&index);
            if self.day_selection.is_some() || self.reordering.is_some() {
                self.day_entries.clear_selection();
            } else {
                self.day_entries.select_id(index);
            }
        } else {
            self.day_entries.clear_selection();
        }
        self.day_entries.take_events();
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
        let mut entries = self.sorted_entries_on(date);
        self.apply_staged_reorder(&mut entries);
        entries
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

    fn apply_staged_reorder(&self, entries: &mut [usize]) {
        let Some(staged) = self
            .reordering
            .as_ref()
            .filter(|state| state.selected_entries.len() == 1)
            .map(|state| &state.staged)
            .or(self.committed_reorder.as_ref())
        else {
            return;
        };
        let positions = entries
            .iter()
            .enumerate()
            .filter_map(|(position, entry)| staged.contains(entry).then_some(position))
            .collect::<Vec<_>>();
        if positions.len() != staged.len() {
            return;
        }
        for (position, entry) in positions.into_iter().zip(staged) {
            entries[position] = *entry;
        }
    }

    fn refresh_day_entries(&mut self) {
        let rows = self
            .entries_on(self.cursor)
            .into_iter()
            .map(|entry_index| self.day_entry_row(entry_index))
            .collect::<Vec<_>>();
        self.day_entries.set_rows(rows);
        self.day_entries.set_focused(self.focused);
        if let Some(index) = self.highlighted_entry {
            self.day_entries.highlight_id(&index);
            if self.day_selection.is_some() || self.reordering.is_some() {
                self.day_entries.clear_selection();
            } else {
                self.day_entries.select_id(index);
            }
        }
        self.day_entries
            .snap_highlight_centered(self.content_area(self.area));
        self.day_entries.take_events();
    }

    fn day_entry_row(&self, entry_index: usize) -> CalendarDayRow {
        let entry = &self.entries[entry_index];
        let span = (self.span)(entry);
        let marker = self
            .event_marker
            .as_ref()
            .map(|marker| marker(entry))
            .filter(|marker| !marker.is_control())
            .unwrap_or(if span.all_day { '■' } else { '•' });
        let prefix = if span.all_day {
            format!("{marker} all-day ")
        } else {
            format!("{marker} {} ", format_time(span.start.time()))
        };
        CalendarDayRow {
            entry_index,
            prefix,
            entry: self.entry_line(entry_index),
            role: (self.role)(entry),
        }
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
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(72, 12).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
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
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.cursor_date().to_string());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let event_start = self.events.len();
        let outcome = match event {
            TuiEvent::Mouse(mouse)
                if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) =>
            {
                self.click_date(*mouse)
            }
            TuiEvent::Mouse(mouse) if self.view == CalendarView::Day => match mouse.kind {
                MouseEventKind::ScrollUp => self.scroll_month(-1),
                MouseEventKind::ScrollDown => self.scroll_month(1),
                _ => return EventOutcome::Ignored,
            },
            TuiEvent::Key(key) => self.on_key(*key),
            _ => return EventOutcome::Ignored,
        };
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
            if matches!(event, TuiEvent::Mouse(_)) {
                ctx.focus(crate::FocusRequest::TargetAt {
                    path: ctx.current_path(),
                    id: FocusId::new(CALENDAR_FOCUS),
                });
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
            result = result.merge(Animated::tick(&mut self.day_entries, dt, settings));
        }
        result
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
