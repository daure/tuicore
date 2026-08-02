use std::hash::Hash;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::{
    CONFIRM_OVERLAY_ID, CONFIRM_SLOT, DATA_FOCUS, DATA_SLOT, DIALOG_FOCUS, ListControl,
    ListControlInput,
};
use crate::components::{DataView, Dropdown, Panel, TextInput};
use crate::{
    Animated, AnimationSettings, AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute,
    FocusCtx, FocusId, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, OverlayLayer, OverlaySpec, TickResult, TreePath, TuiEvent, TuiNode,
};

impl<T, Id, M: 'static> TuiNode<M> for ListControl<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let child = <DataView<T, Id> as TuiNode<M>>::measure(&self.data_view, proposal);
        let item_rows = self.data_view.visible_row_count().max(1);
        let visible_rows = item_rows
            .saturating_add(usize::from(self.editor_active()))
            .min(self.max_rows)
            .min(u16::MAX as usize) as u16;
        let chrome_height = self.data_view.measurement_chrome_height();
        let row_height = self.data_view.configured_row_height();
        let editor_rows = u16::from(self.editor_active()).min(visible_rows);
        let data_height = chrome_height.saturating_add(
            visible_rows
                .saturating_sub(editor_rows)
                .saturating_mul(row_height),
        );
        let horizontal_scrollbar_height = match proposal.width {
            AxisProposal::Unbounded => 0,
            AxisProposal::AtMost(width) | AxisProposal::Exact(width) => {
                let panel_width = if self.panel_visible { 2 } else { 0 };
                let layout = self
                    .data_view
                    .scroll_geometry(Rect::new(
                        0,
                        0,
                        width.saturating_sub(panel_width),
                        data_height,
                    ))
                    .layout;
                u16::from(layout.viewport.height < layout.outer.height)
            }
        };
        let panel_height = if self.panel_visible { 2 } else { 0 };
        let height = chrome_height
            .saturating_add(visible_rows.saturating_mul(row_height))
            .saturating_add(horizontal_scrollbar_height)
            .saturating_add(panel_height);
        LayoutSizeHint::content(
            child
                .preferred
                .width
                .saturating_add(if self.panel_visible { 2 } else { 0 }),
            height,
        )
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        let overlay_bounds = ctx.overlay_bounds();
        self.confirmation_bounds = if overlay_bounds.is_empty() {
            area
        } else {
            overlay_bounds
        };
        let inner = if self.panel_visible {
            Panel::inner_area(area)
        } else {
            area
        };
        let input_height = if self.editor_active() {
            self.data_view.configured_row_height().min(inner.height)
        } else {
            0
        };
        self.data_area = Rect::new(inner.x, inner.y, inner.width, inner.height - input_height);
        let input_area = Rect::new(
            inner.x,
            inner.bottom().saturating_sub(input_height),
            inner.width,
            input_height,
        );
        self.input_area = if self.editor_active() {
            self.full_row_input_area(input_area)
        } else {
            Rect::default()
        };
        ctx.push_slot(ChildKey::new(DATA_SLOT), self.data_area, |ctx| {
            let focus_disabled = ctx.focus_disabled();
            if self.confirmation_dialog.is_some() {
                ctx.set_focus_disabled(true);
            }
            <DataView<T, Id> as TuiNode<M>>::layout(&mut self.data_view, self.data_area, ctx);
            ctx.set_focus_control(FocusId::new(DATA_FOCUS), true);
            if self.editor_active() {
                ctx.set_focus_tab_stop(FocusId::new(DATA_FOCUS), false);
            }
            ctx.set_focus_disabled(focus_disabled);
        });
        if self.editor_active() {
            let index = self.active_field;
            let input_area = self.input_area;
            let input = &mut self.inputs[index];
            ctx.push_slot(Self::input_slot(index), input_area, |ctx| {
                match input {
                    ListControlInput::Text(input) => {
                        <TextInput<M> as TuiNode<M>>::layout(input, input_area, ctx)
                    }
                    ListControlInput::Dropdown(input) => {
                        <Dropdown<String, String> as TuiNode<M>>::layout(
                            input.as_mut().expect("dropdown input is present"),
                            input_area,
                            ctx,
                        )
                    }
                };
            });
        }
        if let Some(dialog) = self.confirmation_dialog.child_mut() {
            let bounds = self.confirmation_bounds;
            let hint = dialog.measure(LayoutProposal::at_most(bounds.width, bounds.height));
            let width = hint.preferred.width.min(bounds.width);
            let height = hint.preferred.height.min(bounds.height);
            self.confirmation_area = Rect::new(
                bounds.x + bounds.width.saturating_sub(width) / 2,
                bounds.y + bounds.height.saturating_sub(height) / 2,
                width,
                height,
            );
            let mut overlay = OverlaySpec::new(
                CONFIRM_OVERLAY_ID,
                self.confirmation_area,
                self.confirmation_area,
            );
            overlay.bounds = Some(bounds);
            overlay.layer = OverlayLayer::Modal;
            ctx.register_overlay(overlay);
            ctx.with_overlay_bounds(self.confirmation_area, |ctx| {
                ctx.push_slot(ChildKey::new(CONFIRM_SLOT), self.confirmation_area, |ctx| {
                    dialog.layout(self.confirmation_area, ctx);
                    let dialog_focus = FocusId::new(DIALOG_FOCUS);
                    ctx.set_focus_receives_events_before_global_hotkeys(dialog_focus.clone(), true);
                    ctx.set_focus_suppresses_global_hotkeys(dialog_focus, true);
                });
            });
        } else {
            self.confirmation_area = Rect::default();
            self.confirmation_bounds = Rect::default();
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if self.panel_visible {
            self.panel.render(frame, area);
        }
        let data_area = if self.data_area.is_empty() && !area.is_empty() {
            if self.panel_visible {
                Panel::inner_area(area)
            } else {
                area
            }
        } else {
            self.data_area
        };
        <DataView<T, Id> as TuiNode<M>>::render(&self.data_view, frame, data_area, ctx);
        if self.editor_active() {
            match &self.inputs[self.active_field] {
                ListControlInput::Text(input) => {
                    <TextInput<M> as TuiNode<M>>::render(input, frame, self.input_area, ctx)
                }
                ListControlInput::Dropdown(input) => {
                    <Dropdown<String, String> as TuiNode<M>>::render(
                        input.as_ref().expect("dropdown input is present"),
                        frame,
                        self.input_area,
                        ctx,
                    )
                }
            }
        }
        if let Some(dialog) = self.confirmation_dialog.child() {
            crate::components::dialog_layer::dim_backdrop_buffer(
                frame,
                self.confirmation_bounds,
                0.45,
            );
            ctx.push_portal_with_ctx(
                OverlayLayer::Modal,
                0,
                self.confirmation_area,
                |frame, area, _ctx| dialog.render(frame, area),
            );
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.handle_visual_hotkey(event, ctx);
        if let TuiEvent::Key(key) = event
            && self.is_reordering()
            && let Some(outcome) = self.handle_reorder_key(*key, ctx)
        {
            return outcome;
        }
        if self.confirmation_dialog.is_some() {
            return self.confirmation_event(&EventRoute::new(TreePath::new()), event, ctx);
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.editor_active() {
            return self
                .handle_control_key(*key, &EventRoute::new(TreePath::new()), ctx)
                .unwrap_or(EventOutcome::Ignored);
        }
        if self.data_view.has_active_interaction() {
            return EventOutcome::Ignored;
        }
        if let Some(outcome) = self.handle_reorder_key(*key, ctx) {
            return outcome;
        }
        self.handle_control_key(*key, &EventRoute::new(TreePath::new()), ctx)
            .unwrap_or(EventOutcome::Ignored)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.handle_visual_hotkey(event, ctx);
        if let TuiEvent::Key(key) = event
            && self.is_reordering()
            && let Some(outcome) = self.handle_reorder_key(*key, ctx)
        {
            return outcome;
        }
        if self.confirmation_dialog.is_some() {
            return self.confirmation_event(route, event, ctx);
        }
        if self.editor_active()
            && let TuiEvent::Key(key) = event
            && let Some(outcome) = self.handle_control_key(*key, route, ctx)
        {
            return outcome;
        }
        if self.editor_active() {
            let index = self.active_field;
            if let Some(path) = route.path.without_first_if(&Self::input_slot(index)) {
                let (outcome, dropdown_transition) = match &mut self.inputs[index] {
                    ListControlInput::Text(input) => (
                        input.dispatch_event(&EventRoute::new(path), event, ctx),
                        None,
                    ),
                    ListControlInput::Dropdown(input) => {
                        let input = input.as_mut().expect("dropdown input is present");
                        let dropdown = input.event_outcome(event, ctx);
                        (
                            input.apply_event_outcome(dropdown, ctx),
                            Some((dropdown.canceled, dropdown.closed && dropdown.committed)),
                        )
                    }
                };
                if let Some((canceled, committed)) = dropdown_transition {
                    if canceled {
                        self.cancel_editor(true);
                        self.restore_data_focus(route, ctx);
                    } else if committed {
                        let final_field = self.active_field_is_last_visible();
                        if self.advance_field(route, ctx) && final_field {
                            self.restore_data_focus(route, ctx);
                        }
                    }
                }
                return outcome;
            }
        }
        if let Some(path) = route.path.without_first_if(&ChildKey::new(DATA_SLOT)) {
            if !path.is_empty() {
                return self
                    .data_view
                    .dispatch_event(&EventRoute::new(path), event, ctx);
            }
            if self.data_view.has_active_interaction() {
                return self
                    .data_view
                    .dispatch_event(&EventRoute::new(path), event, ctx);
            }
            if !self.editor_active()
                && let TuiEvent::Key(key) = event
                && let Some(outcome) = self.handle_reorder_key(*key, ctx)
            {
                return outcome;
            }
            if let TuiEvent::Key(key) = event
                && let Some(outcome) = self.handle_control_key(*key, route, ctx)
            {
                return outcome;
            }
            return self
                .data_view
                .dispatch_event(&EventRoute::new(path), event, ctx);
        }
        if route.path.is_empty()
            && !self.editor_active()
            && let TuiEvent::Key(key) = event
            && let Some(outcome) = self.handle_reorder_key(*key, ctx)
        {
            return outcome;
        }
        if route.path.is_empty()
            && let TuiEvent::Key(key) = event
            && let Some(outcome) = self.handle_control_key(*key, route, ctx)
        {
            return outcome;
        }
        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let mut result = Animated::tick(&mut self.panel, dt, settings).merge(Animated::tick(
            &mut self.data_view,
            dt,
            settings,
        ));
        for input in &mut self.inputs {
            result = result.merge(match input {
                ListControlInput::Text(input) => Animated::tick(input, dt, settings),
                ListControlInput::Dropdown(input) => Animated::tick(
                    input.as_mut().expect("dropdown input is present"),
                    dt,
                    settings,
                ),
            });
        }
        if let Some(dialog) = self.confirmation_dialog.child_mut() {
            result = result.merge(dialog.tick(dt, settings));
        }
        result
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if let Some(dialog) = self.confirmation_dialog.child_mut() {
            if let Some(target) = target.for_child(&ChildKey::new(CONFIRM_SLOT)) {
                self.panel.set_focused(focused, ctx.animation());
                dialog.dispatch_focus(&target, focused, ctx);
            }
            return;
        }
        let editor_active = self.editor_active();
        for (index, input) in self.inputs.iter_mut().enumerate() {
            if let Some(target) = target.for_child(&Self::input_slot(index)) {
                self.panel.set_focused(focused, ctx.animation());
                match input {
                    ListControlInput::Text(input) => input.dispatch_focus(&target, focused, ctx),
                    ListControlInput::Dropdown(input) => input
                        .as_mut()
                        .expect("dropdown input is present")
                        .dispatch_focus(&target, focused, ctx),
                }
                if !focused && editor_active && index == self.active_field && !input.is_focused() {
                    self.cancel_editor(false);
                    ctx.request_layout();
                    ctx.request_redraw();
                }
                return;
            }
        }
        if let Some(target) = target.for_child(&ChildKey::new(DATA_SLOT)) {
            self.panel.set_focused(focused, ctx.animation());
            self.data_view.dispatch_focus(&target, focused, ctx);
            if !focused && self.is_reordering() {
                self.cancel_reorder_for_focus_loss(ctx.animation());
                ctx.request_redraw();
            }
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.data_view.init(ctx);
        for input in &mut self.inputs {
            match input {
                ListControlInput::Text(input) => input.init(ctx),
                ListControlInput::Dropdown(input) => {
                    input.as_mut().expect("dropdown input is present").init(ctx)
                }
            }
        }
        self.confirmation_dialog.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.data_view.mount(ctx);
        for input in &mut self.inputs {
            match input {
                ListControlInput::Text(input) => input.mount(ctx),
                ListControlInput::Dropdown(input) => input
                    .as_mut()
                    .expect("dropdown input is present")
                    .mount(ctx),
            }
        }
        self.confirmation_dialog.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.data_view.unmount(ctx);
        for input in &mut self.inputs {
            match input {
                ListControlInput::Text(input) => input.unmount(ctx),
                ListControlInput::Dropdown(input) => input
                    .as_mut()
                    .expect("dropdown input is present")
                    .unmount(ctx),
            }
        }
        self.confirmation_dialog.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.data_view.destroy(ctx);
        for input in &mut self.inputs {
            match input {
                ListControlInput::Text(input) => input.destroy(ctx),
                ListControlInput::Dropdown(input) => input
                    .as_mut()
                    .expect("dropdown input is present")
                    .destroy(ctx),
            }
        }
        self.confirmation_dialog.destroy(ctx);
    }
}
