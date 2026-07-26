use std::hash::Hash;

use super::{DATA_FOCUS, DATA_SLOT, ListControl, ListControlEvent};
use crate::components::{ConfirmationDialog, ConfirmationDialogOutcome};
use crate::{EventCtx, EventOutcome, EventRoute, TuiEvent, TuiNode};

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn request_remove_confirmation(&mut self) -> bool {
        let Some(config) = &self.remove_confirmation else {
            return false;
        };
        let Some(row_id) = self.data_view.highlighted_id() else {
            return false;
        };
        let Some(row) = self
            .data_view
            .rows()
            .iter()
            .find(|row| self.data_view.row_id(row) == row_id)
        else {
            return false;
        };
        self.pending_remove = Some(row_id);
        self.confirmation_dialog = Some(
            ConfirmationDialog::new(config.title.clone(), (config.formatter)(row))
                .yes_text("Remove")
                .no_text("Keep")
                .keybindings(self.confirmation_keys),
        );
        self.data_view.set_focused(false);
        true
    }

    fn finish_remove_confirmation(
        &mut self,
        outcome: ConfirmationDialogOutcome,
        route: &EventRoute,
        ctx: &mut EventCtx<M>,
    ) {
        if outcome == ConfirmationDialogOutcome::Confirmed
            && let Some(row_id) = self.pending_remove.take()
            && self.data_view.remove_row(&row_id).is_some()
        {
            self.events.push(ListControlEvent::Removed { row_id });
        }
        self.pending_remove = None;
        self.confirmation_dialog = None;
        self.data_view.set_focused(true);
        Self::focus_child(ctx, route, DATA_SLOT, DATA_FOCUS);
        ctx.request_layout();
        ctx.request_redraw();
    }

    pub(super) fn confirmation_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let Some(dialog) = &mut self.confirmation_dialog else {
            return EventOutcome::Ignored;
        };
        let outcome = dialog.event(event, ctx);
        if let Some(result) = dialog.take_outcomes().into_iter().next() {
            self.finish_remove_confirmation(result, route, ctx);
        }
        ctx.stop_propagation();
        outcome
    }
}
