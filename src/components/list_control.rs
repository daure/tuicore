use std::hash::Hash;

mod confirmation;
mod editor;
mod input;
mod node;
mod reorder;
#[cfg(test)]
mod tests;

use ratatui::layout::{Constraint, Rect};

#[cfg(test)]
use super::PanelTitlePosition;
use super::data_view::{DataViewScrollSnapshot, ReorderSnapshot};
use super::{
    ActivationMode, Column, ConfirmationDialog, ConfirmationDialogKeyBindings, DataView, Dropdown,
    DropdownSearchMode, DropdownVariant, Panel, SelectionMode, SelectionTrigger, SortDirection,
    TextInput,
};
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusId, FocusRequest, HotkeyEvent, Key,
    KeyEvent, KeySpec, TreePath, TuiEvent,
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

type Creator<T> = dyn FnMut(Vec<String>, &[T]) -> T;
type RemoveFormatter<T> = dyn Fn(&T) -> String;
type EditGetter<T> = dyn Fn(&T) -> Vec<String>;
type EditMutator<T> = dyn Fn(&mut T, Vec<String>);

struct Editable<T> {
    getter: Box<EditGetter<T>>,
    mutator: Box<EditMutator<T>>,
}

struct RemoveConfirmation<T> {
    title: String,
    formatter: Box<RemoveFormatter<T>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListControlField {
    placeholder: String,
    kind: ListControlFieldKind,
    required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListControlFieldKind {
    Text,
    Dropdown(Vec<String>),
}

impl ListControlField {
    pub fn text(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            kind: ListControlFieldKind::Text,
            required: true,
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
            kind: ListControlFieldKind::Dropdown(options.into_iter().map(Into::into).collect()),
            required: true,
        }
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListControlEvent<Id> {
    Added {
        row_id: Id,
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
    ReorderCancelled {
        row_id: Id,
    },
    ReorderUnavailable {
        reason: ListControlReorderUnavailable,
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
    pub remove: Vec<KeySpec>,
    pub edit: Vec<KeySpec>,
    pub reorder: Vec<KeySpec>,
}

impl Default for ListControlKeyBindings {
    fn default() -> Self {
        Self {
            add: vec![KeySpec::plain('+')],
            remove: vec![KeySpec::plain('x')],
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
    moving_id: Id,
    pending_g: bool,
}

pub struct ListControl<T, Id, M = ()> {
    data_view: DataView<T, Id>,
    panel: Panel,
    panel_visible: bool,
    inputs: Vec<ListControlInput<M>>,
    required_fields: Vec<bool>,
    creator: Box<Creator<T>>,
    editable: Option<Editable<T>>,
    keys: ListControlKeyBindings,
    adding: bool,
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
    reorder: Option<ReorderState<Id>>,
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
                ListControlFieldKind::Dropdown(options) => {
                    options.iter().all(|option| !option.is_empty())
                }
            }),
            "ListControl dropdown option strings must be non-empty because \"\" represents no selection"
        );
        let inputs = fields
            .iter()
            .map(|field| match &field.kind {
                ListControlFieldKind::Text => {
                    ListControlInput::Text(TextInput::new().placeholder(field.placeholder.clone()))
                }
                ListControlFieldKind::Dropdown(options) => ListControlInput::Dropdown(Some({
                    let input = Dropdown::single(options.clone(), Clone::clone, Clone::clone)
                        .variant(DropdownVariant::Filled)
                        .search_mode(DropdownSearchMode::Fuzzy)
                        .placeholder(field.placeholder.clone());
                    if field.required {
                        input
                    } else {
                        input.no_selection_text("No selection")
                    }
                })),
            })
            .collect();
        let required_fields = fields.iter().map(|field| field.required).collect();
        Self {
            data_view: DataView::new(rows, row_id),
            panel: Panel::new(),
            panel_visible: true,
            inputs,
            required_fields,
            creator: Box::new(creator),
            editable: None,
            keys: ListControlKeyBindings::default(),
            adding: false,
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
            reorder: None,
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

    pub fn headers(mut self, headers: bool) -> Self {
        self.data_view = self.data_view.headers(headers);
        self
    }

    pub fn action_bar(mut self, action_bar: bool) -> Self {
        self.data_view = self.data_view.action_bar(action_bar);
        self
    }

    pub fn filter_controls(mut self, enabled: bool) -> Self {
        self.data_view = self.data_view.filter_controls(enabled);
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
        self.remove_confirmation = Some(RemoveConfirmation {
            title: title.into(),
            formatter: Box::new(formatter),
        });
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
        self
    }

    pub fn editable(
        mut self,
        getter: impl Fn(&T) -> Vec<String> + 'static,
        mutator: impl Fn(&mut T, Vec<String>) + 'static,
    ) -> Self {
        self.editable = Some(Editable {
            getter: Box::new(getter),
            mutator: Box::new(mutator),
        });
        self
    }

    pub fn items(&self) -> &[T] {
        self.data_view.rows()
    }

    pub fn data_view(&self) -> &DataView<T, Id> {
        &self.data_view
    }

    pub fn data_view_mut(&mut self) -> &mut DataView<T, Id> {
        &mut self.data_view
    }

    pub fn panel_ref(&self) -> &Panel {
        &self.panel
    }

    pub fn panel_mut(&mut self) -> &mut Panel {
        &mut self.panel
    }

    pub fn is_adding(&self) -> bool {
        self.adding
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub fn is_reordering(&self) -> bool {
        self.reorder.is_some()
    }

    pub fn take_events(&mut self) -> Vec<ListControlEvent<Id>> {
        std::mem::take(&mut self.events)
    }

    fn remove_highlighted(&mut self) -> bool {
        let Some(row_id) = self.data_view.highlighted_id() else {
            return false;
        };
        if self.data_view.remove_row(&row_id).is_none() {
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
        if self.editor_active() {
            if self.inputs[self.active_field].dropdown_is_open() {
                return None;
            } else if crate::keybindings().focus().unfocus_matches(key) {
                self.cancel_editor(true);
                self.restore_data_focus(route, ctx);
            } else if matches!(key.code, Key::Enter) {
                let final_field = self.active_field + 1 == self.inputs.len();
                if self.advance_field(route, ctx) {
                    if final_field {
                        self.restore_data_focus(route, ctx);
                    }
                }
            } else {
                return None;
            }
        } else if crate::keybindings().focus().next_control_matches(key) {
            ctx.focus_next_control();
        } else if crate::keybindings().focus().previous_control_matches(key) {
            ctx.focus_previous_control();
        } else if self.keys.add_matches(key) {
            self.begin_add();
            Self::focus_child(
                ctx,
                route,
                Self::input_slot(0).as_str(),
                self.inputs[0].focus_id(),
            );
            ctx.request_layout();
        } else if self.keys.edit_matches(key) && self.begin_edit() {
            Self::focus_child(
                ctx,
                route,
                Self::input_slot(0).as_str(),
                self.inputs[0].focus_id(),
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
