use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border::Set;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::components::{Column, DataView, SelectionMode, TextInput};
use crate::event::{Key, KeyEvent};
use crate::search::{SearchMode, search_ranked};
use crate::{
    Animated, AnimationSettings, BorderKind, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRequest, FocusTarget, HintSource, HotkeyEvent, HotkeyLabelMode, HotkeyMatch,
    HotkeySequenceMatcher, KeySpec, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize,
    LayoutSizeHint, TickResult, TreePath, TuiEvent, TuiNode, border_chars, border_set,
    hotkey_badge_width, hotkey_edge_spans, hotkey_label_spans, hotkey_sequence_to_event,
    hotkey_underline_style, keybindings, line_width, preset, theme,
};

use super::text_input::{CursorFade, placeholder_line};

const DROPDOWN_BACKDROP_AMOUNT: f64 = 0.55;

const FIELD_FOCUS: &str = "field";
const SEARCH_FOCUS: &str = "input";
const POPUP_BORDER_HEIGHT: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownFocusRegion {
    Field,
    Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownSearchMode {
    None,
    Contains,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownCommitMode {
    Explicit,
    Immediate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownVariant {
    #[default]
    Bordered,
    Filled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownLabelPosition {
    #[default]
    Top,
    Inline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DropdownPopupDirection {
    #[default]
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropdownActionKeys {
    pub open: Vec<KeySpec>,
    pub commit: Vec<KeySpec>,
    pub toggle: Vec<KeySpec>,
}

impl Default for DropdownActionKeys {
    fn default() -> Self {
        Self {
            open: vec![KeySpec::key(Key::Enter), KeySpec::plain(' ')],
            commit: vec![KeySpec::key(Key::Enter)],
            toggle: vec![KeySpec::plain(' ')],
        }
    }
}

impl DropdownActionKeys {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropdownOutcome {
    pub handled: bool,
    pub changed: bool,
    pub opened: bool,
    pub closed: bool,
    pub committed: bool,
    pub canceled: bool,
}

impl DropdownOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        opened: false,
        closed: false,
        committed: false,
        canceled: false,
    };

    fn changed() -> Self {
        Self {
            handled: true,
            changed: true,
            ..Self::IDLE
        }
    }
}

pub struct Dropdown<T, Id> {
    data_view: DataView<T, Id>,
    search_input: TextInput,
    ids: Vec<Id>,
    labels: Vec<String>,
    committed: Vec<Id>,
    opened_committed: Vec<Id>,
    draft: Vec<Id>,
    filtered: Vec<Id>,
    multi: bool,
    open: bool,
    search_mode: DropdownSearchMode,
    commit_mode: DropdownCommitMode,
    close_on_select: bool,
    max_popup_height: Option<u16>,
    auto_focus_search: bool,
    placeholder: String,
    variant: DropdownVariant,
    popup_direction: DropdownPopupDirection,
    centered: bool,
    field_area: Rect,
    focus_path: TreePath,
    overlay_bounds: Rect,
    focus_region: Option<DropdownFocusRegion>,
    label: Option<String>,
    hotkey: Option<String>,
    hotkey_matcher: HotkeySequenceMatcher,
    tab_stop: bool,
    alt_style: bool,
    label_position: DropdownLabelPosition,
    no_selection_text: Option<String>,
    no_selection_highlighted: bool,
    field_cursor_fade: CursorFade,
    pending_hotkey_prefix: Option<String>,
    action_keys: DropdownActionKeys,
}

impl<T, Id> Dropdown<T, Id>
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    pub fn single(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self::new(rows, row_id, label, false)
    }

    pub fn multi(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self::new(rows, row_id, label, true)
    }

    fn new(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
        multi: bool,
    ) -> Self {
        let rows = rows.into_iter().collect::<Vec<_>>();
        let row_id = Rc::new(row_id);
        let label = Rc::new(label);
        let ids = rows.iter().map(|row| row_id(row)).collect::<Vec<_>>();
        let labels = rows.iter().map(|row| label(row)).collect::<Vec<_>>();
        let data_view_row_id = Rc::clone(&row_id);
        let data_view_label = Rc::clone(&label);
        let selection_mode = if multi {
            SelectionMode::Multi
        } else {
            SelectionMode::Single
        };
        let data_view = DataView::new(rows, move |row| data_view_row_id(row))
            .column(Column::text(
                "label",
                "",
                Constraint::Percentage(100),
                move |row| data_view_label(row),
            ))
            .selection_mode(selection_mode)
            .focused(false);

        Self {
            data_view,
            search_input: TextInput::new().placeholder("Search..."),
            filtered: ids.clone(),
            ids,
            labels,
            committed: Vec::new(),
            opened_committed: Vec::new(),
            draft: Vec::new(),
            multi,
            open: false,
            search_mode: DropdownSearchMode::Fuzzy,
            commit_mode: DropdownCommitMode::Explicit,
            close_on_select: !multi,
            max_popup_height: None,
            auto_focus_search: true,
            placeholder: String::from("Select..."),
            variant: DropdownVariant::Bordered,
            popup_direction: DropdownPopupDirection::Down,
            centered: false,
            field_area: Rect::default(),
            focus_path: TreePath::default(),
            overlay_bounds: Rect::default(),
            focus_region: None,
            label: None,
            hotkey: None,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            tab_stop: true,
            alt_style: false,
            label_position: DropdownLabelPosition::Top,
            no_selection_text: None,
            no_selection_highlighted: false,
            field_cursor_fade: CursorFade::default(),
            pending_hotkey_prefix: None,
            action_keys: DropdownActionKeys::default(),
        }
    }

    pub fn search_mode(mut self, mode: DropdownSearchMode) -> Self {
        self.search_mode = mode;
        self.refresh_filter();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn no_selection_text(mut self, text: impl Into<String>) -> Self {
        self.no_selection_text = Some(text.into());
        self
    }

    pub fn action_keys(mut self, keys: DropdownActionKeys) -> Self {
        self.action_keys = keys;
        self
    }

    pub fn set_action_keys(&mut self, keys: DropdownActionKeys) {
        self.action_keys = keys;
    }

    pub fn set_no_selection_text(&mut self, text: impl Into<String>) {
        self.no_selection_text = Some(text.into());
    }

    pub fn clear_no_selection_text(&mut self) {
        self.no_selection_text = None;
        self.no_selection_highlighted = false;
    }

    pub fn commit_mode(mut self, mode: DropdownCommitMode) -> Self {
        self.commit_mode = mode;
        self
    }

    pub fn centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }

    pub fn close_on_select(mut self, close: bool) -> Self {
        self.close_on_select = close;
        self
    }

    pub fn max_popup_height(mut self, height: u16) -> Self {
        self.max_popup_height = Some(height.max(1));
        self
    }

    pub fn auto_focus_search(mut self, auto_focus: bool) -> Self {
        self.auto_focus_search = auto_focus;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub fn clear_label(&mut self) {
        self.label = None;
    }

    pub fn label_position(mut self, position: DropdownLabelPosition) -> Self {
        self.label_position = position;
        self
    }

    pub fn set_label_position(&mut self, position: DropdownLabelPosition) {
        self.label_position = position;
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.hotkey_matcher = HotkeySequenceMatcher::default();
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    pub fn set_tab_stop(&mut self, tab_stop: bool) {
        self.tab_stop = tab_stop;
    }

    pub fn alt_style(mut self, alt_style: bool) -> Self {
        self.alt_style = alt_style;
        self
    }

    pub fn set_alt_style(&mut self, alt_style: bool) {
        self.alt_style = alt_style;
    }

    pub fn variant(mut self, variant: DropdownVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn popup_direction(mut self, direction: DropdownPopupDirection) -> Self {
        self.popup_direction = direction;
        self
    }

    pub fn set_popup_direction(&mut self, direction: DropdownPopupDirection) {
        self.popup_direction = direction;
    }

    pub fn selected(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.committed = self.known_ids(ids);
        if !self.multi {
            self.committed.truncate(1);
        }
        self.draft = self.committed.clone();
        self.no_selection_highlighted = false;
        self.sync_view_selection();
        self
    }

    pub fn selected_one(self, id: Id) -> Self {
        self.selected([id])
    }

    pub fn selected_id(&self) -> Option<Id> {
        self.committed.first().cloned()
    }

    pub fn selected_ids(&self) -> Vec<Id> {
        self.committed.clone()
    }

    pub fn search_query(&self) -> &str {
        self.search_input.current_value()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_focused(&self) -> bool {
        self.focus_region.is_some()
    }

    pub fn open(&mut self) -> DropdownOutcome {
        if self.open {
            return DropdownOutcome::HANDLED;
        }

        self.open = true;
        self.opened_committed = self.committed.clone();
        self.draft = self.committed.clone();
        self.highlight_committed();
        if !self.multi && self.draft.is_empty() && self.no_selection_text.is_none() {
            self.set_single_draft_from_highlight();
        }
        self.refresh_filter();
        self.no_selection_highlighted = self.show_no_selection_row() && self.committed.is_empty();
        self.sync_view_selection();
        self.sync_child_focus();
        DropdownOutcome {
            handled: true,
            changed: true,
            opened: true,
            ..DropdownOutcome::IDLE
        }
    }

    pub fn close(&mut self) -> DropdownOutcome {
        if !self.open {
            return DropdownOutcome::HANDLED;
        }
        let had_focus = self.is_focused();
        self.open = false;
        self.no_selection_highlighted = false;
        self.opened_committed.clear();
        self.clear_search_query();
        if had_focus {
            self.sync_child_focus();
        } else {
            self.clear_focus();
        }
        DropdownOutcome {
            handled: true,
            changed: true,
            closed: true,
            ..DropdownOutcome::IDLE
        }
    }

    fn clear_search_query(&mut self) {
        if !self.search_input.current_value().is_empty() {
            self.search_input.set_value("");
            self.refresh_filter();
        }
    }

    pub fn cancel(&mut self) -> DropdownOutcome {
        if !self.opened_committed.is_empty() || !self.committed.is_empty() {
            self.committed = self.opened_committed.clone();
        }
        self.draft = self.committed.clone();
        self.sync_view_selection();
        let mut outcome = self.close();
        outcome.canceled = true;
        outcome.handled = true;
        outcome.changed = true;
        outcome
    }

    pub fn commit(&mut self) -> DropdownOutcome {
        if !self.multi && self.draft.is_empty() && self.no_selection_text.is_none() {
            self.set_single_draft_from_highlight();
        }
        let changed = self.committed != self.draft;
        self.committed = self.draft.clone();
        let mut outcome = self.close();
        outcome.committed = true;
        outcome.changed = outcome.changed || changed;
        outcome
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>, area: Rect) -> DropdownOutcome {
        let key = key.into();
        if !self.open {
            return self.on_closed_key(key, area);
        }

        if self.is_cancel_key(key) {
            return self.cancel();
        }

        if matches_any(&self.action_keys.commit, key) {
            if self.commit_mode == DropdownCommitMode::Immediate {
                return self.close();
            }
            return self.commit();
        }
        if self.multi && matches_any(&self.action_keys.toggle, key) {
            return self.toggle_highlighted();
        }

        let keys = keybindings();
        if keys.dropdown().select_matches(key) {
            return self.select_highlighted();
        }
        if keys.dropdown().next_matches(key) {
            return self.navigate(Key::Down, area);
        }
        if keys.dropdown().previous_matches(key) {
            return self.navigate(Key::Up, area);
        }
        if keys.dropdown().page_next_matches(key) {
            return self.navigate(Key::PageDown, area);
        }
        if keys.dropdown().page_previous_matches(key) {
            return self.navigate(Key::PageUp, area);
        }

        if self.search_enabled() {
            let input = self.search_input.on_key(key);
            if input.changed {
                self.refresh_filter();
                if !self.multi {
                    self.set_single_draft_from_highlight();
                    self.sync_view_selection();
                    if self.commit_mode == DropdownCommitMode::Immediate {
                        self.committed = self.draft.clone();
                    }
                }
            }
            if input.needs_redraw() {
                return DropdownOutcome {
                    handled: true,
                    changed: input.changed,
                    committed: input.changed
                        && !self.multi
                        && self.commit_mode == DropdownCommitMode::Immediate,
                    ..DropdownOutcome::IDLE
                };
            }
        }

        DropdownOutcome::IDLE
    }

    fn is_cancel_key(&self, key: KeyEvent) -> bool {
        keybindings().focus().unfocus_matches(key)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        if self.open && !self.overlay_bounds.is_empty() {
            self.render_popup_overlay(frame, self.overlay_bounds);
        } else {
            let field_area = self.field_area(area);
            self.render_field(frame, field_area);
        }
    }

    pub fn render_popup_overlay(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }

        let popup_area = self.popup_overlay_area(bounds);
        if !popup_area.is_empty() {
            let field_area = self.effective_field_area(bounds);
            super::dialog::dim_backdrop_buffer_except(
                frame,
                bounds,
                DROPDOWN_BACKDROP_AMOUNT,
                &[field_area, popup_area],
            );
            self.render_field(frame, field_area);
            self.render_popup(frame, popup_area);
        }
    }

    pub fn popup_overlay_area(&self, bounds: Rect) -> Rect {
        let field_area = self.effective_field_area(bounds);
        self.popup_area_for(field_area, bounds)
    }

    pub fn layout_overlay<M>(
        &mut self,
        area: Rect,
        overlay_bounds: Rect,
        ctx: &mut LayoutCtx,
    ) -> LayoutResult {
        self.field_area = self.field_area(area);
        self.focus_path = ctx.current_path();
        self.overlay_bounds = overlay_bounds;
        if !self.open {
            if let Some(ref h) = self.hotkey {
                ctx.register_focusable_with_hotkey_sequences(
                    FocusId::new(FIELD_FOCUS),
                    self.field_area,
                    true,
                    vec![h.clone()],
                );
                ctx.set_focus_tab_stop(FocusId::new(FIELD_FOCUS), self.tab_stop);
            } else {
                ctx.register_focusable(FocusId::new(FIELD_FOCUS), self.field_area, true);
                ctx.set_focus_tab_stop(FocusId::new(FIELD_FOCUS), self.tab_stop);
            }
            return LayoutResult::new(self.field_area);
        }

        let popup_area = self.popup_area_for(self.field_area, overlay_bounds);
        let [search_area, list_area] = self.popup_inner_areas(popup_area);
        let rows_area = self.popup_rows_area(list_area);
        if self.search_enabled() && self.auto_focus_search {
            ctx.register_focusable(FocusId::new(SEARCH_FOCUS), search_area, true);
            ctx.set_focus_tab_stop(FocusId::new(SEARCH_FOCUS), self.tab_stop);
            ctx.set_focus_suppresses_global_hotkeys(FocusId::new(SEARCH_FOCUS), true);
        } else {
            ctx.register_focusable(FocusId::new(FIELD_FOCUS), self.field_area, true);
            ctx.set_focus_tab_stop(FocusId::new(FIELD_FOCUS), self.tab_stop);
            if self.search_enabled() {
                ctx.set_focus_suppresses_global_hotkeys(FocusId::new(FIELD_FOCUS), true);
            }
        }
        let mut child_ctx = LayoutCtx::new();
        <DataView<T, Id> as TuiNode<M>>::layout(&mut self.data_view, rows_area, &mut child_ctx);
        LayoutResult::new(self.field_area)
    }

    fn on_closed_key(&mut self, key: KeyEvent, area: Rect) -> DropdownOutcome {
        let keys = keybindings();
        if keys.dropdown().next_matches(key) {
            let mut outcome = self.open();
            let nav = self.navigate(Key::Down, area);
            outcome.handled |= nav.handled;
            outcome.changed |= nav.changed;
            return outcome;
        }
        if keys.dropdown().previous_matches(key) {
            let mut outcome = self.open();
            let nav = self.navigate(Key::Up, area);
            outcome.handled |= nav.handled;
            outcome.changed |= nav.changed;
            return outcome;
        }

        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) => return self.open(),
            HotkeyMatch::Pending | HotkeyMatch::Canceled => return DropdownOutcome::HANDLED,
            HotkeyMatch::Ignored => {}
        }

        if self
            .hotkey
            .as_deref()
            .and_then(hotkey_sequence_to_event)
            .is_some_and(|hotkey| keys_match(hotkey, key))
        {
            return self.open();
        }

        if matches_any(&self.action_keys.open, key) {
            self.open()
        } else {
            DropdownOutcome::IDLE
        }
    }

    fn navigate(&mut self, key: Key, area: Rect) -> DropdownOutcome {
        if let Some(outcome) = self.navigate_no_selection_row(key) {
            return outcome;
        }

        let moved_to_no_selection = self.should_move_to_no_selection(key);
        let outcome = self
            .data_view
            .on_key(KeyEvent::from(key), self.list_area(area));
        if moved_to_no_selection {
            self.draft.clear();
            self.sync_view_selection();
            self.set_no_selection_highlighted(true);
            if self.commit_mode == DropdownCommitMode::Immediate {
                self.committed.clear();
            }
            return DropdownOutcome::changed();
        }
        self.set_no_selection_highlighted(false);
        if !self.multi {
            self.set_single_draft_from_highlight();
            self.sync_view_selection();
            if self.commit_mode == DropdownCommitMode::Immediate {
                self.committed = self.draft.clone();
            }
        }
        DropdownOutcome {
            handled: outcome.handled,
            changed: outcome.changed,
            committed: !self.multi && self.commit_mode == DropdownCommitMode::Immediate,
            ..DropdownOutcome::IDLE
        }
    }

    fn toggle_highlighted(&mut self) -> DropdownOutcome {
        let Some(id) = self.data_view.highlighted_id() else {
            return DropdownOutcome::HANDLED;
        };
        self.data_view.toggle_selected(id);
        self.draft = self.data_view.selected_ids();
        if self.close_on_select {
            self.committed = self.draft.clone();
            let mut outcome = self.close();
            outcome.committed = true;
            return outcome;
        }
        DropdownOutcome::changed()
    }

    fn select_highlighted(&mut self) -> DropdownOutcome {
        if self.no_selection_highlighted {
            self.draft.clear();
            self.sync_view_selection();
            return self.commit();
        }

        if self.multi {
            return self.toggle_highlighted();
        }

        self.set_single_draft_from_highlight();
        self.sync_view_selection();
        self.commit()
    }

    fn refresh_filter(&mut self) {
        let query = self.search_input.current_value();
        let query_empty = query.is_empty();
        let filtered = match self.search_mode {
            DropdownSearchMode::None if query_empty => self.ids.clone(),
            DropdownSearchMode::None => self.ids.clone(),
            DropdownSearchMode::Contains if query_empty => self.ids.clone(),
            DropdownSearchMode::Fuzzy if query_empty => self.ids.clone(),
            DropdownSearchMode::Contains => self.search(SearchMode::Contains),
            DropdownSearchMode::Fuzzy => self.search(SearchMode::Fuzzy),
        };

        self.filtered = filtered;
        if !self.show_no_selection_row() {
            self.set_no_selection_highlighted(false);
        }
        if self.search_mode == DropdownSearchMode::None || query_empty {
            self.data_view.clear_visible_row_ids();
        } else {
            self.data_view.set_visible_row_ids(self.filtered.clone());
        }
    }

    fn search(&self, mode: SearchMode) -> Vec<Id> {
        search_ranked(self.search_input.current_value(), &self.labels, mode)
            .into_iter()
            .map(|matched| self.ids[matched.index].clone())
            .collect()
    }

    fn sync_view_selection(&mut self) {
        self.data_view.clear_selection();
        for id in self.draft.clone() {
            self.data_view.select_id(id);
        }
        self.data_view.drain_events();
    }

    fn highlight_committed(&mut self) {
        if let Some(id) = self.committed.first() {
            self.data_view.highlight_id(id);
            self.data_view.drain_events();
        }
    }

    fn set_single_draft_from_highlight(&mut self) {
        let Some(id) = self.data_view.highlighted_id() else {
            self.draft.clear();
            return;
        };
        self.draft = vec![id];
    }

    fn known_ids(&self, ids: impl IntoIterator<Item = Id>) -> Vec<Id> {
        ids.into_iter()
            .filter(|id| self.ids.iter().any(|known| known == id))
            .collect()
    }

    fn search_enabled(&self) -> bool {
        self.search_mode != DropdownSearchMode::None
    }

    fn clear_focus(&mut self) {
        self.set_focus_region(None);
    }

    fn set_focus_region(&mut self, region: Option<DropdownFocusRegion>) {
        if self.focus_region != region {
            self.field_cursor_fade.reset();
        }
        self.focus_region = region;
        self.sync_child_focus();
    }

    fn sync_child_focus(&mut self) {
        self.search_input
            .set_focused(self.open && self.focus_region == Some(DropdownFocusRegion::Search));
        self.data_view.set_focused(
            self.open && self.focus_region.is_some() && !self.no_selection_highlighted,
        );
    }

    fn set_no_selection_highlighted(&mut self, highlighted: bool) {
        if self.no_selection_highlighted == highlighted {
            return;
        }
        self.no_selection_highlighted = highlighted;
        self.sync_child_focus();
    }

    fn navigate_no_selection_row(&mut self, key: Key) -> Option<DropdownOutcome> {
        if !self.no_selection_highlighted {
            return None;
        }
        if matches!(key, Key::Down | Key::PageDown) {
            self.set_no_selection_highlighted(false);
            if !self.multi {
                self.set_single_draft_from_highlight();
                self.sync_view_selection();
                if self.commit_mode == DropdownCommitMode::Immediate {
                    self.committed = self.draft.clone();
                }
            }
            return Some(DropdownOutcome::changed());
        }
        if matches!(key, Key::Up | Key::PageUp) {
            return Some(DropdownOutcome::HANDLED);
        }
        None
    }

    fn should_move_to_no_selection(&self, key: Key) -> bool {
        matches!(key, Key::Up | Key::PageUp)
            && self.show_no_selection_row()
            && self.first_visible_is_highlighted()
    }

    fn first_visible_is_highlighted(&self) -> bool {
        self.filtered
            .first()
            .is_some_and(|first| self.data_view.highlighted_id().as_ref() == Some(first))
    }

    fn target_matches_focus_region(&self, target: Option<&FocusId>) -> bool {
        match (target.map(FocusId::as_str), self.focus_region) {
            (Some(SEARCH_FOCUS), Some(DropdownFocusRegion::Search)) => true,
            (Some(FIELD_FOCUS), Some(DropdownFocusRegion::Field)) => true,
            _ => false,
        }
    }

    fn is_opening_search_field_blur(&self, target: Option<&FocusId>) -> bool {
        self.open
            && self.search_enabled()
            && self.auto_focus_search
            && self.focus_region == Some(DropdownFocusRegion::Field)
            && target.is_some_and(|id| id.as_str() == FIELD_FOCUS)
    }

    fn popup_inner_areas(&self, popup_area: Rect) -> [Rect; 2] {
        if popup_area.is_empty() {
            return [popup_area, popup_area];
        }
        let popup_inner = if self.popup_has_border() {
            Block::default().borders(Borders::ALL).inner(popup_area)
        } else {
            popup_area
        };
        let search_height = u16::from(self.search_enabled());
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(popup_inner)
    }

    fn selected_summary(&self) -> String {
        let ids = &self.committed;
        if ids.is_empty() {
            return self.empty_summary();
        }
        if self.multi && ids.len() > 1 {
            return format!("{} selected", ids.len());
        }
        self.label_for(&ids[0])
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn empty_summary(&self) -> String {
        self.no_selection_text
            .clone()
            .unwrap_or_else(|| self.placeholder.clone())
    }

    fn show_no_selection_row(&self) -> bool {
        self.no_selection_text.is_some() && self.search_input.current_value().is_empty()
    }

    fn label_for(&self, id: &Id) -> Option<String> {
        self.ids
            .iter()
            .position(|known| known == id)
            .map(|index| self.labels[index].clone())
    }

    #[cfg(test)]
    fn areas(&self, area: Rect) -> [Rect; 2] {
        let field_area = self.field_area(area);
        let popup_area = self.popup_area_for(field_area, area);

        [field_area, popup_area]
    }

    fn field_area(&self, area: Rect) -> Rect {
        let field_height = self.field_height(area);
        Rect::new(area.x, area.y, area.width, field_height)
    }

    fn effective_field_area(&self, bounds: Rect) -> Rect {
        if self.field_area.is_empty() {
            self.field_area(bounds)
        } else {
            self.field_area
        }
    }

    fn popup_area_for(&self, field_area: Rect, bounds: Rect) -> Rect {
        if !self.open || field_area.is_empty() || bounds.is_empty() {
            return Rect::default();
        }

        if self.centered {
            let popup_width = field_area.width.max(40).min(bounds.width);
            let popup_height = self
                .popup_content_height(popup_width)
                .min(self.effective_max_popup_height())
                .min(bounds.height);
            let popup_x = bounds.x + (bounds.width.saturating_sub(popup_width)) / 2;
            let popup_y = bounds.y + (bounds.height.saturating_sub(popup_height)) / 2;
            let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);
            return clip_rect(popup_area, bounds);
        }

        let desired_height = self
            .popup_content_height(field_area.width)
            .min(self.effective_max_popup_height());
        let (popup_y, available_height) = match self.popup_direction {
            DropdownPopupDirection::Down => {
                let y = field_area
                    .y
                    .saturating_add(field_area.height)
                    .saturating_sub(self.popup_overlap());
                let available = bounds.y.saturating_add(bounds.height).saturating_sub(y);
                (y, available)
            }
            DropdownPopupDirection::Up => {
                let available = field_area.y.saturating_sub(bounds.y);
                let height = desired_height.min(available);
                (field_area.y.saturating_sub(height), available)
            }
        };
        let popup_height = desired_height.min(available_height);
        let popup_area = Rect::new(field_area.x, popup_y, field_area.width, popup_height);

        clip_rect(popup_area, bounds)
    }

    fn field_height(&self, area: Rect) -> u16 {
        let base_height = match self.variant {
            DropdownVariant::Bordered => 3,
            DropdownVariant::Filled => 1,
        };
        if self.alt_style {
            let label_height = u16::from(self.label_position == DropdownLabelPosition::Top);
            area.height.min(base_height + label_height)
        } else {
            area.height.min(base_height)
        }
    }

    fn popup_content_height(&self, width: u16) -> u16 {
        self.popup_border_height()
            .saturating_add(u16::from(self.search_enabled()))
            .saturating_add(self.visible_popup_rows())
            .saturating_add(u16::from(self.needs_horizontal_scrollbar(width)))
    }

    fn popup_border_height(&self) -> u16 {
        if self.popup_has_border() {
            POPUP_BORDER_HEIGHT
        } else {
            0
        }
    }

    fn popup_has_border(&self) -> bool {
        self.variant == DropdownVariant::Bordered
    }

    fn popup_overlap(&self) -> u16 {
        u16::from(self.popup_has_border())
    }

    fn visible_popup_rows(&self) -> u16 {
        if self.filtered.is_empty() {
            return 1;
        }
        let no_selection_row = usize::from(self.show_no_selection_row());
        self.filtered
            .len()
            .saturating_add(no_selection_row)
            .min(usize::from(u16::MAX)) as u16
    }

    fn needs_horizontal_scrollbar(&self, width: u16) -> bool {
        let viewport_width = width.saturating_sub(if self.popup_has_border() { 2 } else { 0 });
        let prefix_width = if self.multi { 2 } else { 0 };
        let content_width = self
            .filtered
            .iter()
            .filter_map(|id| self.label_for(id))
            .map(|label| line_width(&Line::from(label)).saturating_add(prefix_width))
            .max()
            .unwrap_or_else(|| line_width(&Line::from("No results")));
        content_width > viewport_width as usize
    }

    fn measured_field_width(&self) -> u16 {
        let summary_width = line_width(&Line::from(self.selected_summary()));
        let mut width = match self.variant {
            DropdownVariant::Bordered => summary_width.saturating_add(2),
            DropdownVariant::Filled
                if self.alt_style && self.label_position == DropdownLabelPosition::Inline =>
            {
                line_width(&self.inline_filled_line(Style::default())).saturating_add(2)
            }
            DropdownVariant::Filled if self.alt_style => summary_width.saturating_add(2),
            DropdownVariant::Filled => summary_width.saturating_add(3),
        };

        if self.variant == DropdownVariant::Bordered && !self.alt_style {
            if let Some(label) = &self.label {
                width = width.max(
                    line_width(&Line::from(bounded_title(label, usize::MAX))).saturating_add(4),
                );
            }
            if let Some(hotkey) = &self.hotkey {
                width = width.max(hotkey_badge_width(hotkey));
            }
        }

        if self.alt_style && self.label_position == DropdownLabelPosition::Top {
            width = width.max(self.alt_label_line_width());
        }

        width.min(u16::MAX as usize) as u16
    }

    fn alt_label_line_width(&self) -> usize {
        let label = self.label.clone().unwrap_or_default();
        if let Some(hotkey) = &self.hotkey {
            line_width(&Line::from(hotkey_label_spans(
                &label,
                Some(hotkey.as_str()),
                HotkeyLabelMode::Inline,
                None,
                Style::default(),
                Style::default(),
            )))
        } else {
            line_width(&Line::from(label))
        }
    }

    fn effective_max_popup_height(&self) -> u16 {
        self.max_popup_height
            .unwrap_or_else(|| preset().dropdown().max_popup_height())
            .max(1)
    }

    fn list_area(&self, area: Rect) -> Rect {
        let popup_area = self.popup_overlay_area(area);
        if popup_area.is_empty() {
            return popup_area;
        }
        let [_, list_area] = self.popup_inner_areas(popup_area);
        self.popup_rows_area(list_area)
    }

    fn popup_rows_area(&self, list_area: Rect) -> Rect {
        let no_selection_height = u16::from(self.show_no_selection_row());
        Rect::new(
            list_area.x,
            list_area.y.saturating_add(no_selection_height),
            list_area.width,
            list_area.height.saturating_sub(no_selection_height),
        )
    }

    fn render_field(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        if self.alt_style && self.label_position == DropdownLabelPosition::Top {
            let label_area = Rect::new(area.x, area.y, area.width, area.height.min(1));
            let mut text = String::new();
            if let Some(ref l) = self.label {
                text.push_str(l);
            }
            if let Some(ref h) = self.hotkey {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&format!("|{h}|"));
            }
            if !text.is_empty() && !label_area.is_empty() {
                let theme = theme();
                let style = Style::default()
                    .fg(if self.is_focused() {
                        theme.accent_fg()
                    } else {
                        theme.muted_fg()
                    })
                    .add_modifier(Modifier::BOLD);
                if let Some(ref h) = self.hotkey {
                    let label = self.label.clone().unwrap_or_default();
                    let spans = hotkey_label_spans(
                        &label,
                        Some(h.as_str()),
                        HotkeyLabelMode::Inline,
                        self.pending_hotkey_prefix.as_deref(),
                        style,
                        hotkey_underline_style(style),
                    );
                    frame.render_widget(Paragraph::new(Line::from(spans)), label_area);
                } else {
                    frame.render_widget(Paragraph::new(text).style(style), label_area);
                }
            }
            let dropdown_area = Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            );
            if !dropdown_area.is_empty() {
                match self.variant {
                    DropdownVariant::Bordered => self.render_bordered_field(frame, dropdown_area),
                    DropdownVariant::Filled => self.render_filled_field(frame, dropdown_area),
                }
            }
        } else {
            match self.variant {
                DropdownVariant::Bordered => self.render_bordered_field(frame, area),
                DropdownVariant::Filled => self.render_filled_field(frame, area),
            }
        }
    }

    fn render_bordered_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let border = preset().border();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(Style::default().fg(if self.is_focused() {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let text = if self.committed.is_empty() {
            let placeholder_style = Style::default().fg(theme.muted_fg());
            placeholder_line(
                &self.empty_summary(),
                inner.width as usize,
                self.focus_region == Some(DropdownFocusRegion::Field),
                self.field_cursor_fade.style(placeholder_style),
                placeholder_style,
            )
        } else {
            Line::from(Span::styled(
                self.selected_summary(),
                Style::default().fg(theme.text_fg()),
            ))
        };
        frame.render_widget(Paragraph::new(text), inner);

        if !self.alt_style {
            if let Some(ref label) = self.label {
                self.render_title(frame, area, label, Alignment::Left, area.y);
            }
            if let Some(ref hotkey) = self.hotkey {
                self.render_inset_title(
                    frame,
                    area,
                    border,
                    hotkey,
                    Alignment::Right,
                    area.y + area.height.saturating_sub(1),
                );
            }
        }
    }

    fn render_filled_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let base_style = Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg());
        let text_style = if self.committed.is_empty() {
            base_style.add_modifier(Modifier::DIM)
        } else {
            base_style
        };

        frame.render_widget(Paragraph::new("").style(base_style), area);

        let arrow_x = area.x + area.width.saturating_sub(2);
        let alt_trigger = self.alt_style;
        let inline_trigger = alt_trigger && self.label_position == DropdownLabelPosition::Inline;
        let text_area = if alt_trigger {
            Rect::new(area.x, area.y, area.width.saturating_sub(2), 1)
        } else {
            Rect::new(
                area.x.saturating_add(1),
                area.y,
                area.width.saturating_sub(3),
                1,
            )
        };
        if !text_area.is_empty() {
            let text = if inline_trigger {
                self.inline_filled_line(text_style)
            } else if self.committed.is_empty() && self.no_selection_text.is_some() {
                Line::from(Span::styled(self.empty_summary(), text_style))
            } else if self.committed.is_empty() {
                placeholder_line(
                    &self.empty_summary(),
                    text_area.width as usize,
                    self.focus_region == Some(DropdownFocusRegion::Field),
                    self.field_cursor_fade.style(text_style),
                    text_style,
                )
            } else {
                Line::from(Span::styled(self.selected_summary(), text_style))
            };
            frame.render_widget(Paragraph::new(text), text_area);
        }

        let arrow_area = Rect::new(arrow_x, area.y, 1, 1);
        frame.render_widget(
            Paragraph::new("")
                .style(base_style)
                .alignment(Alignment::Right),
            arrow_area,
        );
    }

    fn render_popup(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        frame.render_widget(Clear, area);
        let popup_content_style = self.popup_content_style();
        let inner = if self.popup_has_border() {
            let border = if self.variant == DropdownVariant::Bordered && !self.centered {
                connected_popup_border_set(preset().border())
            } else {
                border_set(preset().border())
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border)
                .border_style(Style::default().fg(if self.is_focused() {
                    theme.accent_fg()
                } else {
                    theme.border_fg()
                }));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            frame.render_widget(
                Paragraph::new("").style(popup_content_style.unwrap_or_default()),
                area,
            );
            area
        };

        let search_height = u16::from(self.search_enabled());
        let [search_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(inner);
        if self.search_enabled() {
            if let Some(style) = popup_content_style {
                self.search_input
                    .render_with_style(frame, search_area, style);
            } else {
                self.search_input.render(frame, search_area);
            }
        }
        let no_selection_height = u16::from(self.show_no_selection_row());
        let no_selection_area = Rect::new(
            list_area.x,
            list_area.y,
            list_area.width,
            no_selection_height,
        );
        let rows_area = Rect::new(
            list_area.x,
            list_area.y.saturating_add(no_selection_height),
            list_area.width,
            list_area.height.saturating_sub(no_selection_height),
        );
        if self.show_no_selection_row() {
            self.render_no_selection_row(frame, no_selection_area, popup_content_style);
        }

        if self.filtered.is_empty() {
            let line = Line::styled(
                "No results",
                popup_content_style
                    .unwrap_or_default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::ITALIC),
            );
            frame.render_widget(
                Paragraph::new(line).style(popup_content_style.unwrap_or_default()),
                rows_area,
            );
        } else {
            self.data_view
                .render_with_row_style(frame, rows_area, popup_content_style);
        }
    }

    fn inline_filled_line(&self, base_style: Style) -> Line<'static> {
        let mut spans = Vec::new();
        if let Some(label) = &self.label {
            spans.push(Span::styled(format!("{label}: "), base_style));
        }

        let value_style = if self.committed.is_empty() {
            base_style.add_modifier(Modifier::DIM)
        } else {
            base_style.add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(self.selected_summary(), value_style));

        if let Some(hotkey) = &self.hotkey {
            spans.extend(hotkey_label_spans(
                "",
                Some(hotkey.as_str()),
                HotkeyLabelMode::Inline,
                self.pending_hotkey_prefix.as_deref(),
                base_style,
                hotkey_underline_style(base_style),
            ));
        }

        Line::from(spans)
    }

    fn render_no_selection_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        popup_content_style: Option<Style>,
    ) {
        let Some(text) = &self.no_selection_text else {
            return;
        };
        if area.is_empty() {
            return;
        }

        let theme = theme();
        let style = if self.no_selection_highlighted {
            Style::default()
                .fg(theme.highlight_fg())
                .bg(theme.highlight_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            popup_content_style.unwrap_or_default().fg(theme.muted_fg())
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text.clone(), style))).style(style),
            area,
        );
    }

    fn popup_content_style(&self) -> Option<Style> {
        (self.variant == DropdownVariant::Filled).then(|| Style::default().bg(theme().border_fg()))
    }

    fn render_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        alignment: Alignment,
        y: u16,
    ) {
        if area.width <= 4 {
            return;
        }

        let max_width = area.width.saturating_sub(4) as usize;
        let title = bounded_title(title, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let x = match alignment {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(2),
        };
        let theme = theme();
        let style = Style::default()
            .fg(if self.is_focused() {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            })
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(title, style))),
            Rect::new(x, y, width, 1),
        );
    }

    fn render_inset_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        border: BorderKind,
        title: &str,
        alignment: Alignment,
        y: u16,
    ) {
        if area.width <= 4 {
            return;
        }

        let theme = theme();
        let border_style = Style::default().fg(if self.is_focused() {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        let title_style = Style::default().fg(if self.is_focused() {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
        let width = hotkey_badge_width(title).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let line = Line::from(hotkey_edge_spans(
            title,
            self.pending_hotkey_prefix.as_deref(),
            border,
            border_style,
            title_style,
            hotkey_underline_style(title_style),
        ));
        let x = match alignment {
            Alignment::Left | Alignment::Center => area.x.saturating_add(1),
            Alignment::Right => area.x + area.width.saturating_sub(width),
        };

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn request_open_focus<M>(&self, ctx: &mut EventCtx<M>) {
        if self.search_enabled() && self.auto_focus_search {
            ctx.focus(FocusRequest::TargetAt {
                path: self.focus_path.clone(),
                id: FocusId::new(SEARCH_FOCUS),
            });
        }
    }
}

impl<T, Id, M> TuiNode<M> for Dropdown<T, Id>
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let mut height = match self.variant {
            DropdownVariant::Bordered => 3,
            DropdownVariant::Filled => 1,
        };
        if self.alt_style && self.label_position == DropdownLabelPosition::Top {
            height += 1;
        }
        let width = self.measured_field_width();
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(width, height),
            preferred: LayoutSize::new(width, height),
            expand: crate::AxisExpand {
                width: true,
                height: false,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.layout_overlay::<M>(area, area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
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
                    if self
                        .hotkey
                        .as_deref()
                        .is_some_and(|hotkey| hotkey_matches_sequence(hotkey, sequence))
                    {
                        let outcome = self.open();
                        if outcome.opened {
                            ctx.request_layout();
                            self.request_open_focus(ctx);
                        }
                        ctx.request_redraw();
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    return EventOutcome::Ignored;
                }
            }
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let bindings = keybindings();
        let focus_keys = bindings.focus();
        if self.open && focus_keys.next_matches(*key) {
            self.cancel();
            ctx.focus_next();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if self.open && focus_keys.previous_matches(*key) {
            self.cancel();
            ctx.focus_previous();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key, self.overlay_bounds);
        if outcome.opened || outcome.closed {
            ctx.request_layout();
        }
        if outcome.handled || outcome.changed {
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if !route.path.is_empty() {
            return EventOutcome::Ignored;
        }
        self.event(event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if focused && target.is_some_and(|id| id.as_str() == SEARCH_FOCUS) {
            self.set_focus_region(Some(DropdownFocusRegion::Search));
        } else if focused && target.is_some_and(|id| id.as_str() == FIELD_FOCUS) {
            self.set_focus_region(Some(DropdownFocusRegion::Field));
        } else if !focused && self.is_opening_search_field_blur(target) {
            ctx.request_redraw();
        } else if !focused && self.open && self.target_matches_focus_region(target) {
            self.cancel();
            self.clear_focus();
            ctx.request_redraw();
        } else if !focused && self.target_matches_focus_region(target) {
            self.clear_focus();
        }
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        }
    }
}

impl<T, Id> Animated for Dropdown<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        Animated::tick(&mut self.data_view, dt, settings)
            .merge(Animated::tick(&mut self.search_input, dt, settings))
            .merge(self.field_cursor_fade.tick(
                self.focus_region == Some(DropdownFocusRegion::Field),
                dt,
                settings,
            ))
            .merge(hotkey_tick)
    }
}

fn connected_popup_border_set(border: crate::BorderKind) -> Set<'static> {
    let chars = border_chars(border);
    Set {
        top_left: chars.left_join,
        top_right: chars.right_join,
        bottom_left: chars.bottom_left,
        bottom_right: chars.bottom_right,
        vertical_left: chars.vertical,
        vertical_right: chars.vertical,
        horizontal_top: chars.horizontal,
        horizontal_bottom: chars.horizontal,
    }
}

fn clip_rect(area: Rect, bounds: Rect) -> Rect {
    let x = area.x.max(bounds.x);
    let y = area.y.max(bounds.y);
    let right = area
        .x
        .saturating_add(area.width)
        .min(bounds.x.saturating_add(bounds.width));
    let bottom = area
        .y
        .saturating_add(area.height)
        .min(bounds.y.saturating_add(bounds.height));
    Rect::new(x, y, right.saturating_sub(x), bottom.saturating_sub(y))
}

fn bounded_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut value = format!(" {title} ");
    if line_width(&Line::from(value.as_str())) > max_width {
        value = truncate_cells(&value, max_width);
    }
    value
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();

    for ch in value.chars() {
        let ch_width = char_width(ch);
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }

    truncated
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    line_width(&Line::from(value))
}

fn keys_match(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
        (a, b) => a == b,
    }
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn hotkey_matches_sequence(hotkey: &str, sequence: &str) -> bool {
    crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
}

#[cfg(test)]
mod tests {
    use std::hash::Hash;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Style};

    use super::*;
    use crate::event::KeyModifiers;
    use crate::{
        ChildKey, EventCtx, EventRoute, Flex, FlexItem, FocusCtx, FocusId, FocusRequest,
        KeyBindings, KeySpec, LayoutCtx, LayoutProposal, Propagation, TuiEvent, TuiNode,
    };

    fn single_dropdown() -> Dropdown<&'static str, &'static str> {
        Dropdown::single(ROWS, |row| *row, |row| row.to_string())
    }

    fn multi_dropdown() -> Dropdown<&'static str, &'static str> {
        Dropdown::multi(ROWS, |row| *row, |row| row.to_string())
    }

    fn numeric_dropdown(count: u8) -> Dropdown<u8, u8> {
        Dropdown::single(0..count, |row| *row, |row| row.to_string())
    }

    const ROWS: [&str; 3] = ["Alpha", "Beta", "Gamma"];
    const AREA: Rect = Rect::new(0, 0, 24, 10);

    struct KeyBindingsGuard {
        previous: KeyBindings,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl KeyBindingsGuard {
        fn replace(next: KeyBindings) -> Self {
            let lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
            let previous = keybindings();
            crate::set_keybindings(next);
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for KeyBindingsGuard {
        fn drop(&mut self) {
            crate::set_keybindings(self.previous.clone());
        }
    }

    #[test]
    fn custom_action_keys_open_and_commit_dropdown() {
        let keys = DropdownActionKeys {
            open: vec![KeySpec::plain('o')],
            commit: vec![KeySpec::plain('c')],
            toggle: vec![KeySpec::plain('t')],
        };
        let mut dropdown = single_dropdown().action_keys(keys);

        assert!(!dropdown.on_key(KeyEvent::from(Key::Enter), AREA).handled);
        assert!(dropdown.on_key(KeyEvent::from(Key::Char('o')), AREA).opened);
        assert!(
            dropdown
                .on_key(KeyEvent::from(Key::Char('c')), AREA)
                .committed
        );
    }

    #[test]
    fn open_popup_dims_backdrop_but_not_trigger() {
        let mut dropdown = single_dropdown()
            .selected_one("Beta")
            .variant(DropdownVariant::Filled);
        dropdown.open();
        let mut layout = LayoutCtx::new();
        dropdown.layout_overlay::<()>(Rect::new(0, 0, 12, 1), AREA, &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(24, 10)).expect("terminal should build");

        terminal
            .draw(|frame| {
                frame.buffer_mut().set_string(
                    0,
                    9,
                    "X",
                    Style::default()
                        .fg(Color::Rgb(200, 200, 200))
                        .bg(Color::Rgb(10, 20, 30)),
                );
                dropdown.render(frame, Rect::new(0, 0, 12, 1));
            })
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let backdrop_cell = buffer.cell((0, 9)).unwrap();
        assert_ne!(backdrop_cell.fg, Color::Rgb(200, 200, 200));
        assert!(backdrop_cell.modifier.contains(Modifier::DIM));

        let trigger_text = (0..12)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(trigger_text.contains("Beta"), "{trigger_text}");
        assert!(
            !buffer
                .cell((1, 0))
                .unwrap()
                .modifier
                .contains(Modifier::DIM)
        );
    }

    #[test]
    fn open_clones_committed_selection_to_draft() {
        let mut dropdown = single_dropdown().selected_one("Beta");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.cancel();

        assert_eq!(dropdown.selected_id(), Some("Beta"));
    }

    #[test]
    fn enter_commits_single_draft() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Beta"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn ctrl_d_and_ctrl_u_page_navigate_defaults() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('d'), AREA);
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Gamma"));

        dropdown.on_key(ctrl('u'), AREA);
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
    }

    #[test]
    fn closed_ctrl_j_and_ctrl_k_open_with_navigating() {
        let mut dropdown = single_dropdown();
        let outcome = dropdown.on_key(ctrl('j'), AREA);
        assert!(outcome.opened);
        assert!(dropdown.is_open());
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));

        let mut dropdown = single_dropdown();
        let outcome = dropdown.on_key(ctrl('k'), AREA);
        assert!(outcome.opened);
        assert!(dropdown.is_open());
        assert_eq!(dropdown.data_view.highlighted_id(), Some("Alpha"));
    }

    #[test]
    fn closed_plain_j_and_k_do_not_open() {
        for key in [char_key('j'), char_key('k')] {
            let mut dropdown = single_dropdown();

            let outcome = dropdown.on_key(key, AREA);

            assert!(!outcome.opened);
            assert!(!dropdown.is_open());
        }
    }

    #[test]
    fn ctrl_d_and_ctrl_u_page_navigation_moves_by_visible_page_step() {
        let mut dropdown = numeric_dropdown(20);

        dropdown.open();
        dropdown.on_key(char_key('1'), AREA);
        dropdown.on_key(ctrl('d'), AREA);

        assert_eq!(dropdown.search_query(), "1");
        assert!(dropdown.data_view.highlighted_id().unwrap() > 1);

        dropdown.on_key(ctrl('u'), AREA);
        assert_eq!(dropdown.search_query(), "1");
        assert_eq!(dropdown.data_view.highlighted_id(), Some(1));
    }

    #[test]
    fn escape_rolls_back_single_draft() {
        let mut dropdown = single_dropdown().selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.selected_id(), Some("Alpha"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn configured_unfocus_key_cancels_open_dropdown() {
        let _guard =
            KeyBindingsGuard::replace(KeyBindings::new().with_focus_unfocus([KeySpec::plain('q')]));
        let mut dropdown = single_dropdown().selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(char_key('q'), AREA);

        assert_eq!(dropdown.selected_id(), Some("Alpha"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn typing_search_filters_rows_before_commit() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(char_key('a'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn enter_commit_clears_search_query() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
        assert_eq!(dropdown.search_query(), "");
    }

    #[test]
    fn escape_cancel_clears_search_query_and_filter() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.search_query(), "");
        assert_eq!(dropdown.filtered, ROWS.to_vec());
    }

    #[test]
    fn tab_while_open_cancels_and_requests_next_focus() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        let mut ctx = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(Key::Tab.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(dropdown.search_query(), "");
        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Next));
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn hotkey_open_requests_search_focus_at_dropdown_path() {
        let mut flex: Flex<()> = Flex::row()
            .child("first", single_dropdown().hotkey("f"), FlexItem::fixed(12))
            .child("second", single_dropdown().hotkey("s"), FlexItem::fixed(12));
        let mut layout = LayoutCtx::new();
        flex.layout(AREA, &mut layout);
        let target = layout
            .focus_targets()
            .iter()
            .find(|target| target.hotkey_sequences == ["s".to_string()])
            .expect("second dropdown target should exist")
            .clone();
        let mut ctx = EventCtx::<()>::default();

        let outcome = flex.dispatch_event(
            &EventRoute::new(target.path.clone()),
            &TuiEvent::Hotkey(HotkeyEvent::Commit("s".into())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: target.path,
                id: FocusId::new(SEARCH_FOCUS),
            })
        );
    }

    #[test]
    fn dropdown_navigation_preserves_search_query() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(char_key('a'), AREA);
        dropdown.on_key(ctrl('j'), AREA);

        assert_eq!(dropdown.search_query(), "a");
    }

    #[test]
    fn contains_search_requires_contiguous_match() {
        let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::Contains);

        dropdown.open();
        dropdown.on_key(char_key('m'), AREA);
        dropdown.on_key(char_key('m'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn disabled_search_ignores_typing() {
        let mut dropdown = single_dropdown().search_mode(DropdownSearchMode::None);

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.search_query(), "");
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn immediate_commit_updates_selection_while_open() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        let outcome = dropdown.on_key(ctrl('j'), AREA);

        assert!(outcome.committed);
        assert!(dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Beta"));
    }

    #[test]
    fn immediate_commit_updates_selection_while_filtering() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        let outcome = dropdown.on_key(char_key('g'), AREA);

        assert!(outcome.committed);
        assert!(dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn immediate_enter_closes_without_changing_current_selection() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        let outcome = dropdown.on_key(Key::Enter, AREA);

        assert!(outcome.closed);
        assert!(!dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Gamma"));
    }

    #[test]
    fn immediate_escape_restores_value_from_before_open() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert!(!dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn immediate_ctrl_left_bracket_restores_value_from_before_open() {
        let mut dropdown = single_dropdown()
            .commit_mode(DropdownCommitMode::Immediate)
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(char_key('g'), AREA);
        dropdown.on_key(ctrl('['), AREA);

        assert!(!dropdown.is_open());
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn explicit_single_keeps_trigger_value_until_commit() {
        let mut dropdown = single_dropdown().selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);

        assert_eq!(dropdown.selected_summary(), "Alpha");
        assert_eq!(dropdown.selected_id(), Some("Alpha"));
    }

    #[test]
    fn open_highlights_committed_selection() {
        let mut dropdown = single_dropdown()
            .search_mode(DropdownSearchMode::None)
            .selected_one("Beta");

        dropdown.open();

        assert_eq!(dropdown.data_view.highlighted_id(), Some("Beta"));
    }

    #[test]
    fn searchable_dropdown_keeps_field_focus_until_runtime_focuses_search() {
        let mut dropdown = single_dropdown();
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let field = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

        dropdown.dispatch_focus(&field, true, &mut focus);
        dropdown.open();

        assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Field));
        assert!(dropdown.data_view.focused_for_test());

        let mut open_layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut open_layout);
        let search = open_layout.focus_targets()[0].clone();
        dropdown.dispatch_focus(&field, false, &mut focus);
        dropdown.dispatch_focus(&search, true, &mut focus);

        assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
        assert!(dropdown.data_view.focused_for_test());
    }

    #[test]
    fn open_preserves_unfocused_state() {
        let mut dropdown = single_dropdown();

        dropdown.open();

        assert!(dropdown.is_open());
        assert!(!dropdown.is_focused());
        assert!(!dropdown.data_view.focused_for_test());
    }

    #[test]
    fn multi_toggle_then_escape_rolls_back() {
        let mut dropdown = multi_dropdown().selected(["Alpha"]);

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(Key::Char(' '), AREA);
        dropdown.on_key(Key::Esc, AREA);

        assert_eq!(dropdown.selected_ids(), vec!["Alpha"]);
    }

    #[test]
    fn ctrl_space_toggles_highlighted_multi_row() {
        let mut dropdown = multi_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        dropdown.on_key(ctrl(' '), AREA);

        assert_eq!(dropdown.draft, vec!["Beta"]);
    }

    #[test]
    fn ctrl_space_commits_highlighted_single_row() {
        let mut dropdown = single_dropdown();

        dropdown.open();
        dropdown.on_key(ctrl('j'), AREA);
        let outcome = dropdown.on_key(ctrl(' '), AREA);

        assert!(outcome.committed);
        assert_eq!(dropdown.selected_id(), Some("Beta"));
        assert!(!dropdown.is_open());
    }

    #[test]
    fn closed_layout_registers_field_focus() {
        let mut dropdown = single_dropdown();
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
        assert!(targets[0].path.is_empty());
    }

    #[test]
    fn filled_variant_registers_compact_field_focus() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id.as_str(), FIELD_FOCUS);
        assert_eq!(targets[0].area.height, 1);
    }

    #[test]
    fn bordered_dropdown_measure_reports_field_height() {
        let dropdown = single_dropdown();

        let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 3);
        assert!(!hint.expand.height);
    }

    #[test]
    fn filled_dropdown_measure_reports_compact_field_height() {
        let dropdown = single_dropdown().variant(DropdownVariant::Filled);

        let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 1);
        assert!(!hint.expand.height);
    }

    #[test]
    fn flex_fit_content_uses_dropdown_variant_height() {
        let mut bordered: Flex<()> =
            Flex::column().child("dropdown", single_dropdown(), FlexItem::fit_content());
        let mut filled: Flex<()> = Flex::column().child(
            "dropdown",
            single_dropdown().variant(DropdownVariant::Filled),
            FlexItem::fit_content(),
        );
        let mut ctx = LayoutCtx::new();

        bordered.layout(Rect::new(0, 0, 24, 10), &mut ctx);
        filled.layout(Rect::new(0, 0, 24, 10), &mut ctx);

        assert_eq!(
            bordered
                .child_rect(&ChildKey::from("dropdown"))
                .unwrap()
                .height,
            3
        );
        assert_eq!(
            filled
                .child_rect(&ChildKey::from("dropdown"))
                .unwrap()
                .height,
            1
        );
    }

    #[test]
    fn flex_horizontal_fit_content_allocates_width_based_on_text() {
        let mut flex: Flex<()> = Flex::row().child(
            "dropdown",
            single_dropdown().selected_one("Beta"),
            FlexItem::fit_content(),
        );
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 40, 3), &mut ctx);

        // "Beta" is 4 cells, plus 2 border cells = 6 width.
        assert_eq!(
            flex.child_rect(&ChildKey::from("dropdown")).unwrap().width,
            6
        );
    }

    #[test]
    fn flex_fit_content_uses_display_width_for_dropdown_text() {
        let mut flex: Flex<()> = Flex::row().child(
            "dropdown",
            Dropdown::single(["界"], |row| *row, |row| row.to_string()).selected_one("界"),
            FlexItem::fit_content(),
        );
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 40, 3), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("dropdown")).unwrap().width,
            4
        );
    }

    #[test]
    fn open_layout_returns_trigger_field_area_only() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        let result = dropdown.layout_overlay::<()>(Rect::new(0, 0, 24, 3), AREA, &mut ctx);

        assert_eq!(result.area, Rect::new(0, 0, 24, 3));
    }

    #[test]
    fn filled_variant_renders_filled_trigger_with_nerd_font_chevron() {
        let dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .selected_one("Beta");
        let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().bg, theme().highlight_bg());
        assert_eq!(buffer.cell((1, 0)).unwrap().symbol(), "B");
        assert_eq!(buffer.cell((10, 0)).unwrap().symbol(), "");
    }

    #[test]
    fn filled_inline_label_renders_label_value_and_hotkey_on_one_line() {
        let dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .label("Lane")
            .hotkey("4")
            .alt_style(true)
            .label_position(DropdownLabelPosition::Inline)
            .selected_one("Gamma");
        let mut terminal = Terminal::new(TestBackend::new(24, 1)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let row = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.starts_with("Lane: Gamma |4|"));
        assert!(row.contains("Lane: Gamma |4|"));
        assert!(
            buffer
                .cell((7, 0))
                .unwrap()
                .modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn filled_alt_top_label_trigger_has_no_leading_padding() {
        let dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .label("Work")
            .hotkey("5")
            .alt_style(true)
            .selected_one("Gamma");
        let mut terminal = Terminal::new(TestBackend::new(24, 2)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let row = (0..24)
            .map(|x| buffer.cell((x, 1)).unwrap().symbol())
            .collect::<String>();
        assert!(row.starts_with("Gamma"));
    }

    #[test]
    fn no_selection_text_renders_empty_value_and_popup_option() {
        let mut dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .no_selection_text("--None--");
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 16, 1),
            Rect::new(0, 0, 16, 8),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(16, 8)).expect("terminal should build");

        terminal
            .draw(|frame| {
                dropdown.render(frame, Rect::new(0, 0, 16, 1));
                dropdown.render_popup_overlay(frame, frame.area());
            })
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let field = (0..16)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        let option = (0..16)
            .map(|x| buffer.cell((x, 2)).unwrap().symbol())
            .collect::<String>();
        assert!(field.contains("--None--"));
        assert!(option.contains("--None--"));
    }

    #[test]
    fn no_selection_text_can_be_selected_to_clear_value() {
        let mut dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .no_selection_text("--None--")
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('k'), AREA);
        dropdown.on_key(Key::Enter, AREA);

        assert_eq!(dropdown.selected_id(), None);
    }

    #[test]
    fn immediate_no_selection_text_clears_value_when_highlighted() {
        let mut dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .search_mode(DropdownSearchMode::None)
            .commit_mode(DropdownCommitMode::Immediate)
            .no_selection_text("--None--")
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('k'), AREA);

        assert_eq!(dropdown.selected_id(), None);
    }

    #[test]
    fn no_selection_highlight_uses_same_style_as_focused_rows() {
        let mut dropdown = single_dropdown()
            .variant(DropdownVariant::Filled)
            .search_mode(DropdownSearchMode::None)
            .no_selection_text("--None--")
            .selected_one("Alpha");

        dropdown.open();
        dropdown.on_key(ctrl('k'), AREA);
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 16, 1),
            Rect::new(0, 0, 16, 8),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(16, 8)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render_popup_overlay(frame, frame.area()))
            .expect("dropdown should render");

        let cell = terminal.backend().buffer().cell((0, 1)).unwrap();
        assert_eq!(cell.fg, theme().highlight_fg());
        assert_eq!(cell.bg, theme().highlight_bg());
        assert!(cell.modifier.contains(Modifier::BOLD));

        let blank_cell = terminal.backend().buffer().cell((15, 1)).unwrap();
        assert_eq!(blank_cell.bg, theme().highlight_bg());
    }

    #[test]
    fn focused_bordered_popup_uses_accent_border() {
        let mut dropdown = single_dropdown();
        let mut initial_layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut initial_layout);
        let field = initial_layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::new(AnimationSettings::default());
        dropdown.dispatch_focus(&field, true, &mut focus);
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 12, 3),
            Rect::new(0, 0, 12, 8),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, Rect::new(0, 0, 12, 3)))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 2)).unwrap().fg, theme().accent_fg());
    }

    #[test]
    fn open_render_draws_trigger_without_inline_popup() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_ne!(buffer.cell((0, 2)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), " ");
    }

    #[test]
    fn open_layout_registers_single_external_search_focus() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].id.as_str(), "input");
        assert!(targets[0].path.is_empty());
    }

    #[test]
    fn open_search_dropdown_suppresses_global_hotkeys_on_field_focus() {
        let mut dropdown = single_dropdown().auto_focus_search(false);
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut ctx);

        let target = ctx
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == FIELD_FOCUS)
            .expect("field focus target");
        assert!(target.suppress_global_hotkeys);
    }

    #[test]
    fn open_layout_focus_targets_use_overlay_popup_areas() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = LayoutCtx::new();

        dropdown.layout_overlay::<()>(Rect::new(0, 0, 24, 3), Rect::new(0, 0, 24, 20), &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets[0].area, Rect::new(1, 3, 22, 1));
    }

    #[test]
    fn tab_from_open_dropdown_cancels_and_requests_next_focus() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(ctx.focus_request(), Some(&crate::FocusRequest::Next));
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn backtab_from_open_dropdown_cancels_and_requests_previous_focus() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut ctx = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::BackTab)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(!dropdown.is_open());
        assert_eq!(ctx.focus_request(), Some(&crate::FocusRequest::Previous));
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn open_dropdown_closes_when_focused_target_blurs() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

        dropdown.dispatch_focus(&target, true, &mut focus);
        dropdown.dispatch_focus(&target, false, &mut focus);

        assert!(!dropdown.is_open());
        assert!(!dropdown.is_focused());
        assert!(focus.redraw_requested());
    }

    #[test]
    fn opening_search_dropdown_does_not_close_during_runtime_field_to_search_transition() {
        let mut dropdown = single_dropdown();
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let field = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::new(AnimationSettings::default());

        dropdown.dispatch_focus(&field, true, &mut focus);
        dropdown.open();
        let mut open_layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut open_layout);
        let search = open_layout.focus_targets()[0].clone();
        dropdown.dispatch_focus(&field, false, &mut focus);
        dropdown.dispatch_focus(&search, true, &mut focus);

        assert!(dropdown.is_open());
        assert_eq!(dropdown.focus_region, Some(DropdownFocusRegion::Search));
    }

    #[test]
    fn open_search_dropdown_can_keep_focus_on_field_when_auto_focus_disabled() {
        let mut dropdown = single_dropdown().auto_focus_search(false);
        dropdown.open();
        let mut layout = LayoutCtx::new();

        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);

        assert_eq!(layout.focus_targets()[0].id.as_str(), "field");
    }

    #[test]
    fn open_layout_sizes_popup_to_visible_items() {
        let mut dropdown = single_dropdown();
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

        assert_eq!(area.height, 3);
    }

    #[test]
    fn bordered_and_filled_popups_size_to_same_content_with_variant_chrome() {
        let mut bordered = single_dropdown();
        bordered.open();
        let mut filled = single_dropdown().variant(DropdownVariant::Filled);
        filled.open();

        let [_, bordered_popup] = bordered.areas(Rect::new(0, 0, 24, 20));
        let [_, filled_popup] = filled.areas(Rect::new(0, 0, 24, 20));
        let [_, bordered_list] = bordered.popup_inner_areas(bordered_popup);
        let [_, filled_list] = filled.popup_inner_areas(filled_popup);

        assert_eq!(bordered_list.height, 3);
        assert_eq!(filled_list.height, 3);
        assert_eq!(bordered_popup.height, 6);
        assert_eq!(filled_popup.height, 4);
    }

    #[test]
    fn bordered_popup_area_overlaps_field_bottom_row() {
        let mut dropdown = single_dropdown();
        dropdown.open();

        let [field_area, popup_area] = dropdown.areas(AREA);

        assert_eq!(popup_area.y, field_area.y + field_area.height - 1);
    }

    #[test]
    fn overlay_popup_extends_beyond_trigger_field_when_bounds_allow() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 24, 3),
            Rect::new(0, 0, 24, 20),
            &mut LayoutCtx::new(),
        );

        let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 24, 20));

        assert_eq!(popup_area, Rect::new(0, 2, 24, 6));
        assert!(popup_area.y + popup_area.height > 3);
    }

    #[test]
    fn centered_popup_overlay_centers_popup_within_bounds() {
        let mut dropdown = single_dropdown().centered(true);
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(2, 2, 24, 3),
            Rect::new(0, 0, 100, 40),
            &mut LayoutCtx::new(),
        );

        let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 100, 40));

        assert_eq!(popup_area, Rect::new(30, 17, 40, 6));
    }

    #[test]
    fn upward_popup_opens_above_trigger() {
        let mut dropdown = single_dropdown().popup_direction(DropdownPopupDirection::Up);
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 10, 24, 1),
            Rect::new(0, 0, 24, 12),
            &mut LayoutCtx::new(),
        );

        let popup_area = dropdown.popup_overlay_area(Rect::new(0, 0, 24, 12));

        assert_eq!(popup_area.y + popup_area.height, 10);
    }

    #[test]
    fn filled_popup_layout_has_no_border_offset() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        dropdown.open();

        let [_, popup_area] = dropdown.areas(AREA);
        let [search_area, list_area] = dropdown.popup_inner_areas(popup_area);

        assert_eq!(search_area.y, popup_area.y);
        assert_eq!(search_area.x, popup_area.x);
        assert_eq!(list_area.y, popup_area.y + 1);
        assert_eq!(list_area.x, popup_area.x);
    }

    #[test]
    fn open_layout_sizes_popup_to_no_results_row() {
        let mut dropdown = single_dropdown();
        dropdown.open();
        dropdown.on_key(char_key('z'), Rect::new(0, 0, 24, 20));

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 20));

        assert_eq!(area.height, 1);
    }

    #[test]
    fn open_layout_caps_popup_at_default_max() {
        let mut dropdown = numeric_dropdown(40);
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

        assert_eq!(area.height, 27);
    }

    #[test]
    fn max_popup_height_overrides_preset_max() {
        let mut dropdown = numeric_dropdown(40).max_popup_height(5);
        dropdown.open();

        let area = open_list_area(&mut dropdown, Rect::new(0, 0, 24, 60));

        assert_eq!(area.height, 2);
    }

    #[test]
    fn filled_popup_caps_height_without_border_chrome() {
        let mut dropdown = numeric_dropdown(40).variant(DropdownVariant::Filled);
        dropdown.open();

        let [_, popup_area] = dropdown.areas(Rect::new(0, 0, 24, 60));
        let [_, list_area] = dropdown.popup_inner_areas(popup_area);

        assert_eq!(popup_area.height, 30);
        assert_eq!(list_area.height, 29);
    }

    #[test]
    fn filled_popup_applies_background_to_content_rows() {
        let mut dropdown = single_dropdown().variant(DropdownVariant::Filled);
        dropdown.open();
        dropdown.layout_overlay::<()>(
            Rect::new(0, 0, 12, 1),
            Rect::new(0, 0, 12, 6),
            &mut LayoutCtx::new(),
        );
        let mut terminal = Terminal::new(TestBackend::new(12, 6)).expect("terminal should build");

        terminal
            .draw(|frame| {
                dropdown.render(frame, Rect::new(0, 0, 12, 1));
                dropdown.render_popup_overlay(frame, frame.area());
            })
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(
            dropdown.popup_content_style().unwrap().bg,
            Some(theme().border_fg())
        );
        assert_eq!(buffer.cell((0, 3)).unwrap().symbol(), "B");
        assert_eq!(buffer.cell((0, 3)).unwrap().bg, theme().border_fg());
    }

    #[test]
    fn node_event_opens_and_requests_layout() {
        let mut dropdown = single_dropdown();
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::default();
        dropdown.dispatch_focus(&target, true, &mut focus);
        let mut event = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut event);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dropdown.is_open());
        assert!(event.layout_requested());
        assert_eq!(event.propagation(), Propagation::Stopped);
    }

    #[test]
    fn hotkey_opens_dropdown() {
        let mut dropdown = single_dropdown().hotkey("d");
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::default();
        dropdown.dispatch_focus(&target, true, &mut focus);
        let mut event = EventCtx::<()>::default();

        let outcome = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('d'))), &mut event);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dropdown.is_open());
    }

    #[test]
    fn uppercase_hotkey_commit_opens_dropdown() {
        let mut dropdown = single_dropdown().hotkey("D");
        let mut event = EventCtx::<()>::default();

        let outcome = dropdown.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("d".to_string())),
            &mut event,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dropdown.is_open());
        assert!(event.layout_requested());
    }

    #[test]
    fn hotkey_commit_focuses_search_when_auto_focus_is_enabled() {
        let mut dropdown = single_dropdown().hotkey("db");
        let mut event = EventCtx::<()>::default();

        let outcome = dropdown.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("db".to_string())),
            &mut event,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(dropdown.is_open());
        assert_eq!(
            event.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::default(),
                id: FocusId::new(SEARCH_FOCUS),
            })
        );
    }

    #[test]
    fn multiletter_hotkey_opens_after_direct_sequence() {
        let mut dropdown = single_dropdown().hotkey("db");

        let pending = dropdown.on_key(KeyEvent::from(Key::Char('d')), AREA);
        let matched = dropdown.on_key(KeyEvent::from(Key::Char('b')), AREA);

        assert!(pending.handled);
        assert!(!pending.opened);
        assert!(matched.handled);
        assert!(matched.opened);
        assert!(dropdown.is_open());
    }

    #[test]
    fn focused_multiletter_hotkey_opens_from_key_events() {
        let mut dropdown = single_dropdown().hotkey("db");
        let mut layout = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, AREA, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let mut focus = FocusCtx::<()>::default();
        dropdown.dispatch_focus(&target, true, &mut focus);
        let mut event = EventCtx::<()>::default();

        let pending = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('d'))), &mut event);
        let matched = dropdown.event(&TuiEvent::Key(KeyEvent::from(Key::Char('b'))), &mut event);

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert!(dropdown.is_open());
    }

    #[test]
    fn dropdown_with_label_and_hotkey_renders_in_borders() {
        let dropdown = single_dropdown().label("Database").hotkey("d");
        let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let top = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(top.contains("Database"));

        let bottom = (0..24)
            .map(|x| buffer.cell((x, 2)).unwrap().symbol())
            .collect::<String>();
        assert!(bottom.contains("┤d│"));
    }

    #[test]
    fn dropdown_with_alternative_style_layout_and_render() {
        let mut dropdown = single_dropdown()
            .label("Search")
            .hotkey("s")
            .alt_style(true);

        let area = Rect::new(0, 0, 24, 4);
        let mut ctx = LayoutCtx::new();
        <Dropdown<_, _> as TuiNode<()>>::layout(&mut dropdown, area, &mut ctx);

        let hint = <Dropdown<_, _> as TuiNode<()>>::measure(&dropdown, LayoutProposal::unbounded());
        assert_eq!(hint.preferred.height, 4);

        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");
        terminal
            .draw(|frame| dropdown.render(frame, frame.area()))
            .expect("dropdown should render");

        let buffer = terminal.backend().buffer();
        let row0 = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row0.contains("Search |s|"));

        let row1 = (0..24)
            .map(|x| buffer.cell((x, 1)).unwrap().symbol())
            .collect::<String>();
        assert!(row1.contains("╭"));
        assert!(row1.contains("╮"));
    }

    fn char_key(value: char) -> KeyEvent {
        KeyEvent {
            code: Key::Char(value),
            modifiers: KeyModifiers::NONE,
        }
    }

    fn ctrl(value: char) -> KeyEvent {
        KeyEvent {
            code: Key::Char(value),
            modifiers: KeyModifiers::CONTROL,
        }
    }

    fn open_list_area<T, Id>(dropdown: &mut Dropdown<T, Id>, area: Rect) -> Rect
    where
        T: 'static,
        Id: Clone + Eq + Hash + 'static,
    {
        let popup_area = dropdown.popup_overlay_area(area);
        dropdown.popup_inner_areas(popup_area)[1]
    }
}
