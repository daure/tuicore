use std::{collections::HashSet, hash::Hash, rc::Rc};

mod confirmation;
mod editor;
mod input;
mod node;
mod reorder;
#[cfg(test)]
mod tests;

use ratatui::{
    layout::{Constraint, Rect},
    text::Text,
};

use super::data_view::{DataViewDisplayAction, DataViewScrollSnapshot, ReorderSnapshot};
use super::ordered_selection::OrderedSelection;
use super::{
    ActivationMode, Column, ConfirmationDialog, ConfirmationDialogKeyBindings, DataView,
    DataViewOutcome, DataViewTypedEvent, Dropdown, DropdownPopupDirection, DropdownSearchMode,
    DropdownVariant, Panel,
    SeasonalEmptyState, SelectionMode, SelectionTrigger, SortDirection, TextInput,
};
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusId, FocusRequest, HotkeyEvent, Key,
    KeyEvent, KeySpec, SearchMode, TreePath, TuiEvent,
};
use confirmation::DynamicChild;
use input::ListControlInput;

const DATA_SLOT: &str = "data";
const INPUT_SLOT: &str = "add-input";
const DATA_FOCUS: &str = "data-view";
pub(super) const INPUT_FOCUS: &str = "input";
pub(super) const DROPDOWN_FOCUS: &str = "field";
const CONFIRM_SLOT: &str = "remove-confirmation";
const DIALOG_FOCUS: &str = "dialog";
const CONFIRM_OVERLAY_ID: u64 = 0x4c49_5354_434f_4e46;
const DEFAULT_MAX_ROWS: usize = 8;
type DropdownRowRenderer = Rc<dyn Fn(&str, &str, &str, DropdownSearchMode) -> Text<'static>>;

type Creator<T> = dyn FnMut(Vec<String>, &[T]) -> T;
type RemoveFormatter<T> = dyn Fn(&T) -> String;
type EditGetter<T> = dyn Fn(&T) -> Vec<String>;
type EditMutator<T> = dyn Fn(&mut T, Vec<String>);
type SameScope<T> = dyn Fn(&T, &T) -> bool;

struct Editable<T> {
    getter: Box<EditGetter<T>>,
    mutator: Box<EditMutator<T>>,
}

struct RemoveConfirmation<T> {
    title: String,
    formatter: Box<RemoveFormatter<T>>,
}

#[derive(Clone)]
pub struct ListControlField {
    placeholder: String,
    kind: ListControlFieldKind,
    required: bool,
    visibility: Option<ListControlFieldVisibility>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ListControlFieldVisibility {
    field_index: usize,
    allowed_values: Vec<String>,
}

#[derive(Clone)]
enum ListControlFieldKind {
    Text,
    Dropdown {
        options: Vec<(String, String)>,
        renderer: Option<DropdownRowRenderer>,
        min_search_chars: usize,
        max_filtered_items: Option<usize>,
        visible_without_search: Option<Vec<String>>,
    },
}

impl ListControlField {
    pub fn text(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            kind: ListControlFieldKind::Text,
            required: true,
            visibility: None,
        }
    }

