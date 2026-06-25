use ratatui::Frame;
use ratatui::layout::Rect;
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, Easing, TickResult, lerp_color};
use crate::event::{HotkeyEvent, Key, KeyEvent, KeyModifiers, TuiEvent};
use crate::theme;
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, HotkeyLabelMode, KeySpec, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, TuiNode, hotkey_label_spans, hotkey_underline_style, line_width,
};

const INPUT_FOCUS: &str = "input";
const PASSWORD_INPUT_FOCUS: &str = "password-input";
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
    hotkey: Option<String>,
    cursor: usize,
    focused: bool,
    insert_mode: bool,
    max_len: Option<usize>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_blur: Option<Box<dyn Fn(String) -> M>>,
    external_editor_key: Option<KeyEvent>,
    keys: TextInputKeyBindings,
    cursor_fade: CursorFade,
    pending_hotkey_prefix: Option<String>,
}

pub struct PasswordInput<M = ()> {
    input: TextInput<M>,
    mask_char: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextInputKeyBindings {
    pub submit: Vec<KeySpec>,
    pub cancel: Vec<KeySpec>,
    pub clear: Vec<KeySpec>,
    pub move_start: Vec<KeySpec>,
    pub move_end: Vec<KeySpec>,
    pub move_left: Vec<KeySpec>,
    pub move_right: Vec<KeySpec>,
    pub move_previous_word: Vec<KeySpec>,
    pub move_next_word: Vec<KeySpec>,
    pub delete_before_cursor: Vec<KeySpec>,
    pub delete_after_cursor: Vec<KeySpec>,
    pub delete_previous_word: Vec<KeySpec>,
    pub delete_next_word: Vec<KeySpec>,
    pub backspace: Vec<KeySpec>,
    pub delete_next: Vec<KeySpec>,
    pub insert_tab: Vec<KeySpec>,
}

impl Default for TextInputKeyBindings {
    fn default() -> Self {
        Self {
            submit: vec![KeySpec::key_with_modifiers(
                Key::Enter,
                KeyModifiers::CONTROL,
            )],
            cancel: vec![
                KeySpec::key(Key::Esc),
                KeySpec::key_with_modifiers(Key::Char('['), KeyModifiers::CONTROL),
            ],
            clear: vec![KeySpec::key_with_modifiers(
                Key::Char('c'),
                KeyModifiers::CONTROL,
            )],
            move_start: vec![
                KeySpec::key_with_modifiers(Key::Char('a'), KeyModifiers::CONTROL),
                KeySpec::key(Key::Home),
            ],
            move_end: vec![
                KeySpec::key_with_modifiers(Key::Char('e'), KeyModifiers::CONTROL),
                KeySpec::key(Key::End),
            ],
            move_left: vec![KeySpec::key(Key::Left)],
            move_right: vec![KeySpec::key(Key::Right)],
            move_previous_word: vec![
                KeySpec::key_with_modifiers(Key::Char('b'), KeyModifiers::ALT),
                KeySpec::key_with_modifiers(Key::Left, KeyModifiers::CONTROL),
            ],
            move_next_word: vec![
                KeySpec::key_with_modifiers(Key::Char('f'), KeyModifiers::ALT),
                KeySpec::key_with_modifiers(Key::Right, KeyModifiers::CONTROL),
            ],
            delete_before_cursor: vec![KeySpec::key_with_modifiers(
                Key::Char('u'),
                KeyModifiers::CONTROL,
            )],
            delete_after_cursor: vec![KeySpec::key_with_modifiers(
                Key::Char('k'),
                KeyModifiers::CONTROL,
            )],
            delete_previous_word: vec![
                KeySpec::key_with_modifiers(Key::Char('w'), KeyModifiers::CONTROL),
                KeySpec::key_with_modifiers(Key::Backspace, KeyModifiers::CONTROL),
            ],
            delete_next_word: vec![
                KeySpec::key_with_modifiers(Key::Char('d'), KeyModifiers::ALT),
                KeySpec::key_with_modifiers(Key::Delete, KeyModifiers::CONTROL),
            ],
            backspace: vec![KeySpec::key(Key::Backspace)],
            delete_next: vec![KeySpec::key(Key::Delete)],
            insert_tab: vec![
                KeySpec::key(Key::Tab),
                KeySpec::key_with_modifiers(Key::Char('i'), KeyModifiers::CONTROL),
            ],
        }
    }
}

impl TextInputKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }
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
            hotkey: None,
            cursor: 0,
            focused: false,
            insert_mode: false,
            max_len: None,
            on_submit: None,
            on_blur: None,
            external_editor_key: Some(ctrl_key('o')),
            keys: TextInputKeyBindings::default(),
            cursor_fade: CursorFade::default(),
            pending_hotkey_prefix: None,
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

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.pending_hotkey_prefix = None;
    }

    fn handle_visual_hotkey(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) {
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                self.pending_hotkey_prefix = Some(prefix.clone());
                ctx.request_redraw();
            }
            HotkeyEvent::Canceled | HotkeyEvent::Commit(_) => {
                if self.pending_hotkey_prefix.take().is_some() {
                    ctx.request_redraw();
                }
            }
        }
    }

    fn handle_focus_hotkey(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) -> bool {
        let HotkeyEvent::Commit(_) = hotkey else {
            return false;
        };

        self.insert_mode = true;
        self.cursor_fade.reset();
        ctx.request_layout();
        ctx.request_redraw();
        ctx.stop_propagation();
        true
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.insert_mode = false;
        }
    }

    pub fn insert_mode(&self) -> bool {
        self.insert_mode
    }

    pub(crate) fn set_insert_mode(&mut self, insert_mode: bool) {
        self.insert_mode = insert_mode;
        self.cursor_fade.reset();
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

    pub fn keybindings(mut self, keys: TextInputKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn set_keybindings(&mut self, keys: TextInputKeyBindings) {
        self.keys = keys;
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

    pub fn on_paste(&mut self, value: impl AsRef<str>) -> InputOutcome {
        let outcome = self.insert_text(value.as_ref().replace('\n', " "));
        if outcome.needs_redraw() {
            self.cursor_fade.reset();
        }
        outcome
    }

    fn on_key_inner(&mut self, key: KeyEvent) -> InputOutcome {
        if matches_any(&self.keys.move_start, key) {
            return self.move_to(0);
        }
        if matches_any(&self.keys.clear, key) {
            return self.clear();
        }
        if matches_any(&self.keys.move_end, key) {
            return self.move_to(self.len_chars());
        }
        if matches_any(&self.keys.delete_before_cursor, key) {
            return self.delete_before_cursor();
        }
        if matches_any(&self.keys.delete_after_cursor, key) {
            return self.delete_after_cursor();
        }
        if matches_any(&self.keys.delete_previous_word, key) {
            return self.delete_previous_word();
        }
        if matches_any(&self.keys.move_previous_word, key) {
            return self.move_previous_word();
        }
        if matches_any(&self.keys.move_next_word, key) {
            return self.move_next_word();
        }
        if matches_any(&self.keys.delete_next_word, key) {
            return self.delete_next_word();
        }
        if matches_any(&self.keys.insert_tab, key) {
            return self.insert_text(TAB_INSERT);
        }
        if matches_any(&self.keys.backspace, key) {
            return self.backspace();
        }
        if matches_any(&self.keys.delete_next, key) {
            return self.delete_next();
        }
        if matches_any(&self.keys.move_left, key) {
            return self.move_left();
        }
        if matches_any(&self.keys.move_right, key) {
            return self.move_right();
        }
        if matches_any(&self.keys.submit, key) {
            return InputOutcome::SUBMITTED;
        }
        if matches_any(&self.keys.cancel, key) {
            return InputOutcome::CANCELED;
        }
        match key.code {
            Key::Char(value) if text_char(key) => self.insert_char(value),
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
        let style = if self.focused && !self.insert_mode {
            selected_input_style(style)
        } else {
            style
        };

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
        let selected = self.focused && !self.insert_mode;
        let value_style = Style::default().fg(if self.focused {
            theme.text_fg()
        } else {
            theme.subtle_fg()
        });
        let value_style = if selected {
            selected_input_style(value_style)
        } else {
            value_style
        };
        let placeholder_style = if selected {
            selected_input_style(Style::default().fg(theme.muted_fg()))
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let hotkey_style = if selected {
            selected_input_style(Style::default())
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let cursor_style = self.cursor_fade.style(value_style);

        if self.value.is_empty() {
            return placeholder_line(
                &self.placeholder,
                self.hotkey.as_deref(),
                width,
                self.focused && self.insert_mode,
                self.pending_hotkey_prefix.as_deref(),
                self.cursor_fade.style(placeholder_style),
                placeholder_style,
            );
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let len = chars.len();
        let start = if self.focused && self.insert_mode {
            visible_start_for_cursor(&chars, self.cursor, width)
        } else {
            0
        };
        let mut spans = Vec::new();
        let mut drawn = 0;

        for position in start..=len {
            if drawn >= width {
                break;
            }
            let remaining = width.saturating_sub(drawn);
            if self.focused && self.insert_mode && position == self.cursor {
                let mut text = display_char(chars.get(position).copied().unwrap_or(' '), remaining);
                if text.is_empty() && remaining > 0 {
                    text.push(' ');
                }
                drawn += cell_width(&text);
                spans.push(Span::styled(text, cursor_style));
                continue;
            }
            if let Some(value) = chars.get(position) {
                let text = display_char(*value, remaining);
                drawn += cell_width(&text);
                spans.push(Span::styled(text, value_style));
            }
        }
        append_unfocused_hotkey(
            &mut spans,
            &mut drawn,
            width,
            self.hotkey.as_deref(),
            self.focused && self.insert_mode,
            self.pending_hotkey_prefix.as_deref(),
            hotkey_style,
        );

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

impl<M> Default for PasswordInput<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> PasswordInput<M> {
    pub fn new() -> Self {
        let mut input = TextInput::new();
        input.external_editor_key = None;
        Self {
            input,
            mask_char: '•',
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.input = self.input.value(value);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.input = self.input.hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.input.set_hotkey(hotkey);
    }

    pub fn clear_hotkey(&mut self) {
        self.input.clear_hotkey();
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.input = self.input.focused(focused);
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.input.set_focused(focused);
    }

    pub fn on_submit(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.input = self.input.on_submit(handler);
        self
    }

    pub fn on_blur(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.input = self.input.on_blur(handler);
        self
    }

    pub fn keybindings(mut self, keys: TextInputKeyBindings) -> Self {
        self.input = self.input.keybindings(keys);
        self
    }

    pub fn set_keybindings(&mut self, keys: TextInputKeyBindings) {
        self.input.set_keybindings(keys);
    }

    pub fn max_len(mut self, max_len: usize) -> Self {
        self.input = self.input.max_len(max_len);
        self
    }

    pub fn mask_char(mut self, mask_char: char) -> Self {
        self.mask_char = mask_char;
        self
    }

    pub fn current_value(&self) -> &str {
        self.input.current_value()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.input.set_value(value);
    }

    pub fn on_key(&mut self, key: impl Into<KeyEvent>) -> InputOutcome {
        self.input.on_key(key)
    }

    pub fn on_paste(&mut self, value: impl AsRef<str>) -> InputOutcome {
        self.input.on_paste(value)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_with_style(frame, area, Style::default());
    }

    pub(crate) fn render_with_style(&self, frame: &mut Frame, area: Rect, style: Style) {
        if area.is_empty() {
            return;
        }
        let style = if self.input.focused && !self.input.insert_mode {
            selected_input_style(style)
        } else {
            style
        };

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
        let selected = self.input.focused && !self.input.insert_mode;
        let value_style = Style::default().fg(if self.input.focused {
            theme.text_fg()
        } else {
            theme.subtle_fg()
        });
        let value_style = if selected {
            selected_input_style(value_style)
        } else {
            value_style
        };
        let placeholder_style = if selected {
            selected_input_style(Style::default().fg(theme.muted_fg()))
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let hotkey_style = if selected {
            selected_input_style(Style::default())
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let cursor_style = self.input.cursor_fade.style(value_style);

        if self.input.value.is_empty() {
            return placeholder_line(
                &self.input.placeholder,
                self.input.hotkey.as_deref(),
                width,
                self.input.focused && self.input.insert_mode,
                self.input.pending_hotkey_prefix.as_deref(),
                self.input.cursor_fade.style(placeholder_style),
                placeholder_style,
            );
        }

        let chars = std::iter::repeat(self.mask_char)
            .take(self.input.len_chars())
            .collect::<Vec<_>>();
        let len = chars.len();
        let start = if self.input.focused && self.input.insert_mode {
            visible_start_for_cursor(&chars, self.input.cursor, width)
        } else {
            0
        };
        let mut spans = Vec::new();
        let mut drawn = 0;

        for position in start..=len {
            if drawn >= width {
                break;
            }
            let remaining = width.saturating_sub(drawn);
            if self.input.focused && self.input.insert_mode && position == self.input.cursor {
                let mut text = display_char(chars.get(position).copied().unwrap_or(' '), remaining);
                if text.is_empty() && remaining > 0 {
                    text.push(' ');
                }
                drawn += cell_width(&text);
                spans.push(Span::styled(text, cursor_style));
                continue;
            }
            if let Some(value) = chars.get(position) {
                let text = display_char(*value, remaining);
                drawn += cell_width(&text);
                spans.push(Span::styled(text, value_style));
            }
        }
        append_unfocused_hotkey(
            &mut spans,
            &mut drawn,
            width,
            self.input.hotkey.as_deref(),
            self.input.focused && self.input.insert_mode,
            self.input.pending_hotkey_prefix.as_deref(),
            hotkey_style,
        );

        Line::from(spans)
    }
}

impl<M> TuiNode<M> for TextInput<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let text = if self.value.is_empty() {
            placeholder_label(&self.placeholder, self.hotkey.as_deref())
        } else {
            label_with_visible_hotkey(
                &self.value,
                self.hotkey.as_deref(),
                !(self.focused && self.insert_mode),
            )
        };
        let width = line_width(&Line::from(text.as_str())).min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(width.max(1), 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(INPUT_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(INPUT_FOCUS), area, true);
        }
        ctx.set_focus_suppresses_global_hotkeys(FocusId::new(INPUT_FOCUS), self.insert_mode);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            self.handle_visual_hotkey(hotkey, ctx);
            if self.handle_focus_hotkey(hotkey, ctx) {
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if let TuiEvent::ExternalEditor(response) = event {
            self.apply_external_editor_response(response);
            self.cursor_fade.reset();
            ctx.request_clear();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if let TuiEvent::Paste(value) = event {
            if !self.insert_mode {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            let outcome = self.on_paste(value);
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.external_editor_key_matches(*key) {
            ctx.request_external_editor(self.value.clone(), 1, self.cursor + 1);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if !self.insert_mode {
            if focus_navigation_key(*key) {
                return EventOutcome::Ignored;
            }
            if KeySpec::key(Key::Enter).matches(*key) {
                self.insert_mode = true;
                self.cursor_fade.reset();
                ctx.request_layout();
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            if matches_any(&self.keys.cancel, *key) {
                self.cursor_fade.reset();
                ctx.request_redraw();
                return EventOutcome::Ignored;
            }
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if matches_any(&self.keys.cancel, *key) {
            self.insert_mode = false;
            self.cursor_fade.reset();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        if outcome.submitted {
            self.insert_mode = false;
            ctx.request_layout();
            if let Some(on_submit) = &self.on_submit {
                ctx.emit(on_submit(self.value.clone()));
            }
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
        self.set_focused(focused);
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

impl<M> TuiNode<M> for PasswordInput<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let text = if self.input.value.is_empty() {
            placeholder_label(&self.input.placeholder, self.input.hotkey.as_deref())
        } else {
            let value = std::iter::repeat(self.mask_char)
                .take(self.input.len_chars())
                .collect::<String>();
            label_with_visible_hotkey(
                &value,
                self.input.hotkey.as_deref(),
                !(self.input.focused && self.input.insert_mode),
            )
        };
        let width = line_width(&Line::from(text.as_str())).min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(width.max(1), 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if let Some(hotkey) = self.input.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(PASSWORD_INPUT_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(PASSWORD_INPUT_FOCUS), area, true);
        }
        ctx.set_focus_suppresses_global_hotkeys(
            FocusId::new(PASSWORD_INPUT_FOCUS),
            self.input.insert_mode,
        );
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            self.input.handle_visual_hotkey(hotkey, ctx);
            if self.input.handle_focus_hotkey(hotkey, ctx) {
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if let TuiEvent::Paste(value) = event {
            if !self.input.insert_mode {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            let outcome = self.on_paste(value);
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if !self.input.insert_mode {
            if focus_navigation_key(*key) {
                return EventOutcome::Ignored;
            }
            if KeySpec::key(Key::Enter).matches(*key) {
                self.input.insert_mode = true;
                self.input.cursor_fade.reset();
                ctx.request_layout();
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            if matches_any(&self.input.keys.cancel, *key) {
                self.input.cursor_fade.reset();
                ctx.request_redraw();
                return EventOutcome::Ignored;
            }
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if matches_any(&self.input.keys.cancel, *key) {
            self.input.insert_mode = false;
            self.input.cursor_fade.reset();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key);
        if outcome.submitted {
            self.input.insert_mode = false;
            ctx.request_layout();
            if let Some(on_submit) = &self.input.on_submit {
                ctx.emit(on_submit(self.input.value.clone()));
            }
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
        self.input.set_focused(focused);
        if focused {
            self.input.cursor_fade.reset();
        } else if let Some(on_blur) = &self.input.on_blur {
            ctx.emit(on_blur(self.input.value.clone()));
        }
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<M> Animated for PasswordInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.input
            .cursor_fade
            .tick(self.input.focused && self.input.insert_mode, dt, settings)
    }
}

impl<M> Animated for TextInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.cursor_fade
            .tick(self.focused && self.insert_mode, dt, settings)
    }
}

pub(crate) fn placeholder_line(
    placeholder: &str,
    hotkey: Option<&str>,
    width: usize,
    focused: bool,
    active_hotkey_prefix: Option<&str>,
    cursor_style: Style,
    placeholder_style: Style,
) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }

    let mut chars = placeholder.chars();
    if !focused {
        return Line::from(truncated_placeholder_spans(
            placeholder,
            hotkey,
            width,
            active_hotkey_prefix,
            placeholder_style,
        ));
    }

    let first = chars.next().unwrap_or(' ');
    let mut spans = vec![Span::styled(first.to_string(), cursor_style)];
    let remaining = width.saturating_sub(cell_width(&first.to_string()));
    spans.extend(truncated_placeholder_spans(
        &chars.collect::<String>(),
        hotkey,
        remaining,
        active_hotkey_prefix,
        placeholder_style,
    ));
    Line::from(spans)
}

pub(crate) fn selected_input_style(style: Style) -> Style {
    let theme = theme();
    style
        .fg(theme.highlight_fg())
        .bg(theme.highlight_bg())
        .add_modifier(Modifier::BOLD)
}

pub(crate) fn focus_navigation_key(key: KeyEvent) -> bool {
    matches!(key.code, Key::Tab | Key::BackTab)
}

pub(crate) fn placeholder_label(placeholder: &str, hotkey: Option<&str>) -> String {
    hotkey_label_spans(
        placeholder,
        hotkey,
        HotkeyLabelMode::Inline,
        None,
        Style::default(),
        Style::default(),
    )
    .into_iter()
    .map(|span| span.content.into_owned())
    .collect()
}

pub(crate) fn label_with_visible_hotkey(
    label: &str,
    hotkey: Option<&str>,
    visible: bool,
) -> String {
    if !visible {
        return label.to_owned();
    }

    hotkey_label_spans(
        label,
        hotkey,
        HotkeyLabelMode::Inline,
        None,
        Style::default(),
        Style::default(),
    )
    .into_iter()
    .map(|span| span.content.into_owned())
    .collect()
}

fn truncated_placeholder_spans(
    placeholder: &str,
    hotkey: Option<&str>,
    width: usize,
    active_hotkey_prefix: Option<&str>,
    placeholder_style: Style,
) -> Vec<Span<'static>> {
    let mut drawn = 0;
    let mut spans = Vec::new();
    for span in hotkey_label_spans(
        placeholder,
        hotkey,
        HotkeyLabelMode::Inline,
        active_hotkey_prefix,
        placeholder_style,
        hotkey_underline_style(placeholder_style),
    ) {
        if drawn >= width {
            break;
        }
        let text = truncate_cells(&span.content, width - drawn);
        if text.is_empty() {
            continue;
        }
        drawn += cell_width(&text);
        spans.push(Span::styled(text, span.style));
    }
    spans
}

pub(crate) fn append_unfocused_hotkey(
    spans: &mut Vec<Span<'static>>,
    drawn: &mut usize,
    width: usize,
    hotkey: Option<&str>,
    focused: bool,
    active_hotkey_prefix: Option<&str>,
    style: Style,
) {
    let Some(hotkey) = hotkey else {
        return;
    };
    if focused || *drawn >= width {
        return;
    }

    for span in hotkey_label_spans(
        "",
        Some(hotkey),
        HotkeyLabelMode::Inline,
        active_hotkey_prefix,
        style,
        hotkey_underline_style(style),
    ) {
        if *drawn >= width {
            break;
        }
        let text = truncate_cells(&span.content, width - *drawn);
        if text.is_empty() {
            continue;
        }
        *drawn += cell_width(&text);
        spans.push(Span::styled(text, span.style));
    }
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

pub(crate) fn text_char(key: KeyEvent) -> bool {
    !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
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
    truncate_cells(&text, max_width)
}

pub(crate) fn cell_width(value: &str) -> usize {
    line_width(&Line::from(value))
}

pub(crate) fn visible_start_for_cursor(chars: &[char], cursor: usize, width: usize) -> usize {
    let mut start = cursor.min(chars.len());
    let mut drawn = 1;
    while start > 0 {
        let char_width = cell_width(&chars[start - 1].to_string()).max(1);
        if drawn + char_width > width {
            break;
        }
        drawn += char_width;
        start -= 1;
    }
    start
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();
    for ch in value.chars() {
        let ch_width = cell_width(&ch.to_string());
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }
    truncated
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod tests;
