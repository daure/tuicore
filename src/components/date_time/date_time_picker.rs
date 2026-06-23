use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use time::PrimitiveDateTime;

use crate::event::{KeyEvent, TuiEvent};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, FocusRequest, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, TickResult, TreePath, TuiNode, keybindings,
};

use super::{
    DatePicker, PickerOutcome, TimeField, TimePicker, finish_event, format_picker_time,
    parse_editor_time, picker_size_hint,
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
    focus_path: TreePath,
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
            focus_path: TreePath::default(),
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
        self.focus_path = ctx.current_path();
        let [date_area, time_area] = self.areas(area);
        ctx.register_focusable(FocusId::new("date-time-picker-date"), date_area, true);
        ctx.register_focusable(FocusId::new("date-time-picker-time"), time_area, true);
        ctx.set_focus_receives_events_before_global_hotkeys(
            FocusId::new("date-time-picker-date"),
            true,
        );
        ctx.set_focus_receives_events_before_global_hotkeys(
            FocusId::new("date-time-picker-time"),
            true,
        );
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::ExternalEditor(response) = event {
            match self.active {
                DateTimePart::Date => {
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
                DateTimePart::Time => {
                    if let Some(time) = parse_editor_time(&response.value) {
                        self.time.set_value(time);
                        if let Some(value) = self.current_value()
                            && let Some(on_select) = &self.on_select
                        {
                            ctx.emit(on_select(value));
                        }
                    }
                    ctx.request_clear();
                    ctx.request_redraw();
                    return EventOutcome::Handled;
                }
            }
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if keybindings()
            .date_time_picker()
            .external_editor_matches(*key)
        {
            match self.active {
                DateTimePart::Date => {
                    let value = self.date.cursor().to_string();
                    ctx.request_external_editor(value.clone(), 1, value.len() + 1);
                }
                DateTimePart::Time => {
                    let value = format_picker_time(self.time.draft_value());
                    let col = match self.time.active_field() {
                        TimeField::Hour => 1,
                        TimeField::Minute => 4,
                        TimeField::Second => 7,
                    };
                    ctx.request_external_editor(value.clone(), 1, col);
                }
            }
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let before_active = self.active;
        let outcome = self.on_key(*key);
        if outcome.handled && self.active != before_active {
            let id = match self.active {
                DateTimePart::Date => "date-time-picker-date",
                DateTimePart::Time => "date-time-picker-time",
            };
            ctx.focus(FocusRequest::TargetAt {
                path: self.focus_path.clone(),
                id: FocusId::new(id),
            });
        }
        if outcome.selected
            && let Some(value) = self.current_value()
            && let Some(on_select) = &self.on_select
        {
            ctx.emit(on_select(value));
        }
        finish_event(ctx, outcome)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if focused {
            if target.is_some_and(|id| id.as_str() == "date-time-picker-time") {
                self.active = DateTimePart::Time;
            } else {
                self.active = DateTimePart::Date;
            }
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
