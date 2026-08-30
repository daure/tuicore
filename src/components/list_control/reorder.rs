use std::hash::Hash;

use super::{
    FlatBlockMoveState, FlatRangeSelectionState, ListControl, ListControlEvent,
    ListControlReorderUnavailable, ReorderState, TreeBlockMoveState, TreeReorderState,
    TreeSelectionState,
};
use crate::components::data_view::{ReorderUnavailableReason, SelectionOverlayPosition};
use crate::{EventCtx, EventOutcome, Key, KeyEvent, KeyModifiers, KeySpec};

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn handle_quick_tree_move(&mut self, key: KeyEvent) -> Option<bool> {
        let outdent = KeySpec::plain('<').matches(key);
        let indent = KeySpec::plain('>').matches(key);
        if !outdent && !indent {
            return None;
        }
        self.clear_tree_selection();
        if !self.data_view.tree_is_mutable() {
            return None;
        }
        if let Some(reason) = self.data_view.tree_edit_unavailable_reason() {
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: unavailable_reason(reason),
            });
            return Some(false);
        }
        let Some(moving_id) = self.data_view.highlighted_id() else {
            return Some(false);
        };
        let result = if outdent {
            self.data_view.outdent_tree_row(&moving_id)
        } else {
            self.data_view.indent_tree_row(&moving_id)
        };
        if let Some(result) = result {
            self.events.push(ListControlEvent::TreeMoved {
                row_id: moving_id,
                parent_id: result.parent_id,
                sibling_index: result.sibling_index,
            });
            Some(true)
        } else {
            Some(false)
        }
    }

    pub(super) fn handle_reorder_key(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if self.flat_block_move.is_some() {
            self.handle_flat_block_move_key(key, ctx);
            return Some(EventOutcome::Handled);
        }
        if self.tree_block_move.is_some() {
            self.handle_tree_block_move_key(key, ctx);
            return Some(EventOutcome::Handled);
        }
        if self.tree_reorder.is_some() {
            self.handle_tree_reorder_key(key, ctx);
            return Some(EventOutcome::Handled);
        }
        if self.reorder.is_none() {
            let reorder_matches = self.reorder_key_matches(key);
            let block_command = reorder_matches
                || matches!(key.code, Key::Char(' ')) && key.modifiers == KeyModifiers::NONE;
            let starts_flat_block = self
                .flat_range_selection
                .as_ref()
                .is_some_and(|state| state.selected.len() >= 2)
                && block_command;
            let consumes_flat_single = self
                .flat_range_selection
                .as_ref()
                .is_some_and(|state| state.selected.len() == 1)
                && block_command;
            let starts_block = self
                .tree_selection
                .as_ref()
                .is_some_and(|state| state.selected.len() >= 2)
                && block_command;
            let consumes_single = self
                .tree_selection
                .as_ref()
                .is_some_and(|state| state.selected.len() == 1)
                && block_command;
            if starts_flat_block {
                if self.begin_flat_block_move(ctx.animation()) {
                    ctx.request_layout();
                }
                ctx.request_redraw();
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            } else if consumes_flat_single {
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            } else if starts_block {
                if self.begin_tree_block_move(ctx.animation()) {
                    ctx.request_layout();
                }
                ctx.request_redraw();
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            } else if consumes_single {
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            } else if !reorder_matches {
                return None;
            }
            if self.data_view.tree_is_mutable() {
                self.begin_tree_reorder(ctx.animation());
            } else if self.reorder_column.is_some() {
                self.begin_reorder(ctx.animation());
            } else {
                return None;
            }
        } else if !self.reorder_is_compatible() {
            self.reject_changed_reorder(ctx.animation());
        } else {
            let top_prefix = self.top_prefix_matches(key);
            if !top_prefix {
                self.clear_pending_reorder_g();
            }
            if self.reorder_key_matches(key)
                || matches!(key.code, Key::Enter | Key::Char(' '))
                    && key.modifiers == KeyModifiers::NONE
            {
                self.commit_reorder(ctx.animation());
            } else if matches!(key.code, Key::Esc)
                || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
            {
                self.cancel_reorder(ctx.animation());
            } else if self.line_up_matches(key) {
                self.move_reorder_line(-1, ctx.animation());
            } else if self.line_down_matches(key) {
                self.move_reorder_line(1, ctx.animation());
            } else if self.page_up_matches(key) {
                let page = self.data_view.visible_page_step(self.data_area);
                self.move_reorder(-(page as isize), ctx.animation());
            } else if self.page_down_matches(key) {
                let page = self.data_view.visible_page_step(self.data_area);
                self.move_reorder(page as isize, ctx.animation());
            } else if self.top_matches(key) {
                self.move_reorder_to(0, ctx.animation());
            } else if self.bottom_matches(key) {
                self.move_reorder_to(usize::MAX, ctx.animation());
            } else if top_prefix {
                self.handle_reorder_g(ctx.animation());
            }
        }

        ctx.request_redraw();
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    pub(super) fn handle_tree_selection_key(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            if self.tree_selection.is_some() {
                self.clear_tree_selection();
                self.data_view
                    .reconcile_selection_to_highlight_on_navigate();
                ctx.request_redraw();
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
            return None;
        }
        let shift = key.modifiers == KeyModifiers::SHIFT;
        let control = key.modifiers == KeyModifiers::CONTROL;
        let direction = self.tree_selection_direction(key, shift || control);
        let range_extension = (shift || control) && direction.is_some();
        if self.tree_selection.is_some() && self.is_navigation_key(key) && !range_extension {
            self.clear_tree_selection();
            ctx.request_redraw();
            return None;
        }
        if !self.tree_selection_available() {
            return None;
        }
        if key.code == Key::Char(' ')
            && (key.modifiers == KeyModifiers::CONTROL
                || key.modifiers == KeyModifiers::NONE
                    && self
                        .tree_selection
                        .as_ref()
                        .is_some_and(|state| !state.range_mode))
        {
            self.toggle_tree_selection_at_highlight();
            ctx.request_redraw();
            ctx.stop_propagation();
            return Some(EventOutcome::Handled);
        }
        if let Some(direction) = direction {
            if shift || control {
                self.update_tree_selection(direction, shift, ctx.animation());
                ctx.request_redraw();
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
        }
        None
    }

    fn tree_selection_available(&self) -> bool {
        self.data_view.tree_is_mutable()
            && self.data_view.tree_edit_unavailable_reason().is_none()
            && !self.data_view.has_automatic_sort()
    }

    fn tree_selection_direction(&self, key: KeyEvent, modified: bool) -> Option<isize> {
        let mut key = key;
        if modified {
            key.modifiers = KeyModifiers::NONE;
            if let Key::Char(character) = key.code {
                key.code = Key::Char(character.to_ascii_lowercase());
            }
        }
        if self.line_up_matches(key) {
            Some(-1)
        } else if self.line_down_matches(key) {
            Some(1)
        } else {
            None
        }
    }

    fn update_tree_selection(
        &mut self,
        delta: isize,
        range: bool,
        mut settings: crate::AnimationSettings,
    ) {
        let Some(current) = self.data_view.highlighted_id() else {
            return;
        };
        let Some((parent_id, siblings)) = self.data_view.tree_siblings(&current) else {
            return;
        };
        let current_index = siblings.iter().position(|id| id == &current).unwrap_or(0);
        let destination_index = current_index
            .saturating_add_signed(delta)
            .min(siblings.len().saturating_sub(1));
        let destination = siblings[destination_index].clone();
        if self
            .tree_selection
            .as_ref()
            .is_some_and(|state| state.range_mode != range)
        {
            self.clear_tree_selection();
        }
        let state = self
            .tree_selection
            .get_or_insert_with(|| TreeSelectionState {
                selected: Vec::new(),
                anchor: None,
                range_mode: range,
            });
        if state
            .selected
            .first()
            .and_then(|id| self.data_view.tree_parent_id(id))
            .as_ref()
            != Some(&parent_id)
        {
            state.selected.clear();
            state.anchor = None;
        }
        if range {
            let anchor = state.anchor.get_or_insert(current.clone()).clone();
            let anchor_index = siblings
                .iter()
                .position(|id| id == &anchor)
                .unwrap_or(current_index);
            let (start, end) = if anchor_index <= destination_index {
                (anchor_index, destination_index)
            } else {
                (destination_index, anchor_index)
            };
            state.selected = siblings[start..=end].to_vec();
            state.range_mode = true;
        } else {
            if state.selected.is_empty() {
                state.selected.push(current.clone());
            }
            state.anchor = Some(current.clone());
            state.range_mode = false;
        }
        state
            .selected
            .sort_by_key(|id| siblings.iter().position(|candidate| candidate == id));
        self.data_view
            .set_selection_overlay(state.selected.clone(), None, 0, false);
        self.data_view.reposition_highlight_silently(&destination);
        settings.enabled = false;
        self.data_view
            .ensure_highlight_visible(self.data_area, settings);
    }

    fn toggle_tree_selection_at_highlight(&mut self) {
        let Some(current) = self.data_view.highlighted_id() else {
            return;
        };
        let Some((parent_id, siblings)) = self.data_view.tree_siblings(&current) else {
            return;
        };
        let selected = {
            let state = self
                .tree_selection
                .get_or_insert_with(|| TreeSelectionState {
                    selected: Vec::new(),
                    anchor: None,
                    range_mode: false,
                });
            state.anchor = Some(current.clone());
            state.range_mode = false;
            if state
                .selected
                .first()
                .and_then(|id| self.data_view.tree_parent_id(id))
                .as_ref()
                != Some(&parent_id)
            {
                state.selected.clear();
            }
            if let Some(index) = state.selected.iter().position(|id| id == &current) {
                state.selected.remove(index);
            } else {
                state.selected.push(current);
            }
            state
                .selected
                .sort_by_key(|id| siblings.iter().position(|candidate| candidate == id));
            state.selected.clone()
        };
        if selected.is_empty() {
            self.clear_tree_selection();
        } else {
            self.data_view
                .set_selection_overlay(selected, None, 0, false);
        }
    }

    pub(super) fn clear_tree_selection(&mut self) {
        self.tree_selection = None;
        self.data_view.clear_selection_overlay();
    }

    pub(super) fn handle_flat_range_selection_key(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            if self.flat_range_selection.is_some() {
                self.clear_flat_range_selection();
                self.data_view
                    .reconcile_selection_to_highlight_on_navigate();
                ctx.request_redraw();
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
            return None;
        }
        let shift = key.modifiers == KeyModifiers::SHIFT;
        let control = key.modifiers == KeyModifiers::CONTROL;
        let direction = self.tree_selection_direction(key, shift || control);
        let range_extension = shift && direction.is_some();
        if self.flat_range_selection.is_some() && self.is_navigation_key(key) && !range_extension {
            self.clear_flat_range_selection();
            ctx.request_redraw();
            return None;
        }
        if !self.flat_range_selection_available() {
            return None;
        }
        if key.code == Key::Char(' ')
            && (control
                || key.modifiers == KeyModifiers::NONE
                    && self
                        .flat_range_selection
                        .as_ref()
                        .is_some_and(|state| !state.range_mode))
        {
            self.toggle_flat_selection_at_highlight();
            ctx.request_redraw();
            ctx.stop_propagation();
            return Some(EventOutcome::Handled);
        }
        if !shift && !control {
            return None;
        }
        let Some(delta) = direction else {
            return None;
        };
        let Some(current) = self.data_view.highlighted_id() else {
            return None;
        };
        let ids = self.flat_scope_ids(&current, self.data_view.reorder_visible_ids());
        if self
            .flat_range_selection
            .as_ref()
            .is_some_and(|state| state.selected.iter().any(|id| !ids.contains(id)))
        {
            self.clear_flat_range_selection();
        }
        let Some(current_index) = ids.iter().position(|id| id == &current) else {
            return None;
        };
        let destination_index = current_index
            .saturating_add_signed(delta)
            .min(ids.len().saturating_sub(1));
        let destination = ids[destination_index].clone();
        let state = self
            .flat_range_selection
            .get_or_insert_with(|| FlatRangeSelectionState {
                selected: Vec::new(),
                anchor: current.clone(),
                range_mode: shift,
            });
        if shift {
            state.extend_range(&ids, &current, &destination);
        } else {
            state.move_with_control(current.clone());
        }
        self.data_view
            .set_selection_overlay(state.selected.clone(), None, 0, false);
        self.data_view.reposition_highlight_silently(&destination);
        let mut settings = ctx.animation();
        settings.enabled = false;
        self.data_view.center_highlight(self.data_area, settings);
        ctx.request_redraw();
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    fn toggle_flat_selection_at_highlight(&mut self) {
        let Some(current) = self.data_view.highlighted_id() else {
            return;
        };
        let ids = self.flat_scope_ids(&current, self.data_view.reorder_visible_ids());
        if !ids.contains(&current) {
            return;
        }
        let selected = {
            let state = self
                .flat_range_selection
                .get_or_insert_with(|| FlatRangeSelectionState {
                    selected: Vec::new(),
                    anchor: current.clone(),
                    range_mode: false,
                });
            if state.selected.iter().any(|id| !ids.contains(id)) {
                state.selected.clear();
                state.anchor = current.clone();
            }
            state.toggle(&ids, current)
        };
        if selected.is_empty() {
            self.clear_flat_range_selection();
        } else {
            self.data_view
                .set_selection_overlay(selected, None, 0, false);
        }
    }

    pub(super) fn clear_flat_range_selection(&mut self) {
        self.flat_range_selection = None;
        self.data_view.clear_selection_overlay();
    }

    fn flat_range_selection_available(&self) -> bool {
        !self.data_view.tree_is_mutable() && self.reorder_column.is_some() && self.reorder.is_none()
    }

    fn flat_block_move_available(&self) -> bool {
        self.flat_range_selection_available()
            && self.reorder_column.as_deref().is_some_and(|column| {
                self.data_view
                    .reorder_snapshot(column)
                    .is_ok_and(|snapshot| snapshot.ids == self.data_view.reorder_visible_ids())
            })
    }

    fn is_navigation_key(&self, key: KeyEvent) -> bool {
        self.display_action(key)
            .is_some_and(|action| !matches!(action, super::DataViewDisplayAction::Activate))
            || self.data_view.is_navigation_key(key)
    }

    fn line_up_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::LineUp)
            || !self.display_uses_custom_bindings() && crate::keybindings().line_up_matches(key)
    }

    fn line_down_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::LineDown)
            || !self.display_uses_custom_bindings() && crate::keybindings().line_down_matches(key)
    }

    fn page_up_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::PageUp)
            || !self.display_uses_custom_bindings() && crate::keybindings().page_up_matches(key)
    }

    fn page_down_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::PageDown)
            || !self.display_uses_custom_bindings() && crate::keybindings().page_down_matches(key)
    }

    fn top_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::Top)
            || !self.display_uses_custom_bindings() && crate::keybindings().home_matches(key)
    }

    fn bottom_matches(&self, key: KeyEvent) -> bool {
        self.display_action(key) == Some(super::DataViewDisplayAction::Bottom)
            || !self.display_uses_custom_bindings()
                && (crate::keybindings().end_matches(key)
                    || crate::keybindings().data_view().bottom_matches(key))
    }

    fn top_prefix_matches(&self, key: KeyEvent) -> bool {
        if self.display_uses_custom_bindings() {
            self.display_top_prefix_matches(key)
        } else {
            crate::keybindings().data_view().top_prefix_matches(key)
        }
    }

    pub(super) fn flat_scope_ids(&self, anchor_id: &Id, ids: Vec<Id>) -> Vec<Id> {
        let Some(same_scope) = self.reorder_scope.as_ref() else {
            return ids;
        };
        let Some(anchor) = self
            .data_view
            .rows()
            .iter()
            .find(|row| self.data_view.row_id(row) == *anchor_id)
        else {
            return Vec::new();
        };
        ids.into_iter()
            .filter(|id| {
                self.data_view
                    .rows()
                    .iter()
                    .find(|row| self.data_view.row_id(row) == *id)
                    .is_some_and(|row| same_scope(anchor, row))
            })
            .collect()
    }

    fn scoped_snapshot_ids(
        &self,
        snapshot: &super::ReorderSnapshot<Id>,
        anchor_id: &Id,
    ) -> Option<Vec<Id>> {
        self.reorder_scope.as_ref().map_or_else(
            || Some(snapshot.ids.clone()),
            |same_scope| {
                self.data_view
                    .reorder_scoped_ids(snapshot, anchor_id, same_scope.as_ref())
            },
        )
    }

    fn scope_is_compatible(
        &self,
        snapshot: &super::ReorderSnapshot<Id>,
        anchor_id: &Id,
        scope_ids: &Option<Vec<Id>>,
    ) -> bool {
        self.reorder_scope.is_none()
            || self.scoped_snapshot_ids(snapshot, anchor_id).as_ref() == scope_ids.as_ref()
    }

    fn begin_flat_block_move(&mut self, settings: crate::AnimationSettings) -> bool {
        if !self.flat_block_move_available() {
            self.clear_flat_range_selection();
            return false;
        }
        let Some(selection) = self.flat_range_selection.as_ref() else {
            return false;
        };
        let column = self
            .reorder_column
            .as_deref()
            .expect("reorder column is present");
        let Ok(snapshot) = self.data_view.reorder_snapshot(column) else {
            self.clear_flat_range_selection();
            return false;
        };
        let Some(highlighted_id) = self.data_view.highlighted_id() else {
            self.clear_flat_range_selection();
            return false;
        };
        let Some(scope_ids) = self.scoped_snapshot_ids(&snapshot, &highlighted_id) else {
            self.clear_flat_range_selection();
            return false;
        };
        let selected = scope_ids
            .iter()
            .filter(|id| selection.selected.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        if selected.len() < 2 || selected.len() != selection.selected.len() {
            self.clear_flat_range_selection();
            return false;
        }
        let placement_id = if selection.range_mode {
            highlighted_id.clone()
        } else {
            selection.anchor.clone()
        };
        let placement_index = snapshot
            .ids
            .iter()
            .position(|id| id == &placement_id)
            .expect("selection placement row exists in reorder snapshot");
        let placement_is_first_selected = selected.first() == Some(&placement_id);
        let visual_target_index = placement_index + usize::from(!placement_is_first_selected);
        let target_index = Self::flat_block_scoped_target_index(
            &snapshot.ids,
            &scope_ids,
            &selected,
            visual_target_index,
        );
        self.data_view.set_selection_overlay(
            selected.clone(),
            Some(if placement_is_first_selected {
                SelectionOverlayPosition::Before(placement_id)
            } else {
                SelectionOverlayPosition::After(placement_id)
            }),
            0,
            true,
        );
        let selected_highlight_id = selected
            .last()
            .expect("selected block is not empty")
            .clone();
        self.flat_block_move = Some(FlatBlockMoveState {
            snapshot,
            scroll_snapshot: self.data_view.scroll_snapshot(),
            selected,
            scope_ids: self.reorder_scope.as_ref().map(|_| scope_ids),
            target_index,
            visual_target_index: Some(visual_target_index),
            highlighted_id,
            pending_g: false,
        });
        self.data_view
            .reposition_highlight_silently(&selected_highlight_id);
        self.position_flat_block_placeholder(settings);
        true
    }

    fn handle_flat_block_move_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) {
        if !self.flat_block_move_is_compatible() {
            self.reject_changed_flat_block_move(ctx.animation());
        } else if key.is_repeat() {
            ctx.stop_propagation();
            return;
        } else if self.reorder_key_matches(key)
            || key.code == Key::Enter && key.modifiers == KeyModifiers::NONE
        {
            self.commit_flat_block_move(ctx.animation());
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_flat_block_move(ctx.animation());
        } else if self.handle_flat_block_target_key(key) {
            self.update_flat_block_overlay();
            self.position_flat_block_placeholder(ctx.animation());
        }
        ctx.request_layout();
        ctx.request_redraw();
        ctx.stop_propagation();
    }

    fn flat_block_move_is_compatible(&self) -> bool {
        let Some(state) = self.flat_block_move.as_ref() else {
            return true;
        };
        self.reorder_column.as_deref().is_some_and(|column| {
            self.data_view
                .reorder_snapshot_matches(column, &state.snapshot)
                && self.data_view.reorder_visible_ids() == state.snapshot.ids
                && self.scope_is_compatible(
                    &state.snapshot,
                    state.selected.first().expect("selected block is not empty"),
                    &state.scope_ids,
                )
        })
    }

    fn flat_block_remaining_ids(&self, state: &FlatBlockMoveState<Id>) -> Vec<Id> {
        state
            .scope_ids
            .as_ref()
            .unwrap_or(&state.snapshot.ids)
            .iter()
            .filter(|id| !state.selected.contains(id))
            .cloned()
            .collect()
    }

    fn move_flat_block_target(&mut self, delta: isize) {
        let target_index = self
            .flat_block_move
            .as_ref()
            .map_or(0, |state| state.target_index.saturating_add_signed(delta));
        self.set_flat_block_target(target_index);
    }

    fn move_flat_block_target_line(&mut self, delta: isize) {
        let Some(state) = self.flat_block_move.as_ref() else {
            return;
        };
        let visual_target_index = move_block_visual_boundary(
            &state.snapshot.ids,
            &state.selected,
            Self::flat_block_visual_target_index(state),
            delta,
        );
        let target_index = Self::flat_block_scoped_target_index(
            &state.snapshot.ids,
            state.scope_ids.as_ref().unwrap_or(&state.snapshot.ids),
            &state.selected,
            visual_target_index,
        );
        let state = self
            .flat_block_move
            .as_mut()
            .expect("flat block move is active");
        state.visual_target_index = Some(visual_target_index);
        state.target_index = target_index;
    }

    fn flat_block_visual_target_index(state: &FlatBlockMoveState<Id>) -> usize {
        state.visual_target_index.unwrap_or_else(|| {
            state
                .scope_ids
                .as_ref()
                .unwrap_or(&state.snapshot.ids)
                .iter()
                .filter(|id| !state.selected.contains(id))
                .nth(state.target_index)
                .and_then(|target| state.snapshot.ids.iter().position(|id| id == target))
                .unwrap_or(state.snapshot.ids.len())
        })
    }

    fn flat_block_scoped_target_index(
        full_ids: &[Id],
        scope_ids: &[Id],
        selected: &[Id],
        visual_target_index: usize,
    ) -> usize {
        full_ids[..visual_target_index.min(full_ids.len())]
            .iter()
            .filter(|id| scope_ids.contains(id) && !selected.contains(id))
            .count()
    }

    fn set_flat_block_target(&mut self, target_index: usize) {
        let remaining = self
            .flat_block_move
            .as_ref()
            .map(|state| self.flat_block_remaining_ids(state).len())
            .unwrap_or(0);
        let state = self
            .flat_block_move
            .as_mut()
            .expect("flat block move is active");
        state.target_index = target_index.min(remaining);
        state.visual_target_index = None;
    }

    fn handle_flat_block_target_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers == KeyModifiers::NONE && self.line_up_matches(key) {
            self.move_flat_block_target_line(-1);
            return true;
        }
        if key.modifiers == KeyModifiers::NONE && self.line_down_matches(key) {
            self.move_flat_block_target_line(1);
            return true;
        }
        let top_prefix = self.top_prefix_matches(key);
        if !top_prefix {
            self.flat_block_move
                .as_mut()
                .expect("flat block move is active")
                .pending_g = false;
        }
        if self.page_up_matches(key) {
            self.move_flat_block_target(
                -(self.data_view.visible_page_step(self.data_area) as isize),
            );
        } else if self.page_down_matches(key) {
            self.move_flat_block_target(self.data_view.visible_page_step(self.data_area) as isize);
        } else if self.top_matches(key) {
            self.set_flat_block_target(0);
        } else if self.bottom_matches(key) {
            self.set_flat_block_target(usize::MAX);
        } else if top_prefix {
            let move_to_top = {
                let state = self
                    .flat_block_move
                    .as_mut()
                    .expect("flat block move is active");
                if state.pending_g {
                    state.pending_g = false;
                    true
                } else {
                    state.pending_g = true;
                    false
                }
            };
            if move_to_top {
                self.set_flat_block_target(0);
            }
        } else {
            return false;
        }
        true
    }

    fn update_flat_block_overlay(&mut self) {
        let Some(state) = self.flat_block_move.as_ref() else {
            return;
        };
        let remaining = self.flat_block_remaining_ids(state);
        let position = state
            .visual_target_index
            .and_then(|index| state.snapshot.ids.get(index))
            .cloned()
            .map(SelectionOverlayPosition::Before)
            .or_else(|| {
                remaining
                    .get(state.target_index)
                    .cloned()
                    .map(SelectionOverlayPosition::Before)
                    .or_else(|| {
                        remaining
                            .last()
                            .cloned()
                            .map(SelectionOverlayPosition::After)
                    })
            })
            .unwrap_or_else(|| {
                SelectionOverlayPosition::After(
                    state
                        .selected
                        .last()
                        .expect("selected block is not empty")
                        .clone(),
                )
            });
        self.data_view
            .set_selection_overlay(state.selected.clone(), Some(position), 0, true);
    }

    fn position_flat_block_placeholder(&mut self, mut settings: crate::AnimationSettings) {
        settings.enabled = false;
        self.data_view
            .center_selection_placeholder(self.data_area, settings);
    }

    fn flat_block_staged_ids(&self, state: &FlatBlockMoveState<Id>) -> Vec<Id> {
        let mut staged = self.flat_block_remaining_ids(state);
        staged.splice(
            state.target_index..state.target_index,
            state.selected.clone(),
        );
        staged
    }

    fn commit_flat_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.flat_block_move.take() else {
            return;
        };
        self.clear_flat_range_selection();
        let scoped_staged = self.flat_block_staged_ids(&state);
        let staged = Self::merge_scope_order(&state.snapshot.ids, &state.scope_ids, &scoped_staged);
        let column = self
            .reorder_column
            .as_deref()
            .expect("reorder is configured");
        if self
            .data_view
            .commit_reorder(column, &staged, &state.snapshot)
        {
            self.data_view
                .reposition_highlight_silently(&state.highlighted_id);
            self.data_view
                .ensure_highlight_visible(self.data_area, settings);
            self.events.push(ListControlEvent::Reordered {
                row_ids: scoped_staged,
            });
        } else {
            self.data_view
                .reposition_highlight_silently(&state.highlighted_id);
            self.data_view
                .restore_scroll(state.scroll_snapshot, self.data_area, settings);
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: ListControlReorderUnavailable::DataChanged,
            });
        }
    }

    pub(super) fn cancel_flat_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.flat_block_move.take() else {
            return;
        };
        self.clear_flat_range_selection();
        self.data_view
            .reposition_highlight_silently(&state.highlighted_id);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderCancelled {
            row_id: state.highlighted_id,
        });
    }

    fn reject_changed_flat_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.flat_block_move.take() else {
            return;
        };
        self.clear_flat_range_selection();
        self.data_view
            .reposition_highlight_silently(&state.highlighted_id);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        });
    }

    fn begin_tree_block_move(&mut self, settings: crate::AnimationSettings) -> bool {
        if !self.tree_selection_available() {
            self.clear_tree_selection();
            return false;
        }
        let Some(selection) = self.tree_selection.as_ref() else {
            return false;
        };
        let Some(first) = selection.selected.first() else {
            return false;
        };
        let Some((parent_id, siblings)) = self.data_view.tree_siblings(first) else {
            return false;
        };
        let selected = siblings
            .iter()
            .filter(|id| selection.selected.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        if selected.len() < 2 || selected.len() != selection.selected.len() {
            return false;
        }
        let Some(snapshot) = self.data_view.tree_edit_snapshot() else {
            return false;
        };
        let placement_id = if selection.range_mode {
            self.data_view
                .highlighted_id()
                .expect("range selection has a highlighted sibling")
        } else {
            selection
                .anchor
                .clone()
                .unwrap_or_else(|| selected.last().expect("selected sibling exists").clone())
        };
        let placement_index = siblings
            .iter()
            .position(|id| id == &placement_id)
            .expect("selection placement sibling exists");
        let placement_is_first_selected = selected.first() == Some(&placement_id);
        let visual_sibling_index = placement_index + usize::from(!placement_is_first_selected);
        let sibling_index = siblings[..visual_sibling_index]
            .iter()
            .filter(|id| !selected.contains(id))
            .count();
        self.data_view.set_selection_overlay(
            selected.clone(),
            Some(if placement_is_first_selected {
                SelectionOverlayPosition::Before(placement_id)
            } else {
                SelectionOverlayPosition::After(placement_id)
            }),
            self.tree_block_target_depth(parent_id.as_ref()),
            true,
        );
        let selected_highlight_id = selected
            .last()
            .expect("selected block is not empty")
            .clone();
        self.tree_block_move = Some(TreeBlockMoveState {
            snapshot,
            scroll_snapshot: self.data_view.scroll_snapshot(),
            expanded_before: self.data_view.tree_expansion_snapshot(),
            source_parent_id: parent_id.clone(),
            parent_id,
            selected,
            sibling_index,
            visual_sibling_index: Some(visual_sibling_index),
            pending_g: false,
        });
        self.data_view
            .reposition_highlight_silently(&selected_highlight_id);
        self.position_tree_block_placeholder(settings);
        true
    }

    fn handle_tree_block_move_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) {
        if !self.tree_block_move_is_compatible() {
            self.reject_changed_tree_block_move(ctx.animation());
        } else if key.is_repeat() {
            ctx.stop_propagation();
            return;
        } else if self.reorder_key_matches(key)
            || key.code == Key::Enter && key.modifiers == KeyModifiers::NONE
        {
            self.commit_tree_block_move(ctx.animation());
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_tree_block_move(ctx.animation());
        } else if self.handle_tree_block_target_key(key) {
            self.update_tree_block_overlay();
            self.position_tree_block_placeholder(ctx.animation());
            ctx.request_redraw();
            ctx.stop_propagation();
            return;
        } else {
            let keys = crate::keybindings();
            let horizontal_move = keys.line_left_matches(key)
                || KeySpec::plain('<').matches(key)
                || keys.line_right_matches(key)
                || KeySpec::plain('>').matches(key);
            if !self.allow_horizontal_moving && horizontal_move {
                ctx.stop_propagation();
                return;
            }
            if keys.line_left_matches(key) || KeySpec::plain('<').matches(key) {
                self.outdent_tree_block_target();
            } else if keys.line_right_matches(key) || KeySpec::plain('>').matches(key) {
                self.indent_tree_block_target();
            } else {
                ctx.request_layout();
                ctx.request_redraw();
                ctx.stop_propagation();
                return;
            }
            self.update_tree_block_overlay();
            self.position_tree_block_placeholder(ctx.animation());
        }
        ctx.request_layout();
        ctx.request_redraw();
        ctx.stop_propagation();
    }

    fn tree_block_move_is_compatible(&self) -> bool {
        self.tree_block_move
            .as_ref()
            .is_none_or(|state| self.data_view.tree_edit_snapshot_matches(&state.snapshot))
    }

    fn move_tree_block_target(&mut self, delta: isize) {
        let sibling_index = self
            .tree_block_move
            .as_ref()
            .map_or(0, |state| state.sibling_index.saturating_add_signed(delta));
        self.set_tree_block_target(sibling_index);
    }

    fn move_tree_block_target_line(&mut self, delta: isize) {
        let Some(state) = self.tree_block_move.as_ref() else {
            return;
        };
        if state.parent_id != state.source_parent_id {
            let sibling_index = state
                .sibling_index
                .saturating_add_signed(delta)
                .min(self.tree_block_target_siblings(state).len());
            let state = self
                .tree_block_move
                .as_mut()
                .expect("tree block move is active");
            state.sibling_index = sibling_index;
            return;
        }
        let siblings = self
            .data_view
            .tree_children_for_parent(state.source_parent_id.as_ref());
        let visual_sibling_index = state.visual_sibling_index.unwrap_or_else(|| {
            self.tree_block_target_siblings(state)
                .get(state.sibling_index)
                .and_then(|target| siblings.iter().position(|id| id == target))
                .unwrap_or(siblings.len())
        });
        let visual_sibling_index =
            move_block_visual_boundary(&siblings, &state.selected, visual_sibling_index, delta);
        let sibling_index = siblings[..visual_sibling_index]
            .iter()
            .filter(|id| !state.selected.contains(id))
            .count();
        let state = self
            .tree_block_move
            .as_mut()
            .expect("tree block move is active");
        state.visual_sibling_index = Some(visual_sibling_index);
        state.sibling_index = sibling_index;
    }

    fn set_tree_block_target(&mut self, sibling_index: usize) {
        let remaining = self
            .tree_block_move
            .as_ref()
            .map(|state| self.tree_block_target_siblings(state).len())
            .unwrap_or(0);
        let state = self
            .tree_block_move
            .as_mut()
            .expect("tree block move is active");
        state.sibling_index = sibling_index.min(remaining);
        state.visual_sibling_index = None;
    }

    fn handle_tree_block_target_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers == KeyModifiers::NONE {
            match key.code {
                Key::Char('k') | Key::Up => {
                    self.move_tree_block_target_line(-1);
                    return true;
                }
                Key::Char('j') | Key::Down => {
                    self.move_tree_block_target_line(1);
                    return true;
                }
                _ => {}
            }
        }
        let keys = crate::keybindings();
        let top_prefix = keys.data_view().top_prefix_matches(key);
        if !top_prefix {
            self.tree_block_move
                .as_mut()
                .expect("tree block move is active")
                .pending_g = false;
        }
        if keys.page_up_matches(key) {
            self.move_tree_block_target(
                -(self.data_view.visible_page_step(self.data_area) as isize),
            );
        } else if keys.page_down_matches(key) {
            self.move_tree_block_target(self.data_view.visible_page_step(self.data_area) as isize);
        } else if keys.home_matches(key) {
            self.set_tree_block_target(0);
        } else if keys.end_matches(key) || keys.data_view().bottom_matches(key) {
            self.set_tree_block_target(usize::MAX);
        } else if top_prefix {
            let move_to_top = {
                let state = self
                    .tree_block_move
                    .as_mut()
                    .expect("tree block move is active");
                if state.pending_g {
                    state.pending_g = false;
                    true
                } else {
                    state.pending_g = true;
                    false
                }
            };
            if move_to_top {
                self.set_tree_block_target(0);
            }
        } else {
            return false;
        }
        true
    }

    fn outdent_tree_block_target(&mut self) {
        let Some(state) = self.tree_block_move.as_mut() else {
            return;
        };
        let Some(parent_id) = state.parent_id.clone() else {
            return;
        };
        let Some(grandparent_id) = self.data_view.tree_parent_id(&parent_id) else {
            return;
        };
        let Some(parent_index) = self
            .data_view
            .tree_siblings(&parent_id)
            .and_then(|(_, siblings)| siblings.iter().position(|id| id == &parent_id))
        else {
            return;
        };
        state.parent_id = grandparent_id;
        state.sibling_index = parent_index + 1;
        state.visual_sibling_index = None;
    }

    fn indent_tree_block_target(&mut self) {
        let Some(state) = self.tree_block_move.as_ref() else {
            return;
        };
        let Some(new_parent) = state
            .sibling_index
            .checked_sub(1)
            .and_then(|index| self.tree_block_target_siblings(state).get(index).cloned())
        else {
            return;
        };
        let sibling_index = self
            .data_view
            .tree_children_for_parent(Some(&new_parent))
            .len();
        self.data_view.expand_tree_row(new_parent.clone());
        let state = self
            .tree_block_move
            .as_mut()
            .expect("tree block move is active");
        state.parent_id = Some(new_parent);
        state.sibling_index = sibling_index;
        state.visual_sibling_index = None;
    }

    fn tree_block_target_siblings(&self, state: &TreeBlockMoveState<Id>) -> Vec<Id> {
        self.data_view
            .tree_children_for_parent(state.parent_id.as_ref())
            .into_iter()
            .filter(|id| !state.selected.contains(id))
            .collect()
    }

    fn tree_block_target_depth(&self, parent_id: Option<&Id>) -> usize {
        parent_id
            .and_then(|id| self.data_view.tree_depth(id))
            .map_or(0, |depth| depth + 1)
    }

    fn update_tree_block_overlay(&mut self) {
        let Some(state) = self.tree_block_move.as_ref() else {
            return;
        };
        let remaining = self.tree_block_target_siblings(state);
        let position = state
            .visual_sibling_index
            .filter(|_| state.parent_id == state.source_parent_id)
            .and_then(|index| {
                self.data_view
                    .tree_children_for_parent(state.source_parent_id.as_ref())
                    .get(index)
                    .cloned()
            })
            .map(SelectionOverlayPosition::Before)
            .or_else(|| {
                remaining
                    .get(state.sibling_index)
                    .cloned()
                    .map(SelectionOverlayPosition::Before)
                    .or_else(|| {
                        remaining
                            .last()
                            .cloned()
                            .map(SelectionOverlayPosition::After)
                    })
                    .or_else(|| {
                        (state.parent_id != state.source_parent_id)
                            .then(|| state.parent_id.clone())
                            .flatten()
                            .map(SelectionOverlayPosition::After)
                    })
            })
            .unwrap_or_else(|| {
                SelectionOverlayPosition::After(
                    state
                        .selected
                        .last()
                        .expect("selected block is not empty")
                        .clone(),
                )
            });
        self.data_view.set_selection_overlay(
            state.selected.clone(),
            Some(position),
            self.tree_block_target_depth(state.parent_id.as_ref()),
            true,
        );
    }

    fn position_tree_block_placeholder(&mut self, mut settings: crate::AnimationSettings) {
        settings.enabled = false;
        self.data_view
            .center_selection_placeholder(self.data_area, settings);
    }

    fn commit_tree_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.tree_block_move.take() else {
            return;
        };
        self.clear_tree_selection();
        if let Some(result) = self.data_view.move_tree_sibling_block(
            &state.selected,
            state.source_parent_id,
            state.parent_id.clone(),
            state.sibling_index,
        ) {
            self.events.push(ListControlEvent::TreeBlockMoved {
                row_ids: state.selected,
                parent_id: result.parent_id,
                sibling_index: result.sibling_index,
            });
        } else {
            self.data_view
                .restore_scroll(state.scroll_snapshot, self.data_area, settings);
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: ListControlReorderUnavailable::DataChanged,
            });
        }
    }

    pub(super) fn cancel_tree_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.tree_block_move.take() else {
            return;
        };
        self.clear_tree_selection();
        self.data_view.restore_tree_expansion(state.expanded_before);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::TreeBlockMoveCancelled {
            row_ids: state.selected,
        });
    }

    fn reject_changed_tree_block_move(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.tree_block_move.take() else {
            return;
        };
        self.clear_tree_selection();
        self.data_view.restore_tree_expansion(state.expanded_before);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        });
    }

    fn handle_tree_reorder_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) {
        let keys = crate::keybindings();
        if !self.tree_reorder_is_compatible() {
            self.reject_changed_tree_reorder(ctx.animation());
        } else if self.reorder_key_matches(key)
            || key.code == Key::Enter && key.modifiers == KeyModifiers::NONE
        {
            self.commit_tree_reorder(ctx.animation());
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_tree_reorder(ctx.animation(), false);
        } else {
            let horizontal_move = keys.line_left_matches(key)
                || KeySpec::plain('<').matches(key)
                || keys.line_right_matches(key)
                || KeySpec::plain('>').matches(key);
            if !self.allow_horizontal_moving && horizontal_move {
                ctx.stop_propagation();
                return;
            }
            let moving_id = self
                .tree_reorder
                .as_ref()
                .expect("tree reorder is active")
                .moving_id
                .clone();
            let moved = if keys.line_up_matches(key) {
                self.data_view.move_tree_sibling(&moving_id, -1)
            } else if keys.line_down_matches(key) {
                self.data_view.move_tree_sibling(&moving_id, 1)
            } else if keys.line_left_matches(key) || KeySpec::plain('<').matches(key) {
                self.data_view.outdent_tree_row(&moving_id)
            } else if keys.line_right_matches(key) || KeySpec::plain('>').matches(key) {
                self.data_view.indent_tree_row(&moving_id)
            } else {
                None
            };
            if moved.is_some() {
                let staged_snapshot = self
                    .data_view
                    .tree_edit_snapshot()
                    .expect("mutable tree has a parent snapshot");
                let state = self.tree_reorder.as_mut().expect("tree reorder is active");
                state.staged_snapshot = staged_snapshot;
                state.changed = true;
                let mut settings = ctx.animation();
                settings.enabled = false;
                self.data_view.center_highlight(self.data_area, settings);
            }
        }
        ctx.request_redraw();
        ctx.stop_propagation();
    }

    fn begin_tree_reorder(&mut self, settings: crate::AnimationSettings) {
        if let Some(reason) = self.data_view.tree_edit_unavailable_reason() {
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: unavailable_reason(reason),
            });
            return;
        }
        let Some(snapshot) = self.data_view.tree_edit_snapshot() else {
            return;
        };
        let Some(moving_id) = self.data_view.highlighted_id() else {
            return;
        };
        let scroll_snapshot = self.data_view.scroll_snapshot();
        self.data_view
            .start_reorder_highlight(moving_id.clone(), settings);
        self.tree_reorder = Some(TreeReorderState {
            staged_snapshot: snapshot.clone(),
            snapshot,
            scroll_snapshot,
            moving_id,
            changed: false,
        });
    }

    fn tree_reorder_is_compatible(&self) -> bool {
        self.tree_reorder.as_ref().is_none_or(|state| {
            self.data_view
                .tree_edit_snapshot_matches(&state.staged_snapshot)
        })
    }

    fn commit_tree_reorder(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.tree_reorder.take() else {
            return;
        };
        self.data_view.clear_reorder_highlight(settings);
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        if state.changed
            && let Some(result) = self.data_view.tree_move_result(&state.moving_id)
        {
            self.events.push(ListControlEvent::TreeMoved {
                row_id: state.moving_id,
                parent_id: result.parent_id,
                sibling_index: result.sibling_index,
            });
        }
    }

    fn cancel_tree_reorder(
        &mut self,
        settings: crate::AnimationSettings,
        clear_highlight_immediately: bool,
    ) {
        let Some(state) = self.tree_reorder.take() else {
            return;
        };
        if clear_highlight_immediately {
            self.data_view.clear_reorder_highlight_immediately();
        } else {
            self.data_view.clear_reorder_highlight(settings);
        }
        self.data_view
            .restore_tree_edit_after_conflict(&state.snapshot, &state.staged_snapshot);
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderCancelled {
            row_id: state.moving_id,
        });
    }

    fn reject_changed_tree_reorder(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.tree_reorder.take() else {
            return;
        };
        self.data_view.clear_reorder_highlight(settings);
        self.data_view
            .restore_tree_edit_after_conflict(&state.snapshot, &state.staged_snapshot);
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.events.push(ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        });
    }

    fn begin_reorder(&mut self, settings: crate::AnimationSettings) {
        let column = self
            .reorder_column
            .as_deref()
            .expect("reorder is configured");
        let snapshot = match self.data_view.reorder_snapshot(column) {
            Ok(snapshot) => snapshot,
            Err(reason) => {
                self.events.push(ListControlEvent::ReorderUnavailable {
                    reason: unavailable_reason(reason),
                });
                return;
            }
        };
        let Some(moving_id) = self.data_view.highlighted_id() else {
            return;
        };
        let Some(scope_ids) = self.scoped_snapshot_ids(&snapshot, &moving_id) else {
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: ListControlReorderUnavailable::DataChanged,
            });
            return;
        };
        self.clear_flat_range_selection();
        let scroll_snapshot = self.data_view.scroll_snapshot();
        let ids = snapshot.ids.clone();
        self.data_view.set_derived_row_order(Some(ids.clone()));
        self.data_view.reposition_highlight_silently(&moving_id);
        self.data_view
            .start_reorder_highlight(moving_id.clone(), settings);
        self.reorder = Some(ReorderState {
            snapshot,
            scroll_snapshot,
            staged: ids,
            scope_ids: self.reorder_scope.as_ref().map(|_| scope_ids),
            moving_id,
            pending_g: false,
        });
    }

    fn reorder_is_compatible(&self) -> bool {
        let Some(state) = &self.reorder else {
            return true;
        };
        let Some(column) = self.reorder_column.as_deref() else {
            return false;
        };
        self.data_view
            .reorder_snapshot_matches(column, &state.snapshot)
            && self.scope_is_compatible(&state.snapshot, &state.moving_id, &state.scope_ids)
    }

    pub(super) fn reorder_states_are_compatible(&self) -> bool {
        self.reorder_is_compatible()
            && self.tree_reorder_is_compatible()
            && self.flat_block_move_is_compatible()
            && self.tree_block_move_is_compatible()
    }

    pub(super) fn restore_block_move_overlay_after_row_replacement(
        &mut self,
        settings: crate::AnimationSettings,
    ) {
        if self.flat_block_move.is_some() {
            self.update_flat_block_overlay();
            self.data_view
                .ensure_selection_placeholder_visible(self.data_area, settings);
        }
        if self.tree_block_move.is_some() {
            self.update_tree_block_overlay();
            self.data_view
                .ensure_selection_placeholder_visible(self.data_area, settings);
        }
    }

    fn move_reorder(&mut self, delta: isize, settings: crate::AnimationSettings) -> bool {
        let Some(state) = &self.reorder else {
            return false;
        };
        let visible_ids = self.reorder_scope_order(state);
        let Some(index) = visible_ids.iter().position(|id| id == &state.moving_id) else {
            return false;
        };
        let target = index
            .saturating_add_signed(delta)
            .min(visible_ids.len().saturating_sub(1));
        self.move_reorder_to(target, settings)
    }

    fn move_reorder_line(&mut self, delta: isize, mut settings: crate::AnimationSettings) -> bool {
        settings.enabled = false;
        self.move_reorder(delta, settings)
    }

    fn move_reorder_to(&mut self, target: usize, settings: crate::AnimationSettings) -> bool {
        let Some(state) = self.reorder.as_ref() else {
            return false;
        };
        let visible_ids = self.reorder_scope_order(state);
        let Some(state) = &mut self.reorder else {
            return false;
        };
        let Some(index) = visible_ids.iter().position(|id| id == &state.moving_id) else {
            return false;
        };
        let target = target.min(visible_ids.len().saturating_sub(1));
        if target == index {
            return false;
        }
        let scope_ids = state.scope_ids.as_ref().unwrap_or(&state.snapshot.ids);
        let mut staged_scope = state
            .staged
            .iter()
            .filter(|id| scope_ids.contains(id))
            .cloned()
            .collect::<Vec<_>>();
        let moving_index = staged_scope
            .iter()
            .position(|id| id == &state.moving_id)
            .expect("reorder snapshot contains moving row");
        let moving_id = staged_scope.remove(moving_index);
        let anchor_id = &visible_ids[target];
        let anchor_index = staged_scope
            .iter()
            .position(|id| id == anchor_id)
            .expect("reorder snapshot contains visible anchor");
        let insertion_index = if target < index {
            anchor_index
        } else {
            anchor_index + 1
        };
        staged_scope.insert(insertion_index, moving_id);
        state.staged =
            Self::merge_scope_order(&state.snapshot.ids, &state.scope_ids, &staged_scope);
        self.data_view
            .set_derived_row_order(Some(state.staged.clone()));
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.data_view.center_highlight(self.data_area, settings);
        true
    }

    fn reorder_scope_order(&self, state: &ReorderState<Id>) -> Vec<Id> {
        let scope_ids = state.scope_ids.as_ref().unwrap_or(&state.snapshot.ids);
        self.data_view
            .reorder_visible_ids()
            .into_iter()
            .filter(|id| scope_ids.contains(id))
            .collect()
    }

    fn merge_scope_order(
        full_ids: &[Id],
        scope_ids: &Option<Vec<Id>>,
        scoped_order: &[Id],
    ) -> Vec<Id> {
        let Some(scope_ids) = scope_ids else {
            return scoped_order.to_vec();
        };
        let mut scoped = scoped_order.iter();
        full_ids
            .iter()
            .map(|id| {
                if scope_ids.contains(id) {
                    scoped
                        .next()
                        .expect("scoped reorder contains every scoped row")
                        .clone()
                } else {
                    id.clone()
                }
            })
            .collect()
    }

    pub(super) fn clear_pending_reorder_g(&mut self) {
        if let Some(state) = &mut self.reorder {
            state.pending_g = false;
        }
        if let Some(state) = &mut self.flat_block_move {
            state.pending_g = false;
        }
        if let Some(state) = &mut self.tree_block_move {
            state.pending_g = false;
        }
    }

    fn handle_reorder_g(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = &mut self.reorder else {
            return;
        };
        if !state.pending_g {
            state.pending_g = true;
            return;
        }
        state.pending_g = false;
        self.move_reorder_to(0, settings);
    }

    fn commit_reorder(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.clear_flat_range_selection();
        self.data_view.clear_reorder_highlight(settings);
        let column = self
            .reorder_column
            .as_deref()
            .expect("reorder is configured");
        if self
            .data_view
            .commit_reorder(column, &state.staged, &state.snapshot)
        {
            self.data_view.set_derived_row_order(None);
            self.data_view
                .reposition_highlight_silently(&state.moving_id);
            self.events.push(ListControlEvent::Reordered {
                row_ids: state.scope_ids.as_ref().map_or_else(
                    || state.staged.clone(),
                    |scope_ids| {
                        state
                            .staged
                            .iter()
                            .filter(|id| scope_ids.contains(id))
                            .cloned()
                            .collect()
                    },
                ),
            });
        } else {
            self.data_view.set_derived_row_order(None);
            self.data_view
                .reposition_highlight_silently(&state.moving_id);
            self.data_view
                .restore_scroll(state.scroll_snapshot, self.data_area, settings);
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: ListControlReorderUnavailable::DataChanged,
            });
        }
    }

    pub(super) fn cancel_reorder(&mut self, settings: crate::AnimationSettings) {
        if self.flat_block_move.is_some() {
            self.cancel_flat_block_move(settings);
        }
        if self.tree_block_move.is_some() {
            self.cancel_tree_block_move(settings);
        }
        if self.tree_reorder.is_some() {
            self.cancel_tree_reorder(settings, false);
        }
        self.cancel_reorder_with_highlight(settings, false);
    }

    pub(super) fn cancel_reorder_for_focus_loss(&mut self, settings: crate::AnimationSettings) {
        if self.flat_block_move.is_some() {
            self.cancel_flat_block_move(settings);
        }
        if self.tree_block_move.is_some() {
            self.cancel_tree_block_move(settings);
        }
        if self.tree_reorder.is_some() {
            self.cancel_tree_reorder(settings, true);
        }
        self.cancel_reorder_with_highlight(settings, true);
    }

    fn cancel_reorder_with_highlight(
        &mut self,
        settings: crate::AnimationSettings,
        clear_highlight_immediately: bool,
    ) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.clear_flat_range_selection();
        if clear_highlight_immediately {
            self.data_view.clear_reorder_highlight_immediately();
        } else {
            self.data_view.clear_reorder_highlight(settings);
        }
        self.data_view.set_derived_row_order(None);
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderCancelled {
            row_id: state.moving_id,
        });
    }

    fn reject_changed_reorder(&mut self, settings: crate::AnimationSettings) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.clear_flat_range_selection();
        self.data_view.clear_reorder_highlight(settings);
        self.data_view.set_derived_row_order(None);
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.data_view
            .restore_scroll(state.scroll_snapshot, self.data_area, settings);
        self.events.push(ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        });
    }

    pub(super) fn reject_reorder_for_data_change(&mut self, settings: crate::AnimationSettings) {
        if self.flat_block_move.is_some() {
            self.reject_changed_flat_block_move(settings);
        }
        if self.tree_block_move.is_some() {
            self.reject_changed_tree_block_move(settings);
        }
        if self.tree_reorder.is_some() {
            self.reject_changed_tree_reorder(settings);
        }
        self.reject_changed_reorder(settings);
    }
}

