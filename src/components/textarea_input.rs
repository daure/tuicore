use ratatui::Frame;
use ratatui::layout::Rect;
use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, TickResult};
use crate::event::{Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::theme;
use crate::{EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx, LayoutResult, TuiNode};

use super::text_input::{
    CursorFade, InputOutcome, edit_in_external_editor, is_alt, is_ctrl, placeholder_line, text_char,
};

const TEXTAREA_FOCUS: &str = "textarea";

pub struct TextareaInput<M = ()> {
    value: String,
    placeholder: String,
    cursor: usize,
    focused: bool,
    max_lines: Option<usize>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_blur: Option<Box<dyn Fn(String) -> M>>,
    cursor_fade: CursorFade,
}

impl<M> Default for TextareaInput<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TextareaInput<M> {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            cursor: 0,
            focused: false,
            max_lines: None,
            on_submit: None,
            on_blur: None,
            cursor_fade: CursorFade::default(),
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.clamp_lines();
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

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines.max(1));
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.clamp_lines();
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
        if is_ctrl(key, 'o') {
            let ranges = self.line_ranges();
            let (line, col) = self.cursor_line_col(&ranges);
            if let Ok(Some((new_value, exit_line, exit_col))) =
                edit_in_external_editor(&self.value, line + 1, col + 1)
            {
                self.value = new_value;
                self.clamp_lines();
                let ranges = self.line_ranges();
                let line_idx = exit_line
                    .saturating_sub(1)
                    .min(ranges.len().saturating_sub(1));
                let range = ranges[line_idx];
                self.cursor = (range.start + exit_col.saturating_sub(1)).min(self.len_chars());
                return InputOutcome {
                    handled: true,
                    changed: true,
                    submitted: false,
                    canceled: false,
                    clear: true,
                };
            } else {
                return InputOutcome {
                    handled: true,
                    changed: false,
                    submitted: false,
                    canceled: false,
                    clear: true,
                };
            }
        }
        if is_ctrl(key, 'd') || (matches!(key.code, Key::Enter) && is_control(key)) {
            return InputOutcome::SUBMITTED;
        }
        if is_ctrl(key, 'a') {
            return self.move_to(self.current_line().start);
        }
        if is_ctrl(key, 'c') {
            return self.clear();
        }
        if is_ctrl(key, 'e') {
            return self.move_to(self.current_line().end);
        }
        if is_ctrl(key, 'u') {
            return self.delete_before_line();
        }
        if is_ctrl(key, 'k') {
            return self.delete_after_line();
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
        if is_ctrl(key, 'p') {
            return self.move_vertical(-1);
        }
        if is_ctrl(key, 'n') {
            return self.move_vertical(1);
        }

        match key.code {
            Key::Char(value) if text_char(key) => self.insert_char(value),
            Key::Enter => self.insert_newline(),
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
            Key::Up => self.move_vertical(-1),
            Key::Down => self.move_vertical(1),
            Key::Home => self.move_to(self.current_line().start),
            Key::End => self.move_to(self.current_line().end),
            Key::Esc => InputOutcome::CANCELED,
            _ => InputOutcome::IDLE,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let lines = self.visible_lines(area.width as usize, area.height as usize);
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn visible_lines(&self, width: usize, height: usize) -> Vec<Line<'static>> {
        if width == 0 || height == 0 {
            return Vec::new();
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
            let mut lines = vec![placeholder_line(
                &self.placeholder,
                width,
                self.focused,
                self.cursor_fade.style(placeholder_style),
                placeholder_style,
            )];
            lines.resize_with(height.min(1), Line::default);
            return lines;
        }

        let ranges = self.line_ranges();
        let (cursor_line, cursor_col) = self.cursor_line_col(&ranges);
        let first_line = cursor_line.saturating_add(1).saturating_sub(height);
        ranges
            .iter()
            .enumerate()
            .skip(first_line)
            .take(height)
            .map(|(line_index, range)| {
                let horizontal = if self.focused && line_index == cursor_line && cursor_col >= width
                {
                    cursor_col.saturating_add(1).saturating_sub(width)
                } else {
                    0
                };
                self.render_line(
                    *range,
                    line_index == cursor_line,
                    horizontal,
                    width,
                    value_style,
                    cursor_style,
                )
            })
            .collect()
    }

    fn render_line(
        &self,
        range: LineRange,
        cursor_line: bool,
        horizontal: usize,
        width: usize,
        value_style: Style,
        cursor_style: Style,
    ) -> Line<'static> {
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut spans = Vec::new();
        let mut drawn = 0;

        for col in horizontal..=range.len() {
            if drawn >= width {
                break;
            }
            let position = range.start + col;
            if self.focused && cursor_line && position == self.cursor {
                let text = if position < range.end {
                    chars.get(position).copied().unwrap_or(' ')
                } else {
                    ' '
                }
                .to_string();
                spans.push(Span::styled(text, cursor_style));
                drawn += 1;
                continue;
            }
            if position < range.end
                && let Some(value) = chars.get(position)
            {
                spans.push(Span::styled(value.to_string(), value_style));
                drawn += 1;
            }
        }

        Line::from(spans)
    }

    fn insert_char(&mut self, value: char) -> InputOutcome {
        self.value.insert(self.byte_index(self.cursor), value);
        self.cursor += 1;
        InputOutcome::CHANGED
    }

    fn insert_newline(&mut self) -> InputOutcome {
        if self
            .max_lines
            .is_some_and(|max_lines| self.line_count() >= max_lines)
        {
            return InputOutcome::HANDLED;
        }
        self.insert_char('\n')
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

    fn move_vertical(&mut self, delta: isize) -> InputOutcome {
        let ranges = self.line_ranges();
        let (line, col) = self.cursor_line_col(&ranges);
        let target_line = (line as isize + delta).clamp(0, ranges.len().saturating_sub(1) as isize);
        let range = ranges[target_line as usize];
        self.move_to(range.start + col.min(range.len()))
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

    fn delete_before_line(&mut self) -> InputOutcome {
        let line = self.current_line();
        if self.cursor == line.start {
            return InputOutcome::HANDLED;
        }
        self.remove_range(line.start, self.cursor);
        self.cursor = line.start;
        InputOutcome::CHANGED
    }

    fn delete_after_line(&mut self) -> InputOutcome {
        let line = self.current_line();
        if self.cursor < line.end {
            self.remove_range(self.cursor, line.end);
            return InputOutcome::CHANGED;
        }
        if self.cursor < self.len_chars() {
            self.remove_range(self.cursor, self.cursor + 1);
            return InputOutcome::CHANGED;
        }
        InputOutcome::HANDLED
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

    fn current_line(&self) -> LineRange {
        let ranges = self.line_ranges();
        let (line, _) = self.cursor_line_col(&ranges);
        ranges[line]
    }

    fn cursor_line_col(&self, ranges: &[LineRange]) -> (usize, usize) {
        ranges
            .iter()
            .enumerate()
            .find_map(|(index, range)| {
                (self.cursor >= range.start && self.cursor <= range.end)
                    .then_some((index, self.cursor.saturating_sub(range.start)))
            })
            .unwrap_or_else(|| {
                let last = ranges.len().saturating_sub(1);
                (last, ranges[last].len())
            })
    }

    fn line_ranges(&self) -> Vec<LineRange> {
        let mut ranges = Vec::new();
        let mut start = 0;
        for (index, value) in self.value.chars().enumerate() {
            if value == '\n' {
                ranges.push(LineRange { start, end: index });
                start = index + 1;
            }
        }
        ranges.push(LineRange {
            start,
            end: self.len_chars(),
        });
        ranges
    }

    fn line_count(&self) -> usize {
        self.value.chars().filter(|value| *value == '\n').count() + 1
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

    fn clamp_lines(&mut self) {
        let Some(max_lines) = self.max_lines else {
            return;
        };

        let mut lines = self.value.split('\n').take(max_lines).collect::<Vec<_>>();
        if lines.is_empty() {
            return;
        }
        self.value = lines.drain(..).collect::<Vec<_>>().join("\n");
    }
}

impl<M> TuiNode<M> for TextareaInput<M> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        ctx.register_focusable(FocusId::new(TEXTAREA_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
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

impl<M> Animated for TextareaInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.cursor_fade.tick(self.focused, dt, settings)
    }
}

#[derive(Debug, Clone, Copy)]
struct LineRange {
    start: usize,
    end: usize,
}

impl LineRange {
    fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

fn is_control(key: KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Propagation;

    #[test]
    fn handled_key_stops_propagation() {
        let mut input = TextareaInput::<()>::new();
        let mut ctx = EventCtx::<()>::default();

        let outcome = input.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn control_enter_submit_emits_message_and_stops_propagation() {
        let mut input = TextareaInput::new()
            .value("first\nsecond")
            .on_submit(|value| format!("submit:{value}"));
        let mut ctx = EventCtx::default();
        let key = KeyEvent {
            code: Key::Enter,
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["submit:first\nsecond".to_string()]);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn control_d_submit_emits_message_and_stops_propagation() {
        let mut input = TextareaInput::new()
            .value("draft")
            .on_submit(|value| format!("submit:{value}"));
        let mut ctx = EventCtx::default();
        let key = KeyEvent {
            code: Key::Char('d'),
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["submit:draft".to_string()]);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn control_c_clears_value_and_stops_propagation() {
        let mut input = TextareaInput::<()>::new().value("first\nsecond");
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
    fn focused_placeholder_draws_cursor_over_first_character() {
        let input = TextareaInput::<()>::new()
            .placeholder("Write multiple lines...")
            .focused(true);

        let lines = input.visible_lines(8, 1);

        assert_eq!(lines[0].spans[0].content.as_ref(), "W");
        assert_eq!(line_text(&lines[0]), "Write mu");
    }

    #[test]
    fn escape_bubbles_to_parent_policy() {
        let mut input = TextareaInput::<()>::new();
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
        let mut input = TextareaInput::<()>::new().value("hello world example");
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
    }

    #[test]
    #[cfg(unix)]
    fn ctrl_o_opens_external_editor() {
        let _guard = crate::ENV_LOCK.lock().unwrap();
        let old_editor = std::env::var("EDITOR").ok();
        unsafe {
            std::env::set_var(
                "EDITOR",
                "sh -c 'for last; do true; done; printf \"edited\\nlines\\n\" > \"$last\"' --",
            );
        }

        let mut input = TextareaInput::<()>::new().value("initial");
        let mut ctx = EventCtx::default();
        let key = KeyEvent {
            code: Key::Char('o'),
            modifiers: KeyModifiers::CONTROL,
        };

        let outcome = input.event(&TuiEvent::Key(key), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(input.current_value(), "edited\nlines\n");
        assert!(ctx.redraw_requested());
        assert!(ctx.clear_requested());

        unsafe {
            if let Some(val) = old_editor {
                std::env::set_var("EDITOR", val);
            } else {
                std::env::remove_var("EDITOR");
            }
        }
    }

    #[test]
    fn on_blur_emits_message_when_focus_lost() {
        let mut input = TextareaInput::new()
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
