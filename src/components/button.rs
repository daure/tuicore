use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Easing, Tween};
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, EventCtx, EventOutcome, FocusCtx,
    FocusId, HintSource, LayoutCtx, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint,
    TickResult, TuiNode, keybindings, line_width, theme,
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
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
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
        let cap_style = Style::default().fg(background);
        let text_style = Style::default()
            .fg(self.text_color.value())
            .bg(background)
            .add_modifier(if self.focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        let label = match &self.hotkey {
            Some(hotkey) => format!("{} ({hotkey})", self.label),
            None => self.label.clone(),
        };

        let mut spans = vec![
            Span::styled("", cap_style),
            Span::styled(label, text_style),
            Span::styled("", cap_style),
        ];

        if self.is_showing_press_feedback() {
            spans.push(Span::raw(" ← pressed"));
        }

        Line::from(spans)
    }

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_ref()?.chars().next().map(|c| KeyEvent {
            code: Key::Char(c),
            modifiers: KeyModifiers::NONE,
        })
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
        } else {
            self.background_color.value()
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
        if let Some(hotkey) = self.hotkey_event() {
            ctx.register_focusable_with_hotkey(FocusId::new(BUTTON_FOCUS), area, true, hotkey);
        } else {
            ctx.register_focusable(FocusId::new(BUTTON_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
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
        self.background_color
            .tick(dt, settings)
            .merge(self.text_color.tick(dt, settings))
            .merge(self.press_feedback.tick(dt, settings))
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

        assert_eq!(
            layout.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('b')))
        );

        let outcome = button.on_key(KeyEvent::from(Key::Char('b')));

        assert!(outcome.handled);
        assert!(outcome.pressed);
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
        assert!(row.contains("button (b)"));
    }

    #[test]
    fn pressed_button_shows_feedback_until_tick_completes() {
        let mut button = Button::<()>::new("Run").hotkey("b");

        button.on_key(KeyEvent::from(Key::Char('b')));

        assert!(line_text(button.line()).contains("pressed"));

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

        assert!(!line_text(button.line()).contains("pressed"));
    }

    fn line_text(line: Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }
}
