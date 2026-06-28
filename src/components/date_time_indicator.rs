use std::marker::PhantomData;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use time::OffsetDateTime;

use super::status_action::{StatusAction, measured_line, register_status_focus};
use crate::{
    Animated, AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, TickResult, TuiNode,
};

const DATE_TIME_FOCUS: &str = "date-time-indicator";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateTimeIndicatorFormat {
    #[default]
    Time,
    DateTime,
}

pub struct DateTimeIndicator<M = ()> {
    icon: String,
    ascii_icon: String,
    use_ascii_icon: bool,
    time_text: String,
    format: DateTimeIndicatorFormat,
    auto_update: bool,
    update_wait: Duration,
    action: StatusAction<M>,
    _marker: PhantomData<fn() -> M>,
}

impl<M> DateTimeIndicator<M> {
    pub fn new() -> Self {
        Self {
            icon: "".to_string(),
            ascii_icon: "clock".to_string(),
            use_ascii_icon: false,
            time_text: format_datetime(local_datetime(), DateTimeIndicatorFormat::Time),
            format: DateTimeIndicatorFormat::Time,
            auto_update: true,
            update_wait: next_update_wait(local_datetime()),
            action: StatusAction::new(),
            _marker: PhantomData,
        }
    }

    pub fn format(mut self, format: DateTimeIndicatorFormat) -> Self {
        self.format = format;
        let now = local_datetime();
        self.time_text = format_datetime(now, format);
        self.update_wait = next_update_wait(now);
        self.auto_update = true;
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    pub fn ascii_icon(mut self, icon: impl Into<String>) -> Self {
        self.ascii_icon = icon.into();
        self
    }

    pub fn use_ascii_icon(mut self, use_ascii_icon: bool) -> Self {
        self.use_ascii_icon = use_ascii_icon;
        self
    }

    pub fn time_text(mut self, text: impl Into<String>) -> Self {
        self.time_text = text.into();
        self.auto_update = false;
        self.update_wait = Duration::ZERO;
        self
    }

    pub fn set_time_text(&mut self, text: impl Into<String>) {
        self.time_text = text.into();
        self.auto_update = false;
        self.update_wait = Duration::ZERO;
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.action.set_hotkey(hotkey);
        self
    }

    pub fn clear_hotkey(&mut self) {
        self.action.clear_hotkey();
    }

    pub fn on_press(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.action.set_on_press(handler);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.action.set_focused_immediate(focused);
        self
    }

    pub fn is_focused(&self) -> bool {
        self.action.focused()
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        self.action.set_focused(focused, settings);
    }

    pub(crate) fn label(&self) -> String {
        let icon = if self.use_ascii_icon {
            &self.ascii_icon
        } else {
            &self.icon
        };
        format!("{icon} {}", self.time_text)
    }

    fn line(&self) -> Line<'static> {
        self.action.line(self.label())
    }

    pub(crate) fn label_spans(&self, base_style: Style, hotkey_style: Style) -> Vec<Span<'static>> {
        self.action
            .label_spans(self.label(), base_style, hotkey_style)
    }
}

impl<M> Default for DateTimeIndicator<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TuiNode<M> for DateTimeIndicator<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        measured_line(self.line(), proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if self.action.hotkey().is_some() || self.action.has_press_handler() {
            register_status_focus(ctx, DATE_TIME_FOCUS, area, self.action.hotkey());
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        frame.render_widget(Paragraph::new(self.line()), area);
    }

    fn event(&mut self, event: &crate::event::TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.action.event(event, ctx)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if self.auto_update {
            ctx.request_redraw();
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        if !self.auto_update {
            return self.action.tick(dt, settings);
        }
        let action_tick = self.action.tick(dt, settings);
        if self.update_wait > dt {
            self.update_wait -= dt;
            return action_tick;
        }

        let now = local_datetime();
        let next = format_datetime(now, self.format);
        self.update_wait = next_update_wait(now);
        if next != self.time_text {
            self.time_text = next;
            TickResult::CHANGED.merge(action_tick)
        } else {
            action_tick
        }
    }
}

fn local_datetime() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn next_update_wait(datetime: OffsetDateTime) -> Duration {
    Duration::from_secs(
        60_u64
            .saturating_sub(datetime.time().second() as u64)
            .max(1),
    )
}

fn format_datetime(datetime: OffsetDateTime, format: DateTimeIndicatorFormat) -> String {
    let time = datetime.time();
    let clock = format!("{:02}:{:02}", time.hour(), time.minute());
    match format {
        DateTimeIndicatorFormat::Time => clock,
        DateTimeIndicatorFormat::DateTime => format!("{} {clock}", datetime.date()),
    }
}
