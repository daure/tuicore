use ratatui::Frame;
use ratatui::layout::Rect;
use std::ops::Deref;
use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, TickResult};
use crate::event::{
    HotkeyEvent, Key, KeyEvent, KeyModifiers, MouseButton, MouseEventKind, TuiEvent,
};
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, FocusRequest, KeySpec, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSizeHint, TuiNode, line_width,
};
use crate::{ScrollAxes, ScrollOffset, ScrollSize, ScrollState, preset, theme, ui::keybindings};

use super::Panel;
use super::text_input::{
    CursorFade, InputChrome, InputOutcome, append_unfocused_hotkey, cell_width, display_char,
    focus_navigation_key, label_with_visible_hotkey, placeholder_label, placeholder_line,
    selected_input_style, text_char, visible_start_for_cursor,
};

const TEXTAREA_FOCUS: &str = "textarea";
const TAB_INSERT: &str = "    ";

pub struct TextareaInput<M = ()> {
    value: String,
    placeholder: String,
    hotkey: Option<String>,
    cursor: usize,
    focused: bool,
    insert_mode: bool,
    max_lines: Option<usize>,
    min_rows: usize,
    max_rows: Option<usize>,
    scroll: ScrollState,
    area: Rect,
    outer_area: Rect,
    chrome: InputChrome,
    panel: Panel,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_blur: Option<Box<dyn Fn(String) -> M>>,
    external_editor_key: Option<KeyEvent>,
    keys: TextareaInputKeyBindings,
    cursor_fade: CursorFade,
    pending_hotkey_prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextareaInputKeyBindings {
    pub submit: Vec<KeySpec>,
    pub cancel: Vec<KeySpec>,
    pub clear: Vec<KeySpec>,
    pub move_line_start: Vec<KeySpec>,
    pub move_line_end: Vec<KeySpec>,
    pub move_left: Vec<KeySpec>,
    pub move_right: Vec<KeySpec>,
    pub move_up: Vec<KeySpec>,
    pub move_down: Vec<KeySpec>,
    pub move_previous_word: Vec<KeySpec>,
    pub move_next_word: Vec<KeySpec>,
    pub delete_before_line: Vec<KeySpec>,
    pub delete_after_line: Vec<KeySpec>,
    pub delete_previous_word: Vec<KeySpec>,
    pub delete_next_word: Vec<KeySpec>,
    pub backspace: Vec<KeySpec>,
    pub delete_next: Vec<KeySpec>,
    pub insert_tab: Vec<KeySpec>,
    pub insert_newline: Vec<KeySpec>,
}

impl Default for TextareaInputKeyBindings {
    fn default() -> Self {
        Self {
            submit: vec![
                KeySpec::key_with_modifiers(Key::Char('d'), KeyModifiers::CONTROL),
                KeySpec::key(Key::Enter),
            ],
            cancel: vec![
                KeySpec::key(Key::Esc),
                KeySpec::key_with_modifiers(Key::Char('['), KeyModifiers::CONTROL),
            ],
            clear: vec![KeySpec::key_with_modifiers(
                Key::Char('c'),
                KeyModifiers::CONTROL,
            )],
            move_line_start: vec![
                KeySpec::key_with_modifiers(Key::Char('a'), KeyModifiers::CONTROL),
                KeySpec::key(Key::Home),
            ],
            move_line_end: vec![
                KeySpec::key_with_modifiers(Key::Char('e'), KeyModifiers::CONTROL),
                KeySpec::key(Key::End),
            ],
            move_left: vec![KeySpec::key(Key::Left)],
            move_right: vec![KeySpec::key(Key::Right)],
            move_up: vec![
                KeySpec::key(Key::Up),
                KeySpec::key_with_modifiers(Key::Char('p'), KeyModifiers::CONTROL),
            ],
            move_down: vec![
                KeySpec::key(Key::Down),
                KeySpec::key_with_modifiers(Key::Char('n'), KeyModifiers::CONTROL),
            ],
            move_previous_word: vec![
                KeySpec::key_with_modifiers(Key::Char('b'), KeyModifiers::ALT),
                KeySpec::key_with_modifiers(Key::Left, KeyModifiers::CONTROL),
            ],
            move_next_word: vec![
                KeySpec::key_with_modifiers(Key::Char('f'), KeyModifiers::ALT),
                KeySpec::key_with_modifiers(Key::Right, KeyModifiers::CONTROL),
            ],
            delete_before_line: vec![KeySpec::key_with_modifiers(
                Key::Char('u'),
                KeyModifiers::CONTROL,
            )],
            delete_after_line: vec![KeySpec::key_with_modifiers(
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
            insert_newline: vec![
                KeySpec::key_with_modifiers(Key::Enter, KeyModifiers::CONTROL),
                KeySpec::key_with_modifiers(Key::Char('j'), KeyModifiers::CONTROL),
            ],
        }
    }
}

impl TextareaInputKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }
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
            hotkey: None,
            cursor: 0,
            focused: false,
            insert_mode: false,
            max_lines: None,
            min_rows: 1,
            max_rows: None,
            scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            area: Rect::default(),
            outer_area: Rect::default(),
            chrome: InputChrome::Plain,
            panel: Panel::new(),
            on_submit: None,
            on_blur: None,
            external_editor_key: Some(ctrl_key('o')),
            keys: TextareaInputKeyBindings::default(),
            cursor_fade: CursorFade::default(),
            pending_hotkey_prefix: None,
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

