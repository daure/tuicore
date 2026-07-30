use std::hash::Hash;

use super::{
    ListControl, ListControlEvent, ListControlReorderUnavailable, ReorderState, TreeReorderState,
};
use crate::components::data_view::ReorderUnavailableReason;
use crate::{EventCtx, EventOutcome, Key, KeyEvent, KeyModifiers, KeySpec};

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn handle_quick_tree_move(&mut self, key: KeyEvent) -> Option<bool> {
        if !self.data_view.tree_is_mutable() {
            return None;
        }
        let outdent = KeySpec::plain('<').matches(key);
        let indent = KeySpec::plain('>').matches(key);
        if !outdent && !indent {
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
        if self.tree_reorder.is_some() {
            self.handle_tree_reorder_key(key, ctx);
            return Some(EventOutcome::Handled);
        }
        if self.reorder.is_none() {
            if !self.keys.reorder_matches(key) {
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
            let keys = crate::keybindings();
            let top_prefix = keys.data_view().top_prefix_matches(key);
            if !top_prefix {
                self.clear_pending_reorder_g();
            }

            if matches!(key.code, Key::Enter | Key::Char(' '))
                && key.modifiers == KeyModifiers::NONE
            {
                self.commit_reorder(ctx.animation());
            } else if matches!(key.code, Key::Esc)
                || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
            {
                self.cancel_reorder(ctx.animation());
            } else if keys.line_up_matches(key) {
                self.move_reorder_line(-1, ctx.animation());
            } else if keys.line_down_matches(key) {
                self.move_reorder_line(1, ctx.animation());
            } else if keys.page_up_matches(key) {
                let page = self.data_view.visible_page_step(self.data_area);
                self.move_reorder(-(page as isize), ctx.animation());
            } else if keys.page_down_matches(key) {
                let page = self.data_view.visible_page_step(self.data_area);
                self.move_reorder(page as isize, ctx.animation());
            } else if keys.home_matches(key) {
                self.move_reorder_to(0, ctx.animation());
            } else if keys.end_matches(key) || keys.data_view().bottom_matches(key) {
                self.move_reorder_to(usize::MAX, ctx.animation());
            } else if top_prefix {
                self.handle_reorder_g(ctx.animation());
            }
        }

        ctx.request_redraw();
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    fn handle_tree_reorder_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) {
        let keys = crate::keybindings();
        if !self.tree_reorder_is_compatible() {
            self.reject_changed_tree_reorder(ctx.animation());
        } else if key.code == Key::Enter && key.modifiers == KeyModifiers::NONE {
            self.commit_tree_reorder(ctx.animation());
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_tree_reorder(ctx.animation(), false);
        } else {
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
    }

    fn move_reorder(&mut self, delta: isize, settings: crate::AnimationSettings) -> bool {
        let Some(state) = &self.reorder else {
            return false;
        };
        let Some(index) = state.staged.iter().position(|id| id == &state.moving_id) else {
            return false;
        };
        let target = index
            .saturating_add_signed(delta)
            .min(state.staged.len().saturating_sub(1));
        self.move_reorder_to(target, settings)
    }

    fn move_reorder_line(&mut self, delta: isize, mut settings: crate::AnimationSettings) -> bool {
        settings.enabled = false;
        self.move_reorder(delta, settings)
    }

    fn move_reorder_to(&mut self, target: usize, settings: crate::AnimationSettings) -> bool {
        let Some(state) = &mut self.reorder else {
            return false;
        };
        let Some(index) = state.staged.iter().position(|id| id == &state.moving_id) else {
            return false;
        };
        let target = target.min(state.staged.len().saturating_sub(1));
        if target == index {
            return false;
        }
        let moving_id = state.staged.remove(index);
        state.staged.insert(target, moving_id);
        self.data_view
            .set_derived_row_order(Some(state.staged.clone()));
        self.data_view
            .reposition_highlight_silently(&state.moving_id);
        self.data_view.center_highlight(self.data_area, settings);
        true
    }

    fn clear_pending_reorder_g(&mut self) {
        if let Some(state) = &mut self.reorder {
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
                row_ids: state.staged,
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
        if self.tree_reorder.is_some() {
            self.cancel_tree_reorder(settings, false);
            return;
        }
        self.cancel_reorder_with_highlight(settings, false);
    }

    pub(super) fn cancel_reorder_for_focus_loss(&mut self, settings: crate::AnimationSettings) {
        if self.tree_reorder.is_some() {
            self.cancel_tree_reorder(settings, true);
            return;
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
