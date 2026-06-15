use std::collections::HashSet;
use std::hash::Hash;
use std::time::Duration;

mod layout;
mod model;
mod render;
mod selection;
#[cfg(test)]
mod tests;
mod tree_rows;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Rect};
use tuirealm::state::State;

use crate::{
    Animated, AnimationSettings, KeyBindings, ScrollAxes, ScrollBehavior, ScrollDelta,
    ScrollOffset, ScrollOutcome, ScrollState, ScrollbarConfig, TickResult, animation_settings,
    keybindings, preset,
};

pub use model::{
    ActivationMode, CellContext, CheckState, Column, DataViewEvent, DataViewOutcome,
    DataViewPagination, DataViewSort, DataViewTypedEvent, SelectionGlyphs, SelectionMode,
    SelectionPropagation, SelectionTrigger, SortDirection, TreeAdapter, TreeGlyphs,
};
use model::{RowIdFn, VisibleRow};

const HORIZONTAL_JUMP: isize = 8;

pub struct DataView<T, Id> {
    rows: Vec<T>,
    columns: Vec<Column<T, Id>>,
    row_id: Box<RowIdFn<T, Id>>,
    tree: Option<TreeAdapter<T, Id>>,
    expanded: HashSet<Id>,
    highlighted: usize,
    focused: bool,
    headers: bool,
    scroll: ScrollState,
    sort: Option<DataViewSort>,
    pagination: Option<DataViewPagination>,
    last_activated: Option<Id>,
    events: Vec<DataViewTypedEvent<Id>>,
    activation_mode: ActivationMode,
    selection_mode: SelectionMode,
    selection_trigger: SelectionTrigger,
    selection_propagation: SelectionPropagation,
    selected: HashSet<Id>,
    selection_glyphs: SelectionGlyphs,
    tree_glyphs: TreeGlyphs,
    pending_g: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HighlightUpdate {
    index_changed: bool,
    activated: bool,
    selection_changed: bool,
}

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub fn new(rows: impl IntoIterator<Item = T>, row_id: impl Fn(&T) -> Id + 'static) -> Self {
        Self {
            rows: rows.into_iter().collect(),
            columns: Vec::new(),
            row_id: Box::new(row_id),
            tree: None,
            expanded: HashSet::new(),
            highlighted: 0,
            focused: false,
            headers: false,
            scroll: ScrollState::from_preset(ScrollAxes::Both, preset().scroll()),
            sort: None,
            pagination: None,
            last_activated: None,
            events: Vec::new(),
            activation_mode: ActivationMode::default(),
            selection_mode: SelectionMode::default(),
            selection_trigger: SelectionTrigger::default(),
            selection_propagation: SelectionPropagation::default(),
            selected: HashSet::new(),
            selection_glyphs: SelectionGlyphs::NERD_FONT,
            tree_glyphs: TreeGlyphs::NERD_FONT,
            pending_g: false,
        }
    }

    pub fn list(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        accessor: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self::new(rows, row_id).column(Column::text(
            "label",
            "",
            Constraint::Percentage(100),
            accessor,
        ))
    }

