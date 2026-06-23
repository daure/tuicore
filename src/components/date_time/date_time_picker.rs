use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use time::{Date, Month, PrimitiveDateTime, Time};

use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx, LayoutProposal, LayoutResult,
    LayoutSizeHint, TickResult, TuiNode, keybindings,
};

use super::{
    DATE_TIME_PICKER_FOCUS, DatePicker, PickerOutcome, TimePicker, finish_event, picker_size_hint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimePickerLayout {
    Horizontal,
    Vertical,
}

pub struct DateTimePicker<M = ()> {
    date: DatePicker<M>,
    time: TimePicker<M>,
    layout: DateTimePickerLayout,
    active: DateTimePart,
    on_select: Option<Box<dyn Fn(PrimitiveDateTime) -> M>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateTimePart {
    Date,
    Time,
}

impl<M> DateTimePicker<M> {
    pub fn new() -> Self {
        Self {
            date: DatePicker::new(),
            time: TimePicker::new(),
            layout: DateTimePickerLayout::Horizontal,
            active: DateTimePart::Date,
            on_select: None,
        }
    }

    pub fn value(mut self, value: Option<PrimitiveDateTime>) -> Self {
        self.date.set_value(value.map(|value| value.date()));
        if let Some(value) = value {
            self.time.set_value(value.time());
        }
        self
    }

    pub fn layout(mut self, layout: DateTimePickerLayout) -> Self {
        self.layout = layout;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(PrimitiveDateTime) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn current_value(&self) -> Option<PrimitiveDateTime> {
        self.date
            .current_value()
            .map(|date| date.with_time(self.time.current_value()))
    }

    pub fn date_mut(&mut self) -> &mut DatePicker<M> {
        &mut self.date
    }

    pub fn time_mut(&mut self) -> &mut TimePicker<M> {
        &mut self.time
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let [date_area, time_area] = self.areas(area);
        self.date.render(frame, date_area);
        self.time.render(frame, time_area);
    }

    fn on_key(&mut self, key: KeyEvent) -> PickerOutcome {
        let bindings = keybindings();
        let focus_keys = bindings.focus();
        if focus_keys.next_matches(key) && self.active == DateTimePart::Date {
            self.active = DateTimePart::Time;
            self.sync_focus(true);
            return PickerOutcome::handled(true);
        }
        if focus_keys.previous_matches(key) && self.active == DateTimePart::Time {
            self.active = DateTimePart::Date;
            self.sync_focus(true);
            return PickerOutcome::handled(true);
        }
        let outcome = match self.active {
            DateTimePart::Date => self.date.on_key(key),
            DateTimePart::Time => self.time.on_key(key),
        };
        if outcome.selected && self.date.current_value().is_some() {
            return PickerOutcome::selected(outcome.changed);
        }
        outcome
    }

    fn areas(&self, area: Rect) -> [Rect; 2] {
        match self.layout {
            DateTimePickerLayout::Horizontal => Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(24), Constraint::Length(14)])
                .areas(area),
            DateTimePickerLayout::Vertical => [
                Rect::new(area.x, area.y, area.width.min(24), area.height.min(10)),
                Rect::new(
                    area.x,
                    area.y.saturating_add(10),
                    area.width.min(12),
                    area.height.saturating_sub(10).min(1),
                ),
            ],
        }
    }

    fn sync_focus(&mut self, focused: bool) {
        self.date
            .set_focused(focused && self.active == DateTimePart::Date);
        self.time
            .set_focused(focused && self.active == DateTimePart::Time);
    }
}

impl<M> Default for DateTimePicker<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> TuiNode<M> for DateTimePicker<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let size = match self.layout {
            DateTimePickerLayout::Horizontal => picker_size_hint(38, 10),
            DateTimePickerLayout::Vertical => picker_size_hint(24, 11),
        };
        size.normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        ctx.register_focusable(FocusId::new(DATE_TIME_PICKER_FOCUS), area, true);
        ctx.set_focus_receives_events_before_global_hotkeys(
            FocusId::new(DATE_TIME_PICKER_FOCUS),
            true,
        );
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::ExternalEditor(response) = event {
            if self.active != DateTimePart::Date {
                return EventOutcome::Ignored;
            }
            let outcome = self.date.apply_external_editor_response(response);
            if outcome.selected
                && let Some(value) = self.current_value()
                && let Some(on_select) = &self.on_select
            {
                ctx.emit(on_select(value));
            }
            ctx.request_clear();
            return finish_event(ctx, outcome);
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.active == DateTimePart::Date
            && keybindings()
                .date_time_picker()
                .external_editor_matches(*key)
        {
            let value = self.date.cursor().to_string();
            ctx.request_external_editor(value.clone(), 1, value.len() + 1);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        if outcome.selected
            && let Some(value) = self.current_value()
            && let Some(on_select) = &self.on_select
        {
            ctx.emit(on_select(value));
        }
        finish_event(ctx, outcome)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if focused {
            self.active = DateTimePart::Date;
        }
        self.sync_focus(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, _dt: StdDuration, _settings: crate::AnimationSettings) -> TickResult {
        TickResult::IDLE
    }
}

#[cfg(test)]
mod tests;
