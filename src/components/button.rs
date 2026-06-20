use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Easing, Tween};
use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, EventCtx, EventOutcome, FocusCtx,
    FocusId, HintSource, HotkeyEvent, HotkeyLabelMode, HotkeyMatch, HotkeySequenceMatcher,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint, TickResult, TuiNode,
    hotkey_label_spans, hotkey_sequence_to_event, keybindings, line_width, theme,
};

const BUTTON_FOCUS: &str = "button";
const PRESS_FEEDBACK: Duration = Duration::from_millis(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonOutcome {
    pub handled: bool,
    pub pressed: bool,
}

impl ButtonOutcome {
    pub const IGNORED: Self = Self {
        handled: false,
        pressed: false,
    };

    pub const PRESSED: Self = Self {
        handled: true,
        pressed: true,
    };
}

pub struct Button<M = ()> {
    label: String,
    hotkey: Option<String>,
    hotkey_label_mode: HotkeyLabelMode,
    hotkey_matcher: HotkeySequenceMatcher,
    pending_hotkey_prefix: Option<String>,
    focused: bool,
    on_press: Option<Box<dyn Fn() -> M>>,
    background_color: ColorTween,
    text_color: ColorTween,
    press_feedback: Tween,
}

impl<M> Button<M> {
    pub fn new(label: impl Into<String>) -> Self {
        let theme = theme();
        Self {
            label: label.into(),
            hotkey: None,
            hotkey_label_mode: HotkeyLabelMode::PreferMnemonic,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            pending_hotkey_prefix: None,
            focused: false,
            on_press: None,
            background_color: ColorTween::idle(theme.border_fg()),
            text_color: ColorTween::idle(theme.text_fg()),
            press_feedback: Tween::idle(0.0),
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = label.into();
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

    pub fn hotkey_label_mode(mut self, mode: HotkeyLabelMode) -> Self {
        self.hotkey_label_mode = mode;
        self
    }

    pub fn set_hotkey_label_mode(&mut self, mode: HotkeyLabelMode) {
        self.hotkey_label_mode = mode;
    }

    pub fn on_press(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_press = Some(Box::new(handler));
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

    pub fn press(&mut self, settings: AnimationSettings) -> ButtonOutcome {
        if settings.enabled {
            self.press_feedback
                .start(1.0, 0.0, PRESS_FEEDBACK, Easing::EaseOutCubic);
        }
        ButtonOutcome::PRESSED
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> ButtonOutcome {
        self.on_key_with_settings(key, AnimationSettings::default())
    }

    pub fn on_key_with_settings(
        &mut self,
        key: impl Into<KeyEvent>,
        settings: AnimationSettings,
    ) -> ButtonOutcome {
        let key = key.into();
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) => return self.press(settings),
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                return ButtonOutcome {
                    handled: true,
                    pressed: false,
                };
            }
            HotkeyMatch::Ignored => {}
        }
        if self
            .hotkey_event()
            .is_some_and(|hotkey| keys_match(hotkey, key))
            || keybindings().button().press_matches(key)
        {
            return self.press(settings);
        }
        ButtonOutcome::IGNORED
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        frame.render_widget(Paragraph::new(self.line()), area);
    }

    fn line(&self) -> Line<'static> {
        let background = self.visible_background_color();
        let text_style = Style::default()
            .fg(self.visible_text_color())
            .bg(background)
            .add_modifier(if self.focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let mut spans = vec![Span::styled(" ", text_style)];
        let active_prefix = if self.hotkey_matcher.prefix().is_empty() {
            self.pending_hotkey_prefix.as_deref().unwrap_or("")
        } else {
            self.hotkey_matcher.prefix()
        };
        spans.extend(hotkey_label_spans(
            &self.label,
            self.hotkey.as_deref(),
            self.hotkey_label_mode,
            Some(active_prefix),
            text_style,
            crate::hotkey_underline_style(text_style),
        ));
        spans.push(Span::styled(" ", text_style));

        Line::from(spans)
    }

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_deref().and_then(hotkey_sequence_to_event)
    }

    fn sync_idle_colors(&mut self) {
        let theme = theme();
        self.background_color.snap_to(if self.focused {
            theme.highlight_bg()
        } else {
            theme.border_fg()
        });
        self.text_color.snap_to(if self.focused {
            theme.highlight_fg()
        } else {
            theme.text_fg()
        });
    }

    fn start_color_transition(&mut self, settings: AnimationSettings) {
        let theme = theme();
        self.background_color.start(
            if self.focused {
                theme.highlight_bg()
            } else {
                theme.border_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.text_color.start(
            if self.focused {
                theme.highlight_fg()
            } else {
                theme.text_fg()
            },
            settings,
            focus_color_animation(),
        );
    }

    fn is_showing_press_feedback(&self) -> bool {
        self.press_feedback.is_active() || self.press_feedback.value() > 0.0
    }

    fn visible_background_color(&self) -> Color {
        if self.is_showing_press_feedback() {
            theme().success_fg()
        } else if self.background_color.is_active() {
            self.background_color.value()
        } else if self.focused {
            theme().highlight_bg()
        } else {
            theme().border_fg()
        }
    }

    fn visible_text_color(&self) -> Color {
        if self.text_color.is_active() {
            self.text_color.value()
        } else if self.focused {
            theme().highlight_fg()
        } else {
            theme().text_fg()
        }
    }
}

impl<M> Default for Button<M> {
    fn default() -> Self {
        Self::new("")
    }
}

impl<M> TuiNode<M> for Button<M>
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
                FocusId::new(BUTTON_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(BUTTON_FOCUS), area, true);
        }
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
                    if self
                        .hotkey
                        .as_deref()
                        .is_some_and(|hotkey| hotkey_matches_sequence(hotkey, sequence))
                    {
                        self.press(ctx.animation());
                        if let Some(on_press) = &self.on_press {
                            ctx.emit(on_press());
                        }
                        ctx.request_redraw();
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
        let outcome = self.on_key_with_settings(*key, ctx.animation());
        if outcome.pressed {
            if let Some(on_press) = &self.on_press {
                ctx.emit(on_press());
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

impl<M> Animated for Button<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        self.background_color
            .tick(dt, settings)
            .merge(self.text_color.tick(dt, settings))
            .merge(self.press_feedback.tick(dt, settings))
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
    fn enter_presses_button_and_stops_propagation() {
        let mut button = Button::<()>::new("Run");
        let mut ctx = EventCtx::default();

        let outcome = button.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn hotkey_registers_and_presses() {
        let mut button = Button::<()>::new("Run").hotkey("b");
        let mut layout = LayoutCtx::new();
        button.layout(Rect::new(0, 0, 20, 1), &mut layout);

        assert_eq!(layout.focus_targets()[0].hotkey_sequences, vec!["b"]);

        let outcome = button.on_key(KeyEvent::from(Key::Char('b')));

        assert!(outcome.handled);
        assert!(outcome.pressed);
    }

    #[test]
    fn visual_hotkey_events_do_not_count_as_button_presses() {
        let mut button = Button::<()>::new("Run").hotkey("b");
        let mut ctx = EventCtx::default();

        let pending = button.event(
            &TuiEvent::Hotkey(HotkeyEvent::Pending("b".to_string())),
            &mut ctx,
        );
        let canceled = button.event(&TuiEvent::Hotkey(HotkeyEvent::Canceled), &mut ctx);

        assert_eq!(pending, EventOutcome::Ignored);
        assert_eq!(canceled, EventOutcome::Ignored);
        assert_eq!(ctx.propagation(), Propagation::Continue);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn multiletter_hotkey_waits_for_completion() {
        let mut button = Button::<()>::new("Open").hotkey("op");

        let pending = button.on_key(KeyEvent::from(Key::Char('o')));
        let matched = button.on_key(KeyEvent::from(Key::Char('p')));

        assert!(pending.handled);
        assert!(!pending.pressed);
        assert!(matched.handled);
        assert!(matched.pressed);
    }

    #[test]
    fn normalized_hotkey_commit_presses_button() {
        let mut button = Button::<()>::new("Save").hotkey("S");
        let mut ctx = EventCtx::default();

        let outcome = button.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("s".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn whitespace_hotkey_commit_presses_button() {
        let mut button = Button::<()>::new("Go").hotkey("g g");
        let mut ctx = EventCtx::default();

        let outcome = button.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("gg".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn renders_label_and_hotkey() {
        let button = Button::<()>::new("button").hotkey("b");
        let mut terminal = Terminal::new(TestBackend::new(32, 1)).expect("terminal should build");

        terminal
            .draw(|frame| button.render(frame, frame.area()))
            .expect("button should render");

        let buffer = terminal.backend().buffer();
        let row = (0..32)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.starts_with(" button "));
        assert!(!row.contains(""));
        assert!(!row.contains(""));
    }

    #[test]
    fn inline_hotkey_mode_keeps_suffix() {
        let button = Button::<()>::new("button")
            .hotkey("b")
            .hotkey_label_mode(HotkeyLabelMode::Inline);
        let mut terminal = Terminal::new(TestBackend::new(32, 1)).expect("terminal should build");

        terminal
            .draw(|frame| button.render(frame, frame.area()))
            .expect("button should render");

        let buffer = terminal.backend().buffer();
        let row = (0..32)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(row.starts_with(" button |b| "));
    }

    #[test]
    fn pressed_button_uses_color_feedback_without_text_suffix() {
        let mut button = Button::<()>::new("Run").hotkey("b");

        button.on_key(KeyEvent::from(Key::Char('b')));

        assert!(button.is_showing_press_feedback());
        assert!(!line_text(button.line()).contains("pressed"));

        Animated::tick(
            &mut button,
            Duration::from_millis(100),
            AnimationSettings::default(),
        );
        Animated::tick(
            &mut button,
            Duration::from_millis(100),
            AnimationSettings::default(),
        );

        assert!(!button.is_showing_press_feedback());
        assert!(!line_text(button.line()).contains("pressed"));
    }

    fn line_text(line: Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
}
