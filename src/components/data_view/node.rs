use std::hash::Hash;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::{
    ChoiceDropdown, DATA_VIEW_FOCUS, DataView, DataViewInteraction, FILTER_DROPDOWN_SLOT,
    HEADER_PICK_TIMEOUT, SEARCH_SLOT, TEXT_INPUT_FOCUS,
};
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRequest, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, TickResult,
    TuiNode, keybindings,
};

impl<T, Id> Animated for DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let mut result = self.scroll.tick(dt, settings).merge(Animated::tick(
            &mut self.search_input,
            dt,
            settings,
        ));
        if matches!(self.interaction, DataViewInteraction::HeaderFilter) {
            self.header_pick_elapsed = self.header_pick_elapsed.saturating_add(dt);
            if self.header_pick_elapsed >= HEADER_PICK_TIMEOUT {
                self.interaction = DataViewInteraction::Grid;
                self.header_pick_elapsed = Duration::ZERO;
                result = result.merge(TickResult::CHANGED);
            } else {
                result = result.merge(TickResult::scheduled_after(
                    HEADER_PICK_TIMEOUT - self.header_pick_elapsed,
                ));
            }
        } else {
            self.header_pick_elapsed = Duration::ZERO;
        }
        if let Some(dropdown) = self.filter_dropdown.as_mut() {
            result = result.merge(Animated::tick(dropdown.as_mut(), dt, settings));
        }
        result
    }
}

impl<T, Id, M> TuiNode<M> for DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = self.columns.len().max(1).min(u16::MAX as usize) as u16;
        let header = self.headers as u16;
        let action_bar = self.action_bar as u16;
        let rows = self.visible_rows().len().min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(
            width,
            action_bar
                .saturating_add(header)
                .saturating_add(rows)
                .max(1),
        )
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let width_changed = self.area.width != 0 && self.area.width != area.width;
        self.area = area;
        if width_changed {
            self.scroll.snap_horizontal_to_start();
        }
        if let Some(hotkey) = &self.hotkey {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(DATA_VIEW_FOCUS),
                area,
                true,
                vec![hotkey.clone()],
            );
        } else {
            ctx.register_focusable(FocusId::new(DATA_VIEW_FOCUS), area, true);
        }
        self.layout_children::<M>(area, ctx);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.render_with_row_style_ctx(frame, area, None, ctx);
    }

    fn event(&mut self, event: &crate::TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if matches!(event, crate::TuiEvent::Yank) {
            if let Some(value) = self.highlighted_json() {
                ctx.copy_to_clipboard(value);
            }
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let crate::TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let popup_open = matches!(self.interaction, DataViewInteraction::FilterValues { .. });
        if !self.focused && !matches!(self.interaction, DataViewInteraction::Search) && !popup_open
        {
            return EventOutcome::Ignored;
        }
        let bindings = keybindings();
        let focus_keys = bindings.focus();
        if focus_keys.next_matches(*key) {
            ctx.focus_next();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if focus_keys.previous_matches(*key) {
            ctx.focus_previous();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let focused_search = matches!(self.interaction, DataViewInteraction::Search);
        let before_interaction = self.interaction.clone();
        let outcome = self.on_key_with_settings(*key, self.area, ctx.animation());
        if Self::search_exited(&before_interaction, &self.interaction) {
            self.focus_self(ctx);
        }
        if matches!(before_interaction, DataViewInteraction::Grid)
            && matches!(self.interaction, DataViewInteraction::Search)
        {
            ctx.focus(FocusRequest::TargetAt {
                path: ctx.current_path().child(ChildKey::new(SEARCH_SLOT)),
                id: FocusId::new(TEXT_INPUT_FOCUS),
            });
            ctx.request_layout();
        }
        if popup_open != matches!(self.interaction, DataViewInteraction::FilterValues { .. }) {
            ctx.request_layout();
            if matches!(self.interaction, DataViewInteraction::FilterValues { .. }) {
                self.focus_filter_dropdown_search(ctx);
            } else if popup_open {
                self.focus_self(ctx);
            }
        }
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled || focused_search || popup_open {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &crate::TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }
        if route
            .path
            .without_first_if(&ChildKey::new(SEARCH_SLOT))
            .is_some()
        {
            if !matches!(self.interaction, DataViewInteraction::Search) {
                return self.event(event, ctx);
            }
            let before_interaction = self.interaction.clone();
            let outcome = self.on_search_event(event, self.area, ctx.animation(), ctx);
            if Self::search_exited(&before_interaction, &self.interaction) {
                self.focus_self(ctx);
            }
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled || outcome.changed || outcome.active || outcome.activated {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if route
            .path
            .without_first_if(&ChildKey::new(FILTER_DROPDOWN_SLOT))
            .is_some()
        {
            let DataViewInteraction::FilterValues { column_id } = self.interaction.clone() else {
                return EventOutcome::Ignored;
            };
            let popup_open = self.filter_dropdown.is_some();
            let outcome =
                self.on_filter_values_event(event, self.area, ctx.animation(), &column_id, ctx);
            if popup_open != self.filter_dropdown.is_some() {
                ctx.request_layout();
                if popup_open {
                    self.focus_self(ctx);
                }
            }
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled || outcome.changed || outcome.active || outcome.activated {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.focused = focused;
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target
            .path
            .without_first_if(&ChildKey::new(SEARCH_SLOT))
            .is_some()
        {
            self.focused = focused;
            self.search_input.set_focused(focused);
            if focused {
                self.interaction = DataViewInteraction::Search;
                self.search_input.set_insert_mode(true);
            } else if matches!(self.interaction, DataViewInteraction::Search) {
                self.interaction = DataViewInteraction::Grid;
            }
            ctx.request_redraw();
            return;
        }
        if let Some(child_target) = target.for_child(&ChildKey::new(FILTER_DROPDOWN_SLOT)) {
            self.focused = focused;
            if let Some(dropdown) = self.filter_dropdown.as_mut() {
                <Box<ChoiceDropdown> as TuiNode<M>>::dispatch_focus(
                    dropdown,
                    &child_target,
                    focused,
                    ctx,
                );
            }
            if !focused && matches!(self.interaction, DataViewInteraction::FilterValues { .. }) {
                self.close_choice_dropdowns();
            }
            ctx.request_redraw();
            return;
        }
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        }
    }
}
