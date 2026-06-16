use tuirealm::command::{Cmd, CmdResult, Direction as CmdDirection, Position};
use tuirealm::component::Component;
use tuirealm::event::{Key, KeyEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::Rect;
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::Paragraph;
use tuirealm::state::{State, StateValue};

use crate::theme;

use super::text_input::{InputOutcome, is_ctrl, text_char};

#[derive(Debug, Clone)]
pub struct TextareaInput {
    value: String,
    placeholder: String,
    cursor: usize,
    focused: bool,
    max_lines: Option<usize>,
}

impl Default for TextareaInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextareaInput {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            placeholder: String::new(),
            cursor: 0,
            focused: false,
            max_lines: None,
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

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines.max(1));
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
        self
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn on_key(&mut self, key: KeyEvent) -> InputOutcome {
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

        match key.code {
            Key::Char(value) if text_char(key) => self.insert_char(value),
            Key::Enter => self.insert_newline(),
            Key::Backspace => self.backspace(),
            Key::Delete => self.delete_next(),
            Key::Left => self.move_left(),
            Key::Right => self.move_right(),
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
        let cursor_style = Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg())
            .add_modifier(Modifier::BOLD);

        if self.value.is_empty() {
            let mut first = if self.focused {
                vec![Span::styled(" ", cursor_style)]
            } else {
                Vec::new()
            };
            let hint_width = width.saturating_sub(first.len());
            let hint: String = self.placeholder.chars().take(hint_width).collect();
            if !hint.is_empty() {
                first.push(Span::styled(hint, placeholder_style));
            }
            let mut lines = vec![Line::from(first)];
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

impl Component for TextareaInput {
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
                self.value = value;
                self.clamp_lines();
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
            Cmd::Move(CmdDirection::Up) => {
                let outcome = self.move_vertical(-1);
                self.cmd_visual(outcome)
            }
            Cmd::Move(CmdDirection::Down) => {
                let outcome = self.move_vertical(1);
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

impl TextareaInput {
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
    key.modifiers
        .contains(tuirealm::event::KeyModifiers::CONTROL)
}
