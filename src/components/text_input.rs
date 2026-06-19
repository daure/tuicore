use ratatui::Frame;
use ratatui::layout::Rect;
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, Easing, TickResult, lerp_color};
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::theme;
use crate::{EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx, LayoutResult, TuiNode};

const INPUT_FOCUS: &str = "input";
const CURSOR_FADE_HALF: Duration = Duration::from_millis(600);
const TAB_WIDTH: usize = 4;
const TAB_INSERT: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputOutcome {
    pub handled: bool,
    pub changed: bool,
    pub submitted: bool,
    pub canceled: bool,
    pub clear: bool,
}

impl InputOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        submitted: false,
        canceled: false,
        clear: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        submitted: false,
        canceled: false,
        clear: false,
    };

    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
        submitted: false,
        canceled: false,
        clear: false,
    };

    pub const SUBMITTED: Self = Self {
        handled: true,
        changed: false,
        submitted: true,
        canceled: false,
        clear: false,
    };

    pub const CANCELED: Self = Self {
        handled: false,
        changed: false,
        submitted: false,
        canceled: true,
        clear: false,
    };

    pub fn needs_redraw(self) -> bool {
        self.handled || self.changed || self.submitted || self.canceled || self.clear
    }
}

pub struct TextInput<M = ()> {
    value: String,
    placeholder: String,
    cursor: usize,
    focused: bool,
    max_len: Option<usize>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_blur: Option<Box<dyn Fn(String) -> M>>,
    external_editor_key: Option<KeyEvent>,
    cursor_fade: CursorFade,
}

