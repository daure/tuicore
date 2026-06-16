use tuirealm::command::{Cmd, CmdResult, Direction as CmdDirection, Position};
use tuirealm::component::Component;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::{State, StateValue};

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputOutcome {
    pub handled: bool,
    pub changed: bool,
    pub submitted: bool,
    pub canceled: bool,
}

impl InputOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        submitted: false,
        canceled: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        submitted: false,
        canceled: false,
    };

    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
        submitted: false,
        canceled: false,
    };

    pub const SUBMITTED: Self = Self {
        handled: true,
        changed: false,
        submitted: true,
        canceled: false,
    };

    pub const CANCELED: Self = Self {
        handled: true,
        changed: false,
        submitted: false,
        canceled: true,
    };

    pub fn needs_redraw(self) -> bool {
        self.handled || self.changed || self.submitted || self.canceled
    }
}

#[derive(Debug, Clone)]
pub struct TextInput {
    value: String,
    placeholder: String,
    cursor: usize,
    focused: bool,
    max_len: Option<usize>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            cursor: 0,
            focused: false,
            max_len: None,
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

    pub fn max_len(mut self, max_len: usize) -> Self {
        self.max_len = Some(max_len);
        self.clamp_value();
        self.cursor = self.cursor.min(self.len_chars());
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn on_key(&mut self, key: KeyEvent) -> InputOutcome {
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

        match key.code {
            Key::Char(value) if text_char(key) => self.insert_char(value),
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_next(),
            Key::Left => self.move_left(),
            Key::Right => self.move_right(),
            Key::Home => self.move_to(0),
            Key::End => self.move_to(self.len_chars()),
            Key::Enter => InputOutcome::SUBMITTED,
            Key::Esc => InputOutcome::CANCELED,
            _ => InputOutcome::IDLE,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(Paragraph::new(self.line(area.width as usize)), area);
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
        let cursor_style = Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg())
            .add_modifier(Modifier::BOLD);

        if self.value.is_empty() {
            if self.focused {
                let mut spans = vec![Span::styled(" ", cursor_style)];
                let hint: String = self
                    .placeholder
                    .chars()
                    .take(width.saturating_sub(1))
                    .collect();
                if !hint.is_empty() {
                    spans.push(Span::styled(hint, placeholder_style));
                }
                return Line::from(spans);
            }
            return Line::from(Span::styled(
                self.placeholder.chars().take(width).collect::<String>(),
                placeholder_style,
            ));
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
            if self.focused && position == self.cursor {
                let text = chars.get(position).copied().unwrap_or(' ').to_string();
                spans.push(Span::styled(text, cursor_style));
                drawn += 1;
                continue;
            }
            if let Some(value) = chars.get(position) {
                spans.push(Span::styled(value.to_string(), value_style));
                drawn += 1;
            }
        }

        Line::from(spans)
    }

    fn insert_char(&mut self, value: char) -> InputOutcome {
        if self
            .max_len
            .is_some_and(|max_len| self.len_chars() >= max_len)
        {
            return InputOutcome::HANDLED;
        }
        self.value.insert(self.byte_index(self.cursor), value);
        self.cursor += 1;
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

    fn state_value(&self) -> State {
        State::Single(StateValue::String(self.value.clone()))
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
}

impl Component for TextInput {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.render(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            Attribute::Text => Some(QueryResult::Owned(AttrValue::String(self.value.clone()))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Focus, AttrValue::Flag(focused)) => self.focused = focused,
            (Attribute::Text, AttrValue::String(value)) => {
                self.value = value.replace('\n', " ");
                self.clamp_value();
                self.cursor = self.cursor.min(self.len_chars());
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        self.state_value()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        match cmd {
            Cmd::Type(value) => {
                let outcome = self.insert_char(value);
                self.cmd_changed(outcome)
            }
            Cmd::Move(CmdDirection::Left) => {
                let outcome = self.move_left();
                self.cmd_visual(outcome)
            }
            Cmd::Move(CmdDirection::Right) => {
                let outcome = self.move_right();
                self.cmd_visual(outcome)
            }
            Cmd::GoTo(Position::Begin) => {
                let outcome = self.move_to(0);
                self.cmd_visual(outcome)
            }
            Cmd::GoTo(Position::End) => {
                let end = self.len_chars();
                let outcome = self.move_to(end);
                self.cmd_visual(outcome)
            }
            Cmd::GoTo(Position::At(cursor)) => {
                let outcome = self.move_to(cursor);
                self.cmd_visual(outcome)
            }
            Cmd::Delete => {
                let outcome = self.delete_next();
                self.cmd_changed(outcome)
            }
            Cmd::Submit => CmdResult::Submit(self.state_value()),
            Cmd::Cancel => CmdResult::Custom("cancel", self.state_value()),
            Cmd::None => CmdResult::NoChange,
            _ => CmdResult::Invalid(cmd),
        }
    }
}

impl TextInput {
    fn cmd_changed(&self, outcome: InputOutcome) -> CmdResult {
        if outcome.changed {
            CmdResult::Changed(self.state_value())
        } else if outcome.handled {
            CmdResult::NoChange
        } else {
            CmdResult::Invalid(Cmd::None)
        }
    }

    fn cmd_visual(&self, outcome: InputOutcome) -> CmdResult {
        if outcome.handled {
            CmdResult::Visual
        } else {
            CmdResult::NoChange
        }
    }
}

pub(crate) fn is_ctrl(key: KeyEvent, value: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, Key::Char(key_value) if key_value.eq_ignore_ascii_case(&value))
}

pub(crate) fn text_char(key: KeyEvent) -> bool {
    !key.modifiers.contains(KeyModifiers::CONTROL) && !key.modifiers.contains(KeyModifiers::ALT)
}