fn move_block_visual_boundary<Id: Eq>(
    ids: &[Id],
    selected: &[Id],
    boundary: usize,
    delta: isize,
) -> usize {
    let mut boundary = boundary.min(ids.len());
    if delta < 0 {
        if boundary == 0 {
            return boundary;
        }
        if selected.contains(&ids[boundary - 1]) {
            boundary -= 1;
            while boundary > 0 && selected.contains(&ids[boundary - 1]) {
                boundary -= 1;
            }
        } else {
            boundary -= 1;
        }
    } else if delta > 0 {
        if boundary == ids.len() {
            return boundary;
        }
        if selected.contains(&ids[boundary]) {
            while boundary < ids.len() && selected.contains(&ids[boundary]) {
                boundary += 1;
            }
        } else {
            boundary += 1;
        }
    }
    boundary
}

fn unavailable_reason(reason: ReorderUnavailableReason) -> ListControlReorderUnavailable {
    match reason {
        ReorderUnavailableReason::Tree => ListControlReorderUnavailable::Tree,
        ReorderUnavailableReason::VisibleSubset => ListControlReorderUnavailable::VisibleSubset,
        ReorderUnavailableReason::TransformActive => ListControlReorderUnavailable::TransformActive,
        ReorderUnavailableReason::Paginated => ListControlReorderUnavailable::Paginated,
        ReorderUnavailableReason::DuplicateRowIds => ListControlReorderUnavailable::DuplicateRowIds,
        ReorderUnavailableReason::DuplicateRankKeys => {
            ListControlReorderUnavailable::DuplicateRankKeys
        }
    }
}
