use std::time::Duration as StdDuration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use time::Time;

use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, HotkeyEvent, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, TickResult, TuiNode, hotkey_underline_style, keybindings, theme,
};

use super::{
    PickerOutcome, TIME_PICKER_FOCUS, finish_event, picker_size_hint, plain_digit, wrap_step,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeField {
    Hour,
    Minute,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimePrecision {
    HourMinute,
    HourMinuteSecond,
}

pub struct TimePicker<M = ()> {
    value: Time,
    draft: Time,
    active_field: TimeField,
    precision: TimePrecision,
    minute_step: u8,
    focused: bool,
    hotkey: Option<String>,
    pending_hotkey_prefix: Option<String>,
    pending_top_prefix: bool,
    typed_digits: String,
    on_select: Option<Box<dyn Fn(Time) -> M>>,
}

impl<M> TimePicker<M> {
    pub fn new() -> Self {
        let value = Time::from_hms(9, 0, 0).expect("valid default time");
        Self {
            value,
            draft: value,
            active_field: TimeField::Hour,
            precision: TimePrecision::HourMinute,
            minute_step: 5,
            focused: false,
            hotkey: None,
            pending_hotkey_prefix: None,
            pending_top_prefix: false,
            typed_digits: String::new(),
            on_select: None,
        }
    }

    pub fn value(mut self, value: Time) -> Self {
        self.set_value(value);
        self
    }

    pub fn precision(mut self, precision: TimePrecision) -> Self {
        self.precision = precision;
        self
    }

    pub fn minute_step(mut self, step: u8) -> Self {
        self.minute_step = step.clamp(1, 60);
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
        self.pending_hotkey_prefix = None;
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.pending_hotkey_prefix = None;
    }

    pub fn on_select(mut self, handler: impl Fn(Time) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn current_value(&self) -> Time {
        self.value
    }

    pub fn draft_value(&self) -> Time {
        self.draft
    }

    pub fn active_field(&self) -> TimeField {
        self.active_field
    }

    pub fn set_value(&mut self, value: Time) {
        self.value = value;
        self.draft = value;
        self.typed_digits.clear();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    #[cfg(test)]
    pub(super) fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> PickerOutcome {
        let key = key.into();
        let bindings = keybindings();
        let date_keys = bindings.date_time_picker();
        if key.code == Key::Char('n') && key.modifiers.is_empty() {
            let now = super::today_time();
            let changed = self.draft != now || self.value != now;
            self.draft = now;
            self.value = now;
            self.typed_digits.clear();
            return PickerOutcome::handled(changed);
        }
        if date_keys.top_prefix_matches(key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                return self.set_active_field_to_min();
            }
            self.pending_top_prefix = true;
            return PickerOutcome::handled(false);
        }
        self.pending_top_prefix = false;
        if date_keys.bottom_matches(key) {
            return self.set_active_field_to_max();
        }
        if let Some(digit) = plain_digit(key) {
            return self.type_digit(digit);
        }
        if bindings.line_left_matches(key) {
            self.typed_digits.clear();
            self.active_field = self.previous_field();
            return PickerOutcome::handled(true);
        }
        if bindings.line_right_matches(key) {
            self.typed_digits.clear();
            self.active_field = self.next_field();
            return PickerOutcome::handled(true);
        }
        if bindings.line_up_matches(key) {
            self.typed_digits.clear();
            self.adjust_active_field(1, 1);
            return PickerOutcome::handled(true);
        }
        if bindings.line_down_matches(key) {
            self.typed_digits.clear();
            self.adjust_active_field(-1, 1);
            return PickerOutcome::handled(true);
        }
        if bindings.page_up_matches(key) {
            self.typed_digits.clear();
            self.adjust_active_field(1, self.active_field_page_step());
            return PickerOutcome::handled(true);
        }
        if bindings.page_down_matches(key) {
            self.typed_digits.clear();
            self.adjust_active_field(-1, self.active_field_page_step());
            return PickerOutcome::handled(true);
        }
        if bindings.home_matches(key) {
            return self.set_active_field_to_min();
        }
        if bindings.end_matches(key) {
            return self.set_active_field_to_max();
        }
        if bindings.button().press_matches(key) {
            self.typed_digits.clear();
            self.value = self.draft;
            return PickerOutcome::selected(true);
        }
        if bindings.focus().unfocus_matches(key) {
            let changed = self.draft != self.value;
            self.draft = self.value;
            self.typed_digits.clear();
            return PickerOutcome::canceled(changed);
        }
        self.typed_digits.clear();
        PickerOutcome::IGNORED
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new(self.time_line()), area);
    }

    fn time_line(&self) -> Line<'static> {
        let time = self.draft;
        let base = Style::default().fg(theme().text_fg());
        let active = Style::default()
            .fg(theme().highlight_fg())
            .bg(theme().highlight_bg())
            .add_modifier(Modifier::BOLD);
        let mut value_spans = vec![
            Span::styled("󰅐 ", base),
            self.field_span(format!("{:02}", time.hour()), TimeField::Hour, base, active),
        ];
        value_spans.push(Span::styled(":", base));
        value_spans.push(self.field_span(
            format!("{:02}", time.minute()),
            TimeField::Minute,
            base,
            active,
        ));
        if self.precision == TimePrecision::HourMinuteSecond {
            value_spans.push(Span::styled(":", base));
            value_spans.push(self.field_span(
                format!("{:02}", time.second()),
                TimeField::Second,
                base,
                active,
            ));
        }
        let mut spans = value_spans;
        if let Some(hotkey) = self.hotkey.as_deref() {
            let hotkey = crate::hotkey::normalize_hotkey(hotkey);
            let active_prefix = self
                .pending_hotkey_prefix
                .as_deref()
                .map(crate::hotkey::normalize_hotkey)
                .filter(|prefix| !prefix.is_empty() && hotkey.starts_with(prefix));
            spans.push(Span::styled(" ", base));
            spans.push(Span::styled("|", base));
            for (index, ch) in hotkey.chars().enumerate() {
                let active = active_prefix
                    .as_deref()
                    .is_some_and(|prefix| index < prefix.chars().count());
                spans.push(Span::styled(
                    ch.to_string(),
                    if active {
                        hotkey_underline_style(base)
                    } else {
                        base
                    },
                ));
            }
            spans.push(Span::styled("|", base));
        }
        Line::from(spans)
    }

    fn time_line_width(&self) -> u16 {
        match self.precision {
            TimePrecision::HourMinute => 8,
            TimePrecision::HourMinuteSecond => 11,
        }
    }

    fn field_span(
        &self,
        text: String,
        field: TimeField,
        base: Style,
        active: Style,
    ) -> Span<'static> {
        Span::styled(
            text,
            if self.focused && self.active_field == field {
                active
            } else {
                base
            },
        )
    }

    fn adjust_active_field(&mut self, delta: i8, step: u8) {
        let hour = self.draft.hour();
        let minute = self.draft.minute();
        let second = self.draft.second();
        let (hour, minute, second) = match self.active_field {
            TimeField::Hour => (wrap_step(hour, delta, step, 24), minute, second),
            TimeField::Minute => (hour, wrap_step(minute, delta, step, 60), second),
            TimeField::Second => (hour, minute, wrap_step(second, delta, step, 60)),
        };
        self.draft = Time::from_hms(hour, minute, second).expect("wrapped time stays valid");
    }

    fn type_digit(&mut self, digit: u8) -> PickerOutcome {
        self.typed_digits.push(char::from(b'0' + digit));
        if self.typed_digits.len() > 2 {
            self.typed_digits.replace_range(..1, "");
        }
        let value = self.typed_digits.parse::<u8>().unwrap_or(digit);
        let changed = self.set_active_field_value(value);
        if self.typed_digits.len() >= 2 {
            self.typed_digits.clear();
            if self.should_advance_after_typing() {
                self.active_field = self.next_field();
            }
        }
        PickerOutcome::handled(changed)
    }

    fn set_active_field_to_min(&mut self) -> PickerOutcome {
        self.typed_digits.clear();
        PickerOutcome::handled(self.set_active_field_value(0))
    }

    fn set_active_field_to_max(&mut self) -> PickerOutcome {
        self.typed_digits.clear();
        PickerOutcome::handled(self.set_active_field_value(self.active_field_max()))
    }

    fn set_active_field_value(&mut self, value: u8) -> bool {
        let hour = self.draft.hour();
        let minute = self.draft.minute();
        let second = self.draft.second();
        let value = value.min(self.active_field_max());
        let next = match self.active_field {
            TimeField::Hour => Time::from_hms(value, minute, second),
            TimeField::Minute => Time::from_hms(hour, value, second),
            TimeField::Second => Time::from_hms(hour, minute, value),
        }
        .expect("clamped time field stays valid");
        let changed = next != self.draft;
        self.draft = next;
        changed
    }

    fn active_field_max(&self) -> u8 {
        match self.active_field {
            TimeField::Hour => 23,
            TimeField::Minute | TimeField::Second => 59,
        }
    }

    fn active_field_page_step(&self) -> u8 {
        match self.active_field {
            TimeField::Hour => 6,
            TimeField::Minute => self.minute_step,
            TimeField::Second => 15,
        }
    }

    fn should_advance_after_typing(&self) -> bool {
        matches!(
            (self.active_field, self.precision),
            (TimeField::Hour, _) | (TimeField::Minute, TimePrecision::HourMinuteSecond)
        )
    }

    fn previous_field(&self) -> TimeField {
        match (self.active_field, self.precision) {
            (TimeField::Hour, TimePrecision::HourMinute) => TimeField::Minute,
            (TimeField::Hour, TimePrecision::HourMinuteSecond) => TimeField::Second,
            (TimeField::Minute, _) => TimeField::Hour,
            (TimeField::Second, _) => TimeField::Minute,
        }
    }

    fn next_field(&self) -> TimeField {
        match (self.active_field, self.precision) {
            (TimeField::Hour, _) => TimeField::Minute,
            (TimeField::Minute, TimePrecision::HourMinute) => TimeField::Hour,
            (TimeField::Minute, TimePrecision::HourMinuteSecond) => TimeField::Second,
            (TimeField::Second, _) => TimeField::Hour,
        }
    }
}