    pub fn style(mut self, chrome: InputChrome) -> Self {
        self.set_style(chrome);
        self
    }

    pub fn panel(mut self, title: impl Into<String>) -> Self {
        self.set_style(InputChrome::panel(title));
        self
    }

    pub fn set_style(&mut self, chrome: InputChrome) {
        self.chrome = chrome;
        self.sync_panel();
    }

    fn sync_panel(&mut self) {
        self.panel = match &self.chrome {
            InputChrome::Plain => Panel::new(),
            InputChrome::Panel(panel) => panel.panel(self.focused, self.hotkey.as_deref()),
        };
    }

    fn inline_hotkey(&self) -> Option<&str> {
        match self.chrome {
            InputChrome::Plain => self.hotkey.as_deref(),
            InputChrome::Panel(_) => None,
        }
    }

    fn is_panel_mode(&self) -> bool {
        matches!(self.chrome, InputChrome::Panel(_))
    }

    fn panel_click_focus(&self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> bool {
        let TuiEvent::Mouse(mouse) = event else {
            return false;
        };
        if !self.is_panel_mode()
            || !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            || !rect_contains(self.outer_area, mouse.column, mouse.row)
        {
            return false;
        }

        ctx.focus(FocusRequest::Target(FocusId::new(TEXTAREA_FOCUS)));
        ctx.stop_propagation();
        true
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        self.hotkey = Some(hotkey.into());
        self.sync_panel();
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.pending_hotkey_prefix = None;
        self.sync_panel();
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
        self.scroll_cursor_into_view(disabled_animation_settings());
        self.cursor_fade.reset();
        ctx.request_layout();
        ctx.request_redraw();
        ctx.stop_propagation();
        true
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self.sync_panel();
        self
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.sync_panel();
        if !focused {
            self.insert_mode = false;
        }
    }

    pub fn insert_mode(&self) -> bool {
        self.insert_mode
    }

    pub fn set_insert_mode(&mut self, insert_mode: bool) {
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

    pub fn keybindings(mut self, keys: TextareaInputKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn set_keybindings(&mut self, keys: TextareaInputKeyBindings) {
        self.keys = keys;
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines.max(1));
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
        self
    }

    pub fn min_rows(mut self, min_rows: usize) -> Self {
        self.min_rows = self
            .max_rows
            .map_or(min_rows.max(1), |max_rows| min_rows.max(1).min(max_rows));
        self
    }

    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows.max(1));
        if let Some(max_rows) = self.max_rows
            && self.min_rows > max_rows
        {
            self.min_rows = max_rows;
        }
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

    pub fn on_paste(&mut self, value: impl AsRef<str>) -> InputOutcome {
        let outcome = self.insert_text(value.as_ref());
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
        if outcome.needs_redraw() {
            self.cursor_fade.reset();
        }
        outcome
    }

    fn on_key_inner(&mut self, key: KeyEvent) -> InputOutcome {
        if matches_any(&self.keys.submit, key) {
            return InputOutcome::SUBMITTED;
        }
        if matches_any(&self.keys.move_line_start, key) {
            return self.move_to(self.current_line().start);
        }
        if matches_any(&self.keys.clear, key) {
            return self.clear();
        }
        if matches_any(&self.keys.move_line_end, key) {
            return self.move_to(self.current_line().end);
        }
        if matches_any(&self.keys.delete_before_line, key) {
            return self.delete_before_line();
        }
        if matches_any(&self.keys.delete_after_line, key) {
            return self.delete_after_line();
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
        if matches_any(&self.keys.move_up, key) {
            return self.move_vertical(-1);
        }
        if matches_any(&self.keys.move_down, key) {
            return self.move_vertical(1);
        }
        if matches_any(&self.keys.insert_tab, key) {
            return self.insert_text(TAB_INSERT);
        }
        if matches_any(&self.keys.insert_newline, key) {
            return self.insert_newline();
        }
        if matches_any(&self.keys.backspace, key) {
            return self.backspace();
        }
        if matches_any(&self.keys.delete_next, key) || delete_forward_key(key) {
            return self.delete_next();
        }
        if matches_any(&self.keys.move_left, key) {
            return self.move_left();
        }
        if matches_any(&self.keys.move_right, key) {
            return self.move_right();
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
        if area.is_empty() {
            return;
        }

        let style = if self.focused && !self.insert_mode {
            selected_input_style(Style::default())
        } else {
            Style::default()
        };
        let area = self.render_chrome(frame, area);
        let geometry = self.scroll_geometry(area);
        let visible = self.visible_lines_from(
            geometry.layout.viewport.width as usize,
            geometry.layout.viewport.height as usize,
            self.scroll.offset().y,
        );
        frame.render_widget(
            Paragraph::new(visible.lines).style(style),
            geometry.layout.viewport,
        );
        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn content_area(&self, area: Rect) -> Rect {
        let height = self.visible_outer_height(area.height);
        let area = Rect::new(area.x, area.y, area.width, height);
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => Panel::inner_area(area),
        }
    }

    fn render_chrome(&self, frame: &mut Frame, area: Rect) -> Rect {
        let height = self.visible_outer_height(area.height);
        let area = Rect::new(area.x, area.y, area.width, height);
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => {
                self.panel.render(frame, area);
                Panel::inner_area(area)
            }
        }
    }

    fn chrome_measure(&self, width: u16, height: u16, proposal: LayoutProposal) -> LayoutSizeHint {
        let (width, height) = match self.chrome {
            InputChrome::Plain => (width, height),
            InputChrome::Panel(_) => (width.saturating_add(2), height.saturating_add(2)),
        };
        LayoutSizeHint::content(width, height).normalized(proposal)
    }

    fn visible_outer_height(&self, available: u16) -> u16 {
        let content = self
            .preferred_rows(self.line_ranges().len())
            .min(u16::MAX as usize) as u16;
        let height = match self.chrome {
            InputChrome::Plain => content,
            InputChrome::Panel(_) => content.saturating_add(2),
        };
        height.min(available)
    }

    fn visible_lines(&self, width: usize, height: usize) -> VisibleLines {
        if width == 0 || height == 0 {
            return VisibleLines::default();
        }

        let theme = theme();
        let selected = self.focused && !self.insert_mode;
        let placeholder_style = if selected {
            selected_input_style(Style::default().fg(theme.muted_fg()))
        } else {
            Style::default().fg(theme.muted_fg())
        };
        if self.value.is_empty() {
            let mut lines = vec![placeholder_line(
                &self.placeholder,
                self.inline_hotkey(),
                width,
                self.focused && self.insert_mode,
                self.pending_hotkey_prefix.as_deref(),
                self.cursor_fade.style(placeholder_style),
                placeholder_style,
            )];
            lines.resize_with(height, Line::default);
            return VisibleLines {
                lines,
                first_line: 0,
            };
        }

        let ranges = self.line_ranges();
        let (cursor_line, cursor_col) = self.cursor_line_col(&ranges);
        let first_line = cursor_line.saturating_add(1).saturating_sub(height);
        self.visible_lines_from_with_cursor(
            width,
            height,
            first_line,
            Some((cursor_line, cursor_col)),
        )
    }

    fn visible_lines_from(&self, width: usize, height: usize, first_line: usize) -> VisibleLines {
        if width == 0 || height == 0 {
            return VisibleLines::default();
        }
        if self.value.is_empty() {
            return self.visible_lines(width, height);
        }
        let ranges = self.line_ranges();
        let (cursor_line, cursor_col) = self.cursor_line_col(&ranges);
        self.visible_lines_from_with_cursor(
            width,
            height,
            first_line,
            Some((cursor_line, cursor_col)),
        )
    }

    fn visible_lines_from_with_cursor(
        &self,
        width: usize,
        height: usize,
        first_line: usize,
        cursor: Option<(usize, usize)>,
    ) -> VisibleLines {
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
        let hotkey_style = if selected {
            selected_input_style(Style::default())
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let cursor_style = self.cursor_fade.style(value_style);
        let ranges = self.line_ranges();
        let (cursor_line, cursor_col) = cursor.unwrap_or((usize::MAX, 0));
        let lines = ranges
            .iter()
            .enumerate()
            .skip(first_line)
            .take(height)
            .map(|(line_index, range)| {
                let line_chars = self
                    .value
                    .chars()
                    .skip(range.start)
                    .take(range.len())
                    .collect::<Vec<_>>();
                let horizontal = if self.focused && self.insert_mode && line_index == cursor_line {
                    visible_start_for_cursor(&line_chars, cursor_col, width)
                } else {
                    0
                };
                self.render_line(
                    *range,
                    line_index == cursor_line,
                    horizontal,
                    width,
                    value_style,
                    (!(self.focused && self.insert_mode)
                        && line_index == ranges.len().saturating_sub(1))
                    .then_some(hotkey_style),
                    cursor_style,
                )
            })
            .collect();
        VisibleLines { lines, first_line }
    }

    fn render_line(
        &self,
        range: LineRange,
        cursor_line: bool,
        horizontal: usize,
        width: usize,
        value_style: Style,
        hotkey_style: Option<Style>,
        cursor_style: Style,
    ) -> Line<'static> {
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut spans = Vec::new();
        let mut drawn = 0;

        for col in horizontal..=range.len() {
            if drawn >= width {
                break;
            }
            let remaining = width.saturating_sub(drawn);
            let position = range.start + col;
            if self.focused && self.insert_mode && cursor_line && position == self.cursor {
                let value = if position < range.end {
                    chars.get(position).copied().unwrap_or(' ')
                } else {
                    ' '
                };
                let text = display_char(value, remaining);
                let text = if text.is_empty() && remaining > 0 {
                    String::from(" ")
                } else {
                    text
                };
                drawn += cell_width(&text);
                spans.push(Span::styled(text, cursor_style));
                continue;
            }
            if position < range.end
                && let Some(value) = chars.get(position)
            {
                let text = display_char(*value, remaining);
                drawn += cell_width(&text);
                spans.push(Span::styled(text, value_style));
            }
        }
        if let Some(hotkey_style) = hotkey_style {
            append_unfocused_hotkey(
                &mut spans,
                &mut drawn,
                width,
                self.inline_hotkey(),
                self.focused && self.insert_mode,
                self.pending_hotkey_prefix.as_deref(),
                hotkey_style,
            );
        }

        Line::from(spans)
    }

    fn insert_char(&mut self, value: char) -> InputOutcome {
        self.insert_text(value.to_string())
    }

    fn insert_text(&mut self, value: impl AsRef<str>) -> InputOutcome {
        let value = value.as_ref();
        if value.is_empty() {
            return InputOutcome::HANDLED;
        }
        let len = value.chars().count();
        self.value.insert_str(self.byte_index(self.cursor), value);
        self.cursor += len;
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

    fn content_size(&self, area: Rect) -> ScrollSize {
        ScrollSize::new(area.width as usize, self.line_ranges().len())
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let height = self.max_rows.map_or(area.height, |max_rows| {
            (area.height as usize).min(max_rows) as u16
        });
        Rect::new(area.x, area.y, area.width, height)
    }

    fn scroll_geometry(&self, area: Rect) -> crate::ScrollGeometry {
        let area = self.scroll_area(area);
        self.scroll.geometry(area, self.content_size(area))
    }

    fn has_vertical_overflow(&self) -> bool {
        let geometry = self.scroll_geometry(self.area);
        geometry.content.height > geometry.viewport.height
    }

    fn scroll_page_key(key: KeyEvent) -> bool {
        let bindings = keybindings();
        bindings.page_up_matches(key) || bindings.page_down_matches(key)
    }

    fn handle_scroll_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) -> bool {
        if self.area.is_empty() || !self.has_vertical_overflow() || !Self::scroll_page_key(key) {
            return false;
        }

        let geometry = self.scroll_geometry(self.area);
        let outcome = self
            .scroll
            .on_key(key, geometry.viewport, geometry.content, ctx.animation());
        if outcome.handled {
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            ctx.stop_propagation();
            return true;
        }
        false
    }

    fn scroll_cursor_into_view(&mut self, settings: AnimationSettings) -> bool {
        if self.area.is_empty() {
            return false;
        }
        let geometry = self.scroll_geometry(self.area);
        let ranges = self.line_ranges();
        let (cursor_line, _) = self.cursor_line_col(&ranges);
        let offset = self.scroll.target_offset();
        let viewport_height = geometry.viewport.height;
        let target_y = if cursor_line < offset.y {
            cursor_line
        } else if cursor_line >= offset.y.saturating_add(viewport_height) {
            cursor_line
                .saturating_add(1)
                .saturating_sub(viewport_height)
        } else {
            offset.y
        };
        self.scroll
            .scroll_to(
                ScrollOffset::new(offset.x, target_y),
                geometry.viewport,
                geometry.content,
                settings,
            )
            .changed
    }

    fn preferred_rows(&self, content_rows: usize) -> usize {
        let rows = content_rows.max(self.min_rows);
        self.max_rows.map_or(rows, |max_rows| rows.min(max_rows))
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

    fn external_editor_key_matches(&self, key: KeyEvent) -> bool {
        self.external_editor_key
            .is_some_and(|expected| key_matches(expected, key))
    }

    fn external_editor_request_position(&self) -> (usize, usize) {
        let ranges = self.line_ranges();
        let (line, col) = self.cursor_line_col(&ranges);
        (line + 1, col + 1)
    }

    fn apply_external_editor_response(&mut self, response: &crate::ExternalEditorResponse) {
        self.value = response.value.clone();
        self.clamp_lines();
        let ranges = self.line_ranges();
        let line_idx = response
            .line
            .saturating_sub(1)
            .min(ranges.len().saturating_sub(1));
        let range = ranges[line_idx];
        let col = response.col.saturating_sub(1).min(range.len());
        self.cursor = (range.start + col).min(self.len_chars());
    }
}

impl<M> TuiNode<M> for TextareaInput<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let lines = if self.value.is_empty() {
            vec![placeholder_label(&self.placeholder, self.inline_hotkey())]
        } else {
            let show_hotkey = !(self.focused && self.insert_mode);
            let mut lines = self
                .value
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if let Some(line) = lines.last_mut() {
                *line = label_with_visible_hotkey(line, self.inline_hotkey(), show_hotkey);
            }
            lines
        };
        let width = lines
            .iter()
            .map(|line| line_width(&Line::from(line.as_str())))
            .max()
            .unwrap_or(1)
            .min(u16::MAX as usize) as u16;
        let height = self.preferred_rows(lines.len()).min(u16::MAX as usize) as u16;
        self.chrome_measure(width.max(1), height, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.outer_area = area;
        self.area = self.content_area(area);
        self.scroll_cursor_into_view(disabled_animation_settings());
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_text_entry_focusable_with_hotkey_sequences(
                FocusId::new(TEXTAREA_FOCUS),
                self.area,
                true,
                vec![hotkey],
                self.insert_mode,
            );
        } else {
            ctx.register_text_entry_focusable(
                FocusId::new(TEXTAREA_FOCUS),
                self.area,
                true,
                self.insert_mode,
            );
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.panel_click_focus(event, ctx) {
            return EventOutcome::Handled;
        }
        if let TuiEvent::Hotkey(hotkey) = event {
            self.handle_visual_hotkey(hotkey, ctx);
            if self.handle_focus_hotkey(hotkey, ctx) {
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if let TuiEvent::ExternalEditor(response) = event {
            self.apply_external_editor_response(response);
            self.scroll_cursor_into_view(disabled_animation_settings());
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
            let scrolled = self.scroll_cursor_into_view(disabled_animation_settings());
            if outcome.changed {
                ctx.request_layout();
            }
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if scrolled {
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
        if self.handle_scroll_key(*key, ctx) {
            return EventOutcome::Handled;
        }
        if self.external_editor_key_matches(*key) {
            let (line, col) = self.external_editor_request_position();
            ctx.request_external_editor(self.value.clone(), line, col);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if delete_forward_key(*key) {
            self.insert_mode = true;
            let outcome = self.on_key(*key);
            let scrolled = self.scroll_cursor_into_view(disabled_animation_settings());
            ctx.request_layout();
            if outcome.needs_redraw() || scrolled {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }
        if !self.insert_mode {
            if focus_navigation_key(*key) {
                return EventOutcome::Ignored;
            }
            if KeySpec::key(Key::Enter).matches(*key)
                || matches_any(&self.keys.insert_newline, *key)
            {
                self.insert_mode = true;
                self.scroll_cursor_into_view(disabled_animation_settings());
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
        let scrolled = self.scroll_cursor_into_view(disabled_animation_settings());
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
        if outcome.changed {
            ctx.request_layout();
        }
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if scrolled {
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
        self.panel.set_focused(focused, ctx.animation());
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
        self.cursor_fade
            .tick(self.focused && self.insert_mode, dt, settings)
            .merge(self.scroll.tick(dt, settings))
            .merge(Animated::tick(&mut self.panel, dt, settings))
    }
}

#[derive(Debug, Clone, Copy)]
struct LineRange {
    start: usize,
    end: usize,
}

#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
struct VisibleLines {
    lines: Vec<Line<'static>>,
    first_line: usize,
}

impl Deref for VisibleLines {
    type Target = [Line<'static>];

    fn deref(&self) -> &Self::Target {
        &self.lines
    }
}

impl LineRange {
    fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

fn ctrl_key(value: char) -> KeyEvent {
    KeyEvent {
        code: Key::Char(value),
        modifiers: KeyModifiers::CONTROL,
    }
}

fn rect_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.right() && y >= area.y && y < area.bottom()
}

fn disabled_animation_settings() -> AnimationSettings {
    AnimationSettings {
        enabled: false,
        ..AnimationSettings::default()
    }
}

fn delete_forward_key(key: KeyEvent) -> bool {
    if matches!(key.code, Key::Char('\u{7f}')) {
        return !key.modifiers.contains(KeyModifiers::ALT);
    }
    key.code == Key::Delete
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

fn key_matches(expected: KeyEvent, actual: KeyEvent) -> bool {
    expected.modifiers == actual.modifiers
        && match (expected.code, actual.code) {
            (Key::Char(expected), Key::Char(actual)) => expected.eq_ignore_ascii_case(&actual),
            _ => expected.code == actual.code,
        }
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

#[cfg(test)]
#[path = "textarea_input_tests.rs"]
mod tests;
