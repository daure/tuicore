use std::hash::Hash;

use super::{ListControl, ListControlEvent, ListControlReorderUnavailable, ReorderState};
use crate::components::data_view::ReorderUnavailableReason;
use crate::{EventCtx, EventOutcome, Key, KeyEvent, KeyModifiers};

const REORDER_STATUS: &str = "Moving";

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn handle_reorder_key(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        if self.reorder.is_none() {
            if self.reorder_column.is_none() || !self.keys.reorder_matches(key) {
                return None;
            }
            self.begin_reorder();
        } else if !self.reorder_is_compatible() {
            self.reject_changed_reorder();
        } else if matches!(key.code, Key::Enter) && key.modifiers == KeyModifiers::NONE {
            self.commit_reorder();
        } else if matches!(key.code, Key::Esc)
            || key.code == Key::Char('[') && key.modifiers == KeyModifiers::CONTROL
        {
            self.cancel_reorder();
        } else if crate::keybindings().line_up_matches(key) {
            self.move_reorder(-1);
        } else if crate::keybindings().line_down_matches(key) {
            self.move_reorder(1);
        }

        ctx.request_redraw();
        ctx.stop_propagation();
        Some(EventOutcome::Handled)
    }

    fn begin_reorder(&mut self) {
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
        let ids = snapshot.ids.clone();
        let previous_bottom_left = self
            .panel
            .title_text(super::PanelTitlePosition::BottomLeft)
            .map(ToOwned::to_owned);
        self.panel.set_bottom_left(REORDER_STATUS);
        self.data_view.set_derived_row_order(Some(ids.clone()));
        self.reorder = Some(ReorderState {
            snapshot,
            staged: ids,
            moving_id,
            previous_bottom_left,
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

    fn move_reorder(&mut self, delta: isize) {
        let Some(state) = &mut self.reorder else {
            return;
        };
        let Some(index) = state.staged.iter().position(|id| id == &state.moving_id) else {
            return;
        };
        let target = index
            .saturating_add_signed(delta)
            .min(state.staged.len().saturating_sub(1));
        if target == index {
            return;
        }
        state.staged.swap(index, target);
        self.data_view
            .set_derived_row_order(Some(state.staged.clone()));
        self.data_view.highlight_id(&state.moving_id);
    }

    fn commit_reorder(&mut self) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.restore_reorder_status(state.previous_bottom_left.clone());
        let column = self
            .reorder_column
            .as_deref()
            .expect("reorder is configured");
        if self
            .data_view
            .commit_reorder(column, &state.staged, &state.snapshot)
        {
            self.data_view.set_derived_row_order(None);
            self.data_view.highlight_id(&state.moving_id);
            self.events.push(ListControlEvent::Reordered {
                row_ids: state.staged,
            });
        } else {
            self.data_view.set_derived_row_order(None);
            self.events.push(ListControlEvent::ReorderUnavailable {
                reason: ListControlReorderUnavailable::DataChanged,
            });
        }
    }

    pub(super) fn cancel_reorder(&mut self) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.restore_reorder_status(state.previous_bottom_left.clone());
        self.data_view.set_derived_row_order(None);
        self.data_view.highlight_id(&state.moving_id);
        self.events.push(ListControlEvent::ReorderCancelled {
            row_id: state.moving_id,
        });
    }

    fn reject_changed_reorder(&mut self) {
        let Some(state) = self.reorder.take() else {
            return;
        };
        self.restore_reorder_status(state.previous_bottom_left);
        self.data_view.set_derived_row_order(None);
        self.events.push(ListControlEvent::ReorderUnavailable {
            reason: ListControlReorderUnavailable::DataChanged,
        });
    }

    fn restore_reorder_status(&mut self, previous: Option<String>) {
        match previous {
            Some(title) => self.panel.set_bottom_left(title),
            None => self
                .panel
                .clear_title(super::PanelTitlePosition::BottomLeft),
        }
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