impl<M> Default for TimePicker<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> TuiNode<M> for TimePicker<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        picker_size_hint(self.time_line_width(), 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(TIME_PICKER_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(TIME_PICKER_FOCUS), area, true);
        }
        ctx.set_focus_receives_events_before_global_hotkeys(FocusId::new(TIME_PICKER_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            match hotkey {
                HotkeyEvent::Pending(prefix) => {
                    self.pending_hotkey_prefix = Some(prefix.clone());
                    ctx.request_redraw();
                    return EventOutcome::Ignored;
                }
                HotkeyEvent::Canceled => {
                    if self.pending_hotkey_prefix.take().is_some() {
                        ctx.request_redraw();
                    }
                    return EventOutcome::Ignored;
                }
                HotkeyEvent::Commit(sequence) => {
                    self.pending_hotkey_prefix = None;
                    if self.hotkey.as_deref().is_some_and(|hotkey| {
                        crate::hotkey::normalize_hotkey(hotkey)
                            == crate::hotkey::normalize_hotkey(sequence)
                    }) {
                        ctx.request_redraw();
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    return EventOutcome::Ignored;
                }
            }
        }
        if let TuiEvent::ExternalEditor(response) = event {
            if let Some(time) = super::parse_editor_time(&response.value) {
                self.set_value(time);
                if let Some(on_select) = &self.on_select {
                    ctx.emit(on_select(self.value));
                }
            }
            ctx.request_clear();
            ctx.request_redraw();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if keybindings()
            .date_time_picker()
            .external_editor_matches(*key)
        {
            let value = super::format_picker_time(self.draft);
            let col = match self.active_field {
                TimeField::Hour => 1,
                TimeField::Minute => 4,
                TimeField::Second => 7,
            };
            ctx.request_external_editor(value.clone(), 1, col);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        if outcome.selected
            && let Some(on_select) = &self.on_select
        {
            ctx.emit(on_select(self.value));
        }
        finish_event(ctx, outcome)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, _dt: StdDuration, _settings: crate::AnimationSettings) -> TickResult {
        TickResult::IDLE
    }
}

#[cfg(test)]
mod tests;
