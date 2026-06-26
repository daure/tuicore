use std::time::Duration;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::event::{Key, KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AxisExpand, ButtonOutcome, ColorTween, EventCtx, EventOutcome,
    FocusId, HintSource, HotkeyEvent, HotkeyLabelMode, HotkeyMatch, HotkeySequenceMatcher,
    LayoutCtx, LayoutProposal, LayoutSize, LayoutSizeHint, TickResult, hotkey_label_spans,
    hotkey_sequence_to_event, keybindings, line_width, theme,
};

pub(super) struct StatusAction<M> {
    hotkey: Option<String>,
    hotkey_label_mode: HotkeyLabelMode,
    hotkey_matcher: HotkeySequenceMatcher,
    pending_hotkey_prefix: Option<String>,
    focused: bool,
    on_press: Option<Box<dyn Fn() -> M>>,
    background_color: ColorTween,
    text_color: ColorTween,
}

impl<M> StatusAction<M> {
    pub(super) fn new() -> Self {
        let theme = theme();
        Self {
            hotkey: None,
            hotkey_label_mode: HotkeyLabelMode::PreferMnemonic,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            pending_hotkey_prefix: None,
            focused: false,
            on_press: None,
            background_color: ColorTween::idle(theme.border_fg()),
            text_color: ColorTween::idle(theme.text_fg()),
        }
    }

    pub(super) fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
    }

    pub(super) fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.hotkey_matcher = HotkeySequenceMatcher::default();
        self.pending_hotkey_prefix = None;
    }

    pub(super) fn hotkey(&self) -> Option<String> {
        self.hotkey.clone()
    }

    pub(super) fn has_press_handler(&self) -> bool {
        self.on_press.is_some()
    }

    pub(super) fn set_on_press(&mut self, handler: impl Fn() -> M + 'static) {
        self.on_press = Some(Box::new(handler));
    }

    pub(super) fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        let theme = theme();
        self.background_color.start(
            if focused {
                theme.highlight_bg()
            } else {
                theme.border_fg()
            },
            settings,
            Default::default(),
        );
        self.text_color.start(
            if focused {
                theme.highlight_fg()
            } else {
                theme.text_fg()
            },
            settings,
            Default::default(),
        );
    }

    pub(super) fn focused(&self) -> bool {
        self.focused
    }

    pub(super) fn set_focused_immediate(&mut self, focused: bool) {
        self.focused = focused;
        self.sync_idle_colors();
    }

    pub(super) fn line(&self, label: String) -> Line<'static> {
        let mut style =
            Style::default()
                .fg(self.visible_text_color())
                .add_modifier(if self.focused {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                });
        if self.focused {
            style = style.bg(self.visible_background_color());
        }
        let mut spans = vec![Span::styled(" ", style)];
        spans.extend(self.label_spans(label, style, crate::hotkey_underline_style(style)));
        spans.push(Span::styled(" ", style));
        Line::from(spans)
    }

    pub(super) fn label_spans(
        &self,
        label: String,
        base_style: Style,
        hotkey_style: Style,
    ) -> Vec<Span<'static>> {
        let active_prefix = if self.hotkey_matcher.prefix().is_empty() {
            self.pending_hotkey_prefix.as_deref().unwrap_or("")
        } else {
            self.hotkey_matcher.prefix()
        };
        hotkey_label_spans(
            &label,
            self.hotkey.as_deref(),
            self.hotkey_label_mode,
            Some(active_prefix),
            base_style,
            hotkey_style,
        )
    }

    pub(super) fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome
    where
        M: 'static,
    {
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
                        self.emit_press(ctx);
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
        if outcome.pressed {
            self.emit_press(ctx);
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn on_key(&mut self, key: KeyEvent) -> ButtonOutcome {
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) => return ButtonOutcome::PRESSED,
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                return ButtonOutcome {
                    handled: true,
                    pressed: false,
                };
            }
            HotkeyMatch::Ignored => {}
        }
        let press = self
            .hotkey
            .as_deref()
            .and_then(hotkey_sequence_to_event)
            .is_some_and(|hotkey| keys_match(hotkey, key))
            || keybindings().button().press_matches(key);
        if press {
            ButtonOutcome::PRESSED
        } else {
            ButtonOutcome::IGNORED
        }
    }

    fn emit_press(&self, ctx: &mut EventCtx<M>)
    where
        M: 'static,
    {
        if let Some(on_press) = &self.on_press {
            ctx.emit(on_press());
        }
        ctx.request_redraw();
        ctx.stop_propagation();
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

    fn visible_background_color(&self) -> ratatui::style::Color {
        if self.background_color.is_active() {
            self.background_color.value()
        } else if self.focused {
            theme().highlight_bg()
        } else {
            theme().border_fg()
        }
    }

    fn visible_text_color(&self) -> ratatui::style::Color {
        if self.text_color.is_active() {
            self.text_color.value()
        } else if self.focused {
            theme().highlight_fg()
        } else {
            theme().text_fg()
        }
    }
}

impl<M> Animated for StatusAction<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        self.background_color
            .tick(dt, settings)
            .merge(self.text_color.tick(dt, settings))
            .merge(hotkey)
    }
}

pub(super) fn measured_line(line: Line<'_>, proposal: LayoutProposal) -> LayoutSizeHint {
    let width = line_width(&line).min(u16::MAX as usize) as u16;
    LayoutSizeHint {
        source: HintSource::Measured,
        min: LayoutSize::new(width, 1),
        preferred: LayoutSize::new(width, 1),
        expand: AxisExpand {
            width: false,
            height: false,
        },
    }
    .normalized(proposal)
}

pub(super) fn register_status_focus(
    ctx: &mut LayoutCtx,
    id: &str,
    area: Rect,
    hotkey: Option<String>,
) {
    if let Some(hotkey) = hotkey {
        ctx.register_focusable_with_hotkey_sequences(FocusId::new(id), area, true, vec![hotkey]);
    } else {
        ctx.register_focusable(FocusId::new(id), area, true);
    }
}

fn keys_match(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.eq_ignore_ascii_case(&b),
        (a, b) => a == b,
    }
}
