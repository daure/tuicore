use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, EventCtx, EventOutcome, FocusCtx,
    FocusId, HintSource, HotkeyEvent, HotkeyMatch, HotkeySequenceMatcher, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, TickResult, TuiNode,
    hotkey_sequence_to_event, keybindings, line_width, theme,
};

const TOGGLE_FOCUS: &str = "toggle";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToggleStyle {
    #[default]
    Switch,
    Checkbox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToggleOutcome {
    pub handled: bool,
    pub changed: bool,
    pub value: bool,
}

impl ToggleOutcome {
    pub const fn ignored(value: bool) -> Self {
        Self {
            handled: false,
            changed: false,
            value,
        }
    }

    pub const fn changed(value: bool) -> Self {
        Self {
            handled: true,
            changed: true,
            value,
        }
    }
}

pub struct Toggle<M = ()> {
    label: String,
    value: bool,
    style: ToggleStyle,
    hotkey: Option<String>,
    hotkey_matcher: HotkeySequenceMatcher,
    focused: bool,
    on_change: Option<Box<dyn Fn(bool) -> M>>,
    switch_color: ColorTween,
    text_color: ColorTween,
    hotkey_color: ColorTween,
    pending_hotkey_prefix: Option<String>,
}

impl<M> Toggle<M> {
    pub fn new(label: impl Into<String>) -> Self {
        let theme = theme();
        Self {
            label: label.into(),
            value: false,
            style: ToggleStyle::default(),
            hotkey: None,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            focused: false,
            on_change: None,
            switch_color: ColorTween::idle(theme.muted_fg()),
            text_color: ColorTween::idle(theme.text_fg()),
            hotkey_color: ColorTween::idle(theme.text_fg()),
            pending_hotkey_prefix: None,
        }
    }

    pub fn checked(mut self, value: bool) -> Self {
        self.value = value;
        self.sync_idle_colors();
        self
    }

    pub fn value(mut self, value: bool) -> Self {
        self.value = value;
        self.sync_idle_colors();
        self
    }

    pub fn set_value(&mut self, value: bool) {
        self.value = value;
        self.sync_idle_colors();
    }

    pub fn is_checked(&self) -> bool {
        self.value
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
    }

    pub fn style(mut self, style: ToggleStyle) -> Self {
        self.style = style;
        self
    }

    pub fn set_style(&mut self, style: ToggleStyle) {
        self.style = style;
    }

    pub fn checkbox(mut self) -> Self {
        self.style = ToggleStyle::Checkbox;
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.hotkey_matcher = HotkeySequenceMatcher::default();
    }

    pub fn on_change(mut self, handler: impl Fn(bool) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self.sync_idle_colors();
        self
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.start_color_transition(settings);
    }

    pub fn toggle(&mut self) -> ToggleOutcome {
        self.value = !self.value;
        if !self.focused {
            self.sync_idle_colors();
        }
        ToggleOutcome::changed(self.value)
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> ToggleOutcome {
        let key = key.into();
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) => return self.toggle(),
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                return ToggleOutcome {
                    handled: true,
                    changed: false,
                    value: self.value,
                };
            }
            HotkeyMatch::Ignored => {}
        }
        if self
            .hotkey_event()
            .is_some_and(|hotkey| keys_match(hotkey, key))
            || keybindings().toggle().toggle_matches(key)
        {
            return self.toggle();
        }
        ToggleOutcome::ignored(self.value)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        frame.render_widget(Paragraph::new(self.line()), area);
    }

    fn line(&self) -> Line<'static> {
        let switch = match (self.style, self.value) {
            (ToggleStyle::Switch, true) => "──●",
            (ToggleStyle::Switch, false) => "○──",
            (ToggleStyle::Checkbox, true) => "[x]",
            (ToggleStyle::Checkbox, false) => "[ ]",
        };
        let switch_style = Style::default()
            .fg(self.visible_switch_color())
            .add_modifier(if self.focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let mut spans = vec![
            Span::styled(switch, switch_style),
            Span::raw(" "),
            Span::styled(
                self.label.clone(),
                Style::default().fg(self.visible_text_color()),
            ),
        ];

        if let Some(ref hotkey) = self.hotkey {
            let base_style = Style::default().fg(self.visible_hotkey_color());
            let highlight_style = base_style.add_modifier(Modifier::BOLD);
            let hotkey = crate::hotkey::normalize_hotkey(hotkey);
            let highlight = self
                .pending_hotkey_prefix
                .as_ref()
                .is_some_and(|prefix| crate::hotkey::normalize_hotkey(&hotkey).starts_with(prefix));
            spans.push(Span::raw(" "));
            spans.push(Span::styled("|", base_style));
            spans.push(Span::styled(
                hotkey,
                if highlight {
                    highlight_style
                } else {
                    base_style
                },
            ));
            spans.push(Span::styled("|", base_style));
        }

        Line::from(spans)
    }

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_deref().and_then(hotkey_sequence_to_event)
    }

    fn visible_switch_color(&self) -> ratatui::style::Color {
        if self.switch_color.is_active() {
            return self.switch_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.highlight_bg()
        } else if self.value {
            theme.success_fg()
        } else {
            theme.muted_fg()
        }
    }

    fn visible_text_color(&self) -> ratatui::style::Color {
        if self.text_color.is_active() {
            return self.text_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.highlight_bg()
        } else {
            theme.text_fg()
        }
    }

    fn visible_hotkey_color(&self) -> ratatui::style::Color {
        if self.hotkey_color.is_active() {
            return self.hotkey_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.highlight_bg()
        } else {
            theme.text_fg()
        }
    }

    fn sync_idle_colors(&mut self) {
        let theme = theme();
        self.switch_color.snap_to(if self.focused {
            theme.highlight_bg()
        } else if self.value {
            theme.success_fg()
        } else {
            theme.muted_fg()
        });
        self.text_color.snap_to(if self.focused {
            theme.highlight_bg()
        } else {
            theme.text_fg()
        });
        self.hotkey_color.snap_to(if self.focused {
            theme.highlight_bg()
        } else {
            theme.text_fg()
        });
    }

    fn start_color_transition(&mut self, settings: AnimationSettings) {
        let theme = theme();
        self.switch_color.start(
            if self.focused {
                theme.highlight_bg()
            } else if self.value {
                theme.success_fg()
            } else {
                theme.muted_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.text_color.start(
            if self.focused {
                theme.highlight_bg()
            } else {
                theme.text_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.hotkey_color.start(
            if self.focused {
                theme.highlight_bg()
            } else {
                theme.text_fg()
            },
            settings,
            focus_color_animation(),
        );
    }
}

impl<M> Default for Toggle<M> {
    fn default() -> Self {
        Self::new("")
    }
}

impl<M> TuiNode<M> for Toggle<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = line_width(&self.line()).min(u16::MAX as usize) as u16;
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(width, 1),
            preferred: LayoutSize::new(width, 1),
            expand: crate::AxisExpand {
                width: false,
                height: false,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(TOGGLE_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(TOGGLE_FOCUS), area, true);
        }
        ctx.set_focus_control(FocusId::new(TOGGLE_FOCUS), true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
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
                    if self
                        .hotkey
                        .as_deref()
                        .is_some_and(|hotkey| hotkey_matches_sequence(hotkey, sequence))
                    {
                        let outcome = self.toggle();
                        if outcome.changed {
                            if let Some(on_change) = &self.on_change {
                                ctx.emit(on_change(self.value));
                            }
                            ctx.request_redraw();
                        }
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    return EventOutcome::Ignored;
                }
            }
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let outcome = self.on_key(*key);
        if outcome.changed {
            if let Some(on_change) = &self.on_change {
                ctx.emit(on_change(self.value));
            }
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<M> Animated for Toggle<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        self.switch_color
            .tick(dt, settings)
            .merge(self.text_color.tick(dt, settings))
            .merge(self.hotkey_color.tick(dt, settings))
            .merge(hotkey_tick)
    }
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn keys_match(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
        (a, b) => a == b,
    }
}

fn hotkey_matches_sequence(hotkey: &str, sequence: &str) -> bool {
    crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::Propagation;

    #[test]
    fn toggle_key_flips_value_and_stops_propagation() {
        let mut toggle = Toggle::<()>::new("Telemetry");
        let mut ctx = EventCtx::default();

        let outcome = toggle.event(&TuiEvent::Key(KeyEvent::from(Key::Char(' '))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(toggle.is_checked());
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn hotkey_registers_and_toggles() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("x");
        let mut layout = LayoutCtx::new();
        toggle.layout(Rect::new(0, 0, 20, 1), &mut layout);

        assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["x"]);

        let outcome = toggle.on_key(KeyEvent::from(Key::Char('x')));

        assert!(outcome.handled);
        assert!(toggle.is_checked());
    }

    #[test]
    fn uppercase_hotkey_commit_toggles() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("X");
        let mut ctx = EventCtx::default();

        let outcome = toggle.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("x".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert!(toggle.is_checked());
    }

    #[test]
    fn multiletter_hotkey_toggles_after_direct_sequence() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("st");

        let pending = toggle.on_key(KeyEvent::from(Key::Char('s')));
        let matched = toggle.on_key(KeyEvent::from(Key::Char('t')));

        assert!(pending.handled);
        assert!(!pending.changed);
        assert!(matched.handled);
        assert!(matched.changed);
        assert!(toggle.is_checked());
    }

    #[test]
    fn focused_multiletter_hotkey_toggles_from_key_events() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("st").focused(true);
        let mut ctx = EventCtx::default();

        let pending = toggle.event(&TuiEvent::Key(KeyEvent::from(Key::Char('s'))), &mut ctx);
        let matched = toggle.event(&TuiEvent::Key(KeyEvent::from(Key::Char('t'))), &mut ctx);

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert!(toggle.is_checked());
    }

    #[test]
    fn renders_compact_switch_label_and_hotkey() {
        let toggle = Toggle::<()>::new("Telemetry").hotkey("x");
        let mut terminal = Terminal::new(TestBackend::new(24, 1)).expect("terminal should build");

        terminal
            .draw(|frame| toggle.render(frame, frame.area()))
            .expect("toggle should render");

        let buffer = terminal.backend().buffer();
        let row = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.contains("○── Telemetry |x|"));
    }

    #[test]
    fn renders_checkbox_style_when_unchecked() {
        let toggle = Toggle::<()>::new("Telemetry").style(ToggleStyle::Checkbox);
        let mut terminal = Terminal::new(TestBackend::new(16, 1)).expect("terminal should build");

        terminal
            .draw(|frame| toggle.render(frame, frame.area()))
            .expect("toggle should render");

        let buffer = terminal.backend().buffer();
        let row = (0..16)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.contains("[ ] Telemetry"));
    }

    #[test]
    fn renders_checkbox_style_when_checked() {
        let toggle = Toggle::<()>::new("Telemetry").checkbox().checked(true);
        let mut terminal = Terminal::new(TestBackend::new(16, 1)).expect("terminal should build");

        terminal
            .draw(|frame| toggle.render(frame, frame.area()))
            .expect("toggle should render");

        let buffer = terminal.backend().buffer();
        let row = (0..16)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.contains("[x] Telemetry"));
    }

    #[test]
    fn focus_and_blur_tween_all_visible_colors() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("x");
        let theme = theme();
        let switch_will_change = toggle.switch_color.value() != theme.highlight_bg();
        let text_will_change = toggle.text_color.value() != theme.highlight_bg();
        let hotkey_will_change = toggle.hotkey_color.value() != theme.highlight_bg();

        toggle.set_focused(true, AnimationSettings::default());

        assert_eq!(toggle.switch_color.is_active(), switch_will_change);
        assert_eq!(toggle.text_color.is_active(), text_will_change);
        assert_eq!(toggle.hotkey_color.is_active(), hotkey_will_change);

        let switch_will_change = toggle.switch_color.value() != theme.muted_fg();
        let text_will_change = toggle.text_color.value() != theme.text_fg();
        let hotkey_will_change = toggle.hotkey_color.value() != theme.text_fg();

        toggle.set_focused(false, AnimationSettings::default());

        assert_eq!(toggle.switch_color.is_active(), switch_will_change);
        assert_eq!(toggle.text_color.is_active(), text_will_change);
        assert_eq!(toggle.hotkey_color.is_active(), hotkey_will_change);
    }

    #[test]
    fn checked_blurred_toggle_does_not_use_focus_color() {
        let toggle = Toggle::<()>::new("Telemetry").checked(true);
        let theme = theme();

        assert_eq!(toggle.switch_color.value(), theme.success_fg());
    }

    #[test]
    fn focused_toggle_uses_highlight_role_and_bold_switch_only() {
        let focused = Toggle::<()>::new("Telemetry").focused(true).line();
        let unfocused = Toggle::<()>::new("Telemetry").line();

        assert_eq!(focused.spans[0].style.fg, Some(theme().highlight_bg()));
        assert_eq!(focused.spans[2].style.fg, Some(theme().highlight_bg()));
        assert!(focused.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(
            !unfocused.spans[0]
                .style
                .add_modifier
                .contains(Modifier::BOLD)
        );
    }

    #[test]
    fn blurred_hotkey_uses_label_color() {
        let toggle = Toggle::<()>::new("Telemetry").hotkey("x");

        assert_eq!(toggle.hotkey_color.value(), toggle.text_color.value());
    }

    #[test]
    fn toggling_while_focused_preserves_focus_tween() {
        let mut toggle = Toggle::<()>::new("Telemetry").hotkey("x");
        toggle.set_focused(true, AnimationSettings::default());
        let switch_active = toggle.switch_color.is_active();
        let text_active = toggle.text_color.is_active();
        let hotkey_active = toggle.hotkey_color.is_active();

        toggle.on_key(KeyEvent::from(Key::Char('x')));

        assert_eq!(toggle.switch_color.is_active(), switch_active);
        assert_eq!(toggle.text_color.is_active(), text_active);
        assert_eq!(toggle.hotkey_color.is_active(), hotkey_active);
    }
}
