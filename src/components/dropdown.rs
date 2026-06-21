use std::hash::Hash;
use std::rc::Rc;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::components::{Column, DataView, SelectionMode, TextInput};
use crate::event::{Key, KeyEvent};
use crate::search::{SearchMode, search_ranked};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, EventCtx, EventOutcome, EventRoute,
    FocusCtx, FocusId, FocusRequest, FocusTarget, HintSource, HotkeyEvent, HotkeyLabelMode,
    HotkeyMatch, HotkeySequenceMatcher, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize,
    LayoutSizeHint, TickResult, TreePath, TuiEvent, TuiNode, Tween, border_set, hotkey_badge_width,
    hotkey_edge_spans, hotkey_label_spans, hotkey_sequence_to_event, hotkey_underline_style,
    keybindings, line_width, preset, theme,
};

use super::text_input::{CursorFade, placeholder_line};

mod types;
mod util;
use types::DropdownFocusRegion;
pub use types::{
    DropdownActionKeys, DropdownCommitMode, DropdownLabelPosition, DropdownOutcome,
    DropdownPopupDirection, DropdownSearchMode, DropdownVariant,
};
use util::{
    bounded_title, clip_rect, connected_popup_border_set, hotkey_matches_sequence, keys_match,
    matches_any,
};

const DROPDOWN_BACKDROP_AMOUNT: f64 = 0.55;

const FIELD_FOCUS: &str = "field";
const SEARCH_FOCUS: &str = "input";
const POPUP_BORDER_HEIGHT: u16 = 2;

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
    backdrop_tween: Tween,
    pending_hotkey_prefix: Option<String>,
    scroll_highlight_on_next_layout: bool,
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
            backdrop_tween: Tween::idle(0.0),
            pending_hotkey_prefix: None,
            scroll_highlight_on_next_layout: false,
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

    fn field_is_focused(&self) -> bool {
        self.focus_region == Some(DropdownFocusRegion::Field)
    }

    fn chrome_is_active(&self) -> bool {
        self.open || self.field_is_focused()
    }

    pub fn open(&mut self) -> DropdownOutcome {
        if self.open {
            return DropdownOutcome::HANDLED;
        }

        self.open = true;
        self.backdrop_tween.snap_to(DROPDOWN_BACKDROP_AMOUNT);
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
        self.scroll_highlight_on_next_layout = true;
        DropdownOutcome {
            handled: true,
            changed: true,
            opened: true,
            ..DropdownOutcome::IDLE
        }
    }

    fn start_backdrop_tween(&mut self, active: bool, settings: AnimationSettings) {
        let target = if active {
            DROPDOWN_BACKDROP_AMOUNT
        } else {
            0.0
        };
        let resolved = settings.resolve(AnimationSpec::default());
        if !resolved.enabled {
            self.backdrop_tween.snap_to(target);
            return;
        }
        self.backdrop_tween.start(
            self.backdrop_tween.value(),
            target,
            resolved.duration,
            resolved.easing,
        );
    }

    pub fn close(&mut self) -> DropdownOutcome {
        if !self.open {
            return DropdownOutcome::HANDLED;
        }
        let had_focus = self.is_focused();
        self.open = false;
        self.backdrop_tween.snap_to(0.0);
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
        if !self.open {
            return DropdownOutcome::HANDLED;
        }

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

        let field_area = self.field_area(area);
        self.render_field(frame, field_area);
    }

    pub fn render_popup_overlay(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }

        let popup_area = self.popup_overlay_area(bounds);
        if !popup_area.is_empty() {
            let field_area = self.effective_field_area(bounds);
            let backdrop = self.backdrop_tween.value();
            if backdrop > 0.0 {
                super::dialog::dim_backdrop_buffer_except(
                    frame,
                    bounds,
                    backdrop,
                    &[field_area, popup_area],
                );
            }
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
        if self.scroll_highlight_on_next_layout {
            self.data_view.snap_highlight_centered(rows_area);
            self.scroll_highlight_on_next_layout = false;
        }
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
            let committed = self.commit_mode == DropdownCommitMode::Immediate;
            if self.commit_mode == DropdownCommitMode::Immediate {
                self.committed.clear();
            }
            return DropdownOutcome {
                committed,
                ..DropdownOutcome::changed()
            };
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
                    .fg(if self.chrome_is_active() {
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
            .border_style(Style::default().fg(if self.chrome_is_active() {
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
                None,
                inner.width as usize,
                self.focus_region == Some(DropdownFocusRegion::Field),
                None,
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
                    None,
                    text_area.width as usize,
                    self.focus_region == Some(DropdownFocusRegion::Field),
                    None,
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
            .fg(if self.chrome_is_active() {
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
        let border_style = Style::default().fg(if self.chrome_is_active() {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        let title_style = Style::default().fg(if self.chrome_is_active() {
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
                            self.backdrop_tween.snap_to(0.0);
                            self.start_backdrop_tween(true, ctx.animation());
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
            self.start_backdrop_tween(false, ctx.animation());
            ctx.focus_next();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if self.open && focus_keys.previous_matches(*key) {
            self.cancel();
            self.start_backdrop_tween(false, ctx.animation());
            ctx.focus_previous();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key, self.overlay_bounds);
        if outcome.opened || outcome.closed {
            if outcome.opened {
                self.backdrop_tween.snap_to(0.0);
            }
            self.start_backdrop_tween(outcome.opened, ctx.animation());
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
            self.start_backdrop_tween(false, ctx.animation());
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
            .merge(self.backdrop_tween.tick(dt, settings))
            .merge(hotkey_tick)
    }
}

#[cfg(test)]
mod tests;
