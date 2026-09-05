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
mod tree_edit;
mod tree_rows;

use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::search::SearchMode;
use crate::{
    AnimationSettings, AnimationSpec, ChildKey, Easing, EventCtx, FocusId, FocusRequest,
    KeyBindings, ScrollAxes, ScrollBehavior, ScrollDelta, ScrollOffset, ScrollOutcome, ScrollState,
    ScrollbarConfig, ScrollbarVisibility, TickResult, Tween, animation_settings, keybindings,
    preset,
};

use super::{
    Dropdown, DropdownCommitMode, DropdownLabelPosition, DropdownOutcome, DropdownSearchMode,
    DropdownVariant, SeasonalEmptyState, text_input::TextInput,
};

pub(crate) use model::SelectionOverlayPosition;
pub use model::{
    ActivationMode, CellContext, CheckState, Column, ColumnSizing, DataViewEvent, DataViewFilter,
    DataViewOutcome, DataViewPagination, DataViewSort, DataViewTransformMode,
    DataViewTransformState, DataViewTypedEvent, SelectionGlyphs, SelectionMode,
    SelectionPropagation, SelectionTrigger, SortDirection, TreeAdapter, TreeGlyphs,
};
pub(crate) use model::{DataViewDisplayAction, ReorderSnapshot, ReorderUnavailableReason};
use model::{DisplayRow, RowIdFn, SelectionOverlay, VisibleRow};
pub(crate) use tree_edit::TreeEditSnapshot;

const HORIZONTAL_JUMP_PERCENT: usize = 70;
const CELL_RIGHT_PADDING: usize = 1;
const DATA_VIEW_FOCUS: &str = "data-view";
const SEARCH_SLOT: &str = "search";
const FILTER_DROPDOWN_SLOT: &str = "filter-dropdown";
const TEXT_INPUT_FOCUS: &str = "input";
const DROPDOWN_SEARCH_FOCUS: &str = "input";
const EMPTY_CHOICE_ID: &str = "";
const HEADER_PICK_TIMEOUT: Duration = Duration::from_secs(1);
const REORDER_HIGHLIGHT_DURATION: Duration = Duration::from_millis(250);
const DEFAULT_EMPTY_MESSAGE: &str = "No results found.";

type ChoiceDropdown = Dropdown<DataViewChoice, String>;
type CopyFormatter<T> = dyn Fn(&T) -> String;
type CopyHotkeyFormatter<T> = dyn Fn(&T) -> Option<String>;
type RowHeightFn<T> = dyn Fn(&T) -> u16;
type RowStyleFn<T> = dyn Fn(&T) -> Option<ratatui::style::Style>;
type LeftGutterMarkerFn<T> = dyn Fn(&T) -> Option<ratatui::text::Span<'static>>;
type SelectionDisabledFn<T> = dyn Fn(&T) -> bool;
type SelectionGlyphHiddenFn<T> = dyn Fn(&T) -> bool;

struct DataViewMetricCache {
    revision: u64,
    viewport_width: usize,
    rendered_column_widths: Vec<usize>,
}