    pub fn column(mut self, column: Column<T, Id>) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = Column<T, Id>>) -> Self {
        self.columns.extend(columns);
        self
    }

    pub fn headers(mut self, headers: bool) -> Self {
        self.headers = headers;
        self
    }

    pub fn tree(mut self, tree: TreeAdapter<T, Id>) -> Self {
        self.tree = Some(tree);
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }

    pub fn tree_glyphs(mut self, glyphs: TreeGlyphs) -> Self {
        self.tree_glyphs = glyphs;
        self
    }

    pub fn activation_mode(mut self, mode: ActivationMode) -> Self {
        self.activation_mode = mode;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn pagination(mut self, page_size: usize) -> Self {
        self.pagination = (page_size > 0).then_some(DataViewPagination { page_size, page: 0 });
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        self.scroll = self.scroll.behavior(behavior);
        self
    }

    pub fn scrollbars(mut self, config: ScrollbarConfig) -> Self {
        self.scroll = self.scroll.scrollbars(config);
        self
    }

    pub fn sort_by(
        &mut self,
        column_id: impl Into<String>,
        direction: SortDirection,
    ) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        self.sort = Some(DataViewSort {
            column_id: column_id.into(),
            direction,
        });
        let update = self.set_highlighted_index_from(
            self.highlighted.min(self.visible_len().saturating_sub(1)),
            before_id,
        );
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    pub fn toggle_sort(&mut self, column_id: impl Into<String>) -> DataViewOutcome {
        let column_id = column_id.into();
        let next = match &self.sort {
            Some(sort)
                if sort.column_id == column_id && sort.direction == SortDirection::Descending =>
            {
                None
            }
            Some(sort) if sort.column_id == column_id => Some(sort.direction.reversed()),
            _ => Some(SortDirection::Ascending),
        };

        if let Some(direction) = next {
            self.sort_by(column_id, direction)
        } else {
            let before_id = self.highlighted_id();
            self.sort = None;
            let update = self.set_highlighted_index_from(
                self.highlighted.min(self.visible_len().saturating_sub(1)),
                before_id,
            );
            DataViewOutcome {
                handled: true,
                changed: true,
                active: false,
                activated: update.activated,
            }
        }
    }

    pub fn next_page(&mut self) -> DataViewOutcome {
        let max_page = self.max_page();
        let before_id = self.highlighted_id();
        let Some(pagination) = &mut self.pagination else {
            return DataViewOutcome::IDLE;
        };
        let next = pagination.page.saturating_add(1).min(max_page);
        let changed = next != pagination.page;
        pagination.page = next;
        let highlight = self.highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index_from(highlight, before_id);
        DataViewOutcome {
            handled: true,
            changed: changed || update.selection_changed,
            active: false,
            activated: update.activated,
        }
    }

    fn next_page_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.next_page();
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            outcome
        }
    }

    pub fn previous_page(&mut self) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        let Some(pagination) = &mut self.pagination else {
            return DataViewOutcome::IDLE;
        };
        let previous = pagination.page.saturating_sub(1);
        let changed = previous != pagination.page;
        pagination.page = previous;
        let highlight = self.highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index_from(highlight, before_id);
        DataViewOutcome {
            handled: true,
            changed: changed || update.selection_changed,
            active: false,
            activated: update.activated,
        }
    }

    fn previous_page_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.previous_page();
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            outcome
        }
    }

    pub fn collapse_all(&mut self) -> DataViewOutcome {
        if self.tree.is_none() || self.expanded.is_empty() {
            return DataViewOutcome::IDLE;
        }
        let before_id = self.highlighted_id();
        self.expanded.clear();
        let (_, update) = self.clamp_visible_state_from(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    fn collapse_all_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.collapse_all();
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            outcome
        }
    }

    pub fn expand_all(&mut self) -> DataViewOutcome {
        if self.tree.is_none() {
            return DataViewOutcome::IDLE;
        }
        let before_id = self.highlighted_id();
        let ids = self.expandable_ids().collect::<HashSet<_>>();
        if ids.is_empty() || self.expanded == ids {
            return DataViewOutcome::IDLE;
        }
        self.expanded = ids;
        let (_, update) = self.clamp_visible_state_from(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    fn expand_all_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.expand_all();
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            outcome
        }
    }

    pub fn highlighted_id(&self) -> Option<Id> {
        self.visible_rows()
            .get(self.highlighted)
            .map(|row| row.id.clone())
    }

    pub fn on_key(&mut self, key: KeyEvent, viewport: Rect) -> DataViewOutcome {
        self.on_key_with_settings(key, viewport, animation_settings())
    }

    pub fn on_key_with_settings(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let keys = keybindings();
        self.on_key_with_settings_and_bindings(key, area, settings, &keys)
    }

    fn on_key_with_settings_and_bindings(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
        keys: &KeyBindings,
    ) -> DataViewOutcome {
        let page = self.visible_page_step(area);
        let data_keys = keys.data_view();
        if let Some(delta) = horizontal_jump(keys, key) {
            self.pending_g = false;
            self.scroll_horizontal_by(delta, area, settings)
        } else if keys.line_up_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.highlighted.saturating_sub(1), area, settings)
        } else if keys.line_down_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.highlighted.saturating_add(1), area, settings)
        } else if keys.line_left_matches(key) {
            self.pending_g = false;
            self.navigate_or_scroll_left(key, area, settings)
        } else if keys.line_right_matches(key) {
            self.pending_g = false;
            self.navigate_or_scroll_right(key, area, settings)
        } else if keys.page_up_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.highlighted.saturating_sub(page), area, settings)
        } else if keys.page_down_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.highlighted.saturating_add(page), area, settings)
        } else if keys.home_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(0, area, settings)
        } else if keys.end_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.visible_len().saturating_sub(1), area, settings)
        } else if data_keys.activate_matches(key) {
            self.pending_g = false;
            self.activate_highlighted()
        } else if data_keys.toggle_selection_matches(key) {
            self.pending_g = false;
            self.toggle_highlighted_selection()
        } else if data_keys.toggle_expansion_matches(key) {
            self.pending_g = false;
            self.toggle_highlighted_expansion(area, settings)
        } else if data_keys.next_page_matches(key) {
            self.pending_g = false;
            self.next_page_with_settings(area, settings)
        } else if data_keys.previous_page_matches(key) {
            self.pending_g = false;
            self.previous_page_with_settings(area, settings)
        } else if data_keys.collapse_all_matches(key) {
            self.pending_g = false;
            self.collapse_all_with_settings(area, settings)
        } else if data_keys.expand_all_matches(key) {
            self.pending_g = false;
            self.expand_all_with_settings(area, settings)
        } else if data_keys.top_prefix_matches(key) {
            self.handle_g(area, settings)
        } else if data_keys.bottom_matches(key) {
            self.pending_g = false;
            self.highlight_with_settings(self.visible_len().saturating_sub(1), area, settings)
        } else {
            self.pending_g = false;
            DataViewOutcome::IDLE
        }
    }

    fn toggle_highlighted_expansion(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let visible = self.visible_rows();
        let Some(row) = visible.get(self.highlighted) else {
            return DataViewOutcome::IDLE;
        };
        if !row.has_children {
            return DataViewOutcome::IDLE;
        }
        let id = row.id.clone();
        drop(visible);
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.clamp_visible_state();
        self.ensure_highlight_visible(area, settings)
            .into_data_view_outcome(true, true)
    }

    fn expand_or_first_child(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let visible = self.visible_rows();
        let Some(row) = visible.get(self.highlighted) else {
            return DataViewOutcome::IDLE;
        };
        if !row.has_children {
            return DataViewOutcome::HANDLED;
        }
        if !row.expanded {
            let id = row.id.clone();
            drop(visible);
            self.expanded.insert(id);
            return self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(true, true);
        }
        let first_child = visible
            .get(self.highlighted.saturating_add(1))
            .is_some_and(|child| child.depth > row.depth);
        drop(visible);
        if first_child {
            self.highlight_with_settings(self.highlighted.saturating_add(1), area, settings)
        } else {
            DataViewOutcome::HANDLED
        }
    }

    fn collapse_or_parent(&mut self, area: Rect, settings: AnimationSettings) -> DataViewOutcome {
        let visible = self.visible_rows();
        let Some(row) = visible.get(self.highlighted) else {
            return DataViewOutcome::IDLE;
        };
        if row.has_children && row.expanded {
            let id = row.id.clone();
            drop(visible);
            self.expanded.remove(&id);
            self.clamp_visible_state();
            return self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(true, true);
        }
        let parent_id = row.parent_id.clone();
        drop(visible);
        if let Some(parent_id) = parent_id
            && let Some(parent_index) = self
                .visible_rows()
                .iter()
                .position(|row| row.id == parent_id)
        {
            self.highlight_with_settings(parent_index, area, settings)
        } else {
            DataViewOutcome::HANDLED
        }
    }

    fn navigate_or_scroll_left(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if self.tree.is_none() {
            return self.scroll_horizontal(key, area, settings);
        }

        let outcome = self.collapse_or_parent(area, settings);
        if outcome.changed || outcome.active || outcome.activated {
            outcome
        } else {
            self.scroll_horizontal(key, area, settings)
        }
    }

    fn navigate_or_scroll_right(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if self.tree.is_none() {
            return self.scroll_horizontal(key, area, settings);
        }

        let outcome = self.expand_or_first_child(area, settings);
        if outcome.changed || outcome.active || outcome.activated {
            outcome
        } else {
            self.scroll_horizontal(key, area, settings)
        }
    }

    fn scroll_horizontal(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let geometry = self.scroll_geometry(area);
        self.scroll
            .on_key(key, geometry.viewport, geometry.content, settings)
            .into_data_view_outcome(true, false)
    }

    fn scroll_horizontal_by(
        &mut self,
        delta: isize,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let geometry = self.scroll_geometry(area);
        self.scroll
            .scroll_by(
                ScrollDelta::new(delta, 0),
                geometry.viewport,
                geometry.content,
                settings,
            )
            .into_data_view_outcome(true, false)
    }

    fn highlight_with_settings(
        &mut self,
        highlighted: usize,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let highlighted = highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index(highlighted);
        let changed = update.index_changed || update.selection_changed;
        let mut outcome = self
            .ensure_highlight_visible(area, settings)
            .into_data_view_outcome(true, changed);
        outcome.activated = update.activated;
        outcome
    }

    fn visible_page_step(&self, area: Rect) -> usize {
        let height = self.scroll_geometry(area).viewport.height.max(1);
        ((height * 3).saturating_add(4)) / 5
    }

    fn handle_g(&mut self, area: Rect, settings: AnimationSettings) -> DataViewOutcome {
        if self.pending_g {
            self.pending_g = false;
            self.highlight_with_settings(0, area, settings)
        } else {
            self.pending_g = true;
            DataViewOutcome::HANDLED
        }
    }

    fn ensure_highlight_visible(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let viewport_height = geometry.viewport.height.max(1);
        let target = self.highlighted.saturating_sub(viewport_height / 2);
        self.scroll.scroll_to(
            ScrollOffset::new(self.scroll.target_offset().x, target),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    fn visible_len(&self) -> usize {
        self.visible_rows().len()
    }

    fn clamp_visible_state(&mut self) -> bool {
        let page_changed = self.clamp_page();
        let highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index(highlighted);
        page_changed || update.index_changed || update.selection_changed || update.activated
    }

    fn set_highlighted_index(&mut self, highlighted: usize) -> HighlightUpdate {
        let before_id = self.highlighted_id();
        self.set_highlighted_index_from(highlighted, before_id)
    }

    fn set_highlighted_index_from(
        &mut self,
        highlighted: usize,
        before_id: Option<Id>,
    ) -> HighlightUpdate {
        let before_index = self.highlighted;
        self.highlighted = highlighted;
        let after_id = self.highlighted_id();
        if before_id == after_id {
            return HighlightUpdate {
                index_changed: before_index != highlighted,
                activated: false,
                selection_changed: false,
            };
        }

        self.events.push(DataViewTypedEvent::HighlightChanged {
            row_id: after_id.clone(),
        });
        let mut activated = false;
        let mut selection_changed = false;
        if let Some(row_id) = after_id {
            if self.selection_trigger == SelectionTrigger::OnNavigate {
                selection_changed = self.select_id_internal(row_id.clone());
            }
            if self.activation_mode == ActivationMode::OnNavigate {
                self.emit_activation(row_id);
                activated = true;
            }
        }

        HighlightUpdate {
            index_changed: before_index != highlighted,
            activated,
            selection_changed,
        }
    }

    fn clamp_visible_state_from(&mut self, before_id: Option<Id>) -> (bool, HighlightUpdate) {
        let page_changed = self.clamp_page();
        let highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index_from(highlighted, before_id);
        (page_changed, update)
    }

    fn clamp_page(&mut self) -> bool {
        let max_page = self.max_page();
        let Some(pagination) = &mut self.pagination else {
            return false;
        };
        let page = pagination.page.min(max_page);
        let changed = page != pagination.page;
        pagination.page = page;
        changed
    }
}

fn horizontal_jump(keys: &KeyBindings, key: KeyEvent) -> Option<isize> {
    let shifted = key.modifiers.contains(KeyModifiers::SHIFT)
        || matches!(key.code, Key::Char(c) if c.is_ascii_uppercase());
    let plain_shift = shifted
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    if !plain_shift {
        return None;
    }

    let base_key = unshift_key(key);
    if keys.line_left_matches(base_key) {
        Some(-HORIZONTAL_JUMP)
    } else if keys.line_right_matches(base_key) {
        Some(HORIZONTAL_JUMP)
    } else {
        None
    }
}

fn unshift_key(mut key: KeyEvent) -> KeyEvent {
    key.modifiers.remove(KeyModifiers::SHIFT);
    if let Key::Char(c) = key.code {
        key.code = Key::Char(c.to_ascii_lowercase());
    }
    key
}

impl<T, Id> Animated for DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.scroll.tick(dt, settings)
    }
}

impl<T, Id> Component for DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.render(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus
            && let AttrValue::Flag(focused) = value
        {
            self.focused = focused;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

trait ScrollOutcomeExt {
    fn into_data_view_outcome(self, handled: bool, changed: bool) -> DataViewOutcome;
}

impl ScrollOutcomeExt for ScrollOutcome {
    fn into_data_view_outcome(self, handled: bool, changed: bool) -> DataViewOutcome {
        DataViewOutcome {
            handled: handled || self.handled,
            changed: changed || self.changed,
            active: self.active,
            activated: false,
        }
    }
}
