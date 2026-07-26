use std::hash::Hash;

use super::{DATA_FOCUS, DATA_SLOT, ListControl, ListControlEvent};
use crate::components::{ConfirmationDialog, ConfirmationDialogOutcome};
use crate::{
    ChildKey, ChildSlot, EventCtx, EventOutcome, EventRoute, LifecycleCtx, TuiEvent, TuiNode,
};

pub(super) struct DynamicChild<C, M> {
    slot: Option<ChildSlot<C, M>>,
    initialized: bool,
    mounted: bool,
}

impl<C, M> Default for DynamicChild<C, M> {
    fn default() -> Self {
        Self {
            slot: None,
            initialized: false,
            mounted: false,
        }
    }
}

impl<C, M> DynamicChild<C, M>
where
    C: TuiNode<M>,
{
    pub(super) fn is_some(&self) -> bool {
        self.slot.is_some()
    }
    pub(super) fn child(&self) -> Option<&C> {
        self.slot.as_ref().map(ChildSlot::child)
    }
    pub(super) fn child_mut(&mut self) -> Option<&mut C> {
        self.slot.as_mut().map(ChildSlot::child_mut)
    }

    fn insert(&mut self, child: C, ctx: &mut EventCtx<M>) {
        debug_assert!(self.slot.is_none());
        let mut slot = ChildSlot::new(ChildKey::new(super::CONFIRM_SLOT), child);
        let mut lifecycle = LifecycleCtx::default();
        if self.mounted {
            slot.mount(&mut lifecycle);
        } else if self.initialized {
            slot.init(&mut lifecycle);
        }
        self.slot = Some(slot);
        merge_lifecycle_effects(ctx, lifecycle);
    }

    fn remove(&mut self, ctx: &mut EventCtx<M>) {
        let Some(mut slot) = self.slot.take() else {
            return;
        };
        let mut lifecycle = LifecycleCtx::default();
        slot.destroy(&mut lifecycle);
        merge_lifecycle_effects(ctx, lifecycle);
    }

    pub(super) fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        if let Some(slot) = &mut self.slot {
            slot.init(ctx);
        }
        self.initialized = true;
    }

    pub(super) fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if let Some(slot) = &mut self.slot {
            slot.mount(ctx);
        }
        self.initialized = true;
        self.mounted = true;
    }

    pub(super) fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if let Some(slot) = &mut self.slot {
            slot.unmount(ctx);
        }
        self.mounted = false;
    }

    pub(super) fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        if let Some(slot) = &mut self.slot {
            slot.destroy(ctx);
        }
        self.initialized = false;
        self.mounted = false;
    }
}

fn merge_lifecycle_effects<M>(ctx: &mut EventCtx<M>, mut lifecycle: LifecycleCtx<M>) {
    if lifecycle.layout_requested() {
        ctx.request_layout();
    }
    if lifecycle.redraw_requested() || lifecycle.tick_requested() {
        ctx.request_redraw();
    }
    for message in lifecycle.drain_messages() {
        ctx.emit(message);
    }
}

impl<T, Id, M: 'static> ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub(super) fn request_remove_confirmation(&mut self, ctx: &mut EventCtx<M>) -> bool {
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
        self.confirmation_dialog.insert(
            ConfirmationDialog::new(config.title.clone(), (config.formatter)(row))
                .yes_text("Delete")
                .no_text("Cancel")
                .keybindings(self.confirmation_keys),
            ctx,
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
        self.confirmation_dialog.remove(ctx);
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
        let Some(dialog) = self.confirmation_dialog.child_mut() else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct LifecycleProbe(Rc<RefCell<Vec<&'static str>>>);

    impl TuiNode<()> for LifecycleProbe {
        fn layout(
            &mut self,
            area: ratatui::layout::Rect,
            _: &mut crate::LayoutCtx,
        ) -> crate::LayoutResult {
            crate::LayoutResult::new(area)
        }

        fn render<'a>(
            &'a self,
            _: &mut ratatui::Frame,
            _: ratatui::layout::Rect,
            _: &mut crate::RenderCtx<'a>,
        ) {
        }

        fn init(&mut self, _: &mut LifecycleCtx<()>) {
            self.0.borrow_mut().push("init");
        }
        fn mount(&mut self, _: &mut LifecycleCtx<()>) {
            self.0.borrow_mut().push("mount");
        }
        fn unmount(&mut self, _: &mut LifecycleCtx<()>) {
            self.0.borrow_mut().push("unmount");
        }
        fn destroy(&mut self, _: &mut LifecycleCtx<()>) {
            self.0.borrow_mut().push("destroy");
        }
    }

    #[test]
    fn mounted_dynamic_child_receives_balanced_lifecycle() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut child = DynamicChild::<LifecycleProbe, ()>::default();
        child.mount(&mut LifecycleCtx::default());
        child.insert(LifecycleProbe(Rc::clone(&events)), &mut EventCtx::default());
        child.remove(&mut EventCtx::default());
        assert_eq!(*events.borrow(), ["init", "mount", "unmount", "destroy"]);
    }
}