pub(crate) fn search_focus_id() -> FocusId {
    FocusId::new(TEXT_INPUT_FOCUS)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DataViewChoice {
    id: String,
    label: String,
}

pub struct DataView<T, Id> {
    focus_id: FocusId,
    rows: Vec<T>,
    visible_row_indices: Option<Vec<usize>>,
    columns: Vec<Column<T, Id>>,
    row_id: Box<RowIdFn<T, Id>>,
    copy_formatter: Option<Box<CopyFormatter<T>>>,
    copy_hotkeys: Vec<(String, Box<CopyHotkeyFormatter<T>>)>,
    empty_state: Option<SeasonalEmptyState>,
    empty_message: String,
    tree: Option<TreeAdapter<T, Id>>,
    expanded: HashSet<Id>,
    highlighted: usize,
    focused: bool,
    show_inactive_highlight: bool,
    focused_events_before_global_hotkeys: bool,
    headers: bool,
    row_height: u16,
    row_height_by: Option<Box<RowHeightFn<T>>>,
    wrap_cells: bool,
    row_style_by: Option<Box<RowStyleFn<T>>>,
    left_gutter_marker_by: Option<Box<LeftGutterMarkerFn<T>>>,
    scroll: ScrollState,
    vertical_scroll: DataViewVerticalScroll,
    sort: Option<DataViewSort>,
    reorder_sort: Option<String>,
    derived_row_order: Option<Vec<Id>>,
    pagination: Option<DataViewPagination>,
    last_activated: Option<Id>,
    events: Vec<DataViewTypedEvent<Id>>,
    activation_mode: ActivationMode,
    selection_mode: SelectionMode,
    selection_trigger: SelectionTrigger,
    selection_propagation: SelectionPropagation,
    selected: HashSet<Id>,
    selection_glyphs: SelectionGlyphs,
    selection_disabled_by: Option<Box<SelectionDisabledFn<T>>>,
    selection_glyph_hidden_by: Option<Box<SelectionGlyphHiddenFn<T>>>,
    selection_disabled_glyph: &'static str,
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
    reorder_highlight: Tween,
    reorder_highlight_id: Option<Id>,
    reorder_highlight_phase: ReorderHighlightPhase,
    reorder_highlight_crossfades: bool,
    scroll_restoration: Option<DataViewScrollRestoration>,
    selection_overlay: Option<SelectionOverlay<Id>>,
    metric_revision: u64,
    metric_cache: Option<DataViewMetricCache>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataViewVerticalScroll {
    #[default]
    Local,
    ParentDelegated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DataViewScrollSnapshot {
    rendered: ScrollOffset,
    target: ScrollOffset,
    active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DataViewScrollRestoration {
    resume_target: Option<ScrollOffset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReorderHighlightPhase {
    Inactive,
    Active,
    Exiting,
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
            focus_id: FocusId::new(DATA_VIEW_FOCUS),
            rows: rows.into_iter().collect(),
            visible_row_indices: None,
            columns: Vec::new(),
            row_id: Box::new(row_id),
            copy_formatter: None,
            copy_hotkeys: Vec::new(),
            empty_state: None,
            empty_message: DEFAULT_EMPTY_MESSAGE.to_string(),
            tree: None,
            expanded: HashSet::new(),
            highlighted: 0,
            focused: false,
            show_inactive_highlight: false,
            focused_events_before_global_hotkeys: true,
            headers: false,
            row_height: 1,
            row_height_by: None,
            wrap_cells: false,
            row_style_by: None,
            left_gutter_marker_by: None,
            scroll: ScrollState::from_preset(ScrollAxes::Both, preset().scroll()),
            vertical_scroll: DataViewVerticalScroll::Local,
            sort: None,
            reorder_sort: None,
            derived_row_order: None,
            pagination: None,
            last_activated: None,
            events: Vec::new(),
            activation_mode: ActivationMode::default(),
            selection_mode: SelectionMode::default(),
            selection_trigger: SelectionTrigger::default(),
            selection_propagation: SelectionPropagation::default(),
            selected: HashSet::new(),
            selection_glyphs: SelectionGlyphs::NERD_FONT,
            selection_glyph_hidden_by: None,
            selection_disabled_by: None,
            selection_disabled_glyph: "󰄲",
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
            reorder_highlight: Tween::idle(0.0),
            reorder_highlight_id: None,
            reorder_highlight_phase: ReorderHighlightPhase::Inactive,
            reorder_highlight_crossfades: false,
            scroll_restoration: None,
            selection_overlay: None,
            metric_revision: 0,
            metric_cache: None,
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

    pub fn focus_id(mut self, id: impl Into<String>) -> Self {
        self.focus_id = FocusId::new(id);
        self
    }

    pub fn column(mut self, column: Column<T, Id>) -> Self {
        self.columns.push(column);
        self.invalidate_metrics();
        self
    }

    pub fn add_column(&mut self, column: Column<T, Id>) {
        self.columns.push(column);
        self.invalidate_metrics();
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = Column<T, Id>>) -> Self {
        self.columns.extend(columns);
        self.invalidate_metrics();
        self
    }

    pub fn add_columns(&mut self, columns: impl IntoIterator<Item = Column<T, Id>>) {
        self.columns.extend(columns);
        self.invalidate_metrics();
    }

    pub fn set_column_wrap_continuation_indent_by(
        &mut self,
        column_id: &str,
        indent: impl Fn(&T) -> usize + 'static,
    ) -> bool {
        let Some(column) = self
            .columns
            .iter_mut()
            .find(|column| column.id == column_id)
        else {
            return false;
        };
        column.continuation_indent = Some(Box::new(indent));
        self.invalidate_metrics();
        true
    }

    pub fn copy_with(mut self, formatter: impl Fn(&T) -> String + 'static) -> Self {
        self.copy_formatter = Some(Box::new(formatter));
        self
    }

    pub fn copy_hotkey(
        mut self,
        sequence: impl Into<String>,
        formatter: impl Fn(&T) -> Option<String> + 'static,
    ) -> Self {
        self.copy_hotkeys
            .push((sequence.into(), Box::new(formatter)));
        self
    }

    pub fn empty_state(mut self, empty_state: SeasonalEmptyState) -> Self {
        self.empty_state = Some(empty_state);
        self
    }

    pub fn empty_message(mut self, message: impl Into<String>) -> Self {
        self.empty_message = message.into();
        self
    }

    pub fn set_empty_state(&mut self, empty_state: SeasonalEmptyState) {
        self.empty_state = Some(empty_state);
    }

    pub fn headers(mut self, headers: bool) -> Self {
        self.headers = headers;
        self.invalidate_metrics();
        self
    }

    pub fn row_height(mut self, row_height: u16) -> Self {
        self.set_row_height(row_height);
        self
    }

    pub fn set_row_height(&mut self, row_height: u16) {
        self.row_height = row_height.max(1);
        self.row_height_by = None;
        self.invalidate_metrics();
    }

    /// Sets a per-row height policy. Returned zero heights are clamped to one.
    pub fn row_height_by(mut self, row_height: impl Fn(&T) -> u16 + 'static) -> Self {
        self.set_row_height_by(row_height);
        self
    }

    /// Replaces the current per-row height policy. Returned zero heights are clamped to one.
    pub fn set_row_height_by(&mut self, row_height: impl Fn(&T) -> u16 + 'static) {
        self.row_height_by = Some(Box::new(row_height));
        self.invalidate_metrics();
    }

    /// Wraps cell text to the available column width and grows rows to keep every line visible.
    pub fn wrap_cells(mut self) -> Self {
        self.set_wrap_cells(true);
        self
    }

    pub fn set_wrap_cells(&mut self, wrap_cells: bool) {
        self.wrap_cells = wrap_cells;
        self.invalidate_metrics();
    }

    /// Sets a per-row style policy.
    pub fn row_style_by(
        mut self,
        row_style: impl Fn(&T) -> Option<ratatui::style::Style> + 'static,
    ) -> Self {
        self.set_row_style_by(row_style);
        self
    }

    /// Replaces the current per-row style policy.
    pub fn set_row_style_by(
        &mut self,
        row_style: impl Fn(&T) -> Option<ratatui::style::Style> + 'static,
    ) {
        self.row_style_by = Some(Box::new(row_style));
    }

    /// Adds a one-cell, row-specific marker before tree and selection gutters.
    pub fn left_gutter_marker_by(
        mut self,
        marker: impl Fn(&T) -> Option<ratatui::text::Span<'static>> + 'static,
    ) -> Self {
        self.set_left_gutter_marker_by(marker);
        self
    }

    /// Replaces the row-specific left-edge marker policy.
    pub fn set_left_gutter_marker_by(
        &mut self,
        marker: impl Fn(&T) -> Option<ratatui::text::Span<'static>> + 'static,
    ) {
        self.left_gutter_marker_by = Some(Box::new(marker));
        self.invalidate_metrics();
    }

    pub fn configured_row_height(&self) -> u16 {
        self.row_height
    }

    pub(super) fn row_height_for(&self, row: &T) -> u16 {
        self.row_height_by
            .as_ref()
            .map_or(self.row_height, |height| height(row))
            .max(1)
    }

    pub fn action_bar(mut self, action_bar: bool) -> Self {
        self.action_bar = action_bar;
        self
    }

    pub fn filter_controls(mut self, enabled: bool) -> Self {
        self.filter_controls = enabled;
        self
    }

    pub fn focused_events_before_global_hotkeys(mut self, enabled: bool) -> Self {
        self.focused_events_before_global_hotkeys = enabled;
        self
    }

    pub fn search_mode(mut self, mode: SearchMode) -> Self {
        self.search_mode = mode;
        self
    }

    pub fn visible_row_ids(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.visible_row_indices = Some(self.row_indices_for_ids(ids));
        self.invalidate_metrics();
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
        self.invalidate_metrics();
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.expanded = ids.into_iter().collect();
        self.invalidate_metrics();
        self
    }

    pub fn tree_glyphs(mut self, glyphs: TreeGlyphs) -> Self {
        self.tree_glyphs = glyphs;
        self.invalidate_metrics();
        self
    }

    pub fn activation_mode(mut self, mode: ActivationMode) -> Self {
        self.activation_mode = mode;
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self.invalidate_metrics();
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.focused != focused {
            self.focused = focused;
            self.invalidate_metrics();
        }
        if !focused {
            self.clear_reorder_highlight_immediately();
        }
    }

    pub fn show_inactive_highlight(mut self, show: bool) -> Self {
        self.show_inactive_highlight = show;
        self
    }

    pub fn set_show_inactive_highlight(&mut self, show: bool) {
        self.show_inactive_highlight = show;
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn is_searching(&self) -> bool {
        matches!(self.interaction, DataViewInteraction::Search)
    }

    pub(crate) fn has_active_interaction(&self) -> bool {
        !matches!(self.interaction, DataViewInteraction::Grid)
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
        self.invalidate_metrics();
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
        if self.transform_state.search.is_empty() && !query.is_empty() {
            self.expand_all();
        }
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
        let visible_ids = self.visible_row_indices.as_ref().map(|indices| {
            indices
                .iter()
                .filter_map(|index| self.rows.get(*index))
                .map(|row| (self.row_id)(row))
                .collect::<Vec<_>>()
        });
        self.rows = rows.into_iter().collect();
        self.invalidate_metrics();
        self.normalize_selection();
        if let Some(ids) = visible_ids {
            self.visible_row_indices = Some(self.row_indices_for_ids(ids));
        }
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    pub fn rows(&self) -> &[T] {
        &self.rows
    }

    pub(crate) fn visible_row_count(&self) -> usize {
        self.visible_rows().len()
    }

    fn invalidate_metrics(&mut self) {
        self.metric_revision = self.metric_revision.wrapping_add(1);
        self.metric_cache = None;
    }

    pub(crate) fn measurement_chrome_height(&self) -> u16 {
        u16::from(self.shows_headers()).saturating_add(u16::from(self.action_bar))
    }

    pub(super) fn visible_columns(&self) -> impl Iterator<Item = &Column<T, Id>> {
        self.columns.iter().filter(|column| column.visible)
    }

    pub(super) fn visible_column_count(&self) -> usize {
        self.visible_columns().count()
    }

    pub(super) fn shows_headers(&self) -> bool {
        self.headers && self.visible_column_count() > 0
    }

    pub fn row_id(&self, row: &T) -> Id {
        (self.row_id)(row)
    }

    pub fn push_row(&mut self, row: T) -> DataViewOutcome {
        let id = (self.row_id)(&row);
        self.rows.push(row);
        self.invalidate_metrics();
        self.clamp_visible_state();
        let mut outcome = self.highlight_id(&id);
        outcome.handled = true;
        outcome.changed = true;
        outcome
    }

    /// Updates a row without changing its position.
    ///
    /// The updater must preserve the row ID. Changing it panics because selection,
    /// expansion, and externally supplied visible-row state are keyed by that ID.
    pub fn update_row(&mut self, id: &Id, update: impl FnOnce(&mut T)) -> Option<DataViewOutcome> {
        let before_id = self.highlighted_id();
        let row = self.rows.iter_mut().find(|row| &(self.row_id)(row) == id)?;
        update(row);
        assert!(
            &(self.row_id)(row) == id,
            "DataView row update must preserve the row ID"
        );
        self.invalidate_metrics();
        self.normalize_selection();
        let (_, highlight) = self.sync_highlight_after_visible_set_change(before_id);
        Some(DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: highlight.activated,
        })
    }

    pub fn remove_row(&mut self, id: &Id) -> Option<T> {
        let index = self.rows.iter().position(|row| &(self.row_id)(row) == id)?;
        let highlighted = self.highlighted;
        let removed = self.rows.remove(index);
        self.invalidate_metrics();
        self.selected.remove(id);
        self.expanded.remove(id);
        if let Some(indices) = &mut self.visible_row_indices {
            indices.retain(|candidate| *candidate != index);
            for candidate in indices {
                if *candidate > index {
                    *candidate -= 1;
                }
            }
        }
        self.clamp_page();
        let next = highlighted.min(self.visible_len().saturating_sub(1));
        self.set_highlighted_index_from(next, Some(id.clone()));
        Some(removed)
    }

    pub fn remove_subtree(&mut self, id: &Id) -> Option<T> {
        if !self.rows.iter().any(|row| &(self.row_id)(row) == id) {
            return None;
        }
        for descendant_id in self.descendant_ids(id) {
            self.remove_row(&descendant_id);
        }
        self.remove_row(id)
    }

    pub fn append_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        self.extend_rows(rows)
    }

    pub fn extend_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        self.rows.extend(rows);
        self.invalidate_metrics();
        self.clamp_visible_state();
        DataViewOutcome::CHANGED
    }

    #[cfg(test)]
    pub(crate) fn focused_for_test(&self) -> bool {
        self.focused
    }

    #[cfg(test)]
    pub(crate) fn vertical_scroll_offset_for_test(&self) -> usize {
        self.scroll.offset().y
    }

    #[cfg(test)]
    pub(crate) fn horizontal_scroll_offset_for_test(&self) -> usize {
        self.scroll.offset().x
    }

    #[cfg(test)]
    pub(crate) fn scroll_animation_state_for_test(&self) -> (ScrollOffset, ScrollOffset, bool) {
        (
            self.scroll.offset(),
            self.scroll.target_offset(),
            self.scroll.is_active(),
        )
    }

    #[cfg(test)]
    pub(crate) fn selection_overlay_active_for_test(&self) -> bool {
        self.selection_overlay.is_some()
    }

    pub fn pagination(mut self, page_size: usize) -> Self {
        self.pagination = (page_size > 0).then_some(DataViewPagination { page_size, page: 0 });
        self
    }

    pub fn sorted_by(mut self, column_id: impl Into<String>, direction: SortDirection) -> Self {
        self.set_sort(column_id.into(), direction);
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

    /// Lets an ancestor [`ScrollContainer`](crate::ScrollContainer) own vertical viewporting.
    /// DataView still owns horizontal scrolling, tree navigation, selection, and rendering.
    pub fn parent_vertical_scroll(mut self) -> Self {
        self.vertical_scroll = DataViewVerticalScroll::ParentDelegated;
        self.scroll = self.scroll.vertical_scrollbar(ScrollbarVisibility::Never);
        self
    }

    pub fn vertical_scroll(mut self, mode: DataViewVerticalScroll) -> Self {
        self.vertical_scroll = mode;
        if mode == DataViewVerticalScroll::ParentDelegated {
            self.scroll = self.scroll.vertical_scrollbar(ScrollbarVisibility::Never);
        }
        self
    }

    pub fn vertical_scroll_mode(&self) -> DataViewVerticalScroll {
        self.vertical_scroll
    }

    pub fn sort_by(
        &mut self,
        column_id: impl Into<String>,
        direction: SortDirection,
    ) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        self.set_sort(column_id.into(), direction);
        let update = self.restore_highlight(before_id.clone());
        DataViewOutcome {
            handled: true,
            changed: true,
            active: false,
            activated: update.activated,
        }
    }

    pub fn clear_sort(&mut self) -> DataViewOutcome {
        let before_id = self.highlighted_id();
        if self.sort.take().is_none() {
            return DataViewOutcome::IDLE;
        }
        self.invalidate_metrics();
        let update = self.restore_highlight(before_id);
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
            self.clear_sort()
        }
    }

    fn set_sort(&mut self, column_id: String, direction: SortDirection) {
        assert!(
            self.columns
                .iter()
                .any(|column| column.id == column_id && column.sort_compare.is_some()),
            "DataView automatic sort column `{column_id}` must be sortable"
        );
        assert!(
            self.reorder_sort.is_none(),
            "DataView automatic sorting and reorder sorting are mutually exclusive"
        );
        self.sort = Some(DataViewSort {
            column_id,
            direction,
        });
        self.invalidate_metrics();
    }

    fn restore_highlight(&mut self, before_id: Option<Id>) -> HighlightUpdate {
        let highlighted = before_id
            .as_ref()
            .and_then(|id| self.visible_rows().iter().position(|row| &row.id == id))
            .unwrap_or_else(|| self.highlighted.min(self.visible_len().saturating_sub(1)));
        self.set_highlighted_index_from(highlighted, before_id)
    }

    pub(crate) fn has_automatic_sort(&self) -> bool {
        self.sort.is_some()
    }

    pub(crate) fn configure_reorder_sort(&mut self, column_id: &str) {
        assert!(
            self.columns
                .iter()
                .any(|column| column.id == column_id && column.reorder.is_some()),
            "ListControl reorder column `{column_id}` must be reorderable"
        );
        assert!(
            self.sort.is_none(),
            "ListControl automatic sorting and reorderable mode are mutually exclusive"
        );
        self.reorder_sort = Some(column_id.to_string());
    }

    pub(crate) fn scroll_snapshot(&mut self) -> DataViewScrollSnapshot {
        self.scroll_restoration = None;
        DataViewScrollSnapshot {
            rendered: self.scroll.offset(),
            target: self.scroll.target_offset(),
            active: self.scroll.is_active(),
        }
    }

    pub(crate) fn restore_scroll(
        &mut self,
        snapshot: DataViewScrollSnapshot,
        area: Rect,
        settings: AnimationSettings,
    ) {
        self.scroll_restoration = None;
        let target = if settings.enabled {
            snapshot.rendered
        } else {
            snapshot.target
        };
        let geometry = self.scroll_geometry(area);
        self.scroll
            .scroll_to(target, geometry.viewport, geometry.content, settings);
        if !settings.enabled {
            return;
        }
        self.scroll_restoration = Some(DataViewScrollRestoration {
            resume_target: (snapshot.active && snapshot.target != snapshot.rendered)
                .then_some(snapshot.target),
        });
        self.advance_scroll_restoration(area, settings);
    }

    fn advance_scroll_restoration(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> TickResult {
        if self.scroll.is_active() {
            return TickResult::IDLE;
        }
        let Some(restoration) = self.scroll_restoration.take() else {
            return TickResult::IDLE;
        };
        let Some(target) = restoration.resume_target else {
            return TickResult::CHANGED;
        };
        let geometry = self.scroll_geometry(area);
        let outcome = self
            .scroll
            .scroll_to(target, geometry.viewport, geometry.content, settings);
        if outcome.active {
            self.scroll_restoration = Some(DataViewScrollRestoration {
                resume_target: None,
            });
        }
        TickResult {
            changed: outcome.changed,
            layout: false,
            active: outcome.active,
            next_tick: None,
        }
    }

    pub(crate) fn reorder_snapshot(
        &self,
        column_id: &str,
    ) -> Result<ReorderSnapshot<Id>, ReorderUnavailableReason> {
        if self.tree.is_some() {
            return Err(ReorderUnavailableReason::Tree);
        }
        if self.visible_row_indices.is_some() {
            return Err(ReorderUnavailableReason::VisibleSubset);
        }
        if self.transform_mode == DataViewTransformMode::External
            && (!self.transform_state.search.trim().is_empty()
                || !self.transform_state.filters.is_empty())
        {
            return Err(ReorderUnavailableReason::TransformActive);
        }
        if self.pagination.is_some() {
            return Err(ReorderUnavailableReason::Paginated);
        }
        let ids = self.row_ids();
        if ids.iter().collect::<HashSet<_>>().len() != ids.len() {
            return Err(ReorderUnavailableReason::DuplicateRowIds);
        }
        let column = self
            .columns
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| column.reorder.as_ref())
            .expect("configured reorder column exists");
        let mut indices = (0..self.rows.len()).collect::<Vec<_>>();
        indices.sort_by(|left, right| (column.compare)(&self.rows[*left], &self.rows[*right]));
        if indices
            .windows(2)
            .any(|pair| (column.compare)(&self.rows[pair[0]], &self.rows[pair[1]]).is_eq())
        {
            return Err(ReorderUnavailableReason::DuplicateRankKeys);
        }
        let ranks = (column.snapshot)(&self.rows, &indices);
        Ok(ReorderSnapshot {
            ids: indices
                .into_iter()
                .map(|index| (self.row_id)(&self.rows[index]))
                .collect(),
            ranks,
        })
    }

    pub(crate) fn set_derived_row_order(&mut self, ids: Option<Vec<Id>>) {
        self.derived_row_order = ids;
        self.invalidate_metrics();
    }

    pub(crate) fn reorder_visible_ids(&self) -> Vec<Id> {
        self.visible_rows().into_iter().map(|row| row.id).collect()
    }

    pub(crate) fn reposition_highlight_silently(&mut self, id: &Id) -> bool {
        let Some(index) = self.visible_rows().iter().position(|row| &row.id == id) else {
            return false;
        };
        self.highlighted = index;
        true
    }

    pub(crate) fn reconcile_selection_to_highlight_on_navigate(&mut self) {
        if self.selection_mode != SelectionMode::Single
            || self.selection_trigger != SelectionTrigger::OnNavigate
        {
            return;
        }
        let Some(id) = self.highlighted_id() else {
            return;
        };
        if self.is_selection_disabled(&id) {
            return;
        }
        self.selected = [id].into_iter().collect();
    }

    pub(crate) fn start_reorder_highlight(&mut self, id: Id, settings: AnimationSettings) {
        let theme = crate::theme();
        self.start_reorder_highlight_with_colors(
            id,
            settings,
            theme.highlight_fg(),
            theme.highlight_bg(),
        );
    }

    fn start_reorder_highlight_with_colors(
        &mut self,
        id: Id,
        settings: AnimationSettings,
        foreground: ratatui::style::Color,
        background: ratatui::style::Color,
    ) {
        let same_row = self.reorder_highlight_id.as_ref() == Some(&id);
        if !same_row {
            self.reorder_highlight.snap_to(0.0);
        }
        self.reorder_highlight_id = Some(id);
        self.reorder_highlight_phase = ReorderHighlightPhase::Active;
        self.reorder_highlight_crossfades = matches!(
            (foreground, background),
            (
                ratatui::style::Color::Rgb(_, _, _),
                ratatui::style::Color::Rgb(_, _, _)
            )
        );
        let animation = settings.resolve(AnimationSpec {
            enabled: None,
            duration: Some(REORDER_HIGHLIGHT_DURATION),
            easing: Some(Easing::EaseInOut),
        });
        if animation.enabled && self.reorder_highlight_crossfades {
            self.reorder_highlight.start(
                self.reorder_highlight.value(),
                1.0,
                animation.duration,
                animation.easing,
            );
        } else {
            self.reorder_highlight.snap_to(1.0);
        }
    }

    pub(crate) fn clear_reorder_highlight(&mut self, settings: AnimationSettings) {
        let animation = settings.resolve(AnimationSpec {
            enabled: None,
            duration: Some(REORDER_HIGHLIGHT_DURATION),
            easing: Some(Easing::EaseInOut),
        });
        if animation.enabled {
            self.reorder_highlight_phase = ReorderHighlightPhase::Exiting;
            self.reorder_highlight.start(
                self.reorder_highlight.value(),
                0.0,
                animation.duration,
                animation.easing,
            );
        } else {
            self.reorder_highlight.snap_to(0.0);
            self.reorder_highlight_id = None;
            self.reorder_highlight_phase = ReorderHighlightPhase::Inactive;
            self.reorder_highlight_crossfades = false;
        }
    }

    pub(crate) fn clear_reorder_highlight_immediately(&mut self) {
        self.reorder_highlight.snap_to(0.0);
        self.reorder_highlight_id = None;
        self.reorder_highlight_phase = ReorderHighlightPhase::Inactive;
        self.reorder_highlight_crossfades = false;
    }

    pub(crate) fn row_has_reorder_highlight(&self, id: &Id) -> bool {
        self.reorder_highlight_id.as_ref() == Some(id)
    }

    fn reorder_highlight_progress(&self) -> f64 {
        if self.reorder_highlight_id.is_none() {
            0.0
        } else if self.reorder_highlight_crossfades {
            self.reorder_highlight.value().clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    #[cfg(test)]
    pub(crate) fn reorder_highlight_progress_for_test(&self) -> f64 {
        self.reorder_highlight_progress()
    }

    pub(crate) fn reorder_snapshot_matches(
        &self,
        column_id: &str,
        snapshot: &ReorderSnapshot<Id>,
    ) -> bool {
        let Ok(current) = self.reorder_snapshot(column_id) else {
            return false;
        };
        if current.ids != snapshot.ids {
            return false;
        }
        let ordered = self.row_indices_for_ids(current.ids);
        let column = self
            .columns
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| column.reorder.as_ref())
            .expect("configured reorder column exists");
        (column.snapshot_matches)(&self.rows, &ordered, snapshot.ranks.as_ref())
    }

    pub(crate) fn reorder_scoped_ids(
        &self,
        snapshot: &ReorderSnapshot<Id>,
        anchor_id: &Id,
        same_scope: &dyn Fn(&T, &T) -> bool,
    ) -> Option<Vec<Id>> {
        let anchor = self
            .rows
            .iter()
            .find(|row| (self.row_id)(row) == *anchor_id)?;
        snapshot
            .ids
            .iter()
            .map(|id| {
                self.rows
                    .iter()
                    .find(|row| (self.row_id)(row) == *id)
                    .map(|row| (id.clone(), row))
            })
            .map(|row| row.map(|(id, row)| same_scope(anchor, row).then_some(id)))
            .collect::<Option<Vec<_>>>()
            .map(|ids| ids.into_iter().flatten().collect())
    }

    pub(crate) fn handle_display_action(
        &mut self,
        action: DataViewDisplayAction,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        match action {
            DataViewDisplayAction::LineUp => self.highlight_line_with_settings(
                self.highlighted.saturating_sub(1),
                area,
                settings,
            ),
            DataViewDisplayAction::LineDown => self.highlight_line_with_settings(
                self.highlighted.saturating_add(1),
                area,
                settings,
            ),
            DataViewDisplayAction::PageUp => self.highlight_centered_with_settings(
                self.highlighted
                    .saturating_sub(self.visible_page_step(area)),
                area,
                settings,
            ),
            DataViewDisplayAction::PageDown => self.highlight_centered_with_settings(
                self.highlighted
                    .saturating_add(self.visible_page_step(area)),
                area,
                settings,
            ),
            DataViewDisplayAction::Top => self.highlight_centered_with_settings(0, area, settings),
            DataViewDisplayAction::Bottom => self.highlight_centered_with_settings(
                self.visible_len().saturating_sub(1),
                area,
                settings,
            ),
            DataViewDisplayAction::Activate => self.activate_highlighted(),
        }
    }

    pub(crate) fn commit_reorder(
        &mut self,
        column_id: &str,
        staged: &[Id],
        snapshot: &ReorderSnapshot<Id>,
    ) -> bool {
        if !self.reorder_snapshot_matches(column_id, snapshot)
            || snapshot.ids.len() != staged.len()
            || snapshot.ids.iter().collect::<HashSet<_>>() != staged.iter().collect::<HashSet<_>>()
        {
            return false;
        }
        let staged_indices = self.row_indices_for_ids(staged.iter().cloned());
        let row_ids_before = self.row_ids();
        let column = self
            .columns
            .iter()
            .find(|column| column.id == column_id)
            .and_then(|column| column.reorder.as_ref())
            .expect("configured reorder column exists");
        let Some(candidate) = (column.apply)(&self.rows, &staged_indices, snapshot.ranks.as_ref())
        else {
            return false;
        };
        let valid = candidate
            .iter()
            .map(|row| (self.row_id)(row))
            .eq(row_ids_before)
            && (column.snapshot_matches)(&candidate, &staged_indices, snapshot.ranks.as_ref());
        if valid {
            self.rows = candidate;
            self.invalidate_metrics();
        }
        valid
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
        let ancestors = before_id
            .as_ref()
            .map(|id| self.visible_tree_ancestor_ids(id))
            .unwrap_or_default();
        self.expanded.clear();
        let visible_ids = self
            .all_visible_rows()
            .into_iter()
            .map(|row| row.id)
            .collect::<HashSet<_>>();
        let target_id = before_id
            .clone()
            .filter(|id| visible_ids.contains(id))
            .or_else(|| ancestors.into_iter().find(|id| visible_ids.contains(id)));
        let all_visible = self.all_visible_rows();
        let position = target_id
            .as_ref()
            .and_then(|id| all_visible.iter().position(|row| &row.id == id))
            .unwrap_or(0);
        let has_visible_rows = !all_visible.is_empty();
        drop(all_visible);
        let (_, update) =
            self.set_highlighted_visible_position_from(position, has_visible_rows, before_id);
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
                .center_highlight(area, settings)
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
        let (_, update) = self.sync_highlight_after_visible_set_change(before_id);
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
                .center_highlight(area, settings)
                .into_data_view_outcome(outcome.handled, outcome.changed);
            scrolled.activated = outcome.activated;
            scrolled
        } else {
            outcome
        }
    }

    fn toggle_all_expansion_with_settings(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let expandable = self.expandable_ids().collect::<HashSet<_>>();
        let all_expanded = expandable.is_subset(&self.expanded);
        if all_expanded {
            self.collapse_all_with_settings(area, settings)
        } else {
            self.expand_all_with_settings(area, settings)
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
        for column in self.visible_columns() {
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
                .lines
                .iter()
                .map(|line| {
                    line.spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n");
            value.insert(column.id.clone(), serde_json::Value::String(text));
        }
        Some(serde_json::Value::Object(value).to_string())
    }

    fn highlighted_copy_value(&self) -> Option<String> {
        if let Some(formatter) = &self.copy_formatter {
            let rows = self.visible_rows();
            return rows.get(self.highlighted).map(|row| formatter(row.row));
        }
        self.highlighted_json()
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

    pub fn reveal_highlighted(&mut self) -> DataViewOutcome {
        let mut settings = animation_settings();
        settings.enabled = false;
        self.reveal_highlighted_with_settings(settings)
    }

    pub fn reveal_highlighted_centered(&mut self) -> DataViewOutcome {
        let mut settings = animation_settings();
        settings.enabled = false;
        self.center_highlight(self.area, settings)
            .into_data_view_outcome(true, false)
    }

    pub fn reveal_highlighted_with_settings(
        &mut self,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        self.ensure_highlight_visible(self.area, settings)
            .into_data_view_outcome(true, false)
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

    pub(crate) fn is_navigation_key(&self, key: KeyEvent) -> bool {
        let keys = keybindings();
        let data_keys = keys.data_view();
        horizontal_jump_direction(&keys, key).is_some()
            || keys.line_up_matches(key)
            || keys.line_down_matches(key)
            || keys.line_left_matches(key)
            || keys.line_right_matches(key)
            || keys.page_up_matches(key)
            || keys.page_down_matches(key)
            || keys.home_matches(key)
            || keys.end_matches(key)
            || data_keys.top_prefix_matches(key)
            || data_keys.bottom_matches(key)
            || data_keys.next_page_matches(key)
            || data_keys.previous_page_matches(key)
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
        } else if let Some(direction) = horizontal_jump_direction(keys, key) {
            self.pending_g = false;
            self.scroll_horizontal_by(direction, area, settings)
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
                self.highlighted
                    .saturating_sub(self.visible_page_step(area)),
                area,
                settings,
            )
        } else if keys.page_down_matches(key) {
            self.pending_g = false;
            self.highlight_centered_with_settings(
                self.highlighted
                    .saturating_add(self.visible_page_step(area)),
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
        } else if data_keys.toggle_all_selection_matches(key) {
            self.pending_g = false;
            self.toggle_all_selection_with_outcome()
        } else if data_keys.toggle_expansion_matches(key) {
            self.pending_g = false;
            self.toggle_highlighted_expansion(area, settings)
        } else if data_keys.next_page_matches(key) {
            self.pending_g = false;
            self.next_page_with_settings(area, settings)
        } else if data_keys.previous_page_matches(key) {
            self.pending_g = false;
            self.previous_page_with_settings(area, settings)
        } else if data_keys.toggle_all_expansion_matches(key) {
            self.pending_g = false;
            self.toggle_all_expansion_with_settings(area, settings)
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
            id: self.focus_id.clone(),
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
        direction: isize,
        area: Rect,
        settings: AnimationSettings,
    ) -> DataViewOutcome {
        let geometry = self.scroll_geometry(area);
        let assigned_width = area.width;
        let step = if assigned_width == 0 {
            0
        } else {
            (usize::from(assigned_width) * HORIZONTAL_JUMP_PERCENT / 100).max(1) as isize
        };
        self.scroll
            .scroll_by(
                ScrollDelta::new(direction.saturating_mul(step), 0),
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

    pub(crate) fn highlight_line_with_settings(
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

    pub(crate) fn highlight_centered_with_settings(
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

    pub(crate) fn visible_page_step(&self, area: Rect) -> usize {
        let (geometry, rows) = self.scroll_geometry_and_row_geometry(area);
        let viewport_capacity = rows.capacity(
            self.scroll.target_offset().y,
            geometry.viewport.height.max(1),
        );
        let basis = self.visible_len().min(viewport_capacity);
        ((basis.saturating_mul(3)).saturating_add(4) / 5).max(1)
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

    pub(crate) fn ensure_highlight_visible(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let (geometry, rows) = self.scroll_geometry_and_row_geometry(area);
        let viewport_height = geometry.viewport.height.max(1);
        let current = self.scroll.target_offset().y;
        let (row_start, row_end) = rows.span(self.highlighted).unwrap_or((0, 0));
        let row_height = row_end.saturating_sub(row_start);
        let target = if row_height >= viewport_height {
            row_start
        } else if row_start < current {
            row_start
        } else if row_end > current.saturating_add(viewport_height) {
            row_end.saturating_sub(viewport_height)
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

    pub(crate) fn ensure_selection_placeholder_visible(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let (geometry, rows) = self.scroll_geometry_and_row_geometry(area);
        let Some(index) = self
            .display_rows()
            .iter()
            .position(|row| matches!(row, DisplayRow::SelectionPlaceholder { .. }))
        else {
            return ScrollOutcome::idle();
        };
        let (start, end) = rows.span(index).unwrap_or((0, 0));
        let viewport = geometry.viewport.height.max(1);
        let current = self.scroll.target_offset().y;
        let target = if start < current {
            start
        } else if end > current.saturating_add(viewport) {
            end.saturating_sub(viewport)
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

    pub(crate) fn center_selection_placeholder(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let (geometry, rows) = self.scroll_geometry_and_row_geometry(area);
        let Some(index) = self
            .display_rows()
            .iter()
            .position(|row| matches!(row, DisplayRow::SelectionPlaceholder { .. }))
        else {
            return ScrollOutcome::idle();
        };
        let (start, end) = rows.span(index).unwrap_or((0, 0));
        let height = end.saturating_sub(start);
        let viewport = geometry.viewport.height.max(1);
        let target = start.saturating_sub(viewport.saturating_sub(height) / 2);
        self.scroll.scroll_to(
            ScrollOffset::new(self.scroll.target_offset().x, target),
            geometry.viewport,
            geometry.content,
            settings,
        )
    }

    pub(crate) fn center_highlight(
        &mut self,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let (geometry, rows) = self.scroll_geometry_and_row_geometry(area);
        let viewport_height = geometry.viewport.height.max(1);
        let (row_start, row_end) = rows.span(self.highlighted).unwrap_or((0, 0));
        let row_height = row_end.saturating_sub(row_start);
        let target = row_start.saturating_sub(viewport_height.saturating_sub(row_height) / 2);
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

    fn visible_tree_ancestor_ids(&self, id: &Id) -> Vec<Id> {
        let rows = self.all_visible_rows();
        let mut parent_id = rows
            .iter()
            .find(|row| &row.id == id)
            .and_then(|row| row.parent_id.clone());
        let mut ancestors = Vec::new();
        let mut visited = HashSet::new();

        while let Some(current_id) = parent_id {
            if !visited.insert(current_id.clone()) {
                break;
            }
            parent_id = rows
                .iter()
                .find(|row| row.id == current_id)
                .and_then(|row| row.parent_id.clone());
            ancestors.push(current_id);
        }

        ancestors
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
        if !outcome.changed {
            return DataViewOutcome::HANDLED;
        }
        let mut scrolled = self
            .center_highlight(area, settings)
            .into_data_view_outcome(outcome.handled, outcome.changed);
        scrolled.active |= outcome.active;
        scrolled.activated |= outcome.activated;
        scrolled
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
        self.invalidate_metrics();
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
        self.invalidate_metrics();
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
        let first_tree_match = (self.tree.is_some() && self.local_transform_active())
            .then(|| {
                all_visible
                    .iter()
                    .position(|row| self.row_matches_transform(row.row))
            })
            .flatten();
        let position = first_tree_match
            .or_else(|| {
                before_id
                    .as_ref()
                    .and_then(|id| all_visible.iter().position(|row| &row.id == id))
            })
            .unwrap_or(0);
        let has_visible_rows = !all_visible.is_empty();
        drop(all_visible);

        self.set_highlighted_visible_position_from(position, has_visible_rows, before_id)
    }

    fn set_highlighted_visible_position_from(
        &mut self,
        position: usize,
        has_visible_rows: bool,
        before_id: Option<Id>,
    ) -> (bool, HighlightUpdate) {
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
        if before_id != after_id {
            self.invalidate_metrics();
        }
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

fn horizontal_jump_direction(keys: &KeyBindings, key: KeyEvent) -> Option<isize> {
    let plain_shift = key.modifiers.contains(KeyModifiers::SHIFT)
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && matches!(key.code, Key::Char(_));
    if !plain_shift {
        return None;
    }

    let base_key = unshift_key(key);
    if keys.line_left_matches(base_key) {
        Some(-1)
    } else if keys.line_right_matches(base_key) {
        Some(1)
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
