use std::collections::HashSet;
use std::hash::Hash;
use std::time::Duration;

use ratatui::layout::{Constraint, Rect};

mod filters;
mod layout;
mod model;
mod node;
mod render;
mod selection;
#[cfg(test)]
mod tests;
mod tree_rows;

#[cfg(test)]
use crate::Animated;
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::search::SearchMode;
use crate::{
    AnimationSettings, ChildKey, EventCtx, FocusId, FocusRequest, KeyBindings, ScrollAxes,
    ScrollBehavior, ScrollDelta, ScrollOffset, ScrollOutcome, ScrollState, ScrollbarConfig,
    animation_settings, keybindings, preset,
};

use super::{
    Dropdown, DropdownCommitMode, DropdownLabelPosition, DropdownOutcome, DropdownSearchMode,
    DropdownVariant, text_input::TextInput,
};

pub use model::{
    ActivationMode, CellContext, CheckState, Column, ColumnSizing, DataViewEvent, DataViewFilter,
    DataViewOutcome, DataViewPagination, DataViewSort, DataViewTransformMode,
    DataViewTransformState, DataViewTypedEvent, SelectionGlyphs, SelectionMode,
    SelectionPropagation, SelectionTrigger, SortDirection, TreeAdapter, TreeGlyphs,
};
use model::{RowIdFn, VisibleRow};

const HORIZONTAL_JUMP: isize = 8;
const CELL_RIGHT_PADDING: usize = 1;
const DATA_VIEW_FOCUS: &str = "data-view";
const SEARCH_SLOT: &str = "search";
const FILTER_DROPDOWN_SLOT: &str = "filter-dropdown";
const TEXT_INPUT_FOCUS: &str = "input";
const DROPDOWN_SEARCH_FOCUS: &str = "input";
const EMPTY_CHOICE_ID: &str = "";
const HEADER_PICK_TIMEOUT: Duration = Duration::from_secs(1);

type ChoiceDropdown = Dropdown<DataViewChoice, String>;