impl<M> Default for TextInput<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TextInput<M> {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            cursor: 0,
            focused: false,
            max_len: None,
            on_submit: None,
            on_blur: None,
            external_editor_key: Some(ctrl_key('o')),
            cursor_fade: CursorFade::default(),
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into().replace('\n', " ");
        self.clamp_value();
        self.cursor = self.len_chars();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn on_submit(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    pub fn on_blur(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.on_blur = Some(Box::new(handler));
        self
    }

    pub fn external_editor_key(mut self, key: Option<KeyEvent>) -> Self {
        self.external_editor_key = key;
        self
    }

    pub fn max_len(mut self, max_len: usize) -> Self {
        self.max_len = Some(max_len);
        self.clamp_value();
        self.cursor = self.cursor.min(self.len_chars());
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into().replace('\n', " ");
        self.clamp_value();
        self.cursor = self.cursor.min(self.len_chars());
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> InputOutcome {
        let outcome = self.on_key_inner(key.into());
        if outcome.needs_redraw() {
            self.cursor_fade.reset();
        }
        outcome
    }

    fn on_key_inner(&mut self, key: KeyEvent) -> InputOutcome {
        let key = key.into();
        if is_ctrl(key, 'a') {
            return self.move_to(0);
        }
        if is_ctrl(key, 'c') {
            return self.clear();
        }
        if is_ctrl(key, 'e') {
            return self.move_to(self.len_chars());
        }
        if is_ctrl(key, 'u') {
            return self.delete_before_cursor();
        }
        if is_ctrl(key, 'k') {
            return self.delete_after_cursor();
        }
        if is_ctrl(key, 'w') {
            return self.delete_previous_word();
        }
        if is_alt(key, 'b') {
            return self.move_previous_word();
        }
        if is_alt(key, 'f') {
            return self.move_next_word();
        }
        if is_alt(key, 'd') {
            return self.delete_next_word();
        }

        match key.code {
            _ if tab_key(key) => self.insert_text(TAB_INSERT),
            Key::Char(value) if text_char(key) => self.insert_char(value),
            Key::Backspace => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.delete_previous_word()
                } else {
                    self.backspace()
                }
            }
            Key::Delete => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.delete_next_word()
                } else {
                    self.delete_next()
                }
            }
            Key::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_previous_word()
                } else {
                    self.move_left()
                }
            }
            Key::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_next_word()
                } else {
                    self.move_right()
                }
            }
            Key::Home => self.move_to(0),
            Key::End => self.move_to(self.len_chars()),
            Key::Enter => InputOutcome::SUBMITTED,
            Key::Esc => InputOutcome::CANCELED,
            _ => InputOutcome::IDLE,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_with_style(frame, area, Style::default());
    }

    pub(crate) fn render_with_style(&self, frame: &mut Frame, area: Rect, style: Style) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(
            Paragraph::new(self.line(area.width as usize)).style(style),
            area,
        );
    }

    fn line(&self, width: usize) -> Line<'static> {
        if width == 0 {
            return Line::default();
        }

        let theme = theme();
        let value_style = Style::default().fg(if self.focused {
            theme.text_fg()
        } else {
            theme.subtle_fg()
        });
        let placeholder_style = Style::default().fg(theme.muted_fg());
        let cursor_style = self.cursor_fade.style(value_style);

        if self.value.is_empty() {
            return placeholder_line(
                &self.placeholder,
                width,
                self.focused,
                self.cursor_fade.style(placeholder_style),
                placeholder_style,
            );
        }

        let len = self.len_chars();
        let start = if self.focused && self.cursor >= width {
            self.cursor.saturating_add(1).saturating_sub(width)
        } else {
            0
        };
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut spans = Vec::new();
        let mut drawn = 0;

        for position in start..=len {
            if drawn >= width {
                break;
            }
            let remaining = width.saturating_sub(drawn);
            if self.focused && position == self.cursor {
                let text = display_char(chars.get(position).copied().unwrap_or(' '), remaining);
                drawn += text.len();
                spans.push(Span::styled(text, cursor_style));
                continue;
            }
            if let Some(value) = chars.get(position) {
                let text = display_char(*value, remaining);
                drawn += text.len();
                spans.push(Span::styled(text, value_style));
            }
        }

        Line::from(spans)
    }

    fn insert_char(&mut self, value: char) -> InputOutcome {
        self.insert_text(value.to_string())
    }

    fn insert_text(&mut self, value: impl AsRef<str>) -> InputOutcome {
        let value = value.as_ref();
        if self
            .max_len
            .is_some_and(|max_len| self.len_chars() >= max_len)
        {
            return InputOutcome::HANDLED;
        }
        let text = if let Some(max_len) = self.max_len {
            value
                .chars()
                .take(max_len.saturating_sub(self.len_chars()))
                .collect::<String>()
        } else {
            value.to_owned()
        };
        if text.is_empty() {
            return InputOutcome::HANDLED;
        }
        let len = text.chars().count();
        self.value.insert_str(self.byte_index(self.cursor), &text);
        self.cursor += len;
        InputOutcome::CHANGED
    }

    fn backspace(&mut self) -> InputOutcome {
        if self.cursor == 0 {
            return InputOutcome::HANDLED;
        }
        self.remove_range(self.cursor - 1, self.cursor);
        self.cursor -= 1;
        InputOutcome::CHANGED
    }

    fn delete_next(&mut self) -> InputOutcome {
        if self.cursor >= self.len_chars() {
            return InputOutcome::HANDLED;
        }
        self.remove_range(self.cursor, self.cursor + 1);
        InputOutcome::CHANGED
    }

    fn move_left(&mut self) -> InputOutcome {
        self.move_to(self.cursor.saturating_sub(1))
    }

    fn move_right(&mut self) -> InputOutcome {
        self.move_to(self.cursor.saturating_add(1).min(self.len_chars()))
    }

    fn move_to(&mut self, cursor: usize) -> InputOutcome {
        let cursor = cursor.min(self.len_chars());
        let changed = cursor != self.cursor;
        self.cursor = cursor;
        if changed {
            InputOutcome::HANDLED
        } else {
            InputOutcome::IDLE
        }
    }

    fn delete_before_cursor(&mut self) -> InputOutcome {
        if self.cursor == 0 {
            return InputOutcome::HANDLED;
        }
        self.remove_range(0, self.cursor);
        self.cursor = 0;
        InputOutcome::CHANGED
    }

    fn delete_after_cursor(&mut self) -> InputOutcome {
        if self.cursor >= self.len_chars() {
            return InputOutcome::HANDLED;
        }
        self.value.truncate(self.byte_index(self.cursor));
        InputOutcome::CHANGED
    }

    fn clear(&mut self) -> InputOutcome {
        if self.value.is_empty() && self.cursor == 0 {
            return InputOutcome::HANDLED;
        }
        self.value.clear();
        self.cursor = 0;
        InputOutcome::CHANGED
    }

    fn delete_previous_word(&mut self) -> InputOutcome {
        if self.cursor == 0 {
            return InputOutcome::HANDLED;
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }

        self.remove_range(start, self.cursor);
        self.cursor = start;
        InputOutcome::CHANGED
    }

    fn move_previous_word(&mut self) -> InputOutcome {
        if self.cursor == 0 {
            return InputOutcome::HANDLED;
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }

        self.move_to(start)
    }

    fn move_next_word(&mut self) -> InputOutcome {
        let len = self.len_chars();
        if self.cursor >= len {
            return InputOutcome::HANDLED;
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let mut end = self.cursor;
        while end < len && !chars[end].is_whitespace() {
            end += 1;
        }
        while end < len && chars[end].is_whitespace() {
            end += 1;
        }

        self.move_to(end)
    }

    fn delete_next_word(&mut self) -> InputOutcome {
        let len = self.len_chars();
        if self.cursor >= len {
            return InputOutcome::HANDLED;
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let mut end = self.cursor;
        while end < len && !chars[end].is_whitespace() {
            end += 1;
        }
        while end < len && chars[end].is_whitespace() {
            end += 1;
        }

        self.remove_range(self.cursor, end);
        InputOutcome::CHANGED
    }

    fn len_chars(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_index(&self, char_index: usize) -> usize {
        if char_index == self.len_chars() {
            return self.value.len();
        }
        self.value
            .char_indices()
            .nth(char_index)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len())
    }

    fn remove_range(&mut self, start: usize, end: usize) {
        let start = self.byte_index(start);
        let end = self.byte_index(end);
        self.value.replace_range(start..end, "");
    }

    fn clamp_value(&mut self) {
        if let Some(max_len) = self.max_len {
            self.value = self.value.chars().take(max_len).collect();
        }
    }

    fn external_editor_key_matches(&self, key: KeyEvent) -> bool {
        self.external_editor_key
            .is_some_and(|expected| key_matches(expected, key))
    }

    fn apply_external_editor_response(&mut self, response: &crate::ExternalEditorResponse) {
        let mut collapsed_cursor = 0;
        let lines: Vec<&str> = response.value.split('\n').collect();
        let target_line_idx = response
            .line
            .saturating_sub(1)
            .min(lines.len().saturating_sub(1));

        for line in lines.iter().take(target_line_idx) {
            collapsed_cursor += line.chars().count() + 1;
        }

        let col_idx = response.col.saturating_sub(1);
        let target_line_chars = lines[target_line_idx].chars().count();
        collapsed_cursor += col_idx.min(target_line_chars);

        self.value = response.value.replace('\n', " ");
        self.clamp_value();
        self.cursor = collapsed_cursor.min(self.len_chars());
    }
}

impl<M> TuiNode<M> for TextInput<M> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        ctx.register_focusable(FocusId::new(INPUT_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::ExternalEditor(response) = event {
            self.apply_external_editor_response(response);
            self.cursor_fade.reset();
            ctx.request_clear();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.external_editor_key_matches(*key) {
            ctx.request_external_editor(self.value.clone(), 1, self.cursor + 1);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        if outcome.submitted
            && let Some(on_submit) = &self.on_submit
        {
            ctx.emit(on_submit(self.value.clone()));
        }
        if outcome.clear {
            ctx.request_clear();
        }
        if outcome.needs_redraw() {
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
        self.focused = focused;
        if focused {
            self.cursor_fade.reset();
        } else if let Some(on_blur) = &self.on_blur {
            ctx.emit(on_blur(self.value.clone()));
        }
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<M> Animated for TextInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.cursor_fade.tick(self.focused, dt, settings)
    }
}

pub(crate) fn placeholder_line(
    placeholder: &str,
    width: usize,
    focused: bool,
    cursor_style: Style,
    placeholder_style: Style,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }

    let mut chars = placeholder.chars();
    if !focused {
        return Line::from(Span::styled(
            chars.take(width).collect::<String>(),
            placeholder_style,
        ));
    }

    let first = chars.next().unwrap_or(' ');
    let mut spans = vec![Span::styled(first.to_string(), cursor_style)];
    let hint: String = chars.take(width.saturating_sub(1)).collect();
    if !hint.is_empty() {
        spans.push(Span::styled(hint, placeholder_style));
    }
    Line::from(spans)
}

#[derive(Debug, Clone)]
pub(crate) struct CursorFade {
    elapsed: Duration,
}

impl Default for CursorFade {
    fn default() -> Self {
        Self {
            elapsed: Duration::ZERO,
        }
    }
}

impl CursorFade {
    pub(crate) fn reset(&mut self) {
        self.elapsed = Duration::ZERO;
    }

    pub(crate) fn tick(
        &mut self,
        focused: bool,
        dt: Duration,
        settings: AnimationSettings,
    ) -> TickResult {
        if !focused {
            return TickResult::IDLE;
        }
        if !settings.enabled {
            let changed = self.elapsed != Duration::ZERO;
            self.reset();
            return TickResult {
                changed,
                active: false,
            };
        }

        let total = CURSOR_FADE_HALF.saturating_mul(2);
        let before = self.elapsed;
        self.elapsed = duration_mod(self.elapsed.saturating_add(dt.min(settings.max_dt)), total);
        TickResult {
            changed: before != self.elapsed,
            active: true,
        }
    }

    pub(crate) fn style(&self, base: Style) -> Style {
        let opacity = self.opacity();
        if opacity <= 0.01 {
            return base;
        }

        let theme = theme();
        let fg = fade_color(
            base.fg.unwrap_or_else(|| theme.text_fg()),
            theme.highlight_fg(),
            opacity,
        );
        let bg = fade_color(theme.selected_bg(), theme.highlight_bg(), opacity);
        base.fg(fg).bg(bg).add_modifier(Modifier::BOLD)
    }

    fn opacity(&self) -> f64 {
        let total = CURSOR_FADE_HALF.saturating_mul(2);
        let progress = self.elapsed.as_secs_f64() / total.as_secs_f64();
        let phase = if progress < 0.5 {
            1.0 - progress * 2.0
        } else {
            (progress - 0.5) * 2.0
        };
        Easing::EaseInOut.apply(phase)
    }
}

fn fade_color(from: Color, to: Color, opacity: f64) -> Color {
    lerp_color(from, to, opacity)
}

fn duration_mod(value: Duration, modulus: Duration) -> Duration {
    Duration::from_secs_f64(value.as_secs_f64() % modulus.as_secs_f64())
}

pub(crate) fn is_ctrl(key: KeyEvent, value: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, Key::Char(key_value) if key_value.eq_ignore_ascii_case(&value))
}

pub(crate) fn is_alt(key: KeyEvent, value: char) -> bool {
    key.modifiers.contains(KeyModifiers::ALT)
        && matches!(key.code, Key::Char(key_value) if key_value.eq_ignore_ascii_case(&value))
}

pub(crate) fn text_char(key: KeyEvent) -> bool {
    !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}

pub(crate) fn tab_key(key: KeyEvent) -> bool {
    matches!(key.code, Key::Tab)
        || (matches!(key.code, Key::Char('i') | Key::Char('I'))
            && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn ctrl_key(value: char) -> KeyEvent {
    KeyEvent {
        code: Key::Char(value),
        modifiers: KeyModifiers::CONTROL,
    }
}

fn key_matches(expected: KeyEvent, actual: KeyEvent) -> bool {
    expected.modifiers == actual.modifiers
        && match (expected.code, actual.code) {
            (Key::Char(expected), Key::Char(actual)) => expected.eq_ignore_ascii_case(&actual),
            _ => expected.code == actual.code,
        }
}

pub(crate) fn display_char(value: char, max_width: usize) -> String {
    let text = if value == '\t' {
        " ".repeat(TAB_WIDTH)
    } else {
        value.to_string()
    };
    text.chars().take(max_width).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Propagation;

    #[test]
    fn handled_key_stops_propagation() {
        let mut input = TextInput::<()>::new();
        let mut ctx = EventCtx::<()>::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn submit_emits_message_and_stops_propagation() {
        let mut input = TextInput::new()
            .value("ship")
            .on_submit(|value| format!("submit:{value}"));
        let mut ctx = EventCtx::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["submit:ship".to_string()]);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn control_c_clears_value_and_stops_propagation() {
        let mut input = TextInput::<()>::new().value("search");
        let mut ctx = EventCtx::<()>::default();
        let key = KeyEvent {
            code: Key::Char('c'),
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "");
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn tab_inserts_tab_character_and_stops_propagation() {
        let mut input = TextInput::<()>::new().value("left");
        let mut ctx = EventCtx::<()>::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Tab)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "left    ");
        assert_eq!(line_text(&input.line(10)), "left    ");
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn control_i_inserts_tab_character_and_stops_propagation() {
        let mut input = TextInput::<()>::new().value("left");
        let mut ctx = EventCtx::<()>::default();
        let key = KeyEvent {
            code: Key::Char('i'),
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "left    ");
        assert_eq!(line_text(&input.line(10)), "left    ");
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn focused_placeholder_draws_cursor_over_first_character() {
        let input = TextInput::<()>::new().placeholder("Ask").focused(true);

        let line = input.line(3);

        assert_eq!(line.spans[0].content.as_ref(), "A");
        assert_eq!(line_text(&line), "Ask");
    }

    #[test]
    fn escape_bubbles_to_parent_policy() {
        let mut input = TextInput::<()>::new();
        let mut ctx = EventCtx::<()>::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);
        let mut parent_observed = false;
        let bubbled = outcome.bubble(&mut ctx, |_ctx| {
            parent_observed = true;
            EventOutcome::Handled
        });

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(bubbled, EventOutcome::Handled);
        assert!(parent_observed);
        assert_eq!(ctx.propagation(), Propagation::Continue);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn word_navigation_and_deletion() {
        let mut input = TextInput::<()>::new().value("hello world example");
        // Start cursor is at the end (19)
        assert_eq!(input.cursor, 19);

        // Ctrl+Left jumps to the start of "example" (12)
        input.on_key(KeyEvent {
            code: Key::Left,
            modifiers: KeyModifiers::CONTROL,
        });
        assert_eq!(input.cursor, 12);

        // Ctrl+Left jumps to the start of "world" (6)
        input.on_key(KeyEvent {
            code: Key::Left,
            modifiers: KeyModifiers::CONTROL,
        });
        assert_eq!(input.cursor, 6);

        // Ctrl+Right jumps to the start of "example" (12)
        input.on_key(KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::CONTROL,
        });
        assert_eq!(input.cursor, 12);

        // Ctrl+Right jumps to the end of input (19)
        input.on_key(KeyEvent {
            code: Key::Right,
            modifiers: KeyModifiers::CONTROL,
        });
        assert_eq!(input.cursor, 19);

        // Move cursor back to "world" (6)
        input.cursor = 6;

        // Ctrl+Backspace deletes "hello " (before cursor)
        input.on_key(KeyEvent {
            code: Key::Backspace,
            modifiers: KeyModifiers::CONTROL,
        });
        assert_eq!(input.current_value(), "world example");
        assert_eq!(input.cursor, 0);

        // Reset text and delete next word (Ctrl+Delete)
        input.set_value("hello world example");
        input.cursor = 6; // start of "world"
        input.on_key(KeyEvent {
            code: Key::Delete,
            modifiers: KeyModifiers::CONTROL,
        });
        // Deletes "world " (from cursor to start of next word)
        assert_eq!(input.current_value(), "hello example");
        assert_eq!(input.cursor, 6);

        // Test Alt+b (word backward)
        input.set_value("hello world example");
        input.cursor = 19;
        input.on_key(KeyEvent {
            code: Key::Char('b'),
            modifiers: KeyModifiers::ALT,
        });
        assert_eq!(input.cursor, 12);

        // Test Alt+f (word forward)
        input.cursor = 6;
        input.on_key(KeyEvent {
            code: Key::Char('f'),
            modifiers: KeyModifiers::ALT,
        });
        assert_eq!(input.cursor, 12);

        // Test Alt+d (delete word forward)
        input.set_value("hello world example");
        input.cursor = 6;
        input.on_key(KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::ALT,
        });
        assert_eq!(input.current_value(), "hello example");
        assert_eq!(input.cursor, 6);
    }

    #[test]
    fn ctrl_o_requests_external_editor() {
        let mut input = TextInput::<()>::new().value("initial");
        let mut ctx = EventCtx::default();
        let key = KeyEvent {
            code: Key::Char('o'),
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "initial");
        assert_eq!(
            ctx.external_editor_request(),
            Some(&crate::ExternalEditorRequest {
                value: "initial".to_string(),
                line: 1,
                col: 8,
            })
        );
        assert!(ctx.redraw_requested());
        assert!(!ctx.clear_requested());
    }

    #[test]
    fn external_editor_response_updates_value_and_clamps_cursor() {
        let mut input = TextInput::<()>::new().value("initial");
        let mut ctx = EventCtx::default();

        let outcome = input.event(
            &TuiEvent::ExternalEditor(crate::ExternalEditorResponse {
                value: "edited\nvalue".to_string(),
                line: 2,
                col: 99,
            }),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "edited value");
        assert_eq!(input.cursor, input.len_chars());
        assert!(ctx.redraw_requested());
        assert!(ctx.clear_requested());
    }

    #[test]
    fn on_blur_emits_message_when_focus_lost() {
        let mut input = TextInput::new()
            .value("hello")
            .on_blur(|value| format!("blur:{value}"));
        let mut ctx = FocusCtx::new(AnimationSettings::default());

        input.focus(None, false, &mut ctx);

        assert_eq!(
            ctx.drain_messages().collect::<Vec<_>>(),
            vec!["blur:hello".to_string()]
        );
    }

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }
}
