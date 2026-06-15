use std::collections::HashSet;
use std::hash::Hash;
use std::time::Duration;

mod layout;
mod model;
mod render;
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
    Animated, AnimationSettings, ScrollAxes, ScrollBehavior, ScrollDelta, ScrollOffset,
    ScrollOutcome, ScrollState, ScrollbarConfig, TickResult, animation_settings, keybindings,
    preset,
};

pub use model::{
    CellContext, Column, DataViewEvent, DataViewOutcome, DataViewPagination, DataViewSort,
    SortDirection, TreeAdapter, TreeGlyphs,
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
    tree_glyphs: TreeGlyphs,
    pending_g: bool,
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
        self.sort = Some(DataViewSort {
            column_id: column_id.into(),
            direction,
        });
        self.highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
        DataViewOutcome::CHANGED
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
            self.sort = None;
            self.highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
            DataViewOutcome::CHANGED
        }
    }

    pub fn next_page(&mut self) -> DataViewOutcome {
        let max_page = self.max_page();
        let Some(pagination) = &mut self.pagination else {
            return DataViewOutcome::IDLE;
        };
        let next = pagination.page.saturating_add(1).min(max_page);
        let changed = next != pagination.page;
        pagination.page = next;
        self.highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
        DataViewOutcome {
            handled: true,
            changed,
            active: false,
            activated: false,
        }
    }

    fn next_page_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.next_page();
        if outcome.changed {
            self.ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed)
        } else {
            outcome
        }
    }

    pub fn previous_page(&mut self) -> DataViewOutcome {
        let Some(pagination) = &mut self.pagination else {
            return DataViewOutcome::IDLE;
        };
        let previous = pagination.page.saturating_sub(1);
        let changed = previous != pagination.page;
        pagination.page = previous;
        self.highlighted = self.highlighted.min(self.visible_len().saturating_sub(1));
        DataViewOutcome {
            handled: true,
            changed,
            active: false,
            activated: false,
        }
    }

    fn previous_page_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.previous_page();
        if outcome.changed {
            self.ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed)
        } else {
            outcome
        }
    }

    pub fn collapse_all(&mut self) -> DataViewOutcome {
        let changed = !self.expanded.is_empty();
        self.expanded.clear();
        let clamped = self.clamp_visible_state();
        DataViewOutcome {
            handled: true,
            changed: changed || clamped,
            active: false,
            activated: false,
        }
    }

    fn collapse_all_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.collapse_all();
        if outcome.changed {
            self.ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed)
        } else {
            outcome
        }
    }

    pub fn expand_all(&mut self) -> DataViewOutcome {
        let ids = self.expandable_ids().collect::<HashSet<_>>();
        let changed = self.expanded != ids;
        self.expanded = ids;
        let clamped = self.clamp_visible_state();
        DataViewOutcome {
            handled: true,
            changed: changed || clamped,
            active: false,
            activated: false,
        }
    }

    fn expand_all_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.expand_all();
        if outcome.changed {
            self.ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed)
        } else {
            outcome
        }
    }

    pub fn highlighted_id(&self) -> Option<Id> {
        self.visible_rows()
            .get(self.highlighted)
            .map(|row| row.id.clone())
    }

    pub fn take_last_activated(&mut self) -> Option<DataViewEvent<Id>> {
        self.last_activated
            .take()
            .map(|row_id| DataViewEvent { row_id })
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
        let page = self.visible_page_step(area);
        let keys = keybindings();
        let data_keys = keys.data_view();
        if let Some(delta) = horizontal_jump(key) {
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

    fn activate_highlighted(&mut self) -> DataViewOutcome {
        let Some(row_id) = self.highlighted_id() else {
            return DataViewOutcome::IDLE;
        };
        self.last_activated = Some(row_id);
        DataViewOutcome {
            handled: true,
            changed: false,
            active: false,
            activated: true,
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
            return DataViewOutcome::HANDLED;
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
        let changed = highlighted != self.highlighted;
        self.highlighted = highlighted;
        self.ensure_highlight_visible(area, settings)
            .into_data_view_outcome(true, changed)
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
        let current = self.scroll.target_offset().y;
        let target = if self.highlighted < current {
            self.highlighted
        } else if self.highlighted >= current.saturating_add(viewport_height) {
            self.highlighted
                .saturating_add(1)
                .saturating_sub(viewport_height)
        } else {
            current
        };
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
        let highlighted_changed = highlighted != self.highlighted;
        self.highlighted = highlighted;
        page_changed || highlighted_changed
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

fn horizontal_jump(key: KeyEvent) -> Option<isize> {
    let shifted = key.modifiers.contains(KeyModifiers::SHIFT)
        || matches!(key.code, Key::Char(c) if c.is_ascii_uppercase());
    let plain_shift = shifted
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    if !plain_shift {
        return None;
    }

    match key.code {
        Key::Left | Key::Char('h') | Key::Char('H') => Some(-HORIZONTAL_JUMP),
        Key::Right | Key::Char('l') | Key::Char('L') => Some(HORIZONTAL_JUMP),
        _ => None,
    }
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