pub(crate) fn search_focus_id() -> FocusId {
    FocusId::new(TEXT_INPUT_FOCUS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DataViewChoice {
    id: String,
    label: String,
}

pub struct DataView<T, Id> {
    rows: Vec<T>,
    visible_row_indices: Option<Vec<usize>>,
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
    hotkey: Option<String>,
    pending_g: bool,
    area: Rect,
    action_bar: bool,
    filter_controls: bool,
    transform_state: DataViewTransformState,
    transform_mode: DataViewTransformMode,
    search_mode: SearchMode,
    interaction: DataViewInteraction,
    search_input: TextInput<()>,
    filter_dropdown: Option<Box<ChoiceDropdown>>,
    header_pick_elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DataViewInteraction {
    Grid,
    Search,
    HeaderFilter,
    FilterValues { column_id: String },
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
            visible_row_indices: None,
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
            hotkey: None,
            pending_g: false,
            area: Rect::default(),
            action_bar: false,
            filter_controls: true,
            transform_state: DataViewTransformState::default(),
            transform_mode: DataViewTransformMode::Local,
            search_mode: SearchMode::Fuzzy,
            interaction: DataViewInteraction::Grid,
            search_input: TextInput::new()
                .placeholder("Search...")
                .hotkey("/")
                .hotkey_focus_enabled(false),
            filter_dropdown: None,
            header_pick_elapsed: Duration::ZERO,
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

    pub fn action_bar(mut self, action_bar: bool) -> Self {
        self.action_bar = action_bar;
        self
    }

    pub fn filter_controls(mut self, enabled: bool) -> Self {
        self.filter_controls = enabled;
        self
    }

    pub fn search_mode(mut self, mode: SearchMode) -> Self {
        self.search_mode = mode;
        self
    }

    pub fn visible_row_ids(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.visible_row_indices = Some(self.row_indices_for_ids(ids));
        self.highlighted = 0;
        self.clamp_page();
        self
    }

    pub fn set_visible_row_ids(&mut self, ids: impl IntoIterator<Item = Id>) -> DataViewOutcome {
        let indices = self.row_indices_for_ids(ids);
        self.replace_visible_row_indices(Some(indices))
    }

    pub fn clear_visible_row_ids(&mut self) -> DataViewOutcome {
        self.replace_visible_row_indices(None)
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

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
    }

    pub fn transform_state(&self) -> &DataViewTransformState {
        &self.transform_state
    }

    pub fn transform_mode(&self) -> DataViewTransformMode {
        self.transform_mode
    }

    pub fn set_transform_mode(&mut self, mode: DataViewTransformMode) -> DataViewOutcome {
        if self.transform_mode == mode {
            return DataViewOutcome::IDLE;
        }
        let before_id = self.highlighted_id();
        self.transform_mode = mode;
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    pub fn set_search_query(&mut self, query: impl Into<String>) -> DataViewOutcome {
        let query = query.into();
        if self.transform_state.search == query {
            return DataViewOutcome::IDLE;
        }
        let before_id = self.highlighted_id();
        self.transform_state.search = query;
        self.search_input
            .set_value(self.transform_state.search.clone());
        self.emit_transform_changed();
        self.outcome_after_transform_change(before_id)
    }

    pub fn clear_search(&mut self) -> DataViewOutcome {
        self.set_search_query(String::new())
    }

    pub fn set_filter(
        &mut self,
        column_id: impl Into<String>,
        value: impl Into<String>,
    ) -> DataViewOutcome {
        let column_id = column_id.into();
        let value = value.into();
        if value.is_empty() {
            return self.clear_filter(&column_id);
        }
        let before_id = self.highlighted_id();
        if let Some(filter) = self
            .transform_state
            .filters
            .iter_mut()
            .find(|filter| filter.column_id == column_id)
        {
            if filter.value == value {
                return DataViewOutcome::IDLE;
            }
            filter.value = value;
        } else {
            self.transform_state
                .filters
                .push(DataViewFilter { column_id, value });
        }
        self.emit_transform_changed();
        self.outcome_after_transform_change(before_id)
    }

    pub fn clear_filter(&mut self, column_id: &str) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        let before_len = self.transform_state.filters.len();
        self.transform_state
            .filters
            .retain(|filter| filter.column_id != column_id);
        if self.transform_state.filters.len() == before_len {
            return DataViewOutcome::IDLE;
        }
        self.emit_transform_changed();
        self.outcome_after_transform_change(before_id)
    }

    pub fn clear_filters(&mut self) -> DataViewOutcome {
        if self.transform_state.filters.is_empty() {
            return DataViewOutcome::IDLE;
        }
        let before_id = self.highlighted_id();
        self.transform_state.filters.clear();
        self.emit_transform_changed();
        self.outcome_after_transform_change(before_id)
    }

    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        self.rows = rows.into_iter().collect();
        self.trim_visible_row_indices();
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    pub fn append_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        self.extend_rows(rows)
    }

    pub fn extend_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        self.rows.extend(rows);
        self.clamp_visible_state();
        DataViewOutcome::CHANGED
    }

    #[cfg(test)]
    pub(crate) fn focused_for_test(&self) -> bool {
        self.focused
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

    pub fn highlighted_json(&self) -> Option<String> {
        let rows = self.visible_rows();
        let row = rows.get(self.highlighted)?;
        let mut value = serde_json::Map::new();
        for column in &self.columns {
            let line = (column.renderer)(
                row.row,
                &CellContext {
                    row_id: row.id.clone(),
                    column_id: column.id.clone(),
                    depth: row.depth,
                    has_children: row.has_children,
                    expanded: row.expanded,
                    highlighted: true,
                    focused: self.focused,
                },
            );
            let text = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            value.insert(column.id.clone(), serde_json::Value::String(text));
        }
        Some(serde_json::Value::Object(value).to_string())
    }

    pub fn highlight_id(&mut self, id: &Id) -> DataViewOutcome {
        let Some(index) = self.visible_rows().iter().position(|row| &row.id == id) else {
            return DataViewOutcome::IDLE;
        };
        let update = self.set_highlighted_index(index);
        DataViewOutcome {
            handled: true,
            changed: update.index_changed || update.selection_changed,
            active: false,
            activated: update.activated,
        }
    }

    pub(crate) fn snap_highlight_centered(&mut self, area: Rect) -> ScrollOutcome {
        let mut settings = animation_settings();
        settings.enabled = false;
        self.center_highlight(area, settings)
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>, viewport: Rect) -> DataViewOutcome {
        self.on_key_with_settings(key, viewport, animation_settings())
    }

    pub fn on_key_with_settings(
        &mut self,
        key: impl Into<KeyEvent>,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let key = key.into();
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
        if !matches!(self.interaction, DataViewInteraction::Grid) {
            return self.on_interaction_key(key, area, settings);
        }
        let page = self.visible_page_step(area);
        let data_keys = keys.data_view();
        if !self.transform_state.search.is_empty() && keys.focus().unfocus_matches(key) {
            self.pending_g = false;
            self.clear_search_preserving_highlight(area, settings)
        } else if self.action_bar && data_keys.clear_search_matches(key) {
            self.pending_g = false;
            self.clear_search_and_enter_insert_mode(area, settings)
        } else if self.filter_controls_enabled() && data_keys.clear_filters_matches(key) {
            self.pending_g = false;
            self.clear_filters_preserving_highlight(area, settings)
        } else if self.action_bar && data_keys.search_matches(key) {
            self.pending_g = false;
            self.interaction = DataViewInteraction::Search;
            self.search_input.set_focused(true);
            self.search_input.set_insert_mode(true);
            DataViewOutcome::CHANGED
        } else if data_keys.filter_matches(key)
            && self.filter_controls_enabled()
            && !self.filterable_columns().is_empty()
        {
            self.pending_g = false;
            self.interaction = DataViewInteraction::HeaderFilter;
            self.header_pick_elapsed = Duration::ZERO;
            DataViewOutcome::CHANGED
        } else if let Some(delta) = horizontal_jump(keys, key) {
            self.pending_g = false;
            self.scroll_horizontal_by(delta, area, settings)
        } else if keys.line_up_matches(key) {
            self.pending_g = false;
            self.highlight_line_with_settings(self.highlighted.saturating_sub(1), area, settings)
        } else if keys.line_down_matches(key) {
            self.pending_g = false;
            self.highlight_line_with_settings(self.highlighted.saturating_add(1), area, settings)
        } else if keys.line_left_matches(key) {
            self.pending_g = false;
            self.navigate_or_scroll_left(key, area, settings)
        } else if keys.line_right_matches(key) {
            self.pending_g = false;
            self.navigate_or_scroll_right(key, area, settings)
        } else if keys.page_up_matches(key) {
            self.pending_g = false;
            self.highlight_centered_with_settings(
                self.highlighted.saturating_sub(page),
                area,
                settings,
            )
        } else if keys.page_down_matches(key) {
            self.pending_g = false;
            self.highlight_centered_with_settings(
                self.highlighted.saturating_add(page),
                area,
                settings,
            )
        } else if keys.home_matches(key) {
            self.pending_g = false;
            self.highlight_centered_with_settings(0, area, settings)
        } else if keys.end_matches(key) {
            self.pending_g = false;
            self.highlight_centered_with_settings(
                self.visible_len().saturating_sub(1),
                area,
                settings,
            )
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

    fn on_interaction_key(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        match self.interaction.clone() {
            DataViewInteraction::Search => self.on_search_key(key, area, settings),
            DataViewInteraction::HeaderFilter => self.on_header_filter_key(key),
            DataViewInteraction::FilterValues { column_id } => {
                self.on_filter_values_key(key, area, settings, &column_id)
            }
            DataViewInteraction::Grid => DataViewOutcome::IDLE,
        }
    }

    fn on_search_key(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if matches!(key.code, Key::Enter) {
            self.interaction = DataViewInteraction::Grid;
            self.search_input.set_focused(false);
            return DataViewOutcome::CHANGED;
        }

        if keybindings().focus().unfocus_matches(key) {
            self.interaction = DataViewInteraction::Grid;
            self.search_input.set_focused(false);
            let mut outcome = self.clear_search_preserving_highlight(area, settings);
            outcome.changed = true;
            return outcome;
        }

        if keybindings().data_view().clear_search_matches(key) {
            return self.clear_search_and_enter_insert_mode(area, settings);
        }

        let before = self.search_input.current_value().to_owned();
        let input_outcome = self.search_input.on_key(key);
        let after = self.search_input.current_value().to_owned();
        if before != after {
            self.set_search_query_with_settings(after, area, settings)
        } else if input_outcome.needs_redraw() {
            DataViewOutcome::CHANGED
        } else {
            DataViewOutcome::HANDLED
        }
    }

    fn on_search_event<M>(
        &mut self,
        event: &TuiEvent,
        area: Rect,
        settings: AnimationSettings,
        ctx: &mut EventCtx<M>,
    ) -> DataViewOutcome {
        if let TuiEvent::ExternalEditor(response) = event {
            let before = self.search_input.current_value().to_owned();
            self.search_input.apply_external_editor_response(response);
            self.search_input.set_insert_mode(false);
            ctx.request_clear();
            ctx.request_layout();
            let after = self.search_input.current_value().to_owned();
            return if before != after {
                self.set_search_query_with_settings(after, area, settings)
            } else {
                DataViewOutcome::CHANGED
            };
        }

        if let TuiEvent::Paste(value) = event {
            if !self.search_input.insert_mode() {
                return DataViewOutcome::HANDLED;
            }
            let before = self.search_input.current_value().to_owned();
            let input_outcome = self.search_input.on_paste(value);
            let after = self.search_input.current_value().to_owned();
            return if before != after {
                self.set_search_query_with_settings(after, area, settings)
            } else if input_outcome.needs_redraw() {
                DataViewOutcome::CHANGED
            } else {
                DataViewOutcome::HANDLED
            };
        }

        let TuiEvent::Key(key) = event else {
            return DataViewOutcome::IDLE;
        };
        if self.search_input.external_editor_key_matches(*key) {
            let (value, line, col) = self.search_input.external_editor_request();
            ctx.request_external_editor(value, line, col);
            return DataViewOutcome::HANDLED;
        }
        self.on_search_key(*key, area, settings)
    }

    fn focus_self<M>(&self, ctx: &mut EventCtx<M>) {
        let current = ctx.current_path();
        let path = if current.keys().last().is_some_and(|key| {
            key == &ChildKey::new(SEARCH_SLOT) || key == &ChildKey::new(FILTER_DROPDOWN_SLOT)
        }) {
            current.parent().unwrap_or(current)
        } else {
            current
        };
        ctx.focus(FocusRequest::TargetAt {
            path,
            id: FocusId::new(DATA_VIEW_FOCUS),
        });
    }

    fn focus_filter_dropdown_search<M>(&self, ctx: &mut EventCtx<M>) {
        ctx.focus(FocusRequest::TargetAt {
            path: ctx
                .current_path()
                .child(ChildKey::new(FILTER_DROPDOWN_SLOT)),
            id: FocusId::new(DROPDOWN_SEARCH_FOCUS),
        });
    }

    fn search_exited(before: &DataViewInteraction, after: &DataViewInteraction) -> bool {
        matches!(before, DataViewInteraction::Search)
            && !matches!(after, DataViewInteraction::Search)
    }

    fn on_header_filter_key(&mut self, key: KeyEvent) -> DataViewOutcome {
        if keybindings().focus().unfocus_matches(key) {
            self.interaction = DataViewInteraction::Grid;
            return DataViewOutcome::CHANGED;
        }
        let Key::Char(value) = key.code else {
            return DataViewOutcome::HANDLED;
        };
        let Some(column_id) = self.filter_column_id_for_key(value) else {
            return DataViewOutcome::HANDLED;
        };
        self.open_filter_values(column_id);
        DataViewOutcome::CHANGED
    }

    fn on_filter_values_key(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
        column_id: &str,
    ) -> DataViewOutcome {
        let Some(dropdown) = self.filter_dropdown.as_mut() else {
            return DataViewOutcome::HANDLED;
        };
        let outcome = dropdown.on_key(key, area);
        self.apply_filter_dropdown_outcome(column_id, outcome, area, settings)
    }

    fn on_filter_values_event<M>(
        &mut self,
        event: &TuiEvent,
        area: Rect,
        settings: AnimationSettings,
        column_id: &str,
        ctx: &mut EventCtx<M>,
    ) -> DataViewOutcome {
        let Some(dropdown) = self.filter_dropdown.as_mut() else {
            return DataViewOutcome::HANDLED;
        };
        let outcome = dropdown.event_outcome(event, ctx);
        self.apply_filter_dropdown_outcome(column_id, outcome, area, settings)
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

    fn highlight_line_with_settings(
        &mut self,
        highlighted: usize,
        area: Rect,
        mut settings: AnimationSettings,
    ) -> DataViewOutcome {
        settings.enabled = false;
        let highlighted = highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index(highlighted);
        let changed = update.index_changed || update.selection_changed;
        let mut outcome = self
            .center_highlight(area, settings)
            .into_data_view_outcome(true, changed);
        outcome.activated = update.activated;
        outcome
    }

    fn highlight_centered_with_settings(
        &mut self,
        highlighted: usize,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let highlighted = highlighted.min(self.visible_len().saturating_sub(1));
        let update = self.set_highlighted_index(highlighted);
        let changed = update.index_changed || update.selection_changed;
        let mut outcome = self
            .center_highlight(area, settings)
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

    fn center_highlight(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
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

    fn set_search_query_with_settings(
        &mut self,
        query: String,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.set_search_query(query);
        if outcome.changed {
            let mut scrolled = self
                .ensure_highlight_visible(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            DataViewOutcome::HANDLED
        }
    }

    fn clear_search_preserving_highlight(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.clear_search();
        self.ensure_visible_after_clear(outcome, area, settings)
    }

    fn clear_search_and_enter_insert_mode(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let mut outcome = self.clear_search_preserving_highlight(area, settings);
        self.interaction = DataViewInteraction::Search;
        self.search_input.set_focused(true);
        self.search_input.set_insert_mode(true);
        outcome.changed = true;
        outcome
    }

    fn clear_filters_preserving_highlight(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let outcome = self.clear_filters();
        self.ensure_visible_after_clear(outcome, area, settings)
    }

    fn ensure_visible_after_clear(
        &mut self,
        outcome: DataViewOutcome,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        if !outcome.changed {
            return DataViewOutcome::HANDLED;
        }
        let mut scrolled = self
            .ensure_highlight_visible(area, settings)
            .into_data_view_outcome(outcome.handled, outcome.changed);
        scrolled.active |= outcome.active;
        scrolled.activated |= outcome.activated;
        scrolled
    }

    fn outcome_after_transform_change(&mut self, before_id: Option<Id>) -> DataViewOutcome {
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    fn emit_transform_changed(&mut self) {
        self.events.push(DataViewTypedEvent::TransformChanged {
            state: self.transform_state.clone(),
        });
    }

    fn trim_visible_row_indices(&mut self) {
        if let Some(indices) = &mut self.visible_row_indices {
            let len = self.rows.len();
            indices.retain(|index| *index < len);
        }
    }

    fn row_indices_for_ids(&self, ids: impl IntoIterator<Item = Id>) -> Vec<usize> {
        let mut used = HashSet::new();
        let mut indices = Vec::new();
        for id in ids {
            if let Some(index) = self.rows.iter().enumerate().find_map(|(index, row)| {
                (!used.contains(&index) && (self.row_id)(row) == id).then_some(index)
            }) {
                used.insert(index);
                indices.push(index);
            }
        }
        indices
    }

    fn replace_visible_row_indices(&mut self, next: Option<Vec<usize>>) -> DataViewOutcome {
        if self.visible_row_indices == next {
            return DataViewOutcome::IDLE;
        }

        let before_id = self.highlighted_id();
        self.visible_row_indices = next;
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    fn sync_highlight_after_visible_set_change(
        &mut self,
        before_id: Option<Id>,
    ) -> (bool, HighlightUpdate) {
        let all_visible = self.all_visible_rows();
        let position = before_id
            .as_ref()
            .and_then(|id| all_visible.iter().position(|row| &row.id == id))
            .unwrap_or(0);
        let has_visible_rows = !all_visible.is_empty();
        drop(all_visible);

        let mut page_changed = false;
        let highlighted = if has_visible_rows {
            if let Some(pagination) = &mut self.pagination {
                let page = position / pagination.page_size;
                page_changed = pagination.page != page;
                pagination.page = page;
                position % pagination.page_size
            } else {
                position
            }
        } else {
            if let Some(pagination) = &mut self.pagination {
                page_changed = pagination.page != 0;
                pagination.page = 0;
            }
            0
        };

        let update = self.set_highlighted_index_from(highlighted, before_id);
        (page_changed, update)
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
    let plain_control = key.modifiers.contains(KeyModifiers::CONTROL)
        && !key
            .modifiers
            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT)
        && matches!(key.code, Key::Char(_));
    if !plain_control {
        return None;
    }

    let base_key = uncontrol_key(key);
    if keys.line_left_matches(base_key) {
        Some(-HORIZONTAL_JUMP)
    } else if keys.line_right_matches(base_key) {
        Some(HORIZONTAL_JUMP)
    } else {
        None
    }
}

fn uncontrol_key(mut key: KeyEvent) -> KeyEvent {
    key.modifiers.remove(KeyModifiers::CONTROL);
    if let Key::Char(c) = key.code {
        key.code = Key::Char(c.to_ascii_lowercase());
    }
    key
}

fn dropdown_outcome(outcome: DropdownOutcome) -> DataViewOutcome {
    DataViewOutcome {
        handled: outcome.handled,
        changed: outcome.changed || outcome.opened || outcome.closed || outcome.canceled,
        active: false,
        activated: false,
    }
}

pub(crate) fn column_key(index: usize) -> Option<char> {
    match index {
        0..=8 => Some((b'1' + index as u8) as char),
        9..=34 => Some((b'a' + (index - 9) as u8) as char),
        _ => None,
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