    /// Creates a dropdown field. Option strings must be non-empty because `""`
    /// represents no selection for optional fields.
    pub fn dropdown(
        placeholder: impl Into<String>,
        options: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            placeholder: placeholder.into(),
            kind: ListControlFieldKind::Dropdown {
                options: options
                    .into_iter()
                    .map(Into::into)
                    .map(|option| (option.clone(), option))
                    .collect(),
                renderer: None,
                min_search_chars: 0,
                max_filtered_items: None,
                visible_without_search: None,
            },
            required: true,
            visibility: None,
        }
    }

    pub fn dropdown_options(
        placeholder: impl Into<String>,
        options: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        Self {
            placeholder: placeholder.into(),
            kind: ListControlFieldKind::Dropdown {
                options: options
                    .into_iter()
                    .map(|(id, label)| (id.into(), label.into()))
                    .collect(),
                renderer: None,
                min_search_chars: 0,
                max_filtered_items: None,
                visible_without_search: None,
            },
            required: true,
            visibility: None,
        }
    }

    pub fn dropdown_options_rich(
        placeholder: impl Into<String>,
        options: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
        renderer: impl Fn(&str, &str, &str, DropdownSearchMode) -> Text<'static> + 'static,
    ) -> Self {
        let mut field = Self::dropdown_options(placeholder, options);
        if let ListControlFieldKind::Dropdown { renderer: slot, .. } = &mut field.kind {
            *slot = Some(Rc::new(renderer));
        }
        field
    }

    pub fn min_search_chars(mut self, count: usize) -> Self {
        if let ListControlFieldKind::Dropdown {
            min_search_chars, ..
        } = &mut self.kind
        {
            *min_search_chars = count;
        }
        self
    }

    pub fn max_filtered_items(mut self, count: usize) -> Self {
        if let ListControlFieldKind::Dropdown {
            max_filtered_items, ..
        } = &mut self.kind
        {
            *max_filtered_items = Some(count);
        }
        self
    }

    pub fn visible_without_search(
        mut self,
        ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        if let ListControlFieldKind::Dropdown {
            visible_without_search,
            ..
        } = &mut self.kind
        {
            *visible_without_search = Some(ids.into_iter().map(Into::into).collect());
        }
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Shows this field only when an earlier field has one of `allowed_values`.
    ///
    /// `ListControl::new_fields` rejects self and later-field references.
    pub fn visible_when(
        mut self,
        field_index: usize,
        allowed_values: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.visibility = Some(ListControlFieldVisibility {
            field_index,
            allowed_values: allowed_values.into_iter().map(Into::into).collect(),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListControlEvent<Id> {
    Added {
        row_id: Id,
    },
    AddedChild {
        row_id: Id,
        parent_id: Id,
    },
    Removed {
        row_id: Id,
    },
    Edited {
        row_id: Id,
    },
    AddCancelled,
    EditCancelled {
        row_id: Id,
    },
    Reordered {
        row_ids: Vec<Id>,
    },
    TreeMoved {
        row_id: Id,
        parent_id: Option<Id>,
        sibling_index: usize,
    },
    TreeBlockMoved {
        row_ids: Vec<Id>,
        parent_id: Option<Id>,
        sibling_index: usize,
    },
    TreeBlockMoveCancelled {
        row_ids: Vec<Id>,
    },
    ReorderCancelled {
        row_id: Id,
    },
    ReorderUnavailable {
        reason: ListControlReorderUnavailable,
    },
    CheckedChanged {
        checked: Vec<Id>,
        added: Vec<Id>,
        removed: Vec<Id>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListControlReorderUnavailable {
    Tree,
    VisibleSubset,
    TransformActive,
    Paginated,
    DuplicateRowIds,
    DuplicateRankKeys,
    DataChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListControlKeyBindings {
    pub add: Vec<KeySpec>,
    pub add_child: Vec<KeySpec>,
    pub remove: Vec<KeySpec>,
    pub edit: Vec<KeySpec>,
    pub reorder: Vec<KeySpec>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListControlDisplayKeyBindings {
    line_up: Vec<KeySpec>,
    line_down: Vec<KeySpec>,
    page_up: Vec<KeySpec>,
    page_down: Vec<KeySpec>,
    top: Vec<KeySpec>,
    top_prefix: Vec<KeySpec>,
    bottom: Vec<KeySpec>,
    activate: Vec<KeySpec>,
    reorder: Vec<KeySpec>,
}

impl ListControlDisplayKeyBindings {
    pub fn line_up(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.line_up = keys.into_iter().collect();
        self
    }

    pub fn line_down(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.line_down = keys.into_iter().collect();
        self
    }

    pub fn page_up(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.page_up = keys.into_iter().collect();
        self
    }

    pub fn page_down(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.page_down = keys.into_iter().collect();
        self
    }

    pub fn top(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.top = keys.into_iter().collect();
        self
    }

    pub fn top_prefix(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.top_prefix = keys.into_iter().collect();
        self
    }

    pub fn bottom(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.bottom = keys.into_iter().collect();
        self
    }

    pub fn activate(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.activate = keys.into_iter().collect();
        self
    }

    pub fn reorder(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.reorder = keys.into_iter().collect();
        self
    }

    fn action(&self, key: KeyEvent) -> Option<DataViewDisplayAction> {
        let matches = |bindings: &[KeySpec]| bindings.iter().any(|binding| binding.matches(key));
        if matches(&self.line_up) {
            Some(DataViewDisplayAction::LineUp)
        } else if matches(&self.line_down) {
            Some(DataViewDisplayAction::LineDown)
        } else if matches(&self.page_up) {
            Some(DataViewDisplayAction::PageUp)
        } else if matches(&self.page_down) {
            Some(DataViewDisplayAction::PageDown)
        } else if matches(&self.top) {
            Some(DataViewDisplayAction::Top)
        } else if matches(&self.bottom) {
            Some(DataViewDisplayAction::Bottom)
        } else if matches(&self.activate) {
            Some(DataViewDisplayAction::Activate)
        } else {
            None
        }
    }

    fn reorder_matches(&self, key: KeyEvent) -> bool {
        self.reorder.iter().any(|binding| binding.matches(key))
    }

    fn top_prefix_matches(&self, key: KeyEvent) -> bool {
        self.top_prefix.iter().any(|binding| binding.matches(key))
    }
}

impl Default for ListControlKeyBindings {
    fn default() -> Self {
        Self {
            add: vec![KeySpec::plain('+')],
            add_child: vec![KeySpec::plain('\\')],
            remove: vec![KeySpec::key_with_modifiers(
                Key::Char('x'),
                crate::KeyModifiers::CONTROL,
            )],
            edit: vec![KeySpec::plain('e')],
            reorder: vec![KeySpec::key_with_modifiers(
                Key::Char('m'),
                crate::KeyModifiers::CONTROL,
            )],
        }
    }
}

impl ListControlKeyBindings {
    pub fn add(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.add = keys.into_iter().collect();
        self
    }

    pub fn remove(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.remove = keys.into_iter().collect();
        self
    }

    pub fn add_child(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.add_child = keys.into_iter().collect();
        self
    }

    pub fn edit(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.edit = keys.into_iter().collect();
        self
    }

    pub fn reorder(mut self, keys: impl IntoIterator<Item = KeySpec>) -> Self {
        self.reorder = keys.into_iter().collect();
        self
    }

    fn add_matches(&self, key: KeyEvent) -> bool {
        self.add.iter().any(|binding| binding.matches(key))
    }

    fn remove_matches(&self, key: KeyEvent) -> bool {
        self.remove.iter().any(|binding| binding.matches(key))
    }

    fn add_child_matches(&self, key: KeyEvent) -> bool {
        self.add_child.iter().any(|binding| binding.matches(key))
    }

    fn edit_matches(&self, key: KeyEvent) -> bool {
        self.edit.iter().any(|binding| binding.matches(key))
    }

    fn reorder_matches(&self, key: KeyEvent) -> bool {
        self.reorder.iter().any(|binding| binding.matches(key))
    }
}

struct ReorderState<Id> {
    snapshot: ReorderSnapshot<Id>,
    scroll_snapshot: DataViewScrollSnapshot,
    staged: Vec<Id>,
    scope_ids: Option<Vec<Id>>,
    moving_id: Id,
    pending_g: bool,
}

struct TreeReorderState<Id> {
    snapshot: super::data_view::TreeEditSnapshot<Id>,
    staged_snapshot: super::data_view::TreeEditSnapshot<Id>,
    scroll_snapshot: DataViewScrollSnapshot,
    moving_id: Id,
    changed: bool,
}

struct TreeSelectionState<Id> {
    selected: Vec<Id>,
    anchor: Option<Id>,
    range_mode: bool,
}

type FlatRangeSelectionState<Id> = OrderedSelection<Id>;

struct FlatBlockMoveState<Id> {
    snapshot: ReorderSnapshot<Id>,
    scroll_snapshot: DataViewScrollSnapshot,
    selected: Vec<Id>,
    scope_ids: Option<Vec<Id>>,
    target_index: usize,
    /// Boundary in complete snapshot/display order, including rows outside the scope.
    visual_target_index: Option<usize>,
    highlighted_id: Id,
    pending_g: bool,
}

struct TreeBlockMoveState<Id> {
    snapshot: super::data_view::TreeEditSnapshot<Id>,
    scroll_snapshot: DataViewScrollSnapshot,
    expanded_before: HashSet<Id>,
    source_parent_id: Option<Id>,
    parent_id: Option<Id>,
    selected: Vec<Id>,
    sibling_index: usize,
    visual_sibling_index: Option<usize>,
    pending_g: bool,
}

pub struct ListControl<T, Id, M = ()> {
    data_view: DataView<T, Id>,
    display_only: bool,
    disabled: bool,
    enabled_border: Option<crate::BorderKind>,
    panel: Panel,
    panel_visible: bool,
    inputs: Vec<ListControlInput<M>>,
    required_fields: Vec<bool>,
    field_visibility: Vec<Option<ListControlFieldVisibility>>,
    creator: Option<Box<Creator<T>>>,
    editable: Option<Editable<T>>,
    keys: ListControlKeyBindings,
    adding: bool,
    adding_parent: Option<Option<Id>>,
    editing: Option<Id>,
    events: Vec<ListControlEvent<Id>>,
    area: Rect,
    data_area: Rect,
    input_area: Rect,
    active_field: usize,
    hotkey: Option<String>,
    pending_hotkey_prefix: Option<String>,
    max_rows: usize,
    remove_confirmation: Option<RemoveConfirmation<T>>,
    confirmation_keys: ConfirmationDialogKeyBindings,
    pending_remove: Option<Id>,
    confirmation_dialog: DynamicChild<ConfirmationDialog<M>, M>,
    confirmation_area: Rect,
    confirmation_bounds: Rect,
    reorder_column: Option<String>,
    reorder_scope: Option<Box<SameScope<T>>>,
    transient_selection_enabled: bool,
    display_keys: Option<ListControlDisplayKeyBindings>,
    display_pending_top_prefix: bool,
    allow_horizontal_moving: bool,
    reorder: Option<ReorderState<Id>>,
    tree_reorder: Option<TreeReorderState<Id>>,
    tree_selection: Option<TreeSelectionState<Id>>,
    flat_range_selection: Option<FlatRangeSelectionState<Id>>,
    flat_block_move: Option<FlatBlockMoveState<Id>>,
    tree_block_move: Option<TreeBlockMoveState<Id>>,
}

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub fn new(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        mut creator: impl FnMut(String, &[T]) -> T + 'static,
    ) -> Self {
        Self::new_fields(
            rows,
            row_id,
            [ListControlField::text("New item")],
            move |mut values, rows| creator(values.remove(0), rows),
        )
    }

    pub fn new_fields(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        fields: impl IntoIterator<Item = ListControlField>,
        creator: impl FnMut(Vec<String>, &[T]) -> T + 'static,
    ) -> Self {
        let fields = fields.into_iter().collect::<Vec<_>>();
        assert!(
            !fields.is_empty(),
            "ListControl requires at least one field"
        );
        assert!(
            fields.iter().all(|field| match &field.kind {
                ListControlFieldKind::Text => true,
                ListControlFieldKind::Dropdown { options, .. } => {
                    options
                        .iter()
                        .all(|(id, label)| !id.is_empty() && !label.is_empty())
                }
            }),
            "ListControl dropdown option strings must be non-empty because \"\" represents no selection"
        );
        assert!(
            fields.iter().enumerate().all(|(index, field)| field
                .visibility
                .as_ref()
                .is_none_or(|visibility| visibility.field_index < index)),
            "ListControl field visibility conditions must reference an earlier field"
        );
        let inputs = fields
            .iter()
            .map(|field| match &field.kind {
                ListControlFieldKind::Text => {
                    ListControlInput::Text(TextInput::new().placeholder(field.placeholder.clone()))
                }
                ListControlFieldKind::Dropdown {
                    options,
                    renderer,
                    min_search_chars,
                    max_filtered_items,
                    visible_without_search,
                } => ListControlInput::Dropdown(Some({
                    let mut input = if let Some(renderer) = renderer {
                        let renderer = Rc::clone(renderer);
                        Dropdown::single_rich(
                            options.clone(),
                            |option| option.0.clone(),
                            |option| option.1.clone(),
                            move |option, query, mode| {
                                renderer(&option.0, &option.1, query, mode)
                            },
                        )
                    } else {
                        Dropdown::single(
                            options.clone(),
                            |option| option.0.clone(),
                            |option| option.1.clone(),
                        )
                    }
                    .variant(DropdownVariant::Filled)
                    .search_mode(DropdownSearchMode::Fuzzy)
                    .min_search_chars(*min_search_chars)
                    .placeholder(field.placeholder.clone());
                    if let Some(limit) = max_filtered_items {
                        input = input.max_filtered_items(*limit);
                    }
                    if let Some(ids) = visible_without_search {
                        input = input.visible_without_search(ids.clone());
                    }
                    if field.required {
                        input
                    } else {
                        input.no_selection_text("No selection")
                    }
                })),
            })
            .collect();
        let required_fields = fields.iter().map(|field| field.required).collect();
        let field_visibility = fields
            .iter()
            .map(|field| field.visibility.clone())
            .collect();
        Self {
            data_view: DataView::new(rows, row_id),
            display_only: false,
            disabled: false,
            enabled_border: None,
            panel: Panel::new(),
            panel_visible: true,
            inputs,
            required_fields,
            field_visibility,
            creator: Some(Box::new(creator)),
            editable: None,
            keys: ListControlKeyBindings::default(),
            adding: false,
            adding_parent: None,
            editing: None,
            events: Vec::new(),
            area: Rect::default(),
            data_area: Rect::default(),
            input_area: Rect::default(),
            active_field: 0,
            hotkey: None,
            pending_hotkey_prefix: None,
            max_rows: DEFAULT_MAX_ROWS,
            remove_confirmation: None,
            confirmation_keys: ConfirmationDialogKeyBindings {
                yes: Some(KeySpec::plain('d')),
                no: Some(KeySpec::plain('c')),
            },
            pending_remove: None,
            confirmation_dialog: DynamicChild::default(),
            confirmation_area: Rect::default(),
            confirmation_bounds: Rect::default(),
            reorder_column: None,
            reorder_scope: None,
            transient_selection_enabled: false,
            display_keys: None,
            display_pending_top_prefix: false,
            allow_horizontal_moving: true,
            reorder: None,
            tree_reorder: None,
            tree_selection: None,
            flat_range_selection: None,
            flat_block_move: None,
            tree_block_move: None,
        }
    }

    /// Creates a panel-less read-only list for embedding in a focused host.
    pub fn display(rows: impl IntoIterator<Item = T>, row_id: impl Fn(&T) -> Id + 'static) -> Self {
        Self {
            data_view: DataView::new(rows, row_id)
                .headers(false)
                .action_bar(false)
                .filter_controls(false),
            display_only: true,
            disabled: false,
            enabled_border: None,
            panel: Panel::new(),
            panel_visible: false,
            inputs: Vec::new(),
            required_fields: Vec::new(),
            field_visibility: Vec::new(),
            creator: None,
            editable: None,
            keys: ListControlKeyBindings::default(),
            adding: false,
            adding_parent: None,
            editing: None,
            events: Vec::new(),
            area: Rect::default(),
            data_area: Rect::default(),
            input_area: Rect::default(),
            active_field: 0,
            hotkey: None,
            pending_hotkey_prefix: None,
            max_rows: DEFAULT_MAX_ROWS,
            remove_confirmation: None,
            confirmation_keys: ConfirmationDialogKeyBindings {
                yes: Some(KeySpec::plain('d')),
                no: Some(KeySpec::plain('c')),
            },
            pending_remove: None,
            confirmation_dialog: DynamicChild::default(),
            confirmation_area: Rect::default(),
            confirmation_bounds: Rect::default(),
            reorder_column: None,
            reorder_scope: None,
            transient_selection_enabled: false,
            display_keys: None,
            display_pending_top_prefix: false,
            allow_horizontal_moving: true,
            reorder: None,
            tree_reorder: None,
            tree_selection: None,
            flat_range_selection: None,
            flat_block_move: None,
            tree_block_move: None,
        }
    }

    pub fn list(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        accessor: impl Fn(&T) -> String + 'static,
        creator: impl FnMut(String, &[T]) -> T + 'static,
    ) -> Self {
        Self::new(rows, row_id, creator).column(Column::text(
            "label",
            "",
            Constraint::Percentage(100),
            accessor,
        ))
    }

    pub fn column(mut self, column: Column<T, Id>) -> Self {
        self.data_view.add_column(column);
        self
    }

    pub fn columns(mut self, columns: impl IntoIterator<Item = Column<T, Id>>) -> Self {
        self.data_view.add_columns(columns);
        self
    }

    pub fn copy_with(mut self, formatter: impl Fn(&T) -> String + 'static) -> Self {
        self.data_view = self.data_view.copy_with(formatter);
        self
    }

    pub fn empty_state(mut self, empty_state: SeasonalEmptyState) -> Self {
        self.data_view = self.data_view.empty_state(empty_state);
        self
    }

    pub fn empty_message(mut self, message: impl Into<String>) -> Self {
        self.data_view = self.data_view.empty_message(message);
        self
    }

    pub fn set_empty_state(&mut self, empty_state: SeasonalEmptyState) {
        self.data_view.set_empty_state(empty_state);
    }

    pub fn headers(mut self, headers: bool) -> Self {
        self.data_view = self.data_view.headers(headers);
        self
    }

    pub fn focus_id(mut self, id: impl Into<String>) -> Self {
        self.data_view = self.data_view.focus_id(id);
        self
    }

    pub fn action_bar(mut self, action_bar: bool) -> Self {
        self.data_view =
            self.data_view
                .action_bar(if self.display_only { false } else { action_bar });
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.set_disabled(disabled);
        self
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        if disabled {
            self.enabled_border = self.panel.border_kind();
            self.panel.set_border(crate::BorderKind::AsciiDashed);
        } else {
            if let Some(border) = self.enabled_border.take() {
                self.panel.set_border(border);
            } else {
                self.panel.clear_border();
            }
        }
        if disabled {
            self.cancel_editor(false);
            let mut settings = crate::AnimationSettings::default();
            settings.enabled = false;
            self.cancel_reorder_for_focus_loss(settings);
            self.clear_tree_selection();
            self.clear_flat_range_selection();
        }
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    pub fn filter_controls(mut self, enabled: bool) -> Self {
        self.data_view =
            self.data_view
                .filter_controls(if self.display_only { false } else { enabled });
        self
    }

    /// Wraps rich display rows to the available column width.
    pub fn wrap_cells(mut self) -> Self {
        self.set_wrap_cells(true);
        self
    }

    pub fn set_wrap_cells(&mut self, wrap_cells: bool) {
        self.data_view.set_wrap_cells(wrap_cells);
    }

    /// Sets a display row-height policy. Returned zero heights are clamped to one.
    pub fn row_height_by(mut self, row_height: impl Fn(&T) -> u16 + 'static) -> Self {
        self.data_view.set_row_height_by(row_height);
        self
    }

    pub fn search_mode(mut self, mode: SearchMode) -> Self {
        self.data_view = self.data_view.search_mode(mode);
        self
    }

    pub fn focused_events_before_global_hotkeys(mut self, enabled: bool) -> Self {
        self.data_view = self.data_view.focused_events_before_global_hotkeys(enabled);
        self
    }

    pub fn activation_mode(mut self, mode: ActivationMode) -> Self {
        self.data_view = self.data_view.activation_mode(mode);
        self
    }

    pub fn selection_mode(mut self, mode: SelectionMode) -> Self {
        self.data_view = self.data_view.selection_mode(mode);
        self
    }

    pub fn selection_trigger(mut self, trigger: SelectionTrigger) -> Self {
        self.data_view = self.data_view.selection_trigger(trigger);
        self
    }

    pub fn selection_propagation(mut self, propagation: super::SelectionPropagation) -> Self {
        self.data_view = self.data_view.selection_propagation(propagation);
        self
    }

    pub fn selection_glyphs(mut self, glyphs: super::SelectionGlyphs) -> Self {
        self.data_view = self.data_view.selection_glyphs(glyphs);
        self
    }

    pub fn selected(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.data_view = self.data_view.selected(ids);
        self
    }

    pub fn tree(mut self, tree: super::TreeAdapter<T, Id>) -> Self {
        self.data_view = self.data_view.tree(tree);
        self
    }

    pub fn allow_horizontal_moving(mut self, allow: bool) -> Self {
        self.allow_horizontal_moving = allow;
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.data_view = self.data_view.expanded(ids);
        self
    }

    pub fn row_height(mut self, row_height: u16) -> Self {
        self.data_view.set_row_height(row_height);
        self
    }

    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = max_rows.max(1);
        self
    }

    pub fn confirm_remove(
        mut self,
        title: impl Into<String>,
        formatter: impl Fn(&T) -> String + 'static,
    ) -> Self {
        if !self.display_only {
            self.remove_confirmation = Some(RemoveConfirmation {
                title: title.into(),
                formatter: Box::new(formatter),
            });
        }
        self
    }

    pub fn confirmation_keybindings(mut self, keys: ConfirmationDialogKeyBindings) -> Self {
        self.confirmation_keys = keys;
        self
    }

    pub fn has_remove_confirmation(&self) -> bool {
        self.remove_confirmation.is_some()
    }

    pub fn is_confirming_remove(&self) -> bool {
        self.confirmation_dialog.is_some()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.panel.set_top_left(title);
        self
    }

    pub fn panel(mut self, panel: Panel) -> Self {
        self.panel = panel;
        if self.disabled {
            self.enabled_border = self.panel.border_kind();
            self.panel.set_border(crate::BorderKind::AsciiDashed);
        }
        self.panel.set_hotkey_badge(self.hotkey.clone());
        self.panel
            .set_pending_hotkey_prefix(self.pending_hotkey_prefix.clone());
        self
    }

    pub fn panel_visible(mut self, visible: bool) -> Self {
        self.panel_visible = visible;
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        let hotkey = hotkey.into();
        self.data_view.set_hotkey(hotkey.clone());
        self.hotkey = Some(hotkey.clone());
        self.panel.set_hotkey_badge(Some(hotkey));
        self
    }

    pub fn keybindings(mut self, keys: ListControlKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn display_keybindings(mut self, keys: ListControlDisplayKeyBindings) -> Self {
        self.set_display_keybindings(keys);
        self
    }

    pub fn set_display_keybindings(&mut self, keys: ListControlDisplayKeyBindings) {
        if self.display_only {
            self.display_keys = Some(keys);
            self.clear_display_pending_top_prefix();
            self.clear_pending_reorder_g();
        }
    }

    pub fn sorted_by(mut self, column_id: impl Into<String>, direction: SortDirection) -> Self {
        assert!(
            self.reorder_column.is_none(),
            "ListControl automatic sorting and reorderable mode are mutually exclusive"
        );
        self.data_view = self.data_view.sorted_by(column_id, direction);
        self
    }

    pub fn reorderable_by(mut self, column_id: impl Into<String>) -> Self {
        assert!(
            !self.data_view.has_automatic_sort(),
            "ListControl automatic sorting and reorderable mode are mutually exclusive"
        );
        let column_id = column_id.into();
        self.data_view.configure_reorder_sort(&column_id);
        self.reorder_column = Some(column_id);
        self.reorder_scope = None;
        self
    }

    /// Enables transient range and sparse selections without enabling reordering.
    pub fn transient_selection(mut self, enabled: bool) -> Self {
        self.transient_selection_enabled = enabled;
        self
    }

    pub fn reorderable_by_scoped(
        mut self,
        column_id: impl Into<String>,
        same_scope: impl Fn(&T, &T) -> bool + 'static,
    ) -> Self {
        assert!(
            !self.data_view.has_automatic_sort(),
            "ListControl automatic sorting and reorderable mode are mutually exclusive"
        );
        let column_id = column_id.into();
        self.data_view.configure_reorder_sort(&column_id);
        self.reorder_column = Some(column_id);
        self.reorder_scope = Some(Box::new(same_scope));
        self
    }

    pub fn editable(
        mut self,
        getter: impl Fn(&T) -> Vec<String> + 'static,
        mutator: impl Fn(&mut T, Vec<String>) + 'static,
    ) -> Self {
        if !self.display_only {
            self.editable = Some(Editable {
                getter: Box::new(getter),
                mutator: Box::new(mutator),
            });
        }
        self
    }

    pub fn items(&self) -> &[T] {
        self.data_view.rows()
    }

    pub fn data_view(&self) -> &DataView<T, Id> {
        &self.data_view
    }

    pub fn transient_selected_ids(&self) -> Vec<Id> {
        self.flat_range_selection
            .as_ref()
            .map(|selection| selection.selected.clone())
            .or_else(|| {
                self.tree_selection
                    .as_ref()
                    .map(|selection| selection.selected.clone())
            })
            .unwrap_or_default()
    }

    pub fn clear_transient_selection(&mut self) {
        self.clear_tree_selection();
        self.clear_flat_range_selection();
        self.data_view
            .reconcile_selection_to_highlight_on_navigate();
    }

    /// Updates the displayed row highlight without changing ListControl interactions.
    pub fn set_highlighted_id(&mut self, id: &Id) -> DataViewOutcome {
        self.data_view.highlight_id(id)
    }

    pub fn set_rows(&mut self, rows: impl IntoIterator<Item = T>) -> DataViewOutcome {
        let outcome = self.data_view.set_rows(rows);
        self.restore_transient_selection_after_row_replacement();
        if self.is_reordering() {
            if !self.reorder_states_are_compatible() {
                self.reject_reorder_for_data_change(crate::AnimationSettings::default());
            } else {
                self.restore_block_move_overlay_after_row_replacement(
                    crate::AnimationSettings::default(),
                );
            }
        }
        outcome
    }

    pub fn dropdown_search_query(&self, field_index: usize) -> Option<&str> {
        let ListControlInput::Dropdown(Some(dropdown)) = self.inputs.get(field_index)? else {
            return None;
        };
        Some(dropdown.search_query())
    }

    pub fn set_dropdown_search_mode(&mut self, field_index: usize, mode: DropdownSearchMode) {
        let Some(ListControlInput::Dropdown(Some(dropdown))) = self.inputs.get_mut(field_index)
        else {
            return;
        };
        dropdown.set_search_mode(mode);
    }

    pub fn set_dropdown_popup_direction(
        &mut self,
        field_index: usize,
        direction: DropdownPopupDirection,
    ) {
        let Some(ListControlInput::Dropdown(Some(dropdown))) = self.inputs.get_mut(field_index)
        else {
            return;
        };
        dropdown.set_popup_direction(direction);
    }

    pub fn set_dropdown_external_loading(&mut self, field_index: usize, loading: bool) {
        let Some(ListControlInput::Dropdown(Some(dropdown))) = self.inputs.get_mut(field_index)
        else {
            return;
        };
        dropdown.set_external_loading(loading);
    }

    pub fn set_dropdown_rows(
        &mut self,
        field_index: usize,
        rows: impl IntoIterator<Item = (String, String)>,
    ) {
        let Some(ListControlInput::Dropdown(Some(dropdown))) = self.inputs.get_mut(field_index)
        else {
            return;
        };
        dropdown.set_rows(rows);
        dropdown.set_row_height(2);
    }

    pub fn data_view_mut(&mut self) -> &mut DataView<T, Id> {
        let mut settings = crate::AnimationSettings::default();
        settings.enabled = false;
        self.cancel_flat_block_move(settings);
        self.cancel_tree_block_move(settings);
        self.clear_tree_selection();
        self.clear_flat_range_selection();
        &mut self.data_view
    }

    fn restore_transient_selection_after_row_replacement(&mut self) {
        let display_ids = self.data_view.reorder_visible_ids();
        if let Some(anchor) = self
            .flat_range_selection
            .as_ref()
            .map(|selection| selection.anchor.clone())
        {
            let scope_ids = self.flat_scope_ids(&anchor, display_ids.clone());
            let selection = self
                .flat_range_selection
                .as_mut()
                .expect("flat selection remains active");
            if scope_ids.is_empty() || !selection.reconcile(&scope_ids) {
                self.clear_flat_range_selection();
                return;
            }
            self.data_view
                .set_selection_overlay(selection.selected.clone(), None, 0, false);
        } else if let Some(selection) = self.tree_selection.as_ref() {
            let selected = display_ids
                .iter()
                .filter(|id| selection.selected.contains(id))
                .cloned()
                .collect::<Vec<_>>();
            if selected.is_empty() {
                self.clear_tree_selection();
                return;
            }
            let selection = self
                .tree_selection
                .as_mut()
                .expect("tree selection remains active");
            if selection
                .anchor
                .as_ref()
                .is_some_and(|anchor| !display_ids.contains(anchor))
            {
                selection.anchor = selected.first().cloned();
            }
            selection.selected = selected.clone();
            self.data_view
                .set_selection_overlay(selected, None, 0, false);
        }
    }

    fn display_action(&self, key: KeyEvent) -> Option<DataViewDisplayAction> {
        self.display_keys.as_ref().and_then(|keys| keys.action(key))
    }

    fn display_reorder_matches(&self, key: KeyEvent) -> bool {
        self.display_keys
            .as_ref()
            .is_some_and(|keys| keys.reorder_matches(key))
    }

    fn reorder_key_matches(&self, key: KeyEvent) -> bool {
        if self.display_uses_custom_bindings() {
            self.display_reorder_matches(key)
        } else {
            self.keys.reorder_matches(key)
        }
    }

    fn display_uses_custom_bindings(&self) -> bool {
        self.display_only && self.display_keys.is_some()
    }

    fn handle_display_data_key(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if self.display_top_prefix_matches(key) {
            if self.display_pending_top_prefix {
                self.display_pending_top_prefix = false;
                return self.handle_display_data_action(DataViewDisplayAction::Top, ctx);
            }
            self.display_pending_top_prefix = true;
            ctx.stop_propagation();
            return Some(EventOutcome::Handled);
        }
        self.display_pending_top_prefix = false;
        let action = self.display_action(key)?;
        self.handle_display_data_action(action, ctx)
    }

    fn handle_display_data_action(
        &mut self,
        action: DataViewDisplayAction,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        let outcome = self
            .data_view
            .handle_display_action(action, self.data_area, ctx.animation());
        if outcome.needs_redraw() {
            ctx.request_redraw();
            ctx.request_layout();
        }
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    fn display_top_prefix_matches(&self, key: KeyEvent) -> bool {
        self.display_keys
            .as_ref()
            .is_some_and(|keys| keys.top_prefix_matches(key))
    }

    fn display_suppresses_global_key(&self, key: KeyEvent) -> bool {
        if !self.display_uses_custom_bindings() {
            return false;
        }
        let keys = crate::keybindings();
        self.data_view.is_navigation_key(key) || keys.data_view().activate_matches(key)
    }

    pub fn take_data_view_events(&mut self) -> Vec<DataViewTypedEvent<Id>> {
        self.data_view.take_events()
    }

    pub fn panel_ref(&self) -> &Panel {
        &self.panel
    }

    pub fn panel_mut(&mut self) -> &mut Panel {
        &mut self.panel
    }

    /// Applies the embedding host's focused state to display rows.
    pub fn set_display_focused(&mut self, focused: bool) {
        let losing_focus = self.data_view.is_focused() && !focused;
        self.data_view.set_focused(focused);
        if !focused {
            self.clear_display_pending_top_prefix();
            if losing_focus {
                self.cancel_reorder_for_focus_loss(crate::AnimationSettings::default());
            }
        }
    }

    pub(crate) fn clear_display_pending_top_prefix(&mut self) {
        self.display_pending_top_prefix = false;
    }

    pub fn is_adding(&self) -> bool {
        self.adding
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub fn is_reordering(&self) -> bool {
        self.reorder.is_some()
            || self.tree_reorder.is_some()
            || self.flat_block_move.is_some()
            || self.tree_block_move.is_some()
    }

    pub fn take_events(&mut self) -> Vec<ListControlEvent<Id>> {
        self.events
            .extend(self.data_view.drain_selection_changes().into_iter().map(
                |(checked, added, removed)| ListControlEvent::CheckedChanged {
                    checked,
                    added,
                    removed,
                },
            ));
        std::mem::take(&mut self.events)
    }

    fn remove_highlighted(&mut self) -> bool {
        if self.display_only {
            return false;
        }
        self.clear_tree_selection();
        self.clear_flat_range_selection();
        let Some(row_id) = self.data_view.highlighted_id() else {
            return false;
        };
        if self.data_view.remove_subtree(&row_id).is_none() {
            return false;
        }
        self.events.push(ListControlEvent::Removed { row_id });
        true
    }

    fn focus_child(ctx: &mut EventCtx<M>, route: &EventRoute, slot: &str, id: &str) {
        let current = ctx.current_path();
        let parent = current
            .strip_suffix(&route.path)
            .unwrap_or_else(TreePath::new);
        ctx.focus(FocusRequest::TargetAt {
            path: parent.child(ChildKey::new(slot)),
            id: FocusId::new(id),
        });
    }

    fn input_slot(index: usize) -> ChildKey {
        if index == 0 {
            ChildKey::new(INPUT_SLOT)
        } else {
            ChildKey::new(format!("{INPUT_SLOT}-{index}"))
        }
    }

    fn full_row_input_area(&self, input_area: Rect) -> Rect {
        let columns =
            self.data_view
                .visible_column_rects(self.data_area, input_area.y, input_area.height);
        let Some(left) = columns
            .iter()
            .filter(|area| !area.is_empty())
            .map(|area| area.x)
            .min()
        else {
            return input_area;
        };
        let right = columns
            .iter()
            .filter(|area| !area.is_empty())
            .map(|area| area.right())
            .max()
            .unwrap_or(input_area.right());
        Rect::new(
            left,
            input_area.y,
            right.saturating_sub(left),
            input_area.height,
        )
    }

    fn handle_control_key(
        &mut self,
        key: KeyEvent,
        route: &EventRoute,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if self.display_only {
            return None;
        }
        if self.editor_active() {
            if self.inputs[self.active_field].dropdown_is_open() {
                return None;
            } else if crate::keybindings().focus().unfocus_matches(key) {
                self.cancel_editor(true);
                self.restore_data_focus(route, ctx);
            } else if matches!(key.code, Key::Enter) {
                let final_field = self.active_field_is_last_visible();
                if self.advance_field(route, ctx) {
                    if final_field {
                        self.restore_data_focus(route, ctx);
                    }
                }
            } else {
                return None;
            }
        } else if let Some(changed) = self.handle_quick_tree_move(key) {
            if changed {
                ctx.request_layout();
            }
        } else if self.keys.add_child_matches(key) && self.begin_add_child() {
            let index = self.active_field;
            Self::focus_child(
                ctx,
                route,
                Self::input_slot(index).as_str(),
                self.inputs[index].focus_id(),
            );
            ctx.request_layout();
        } else if self.keys.add_matches(key) {
            self.begin_add_sibling();
            let index = self.active_field;
            Self::focus_child(
                ctx,
                route,
                Self::input_slot(index).as_str(),
                self.inputs[index].focus_id(),
            );
            ctx.request_layout();
        } else if self.keys.edit_matches(key) && self.begin_edit() {
            let index = self.active_field;
            Self::focus_child(
                ctx,
                route,
                Self::input_slot(index).as_str(),
                self.inputs[index].focus_id(),
            );
            ctx.request_layout();
        } else if self.keys.remove_matches(key) {
            if self.remove_confirmation.is_some() {
                if self.request_remove_confirmation(ctx) {
                    Self::focus_child(ctx, route, CONFIRM_SLOT, DIALOG_FOCUS);
                    ctx.request_layout();
                }
            } else if self.remove_highlighted() {
                ctx.request_layout();
            }
        } else {
            return None;
        }
        ctx.request_redraw();
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    fn handle_visual_hotkey(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) {
        let TuiEvent::Hotkey(hotkey) = event else {
            return;
        };
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                self.pending_hotkey_prefix = Some(prefix.clone());
                self.panel.set_pending_hotkey_prefix(Some(prefix.clone()));
                ctx.request_redraw();
            }
            HotkeyEvent::Canceled | HotkeyEvent::Commit(_) => {
                if self.pending_hotkey_prefix.take().is_some() {
                    self.panel.set_pending_hotkey_prefix(None);
                    ctx.request_redraw();
                }
            }
        }
    }
}
