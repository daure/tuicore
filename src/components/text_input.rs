use ratatui::Frame;
use ratatui::layout::Rect;
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, Easing, TickResult, lerp_color};
use crate::event::{
    HotkeyEvent, Key, KeyEvent, KeyModifiers, MouseButton, MouseEventKind, TuiEvent,
};
use crate::theme;
use crate::{
    EventCtx, EventOutcome, FocusCtx, FocusId, FocusRequest, HotkeyLabelMode, KeySpec, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, TuiNode, hotkey_label_spans,
    hotkey_underline_style, line_width,
};

use super::Panel;

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
    disabled: bool,
    hotkey: Option<String>,
    hotkey_focus_enabled: bool,
    cursor: usize,
    focused: bool,
    insert_mode: bool,
    max_len: Option<usize>,
    on_change: Option<Box<dyn Fn(String) -> M>>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_edit_end: Option<Box<dyn Fn(String) -> M>>,
    external_editor_key: Option<KeyEvent>,
    keys: TextInputKeyBindings,
    cursor_fade: CursorFade,
    pending_hotkey_prefix: Option<String>,
    chrome: InputChrome,
    panel: Panel,
    area: Rect,
}

pub struct PasswordInput<M = ()> {
    input: TextInput<M>,
    mask_char: char,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputChrome {
    Plain,
    Panel(InputPanelChrome),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputPanelChrome {
    top_left: Option<String>,
    top_right: Option<String>,
    bottom_left: Option<String>,
    bottom_right: Option<String>,
}

impl InputChrome {
    pub fn plain() -> Self {
        Self::Plain
    }

    pub fn panel(title: impl Into<String>) -> Self {
        Self::Panel(InputPanelChrome::new().top_left(title))
    }

    pub fn panel_chrome(chrome: InputPanelChrome) -> Self {
        Self::Panel(chrome)
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        if let Self::Panel(panel) = &mut self {
            panel.top_left = Some(title.into());
        }
        self
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        if let Self::Panel(panel) = &mut self {
            panel.top_right = Some(title.into());
        }
        self
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        if let Self::Panel(panel) = &mut self {
            panel.bottom_left = Some(title.into());
        }
        self
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        if let Self::Panel(panel) = &mut self {
            panel.bottom_right = Some(title.into());
        }
        self
    }
}

impl InputPanelChrome {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.top_left = Some(title.into());
        self
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.top_right = Some(title.into());
        self
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.bottom_left = Some(title.into());
        self
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.bottom_right = Some(title.into());
        self
    }

    pub(crate) fn panel(&self, focused: bool, hotkey: Option<&str>) -> Panel {
        let mut panel = Panel::new().focused(focused);
        if let Some(title) = &self.top_left {
            panel = panel.top_left(title.clone());
        }
        if let Some(title) = &self.top_right {
            panel = panel.top_right(title.clone());
        }
        if let Some(title) = &self.bottom_left {
            panel = panel.bottom_left(title.clone());
        }
        if let Some(title) = &self.bottom_right {
            panel = panel.bottom_right(title.clone());
        }
        if let Some(hotkey) = hotkey {
            panel = panel.hotkey(hotkey.to_owned());
        }
        panel
    }
}

impl Default for InputChrome {
    fn default() -> Self {
        Self::Plain
    }
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
            submit: vec![
                KeySpec::key(Key::Enter),
                KeySpec::key_with_modifiers(Key::Enter, KeyModifiers::CONTROL),
            ],
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
            disabled: false,
            hotkey: None,
            hotkey_focus_enabled: true,
            cursor: 0,
            focused: false,
            insert_mode: false,
            max_len: None,
            on_change: None,
            on_submit: None,
            on_edit_end: None,
            external_editor_key: Some(ctrl_key('o')),
            keys: TextInputKeyBindings::default(),
            cursor_fade: CursorFade::default(),
            pending_hotkey_prefix: None,
            chrome: InputChrome::Plain,
            panel: Panel::new(),
            area: Rect::default(),
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.set_disabled(disabled);
        self
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
        if disabled {
            self.insert_mode = false;
        }
        self.cursor_fade.reset();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn cursor_visible(&self) -> bool {
        self.focused && (self.insert_mode || self.disabled)
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
        let mut panel = match &self.chrome {
            InputChrome::Plain => Panel::new(),
            InputChrome::Panel(panel) => panel.panel(self.focused, self.hotkey.as_deref()),
        };
        panel.set_pending_hotkey_prefix(self.pending_hotkey_prefix.clone());
        self.panel = panel;
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

    fn panel_click_focus(
        &self,
        event: &TuiEvent,
        focus_id: FocusId,
        ctx: &mut EventCtx<M>,
    ) -> bool {
        let TuiEvent::Mouse(mouse) = event else {
            return false;
        };
        if !self.is_panel_mode()
            || !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
            || !rect_contains(self.area, mouse.column, mouse.row)
        {
            return false;
        }

        ctx.focus(FocusRequest::TargetAt {
            path: ctx.current_path(),
            id: focus_id,
        });
        ctx.stop_propagation();
        true
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub(crate) fn hotkey_focus_enabled(mut self, enabled: bool) -> Self {
        self.hotkey_focus_enabled = enabled;
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
                self.sync_panel();
                ctx.request_redraw();
            }
            HotkeyEvent::Canceled | HotkeyEvent::Commit(_) => {
                if self.pending_hotkey_prefix.take().is_some() {
                    self.sync_panel();
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

    pub(crate) fn external_editor_request(&self) -> (String, usize, usize) {
        (self.value.clone(), 1, self.cursor + 1)
    }

    pub(crate) fn set_insert_mode(&mut self, insert_mode: bool) {
        self.insert_mode = insert_mode;
        self.cursor_fade.reset();
    }

    pub fn on_submit(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.on_submit = Some(Box::new(handler));
        self
    }

    pub fn on_change(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn on_edit_end(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.on_edit_end = Some(Box::new(handler));
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
        if self.disabled {
            return InputOutcome::HANDLED;
        }
        let outcome = self.insert_text(value.as_ref().replace('\n', " "));
        if outcome.needs_redraw() {
            self.cursor_fade.reset();
        }
        outcome
    }

    fn on_key_inner(&mut self, key: KeyEvent) -> InputOutcome {
        if self.disabled {
            return self.on_disabled_key(key);
        }
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
        if matches_any(&self.keys.delete_next, key) || delete_forward_key(key) {
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

    fn on_disabled_key(&mut self, key: KeyEvent) -> InputOutcome {
        if KeySpec::key(Key::Enter).matches(key) || matches_any(&self.keys.submit, key) {
            return InputOutcome::SUBMITTED;
        }
        if matches_any(&self.keys.move_start, key) {
            return self.move_to(0);
        }
        if matches_any(&self.keys.move_end, key) {
            return self.move_to(self.len_chars());
        }
        if matches_any(&self.keys.move_previous_word, key) {
            return self.move_previous_word();
        }
        if matches_any(&self.keys.move_next_word, key) {
            return self.move_next_word();
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
        if matches_any(&self.keys.clear, key)
            || matches_any(&self.keys.delete_before_cursor, key)
            || matches_any(&self.keys.delete_after_cursor, key)
            || matches_any(&self.keys.delete_previous_word, key)
            || matches_any(&self.keys.delete_next_word, key)
            || matches_any(&self.keys.insert_tab, key)
            || matches_any(&self.keys.backspace, key)
            || matches_any(&self.keys.delete_next, key)
            || delete_forward_key(key)
            || matches!(key.code, Key::Char(_) if text_char(key))
        {
            return InputOutcome::HANDLED;
        }
        InputOutcome::IDLE
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let outer_area = area;
        let area = self.render_chrome(frame, area);
        self.render_with_style(frame, area, Style::default());
        if self.disabled {
            dim_buffer_area(frame, outer_area, theme().subtle_fg());
            if self.is_panel_mode() {
                restore_disabled_chrome_labels(frame, outer_area, theme().muted_fg());
            }
        }
    }

    fn content_area(&self, area: Rect) -> Rect {
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => Panel::inner_area(area),
        }
    }

    fn render_chrome(&self, frame: &mut Frame, area: Rect) -> Rect {
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

    pub(crate) fn render_with_style(&self, frame: &mut Frame, area: Rect, style: Style) {
        if area.is_empty() {
            return;
        }
        let style = if self.focused && !self.insert_mode && !self.disabled {
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
        let selected = self.focused && !self.insert_mode && !self.disabled;
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
                self.inline_hotkey(),
                width,
                self.cursor_visible(),
                self.pending_hotkey_prefix.as_deref(),
                self.cursor_fade.style(placeholder_style),
                placeholder_style,
            );
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let len = chars.len();
        let start = if self.cursor_visible() {
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
            if self.cursor_visible() && position == self.cursor {
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
            self.inline_hotkey(),
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
        let text_immediately_follows_cursor = chars
            .get(self.cursor)
            .is_some_and(|value| !value.is_whitespace());
        let mut start = self.cursor;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].is_whitespace() {
            start -= 1;
        }
        if !text_immediately_follows_cursor {
            while start > 0 && chars[start - 1].is_whitespace() {
                start -= 1;
            }
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

    pub(crate) fn external_editor_key_matches(&self, key: KeyEvent) -> bool {
        self.external_editor_key
            .is_some_and(|expected| key_matches(expected, key))
    }

    pub(crate) fn apply_external_editor_response(
        &mut self,
        response: &crate::ExternalEditorResponse,
    ) {
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

    fn emit_change_if_needed(&self, previous_value: &str, ctx: &mut EventCtx<M>) {
        if self.value != previous_value
            && let Some(on_change) = &self.on_change
        {
            ctx.emit(on_change(self.value.clone()));
        }
    }

    fn emit_edit_end(&self, ctx: &mut EventCtx<M>) {
        if let Some(on_edit_end) = &self.on_edit_end {
            ctx.emit(on_edit_end(self.value.clone()));
        }
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

    pub fn style(mut self, chrome: InputChrome) -> Self {
        self.input = self.input.style(chrome);
        self
    }

    pub fn panel(mut self, title: impl Into<String>) -> Self {
        self.input = self.input.panel(title);
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

    pub fn on_change(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }

    pub fn on_edit_end(mut self, handler: impl Fn(String) -> M + 'static) -> Self {
        self.input = self.input.on_edit_end(handler);
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

    pub fn insert_mode(&self) -> bool {
        self.input.insert_mode()
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
        let area = self.input.render_chrome(frame, area);
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
                self.input.inline_hotkey(),
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
            self.input.inline_hotkey(),
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
            placeholder_label(&self.placeholder, self.inline_hotkey())
        } else {
            label_with_visible_hotkey(
                &self.value,
                self.inline_hotkey(),
                !(self.focused && self.insert_mode),
            )
        };
        let width = line_width(&Line::from(text.as_str())).min(u16::MAX as usize) as u16;
        self.chrome_measure(width.max(1), 1, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        let focus_area = self.content_area(area);
        if let Some(hotkey) = self.hotkey.clone().filter(|_| self.hotkey_focus_enabled) {
            ctx.register_text_entry_focusable_with_hotkey_sequences(
                FocusId::new(INPUT_FOCUS),
                focus_area,
                true,
                vec![hotkey],
                self.insert_mode,
            );
        } else {
            ctx.register_text_entry_focusable(
                FocusId::new(INPUT_FOCUS),
                focus_area,
                true,
                self.insert_mode,
            );
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.panel_click_focus(event, FocusId::new(INPUT_FOCUS), ctx) {
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
            if self.disabled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            let was_editing = self.insert_mode;
            let previous_value = self.value.clone();
            self.apply_external_editor_response(response);
            self.emit_change_if_needed(&previous_value, ctx);
            self.insert_mode = false;
            if was_editing {
                self.emit_edit_end(ctx);
            }
            self.cursor_fade.reset();
            ctx.request_clear();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if let TuiEvent::Paste(value) = event {
            if !self.insert_mode {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            let previous_value = self.value.clone();
            let outcome = self.on_paste(value);
            self.emit_change_if_needed(&previous_value, ctx);
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.value.clone());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.disabled {
            if self.external_editor_key_matches(*key) {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            if self.insert_mode && matches_any(&self.keys.cancel, *key) {
                self.insert_mode = false;
                self.cursor_fade.reset();
                ctx.request_layout();
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            let outcome = self.on_key(*key);
            if outcome.submitted
                && self.focused
                && let Some(on_submit) = &self.on_submit
            {
                ctx.emit(on_submit(self.value.clone()));
            }
            if outcome.submitted {
                self.insert_mode = !self.insert_mode;
                ctx.request_layout();
            }
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            return EventOutcome::Ignored;
        }
        if self.external_editor_key_matches(*key) {
            if !self.insert_mode {
                if let Some(on_submit) = &self.on_submit {
                    ctx.emit(on_submit(self.value.clone()));
                }
                self.insert_mode = true;
                ctx.request_layout();
                ctx.request_redraw();
            }
            ctx.request_external_editor(self.value.clone(), 1, self.cursor + 1);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if delete_forward_key(*key) {
            self.insert_mode = true;
            let previous_value = self.value.clone();
            let outcome = self.on_key(*key);
            self.emit_change_if_needed(&previous_value, ctx);
            ctx.request_layout();
            if outcome.needs_redraw() {
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
            if matches_any(&self.keys.submit, *key) {
                if self.focused
                    && let Some(on_submit) = &self.on_submit
                {
                    ctx.emit(on_submit(self.value.clone()));
                }
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
            return EventOutcome::Ignored;
        }
        if matches_any(&self.keys.cancel, *key) {
            self.insert_mode = false;
            self.emit_edit_end(ctx);
            self.cursor_fade.reset();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let previous_value = self.value.clone();
        let outcome = self.on_key(*key);
        self.emit_change_if_needed(&previous_value, ctx);
        if outcome.submitted {
            self.insert_mode = false;
            self.emit_edit_end(ctx);
            ctx.request_layout();
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
        let was_editing = self.insert_mode;
        self.set_focused(focused);
        self.panel.set_focused(focused, ctx.animation());
        if focused {
            self.cursor_fade.reset();
        } else if was_editing && let Some(on_edit_end) = &self.on_edit_end {
            ctx.emit(on_edit_end(self.value.clone()));
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
            placeholder_label(&self.input.placeholder, self.input.inline_hotkey())
        } else {
            let value = std::iter::repeat(self.mask_char)
                .take(self.input.len_chars())
                .collect::<String>();
            label_with_visible_hotkey(
                &value,
                self.input.inline_hotkey(),
                !(self.input.focused && self.input.insert_mode),
            )
        };
        let width = line_width(&Line::from(text.as_str())).min(u16::MAX as usize) as u16;
        self.input.chrome_measure(width.max(1), 1, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.input.area = area;
        let focus_area = self.input.content_area(area);
        if let Some(hotkey) = self.input.hotkey.clone() {
            ctx.register_text_entry_focusable_with_hotkey_sequences(
                FocusId::new(PASSWORD_INPUT_FOCUS),
                focus_area,
                true,
                vec![hotkey],
                self.input.insert_mode,
            );
        } else {
            ctx.register_text_entry_focusable(
                FocusId::new(PASSWORD_INPUT_FOCUS),
                focus_area,
                true,
                self.input.insert_mode,
            );
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self
            .input
            .panel_click_focus(event, FocusId::new(PASSWORD_INPUT_FOCUS), ctx)
        {
            return EventOutcome::Handled;
        }
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
            let previous_value = self.input.value.clone();
            let outcome = self.on_paste(value);
            self.input.emit_change_if_needed(&previous_value, ctx);
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
        if delete_forward_key(*key) {
            self.input.insert_mode = true;
            let previous_value = self.input.value.clone();
            let outcome = self.on_key(*key);
            self.input.emit_change_if_needed(&previous_value, ctx);
            ctx.request_layout();
            if outcome.needs_redraw() {
                ctx.request_redraw();
            }
            if outcome.handled {
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
        }
        if !self.input.insert_mode {
            if focus_navigation_key(*key) {
                return EventOutcome::Ignored;
            }
            if matches_any(&self.input.keys.submit, *key) {
                if self.input.focused
                    && let Some(on_submit) = &self.input.on_submit
                {
                    ctx.emit(on_submit(self.input.value.clone()));
                }
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
            return EventOutcome::Ignored;
        }
        if matches_any(&self.input.keys.cancel, *key) {
            self.input.insert_mode = false;
            self.input.emit_edit_end(ctx);
            self.input.cursor_fade.reset();
            ctx.request_layout();
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let previous_value = self.input.value.clone();
        let outcome = self.on_key(*key);
        self.input.emit_change_if_needed(&previous_value, ctx);
        if outcome.submitted {
            self.input.insert_mode = false;
            self.input.emit_edit_end(ctx);
            ctx.request_layout();
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
        let was_editing = self.input.insert_mode;
        self.input.set_focused(focused);
        self.input.panel.set_focused(focused, ctx.animation());
        if focused {
            self.input.cursor_fade.reset();
        } else if was_editing && let Some(on_edit_end) = &self.input.on_edit_end {
            ctx.emit(on_edit_end(self.input.value.clone()));
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
            .merge(Animated::tick(&mut self.input.panel, dt, settings))
    }
}

impl<M> Animated for TextInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.cursor_fade
            .tick(self.focused && self.insert_mode, dt, settings)
            .merge(Animated::tick(&mut self.panel, dt, settings))
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

pub(crate) fn dim_buffer_area(frame: &mut Frame, area: Rect, color: Color) {
    let area = area.intersection(frame.area());
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = frame
                .buffer_mut()
                .cell_mut((x, y))
                .expect("coordinates are inside the frame");
            cell.set_fg(color);
            cell.modifier.insert(Modifier::DIM);
        }
    }
}

pub(crate) fn restore_disabled_chrome_labels(frame: &mut Frame, area: Rect, color: Color) {
    let area = area.intersection(frame.area());
    if area.is_empty() {
        return;
    }
    for x in area.x..area.right() {
        restore_disabled_chrome_label_cell(frame, x, area.y, color);
        if area.height > 1 {
            restore_disabled_chrome_label_cell(frame, x, area.bottom() - 1, color);
        }
    }
    for y in area.y.saturating_add(1)..area.bottom().saturating_sub(1) {
        restore_disabled_chrome_label_cell(frame, area.x, y, color);
        if area.width > 1 {
            restore_disabled_chrome_label_cell(frame, area.right() - 1, y, color);
        }
    }
}

fn restore_disabled_chrome_label_cell(frame: &mut Frame, x: u16, y: u16, color: Color) {
    let cell = frame
        .buffer_mut()
        .cell_mut((x, y))
        .expect("coordinates are inside the frame");
    if !cell.symbol().chars().any(char::is_alphanumeric) {
        return;
    }
    cell.set_fg(color);
    cell.modifier.remove(Modifier::DIM);
}

pub(crate) fn focus_navigation_key(key: KeyEvent) -> bool {
    matches!(key.code, Key::Tab | Key::BackTab)
}

fn rect_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.right() && y >= area.y && y < area.bottom()
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
                layout: false,
                active: false,
                next_tick: None,
            };
        }

        let total = CURSOR_FADE_HALF.saturating_mul(2);
        let before = self.elapsed;
        self.elapsed = duration_mod(self.elapsed.saturating_add(dt.min(settings.max_dt)), total);
        TickResult {
            changed: before != self.elapsed,
            layout: false,
            active: true,
            next_tick: None,
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
