use ratatui::Frame;
use ratatui::layout::Rect;
use std::ops::Deref;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::thread;
use std::time::Duration;

use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::Paragraph;

use crate::animation::{Animated, AnimationSettings, TickResult};
use crate::event::{
    HotkeyEvent, Key, KeyEvent, KeyModifiers, MouseButton, MouseEventKind, TuiEvent,
};
use crate::hotkey::normalize_hotkey;
use crate::{
    AxisProposal, BorderKind, EventCtx, EventOutcome, FocusCtx, FocusId, FocusRequest, KeySpec,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, ThemeName, TuiNode,
    line_width,
};
use crate::{ScrollAxes, ScrollOffset, ScrollSize, ScrollState, preset, theme, ui::keybindings};

use super::syntax_highlighter::highlight_text;
use super::text_input::{
    CursorFade, InputChrome, InputOutcome, append_unfocused_hotkey, cell_width,
    disabled_input_style, display_char, focus_navigation_key, label_with_visible_hotkey,
    placeholder_label, placeholder_line, selected_input_style, text_char, visible_start_for_cursor,
};
use super::{Language, Panel};

const TEXTAREA_FOCUS: &str = "textarea";
const TAB_INSERT: &str = "    ";
const SYNTAX_POLL_INTERVAL: Duration = Duration::from_millis(16);
const SYNTAX_QUEUE_CAPACITY: usize = 1;
static SYNTAX_WORKER: OnceLock<Option<SyncSender<SyntaxRequest>>> = OnceLock::new();

pub struct TextareaInput<M = ()> {
    value: String,
    placeholder: String,
    disabled: bool,
    hotkey: Option<String>,
    editor_hotkey: Option<String>,
    action_hotkeys: Vec<(String, Box<dyn Fn(String) -> M>)>,
    cursor: usize,
    focused: bool,
    insert_mode: bool,
    max_lines: Option<usize>,
    min_rows: usize,
    max_rows: Option<usize>,
    wrap: bool,
    scroll: ScrollState,
    area: Rect,
    outer_area: Rect,
    chrome: InputChrome,
    panel: Panel,
    on_change: Option<Box<dyn Fn(String) -> M>>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    on_edit_end: Option<Box<dyn Fn(String) -> M>>,
    external_editor_key: Option<KeyEvent>,
    external_editor_file_extension: Option<String>,
    language: Option<Language>,
    syntax_revision: u64,
    syntax_cache: Option<SyntaxCache>,
    stale_syntax_prefix_len: usize,
    syntax_job: Option<SyntaxJob>,
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
                KeySpec::key(Key::Enter),
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
            disabled: false,
            hotkey: None,
            editor_hotkey: None,
            action_hotkeys: Vec::new(),
            cursor: 0,
            focused: false,
            insert_mode: false,
            max_lines: None,
            min_rows: 1,
            max_rows: None,
            wrap: true,
            scroll: ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll()),
            area: Rect::default(),
            outer_area: Rect::default(),
            chrome: InputChrome::Plain,
            panel: Panel::new(),
            on_change: None,
            on_submit: None,
            on_edit_end: None,
            external_editor_key: Some(ctrl_key('o')),
            external_editor_file_extension: None,
            language: None,
            syntax_revision: 0,
            syntax_cache: None,
            stale_syntax_prefix_len: 0,
            syntax_job: None,
            keys: TextareaInputKeyBindings::default(),
            cursor_fade: CursorFade::default(),
            pending_hotkey_prefix: None,
        }
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.set_value(value);
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
        self.sync_panel();
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }

    fn cursor_visible(&self) -> bool {
        self.focused && self.insert_mode && !self.disabled
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
            InputChrome::Panel(panel) => panel.panel_badge(self.focused, self.display_hotkey()),
        };
        if self.disabled {
            panel = panel.border(BorderKind::RoundedDashed);
        }
        panel.set_pending_hotkey_prefix(self.pending_hotkey_prefix.clone());
        self.panel = panel;
    }

    fn display_hotkey(&self) -> Option<String> {
        let mut hotkeys = self.hotkey.clone().into_iter().collect::<Vec<_>>();
        if !self.disabled {
            hotkeys.extend(self.editor_hotkey.iter().cloned());
            hotkeys.extend(
                self.action_hotkeys
                    .iter()
                    .map(|(sequence, _)| sequence.clone()),
            );
        }
        (!hotkeys.is_empty()).then(|| hotkeys.join("·"))
    }

    fn inline_hotkey(&self) -> Option<String> {
        match self.chrome {
            InputChrome::Plain => self.display_hotkey(),
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

        ctx.focus(FocusRequest::TargetAt {
            path: ctx.current_path(),
            id: FocusId::new(TEXTAREA_FOCUS),
        });
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
        if self.editor_hotkey.is_none() && self.action_hotkeys.is_empty() {
            self.pending_hotkey_prefix = None;
        }
        self.sync_panel();
    }

    pub fn editor_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_editor_hotkey(hotkey);
        self
    }

    pub fn set_editor_hotkey(&mut self, hotkey: impl Into<String>) {
        self.editor_hotkey = Some(hotkey.into());
        self.sync_panel();
    }

    pub fn clear_editor_hotkey(&mut self) {
        self.editor_hotkey = None;
        if self.hotkey.is_none() && self.action_hotkeys.is_empty() {
            self.pending_hotkey_prefix = None;
        }
        self.sync_panel();
    }

    pub fn action_hotkey(
        mut self,
        sequence: impl Into<String>,
        on_trigger: impl Fn(String) -> M + 'static,
    ) -> Self {
        self.action_hotkeys
            .push((sequence.into(), Box::new(on_trigger)));
        self.sync_panel();
        self
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
        let HotkeyEvent::Commit(sequence) = hotkey else {
            return false;
        };

        if self
            .editor_hotkey
            .as_deref()
            .is_some_and(|hotkey| normalize_hotkey(hotkey) == normalize_hotkey(sequence))
        {
            if !self.disabled {
                if !self.insert_mode {
                    if let Some(on_submit) = &self.on_submit {
                        ctx.emit(on_submit(self.value.clone()));
                    }
                    self.begin_insert_mode();
                    ctx.request_layout();
                    ctx.request_redraw();
                }
                let (line, col) = self.external_editor_request_position();
                self.request_external_editor(ctx, line, col);
            }
            ctx.stop_propagation();
            return true;
        }

        if let Some((_, on_trigger)) = self
            .action_hotkeys
            .iter()
            .find(|(hotkey, _)| normalize_hotkey(hotkey) == normalize_hotkey(sequence))
        {
            if !self.disabled {
                ctx.emit(on_trigger(self.value.clone()));
            }
            ctx.stop_propagation();
            return true;
        }

        if !self.disabled {
            self.begin_insert_mode();
            self.scroll_cursor_into_view(disabled_animation_settings());
            self.cursor_fade.reset();
            ctx.request_layout();
            ctx.request_redraw();
        }
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
        self.insert_mode = insert_mode && !self.disabled;
        self.cursor_fade.reset();
    }

    fn begin_insert_mode(&mut self) {
        self.cursor = self.len_chars();
        self.insert_mode = true;
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

    pub fn external_editor_file_extension(mut self, extension: impl Into<String>) -> Self {
        self.set_external_editor_file_extension(extension);
        self
    }

    pub fn set_external_editor_file_extension(&mut self, extension: impl Into<String>) {
        self.external_editor_file_extension = Some(extension.into());
    }

    pub fn clear_external_editor_file_extension(&mut self) {
        self.external_editor_file_extension = None;
    }

    pub fn language(mut self, language: Language) -> Self {
        self.set_language(language);
        self
    }

    pub fn set_language(&mut self, language: Language) {
        if self.language != Some(language) {
            self.language = Some(language);
            self.invalidate_syntax_cache();
        }
    }

    pub fn clear_language(&mut self) {
        self.language = None;
        self.invalidate_syntax_cache();
    }

    pub fn current_language(&self) -> Option<Language> {
        self.language
    }

    fn request_external_editor(&self, ctx: &mut EventCtx<M>, line: usize, col: usize) {
        if let Some(extension) = self.external_editor_extension() {
            ctx.request_external_editor_with_extension(self.value.clone(), line, col, extension);
        } else {
            ctx.request_external_editor(self.value.clone(), line, col);
        }
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

    /// Sets minimum content rows. Panel chrome adds two outer rows.
    ///
    /// Zero becomes one. Minimum and maximum normalize so minimum never exceeds maximum,
    /// regardless of builder order.
    pub fn min_rows(mut self, min_rows: usize) -> Self {
        self.min_rows = self
            .max_rows
            .map_or(min_rows.max(1), |max_rows| min_rows.max(1).min(max_rows));
        self
    }

    /// Sets maximum content rows. Panel chrome adds two outer rows.
    ///
    /// Zero becomes one. Minimum and maximum normalize so minimum never exceeds maximum,
    /// regardless of builder order.
    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.max_rows = Some(max_rows.max(1));
        if let Some(max_rows) = self.max_rows
            && self.min_rows > max_rows
        {
            self.min_rows = max_rows;
        }
        self
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.set_wrap(wrap);
        self
    }

    pub fn set_wrap(&mut self, wrap: bool) {
        self.wrap = wrap;
    }

    pub fn current_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.invalidate_syntax_cache();
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
    }

    pub fn move_cursor_to_end(&mut self) {
        self.cursor = self.len_chars();
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
        let outcome = self.insert_text(value.as_ref());
        self.clamp_lines();
        self.cursor = self.cursor.min(self.len_chars());
        if outcome.needs_redraw() {
            self.cursor_fade.reset();
        }
        outcome
    }

    fn on_key_inner(&mut self, key: KeyEvent) -> InputOutcome {
        if self.disabled {
            return self.on_disabled_key(key);
        }
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

    fn on_disabled_key(&mut self, key: KeyEvent) -> InputOutcome {
        if KeySpec::key(Key::Enter).matches(key) || matches_any(&self.keys.submit, key) {
            return InputOutcome::SUBMITTED;
        }
        if matches_any(&self.keys.move_line_start, key) {
            return self.move_to(self.current_line().start);
        }
        if matches_any(&self.keys.move_line_end, key) {
            return self.move_to(self.current_line().end);
        }
        if matches_any(&self.keys.move_previous_word, key) {
            return self.move_previous_word();
        }
        if matches_any(&self.keys.move_next_word, key) {
            return self.move_next_word();
        }
        if matches_any(&self.keys.move_up, key) {
            return self.move_vertical(-1);
        }
        if matches_any(&self.keys.move_down, key) {
            return self.move_vertical(1);
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
            || matches_any(&self.keys.delete_before_line, key)
            || matches_any(&self.keys.delete_after_line, key)
            || matches_any(&self.keys.delete_previous_word, key)
            || matches_any(&self.keys.delete_next_word, key)
            || matches_any(&self.keys.insert_tab, key)
            || matches_any(&self.keys.insert_newline, key)
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
        if area.is_empty() {
            return;
        }

        let navigation_focused = self.focused && !self.insert_mode;
        let style = if navigation_focused && self.disabled {
            disabled_input_style(Style::default())
        } else if navigation_focused && self.language.is_some() {
            syntax_navigation_focus_style(Style::default())
        } else if navigation_focused {
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
        let height = self.visible_outer_height(area.width, area.height);
        let area = Rect::new(area.x, area.y, area.width, height);
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => Panel::inner_area(area),
        }
    }

    fn render_chrome(&self, frame: &mut Frame, area: Rect) -> Rect {
        let height = self.visible_outer_height(area.width, area.height);
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
        let chrome_height = match self.chrome {
            InputChrome::Plain => 0,
            InputChrome::Panel(_) => 2,
        };
        let width = match self.chrome {
            InputChrome::Plain => width,
            InputChrome::Panel(_) => width.saturating_add(2),
        };
        let height = height.saturating_add(chrome_height);
        let min_height =
            (self.min_rows.min(u16::MAX as usize) as u16).saturating_add(chrome_height);
        let mut hint = LayoutSizeHint::content(width, height);
        hint.min.height = min_height;
        hint.normalized(proposal)
    }

    fn visible_outer_height(&self, width: u16, available: u16) -> u16 {
        let content = self.preferred_rows(self.content_rows_for_width(self.inner_width(width)));
        let content = content.min(u16::MAX as usize) as u16;
        let height = match self.chrome {
            InputChrome::Plain => content,
            InputChrome::Panel(_) => content.saturating_add(2),
        };
        height.min(available)
    }

    fn inner_width(&self, width: u16) -> usize {
        match self.chrome {
            InputChrome::Plain => width as usize,
            InputChrome::Panel(_) => width.saturating_sub(2) as usize,
        }
    }

    fn visible_lines(&self, width: usize, height: usize) -> VisibleLines {
        if width == 0 || height == 0 {
            return VisibleLines::default();
        }

        let theme = theme();
        let selected = self.focused && !self.insert_mode;
        let placeholder_style = if selected && self.disabled {
            disabled_input_style(Style::default().fg(theme.muted_fg()))
        } else if selected && self.language.is_some() {
            syntax_navigation_focus_style(Style::default().fg(theme.muted_fg()))
        } else if selected {
            selected_input_style(Style::default().fg(theme.muted_fg()))
        } else {
            Style::default().fg(theme.muted_fg())
        };
        if self.value.is_empty() {
            let mut lines = vec![placeholder_line(
                &self.placeholder,
                self.inline_hotkey().as_deref(),
                width,
                self.cursor_visible(),
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
        let cursor_row = self.cursor_visual_row(width, &ranges, cursor_line, cursor_col);
        let first_line = cursor_row.saturating_add(1).saturating_sub(height);
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
        let theme_name = theme.name();
        let selected = self.focused && !self.insert_mode;
        let syntax_navigation_focused = selected && self.language.is_some();
        let value_style = Style::default().fg(if self.focused {
            theme.text_fg()
        } else {
            theme.subtle_fg()
        });
        let value_style = if selected && self.disabled {
            disabled_input_style(value_style)
        } else if syntax_navigation_focused {
            syntax_navigation_focus_style(value_style)
        } else if selected {
            selected_input_style(value_style)
        } else {
            value_style
        };
        let hotkey_style = if selected && self.disabled {
            disabled_input_style(Style::default())
        } else if syntax_navigation_focused {
            syntax_navigation_focus_style(Style::default().fg(theme.muted_fg()))
        } else if selected {
            selected_input_style(Style::default())
        } else {
            Style::default().fg(theme.muted_fg())
        };
        let cursor_style = self.cursor_fade.style(value_style);
        let ranges = self.line_ranges();
        if self.wrap {
            return self.visible_wrapped_lines_from_with_cursor(
                width,
                height,
                first_line,
                cursor,
                value_style,
                hotkey_style,
                cursor_style,
                theme_name,
                &ranges,
            );
        }
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
                let horizontal = if self.cursor_visible() && line_index == cursor_line {
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
                    theme_name,
                )
            })
            .collect();
        VisibleLines { lines, first_line }
    }

    fn visible_wrapped_lines_from_with_cursor(
        &self,
        width: usize,
        height: usize,
        first_line: usize,
        cursor: Option<(usize, usize)>,
        value_style: Style,
        hotkey_style: Style,
        cursor_style: Style,
        theme_name: ThemeName,
        ranges: &[LineRange],
    ) -> VisibleLines {
        let rows = self.visual_rows(width, ranges);
        let (cursor_line, cursor_col) = cursor.unwrap_or((usize::MAX, 0));
        let last_row = rows.len().saturating_sub(1);
        let lines = rows
            .iter()
            .enumerate()
            .skip(first_line)
            .take(height)
            .map(|(row_index, row)| {
                let cursor_row = row.contains_cursor(cursor_line, cursor_col);
                self.render_line(
                    row.range,
                    cursor_row,
                    0,
                    width,
                    value_style,
                    (!(self.focused && self.insert_mode) && row_index == last_row)
                        .then_some(hotkey_style),
                    cursor_style,
                    theme_name,
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
        theme_name: ThemeName,
    ) -> Line<'static> {
        let chars = self.value.chars().collect::<Vec<_>>();
        let mut spans = Vec::new();
        let mut drawn = 0;
        let syntax_navigation_focused =
            self.focused && !self.insert_mode && !self.disabled && self.language.is_some();

        for col in horizontal..=range.len() {
            if drawn >= width {
                break;
            }
            let remaining = width.saturating_sub(drawn);
            let position = range.start + col;
            if self.cursor_visible() && cursor_line && position == self.cursor {
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
                spans.push(Span::styled(
                    text,
                    self.syntax_style(
                        position,
                        *value,
                        value_style,
                        syntax_navigation_focused,
                        theme_name,
                    ),
                ));
            }
        }
        if let Some(hotkey_style) = hotkey_style {
            append_unfocused_hotkey(
                &mut spans,
                &mut drawn,
                width,
                self.inline_hotkey().as_deref(),
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
        self.invalidate_syntax_cache();
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
        self.invalidate_syntax_cache();
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

    fn content_size_for_width(&self, width: usize) -> ScrollSize {
        ScrollSize::new(width, self.content_rows_for_width(width))
    }

    fn content_rows_for_width(&self, width: usize) -> usize {
        if self.value.is_empty() {
            return 1;
        }
        let ranges = self.line_ranges();
        if self.wrap {
            self.visual_rows(width, &ranges).len()
        } else {
            ranges.len()
        }
    }

    fn visual_rows(&self, width: usize, ranges: &[LineRange]) -> Vec<VisualLineRange> {
        if !self.wrap || width == 0 {
            return ranges
                .iter()
                .enumerate()
                .map(|(line_index, range)| VisualLineRange {
                    line_index,
                    range: *range,
                    start_col: 0,
                    end_col: range.len(),
                    line_len: range.len(),
                })
                .collect();
        }

        let chars = self.value.chars().collect::<Vec<_>>();
        let mut rows = Vec::new();
        let cursor_line_col = self.cursor_visible().then(|| {
            let ranges = self.line_ranges();
            self.cursor_line_col(&ranges)
        });
        for (line_index, range) in ranges.iter().enumerate() {
            let row_width = cursor_line_col
                .filter(|(cursor_line, cursor_col)| {
                    *cursor_line == line_index && *cursor_col == range.len()
                })
                .map_or(width, |_| width.saturating_sub(1).max(1));
            if range.len() == 0 {
                rows.push(VisualLineRange {
                    line_index,
                    range: *range,
                    start_col: 0,
                    end_col: 0,
                    line_len: 0,
                });
                continue;
            }

            let mut start_col = 0;
            let mut col = 0;
            let mut drawn = 0;
            let mut last_space_col = None;
            while col < range.len() {
                let value = chars[range.start + col];
                let char_width = visual_char_width(value, row_width);
                if drawn > 0 && drawn + char_width > row_width {
                    if value.is_whitespace() {
                        rows.push(VisualLineRange {
                            line_index,
                            range: LineRange {
                                start: range.start + start_col,
                                end: range.start + col,
                            },
                            start_col,
                            end_col: col,
                            line_len: range.len(),
                        });
                        col += 1;
                        start_col = col;
                        drawn = 0;
                        last_space_col = None;
                        continue;
                    }
                    if let Some(space_col) = last_space_col
                        && space_col >= start_col
                    {
                        let next_start = space_col + 1;
                        rows.push(VisualLineRange {
                            line_index,
                            range: LineRange {
                                start: range.start + start_col,
                                end: range.start + next_start,
                            },
                            start_col,
                            end_col: next_start,
                            line_len: range.len(),
                        });
                        start_col = next_start;
                        col = next_start;
                        drawn = 0;
                        last_space_col = None;
                        continue;
                    }
                    rows.push(VisualLineRange {
                        line_index,
                        range: LineRange {
                            start: range.start + start_col,
                            end: range.start + col,
                        },
                        start_col,
                        end_col: col,
                        line_len: range.len(),
                    });
                    start_col = col;
                    drawn = 0;
                    last_space_col = None;
                    continue;
                }
                if value.is_whitespace() {
                    last_space_col = Some(col);
                }
                col += 1;
                drawn += char_width.min(row_width).max(1);
            }
            rows.push(VisualLineRange {
                line_index,
                range: LineRange {
                    start: range.start + start_col,
                    end: range.end,
                },
                start_col,
                end_col: range.len(),
                line_len: range.len(),
            });
        }
        rows
    }

    fn cursor_visual_row(
        &self,
        width: usize,
        ranges: &[LineRange],
        cursor_line: usize,
        cursor_col: usize,
    ) -> usize {
        if !self.wrap {
            return cursor_line;
        }
        self.visual_rows(width, ranges)
            .iter()
            .position(|row| row.contains_cursor(cursor_line, cursor_col))
            .unwrap_or_else(|| self.content_rows_for_width(width).saturating_sub(1))
    }

    fn scroll_area(&self, area: Rect) -> Rect {
        let height = self.max_rows.map_or(area.height, |max_rows| {
            (area.height as usize).min(max_rows) as u16
        });
        Rect::new(area.x, area.y, area.width, height)
    }

    fn scroll_geometry(&self, area: Rect) -> crate::ScrollGeometry {
        let area = self.scroll_area(area);
        let mut content = self.content_size_for_width(area.width.saturating_sub(1) as usize);
        let mut layout = self.scroll.layout(area, content);
        for _ in 0..2 {
            let next_content = self.content_size_for_width(layout.viewport.width as usize);
            if next_content == content {
                break;
            }
            content = next_content;
            layout = self.scroll.layout(area, content);
        }
        crate::ScrollGeometry {
            layout,
            viewport: ScrollSize::from_area(layout.viewport),
            content,
        }
    }

    fn has_vertical_overflow(&self) -> bool {
        let geometry = self.scroll_geometry(self.area);
        geometry.content.height > geometry.viewport.height
    }

    fn scroll_page_key(key: KeyEvent) -> bool {
        let bindings = keybindings();
        bindings.page_up_matches(key) || bindings.page_down_matches(key)
    }

    fn scroll_navigation_key(&self, key: KeyEvent) -> bool {
        let bindings = keybindings();
        self.focused
            && !self.insert_mode
            && (bindings.line_up_matches(key)
                || bindings.line_down_matches(key)
                || bindings.top_prefix_matches(key)
                || bindings.bottom_matches(key)
                || bindings.home_matches(key)
                || bindings.end_matches(key))
    }

    fn handle_scroll_key(&mut self, key: KeyEvent, ctx: &mut EventCtx<M>) -> bool {
        if self.area.is_empty()
            || !self.has_vertical_overflow()
            || !(Self::scroll_page_key(key) || self.scroll_navigation_key(key))
        {
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
        let (cursor_line, cursor_col) = self.cursor_line_col(&ranges);
        let cursor_row = self.cursor_visual_row(
            geometry.viewport.width as usize,
            &ranges,
            cursor_line,
            cursor_col,
        );
        let offset = self.scroll.target_offset();
        let viewport_height = geometry.viewport.height;
        let target_y = if cursor_row < offset.y {
            cursor_row
        } else if cursor_row >= offset.y.saturating_add(viewport_height) {
            cursor_row.saturating_add(1).saturating_sub(viewport_height)
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

    fn measure_content_width(&self, natural_width: u16, proposal: LayoutProposal) -> Option<usize> {
        match proposal.width {
            AxisProposal::Unbounded => return None,
            AxisProposal::AtMost(width) => {
                Some(self.inner_width(width).min(natural_width as usize).max(1))
            }
            AxisProposal::Exact(width) => Some(self.inner_width(width).max(1)),
        }
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
        self.invalidate_syntax_cache();
    }

    fn clamp_lines(&mut self) {
        let Some(max_lines) = self.max_lines else {
            return;
        };

        let mut lines = self.value.split('\n').take(max_lines).collect::<Vec<_>>();
        if lines.is_empty() {
            return;
        }
        let clamped = lines.drain(..).collect::<Vec<_>>().join("\n");
        if self.value != clamped {
            self.value = clamped;
            self.invalidate_syntax_cache();
        }
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
        self.invalidate_syntax_cache();
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

    fn invalidate_syntax_cache(&mut self) {
        self.stale_syntax_prefix_len = self.syntax_cache.as_ref().map_or(0, |cache| {
            cache
                .source
                .iter()
                .zip(self.value.chars())
                .take_while(|(cached, current)| *cached == current)
                .count()
        });
        self.syntax_revision = self.syntax_revision.wrapping_add(1);
    }

    fn start_syntax_job(&mut self, theme_name: ThemeName) -> bool {
        let Some(language) = self.language else {
            self.syntax_job = None;
            return false;
        };
        if self.syntax_cache.as_ref().is_some_and(|cache| {
            cache.revision == self.syntax_revision
                && cache.language == language
                && cache.theme_name == theme_name
        }) {
            return false;
        }
        if self.syntax_job.is_some() {
            return true;
        }

        let revision = self.syntax_revision;
        let source = self.value.clone();
        let (sender, receiver) = mpsc::channel();
        let request = SyntaxRequest {
            revision,
            source,
            language,
            theme_name,
            sender,
        };
        let Some(worker) = syntax_worker() else {
            return false;
        };
        match worker.try_send(request) {
            Ok(()) => {
                self.syntax_job = Some(SyntaxJob { receiver });
                true
            }
            Err(TrySendError::Full(_)) => true,
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    fn poll_syntax_job(&mut self) -> bool {
        let Some(job) = self.syntax_job.as_ref() else {
            return false;
        };
        match job.receiver.try_recv() {
            Ok(cache) => {
                self.syntax_job = None;
                if cache.revision == self.syntax_revision
                    && self.language == Some(cache.language)
                    && cache.theme_name == theme().name()
                {
                    self.syntax_cache = Some(cache);
                    self.stale_syntax_prefix_len = 0;
                    true
                } else {
                    false
                }
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.syntax_job = None;
                false
            }
        }
    }

    fn syntax_style(
        &self,
        position: usize,
        value: char,
        value_style: Style,
        syntax_navigation_focused: bool,
        theme_name: ThemeName,
    ) -> Style {
        let Some(cache) = &self.syntax_cache else {
            return value_style;
        };
        if self.language != Some(cache.language) || cache.theme_name != theme_name {
            return value_style;
        }
        if cache.revision != self.syntax_revision
            && (position >= self.stale_syntax_prefix_len
                || cache.source.get(position) != Some(&value))
        {
            return value_style;
        }
        let style = cache
            .styles
            .get(position)
            .copied()
            .map_or(value_style, |syntax| value_style.patch(syntax));
        if syntax_navigation_focused {
            syntax_navigation_focus_style(style)
        } else {
            style
        }
    }

    fn external_editor_extension(&self) -> Option<String> {
        self.external_editor_file_extension
            .clone()
            .or_else(|| self.language.and_then(language_extension))
    }
}

impl<M> TuiNode<M> for TextareaInput<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let lines = if self.value.is_empty() {
            vec![placeholder_label(
                &self.placeholder,
                self.inline_hotkey().as_deref(),
            )]
        } else {
            let show_hotkey = !(self.focused && self.insert_mode);
            let mut lines = self
                .value
                .split('\n')
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if let Some(line) = lines.last_mut() {
                *line =
                    label_with_visible_hotkey(line, self.inline_hotkey().as_deref(), show_hotkey);
            }
            lines
        };
        let width = lines
            .iter()
            .map(|line| line_width(&Line::from(line.as_str())))
            .max()
            .unwrap_or(1)
            .min(u16::MAX as usize) as u16;
        let content_width = self.measure_content_width(width, proposal);
        let rows = if self.wrap {
            content_width.map_or(lines.len(), |width| wrapped_text_rows(&lines, width))
        } else {
            lines.len()
        };
        let height = self.preferred_rows(rows).min(u16::MAX as usize) as u16;
        self.chrome_measure(width.max(1), height, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.outer_area = area;
        self.area = self.content_area(area);
        if self.insert_mode {
            self.scroll_cursor_into_view(disabled_animation_settings());
        }
        let mut hotkeys = self.hotkey.clone().into_iter().collect::<Vec<_>>();
        if !self.disabled
            && let Some(hotkey) = self.editor_hotkey.clone()
        {
            hotkeys.push(hotkey);
        }
        if !self.disabled {
            hotkeys.extend(
                self.action_hotkeys
                    .iter()
                    .map(|(sequence, _)| sequence.clone()),
            );
        }
        if !hotkeys.is_empty() {
            ctx.register_text_entry_focusable_with_hotkey_sequences(
                FocusId::new(TEXTAREA_FOCUS),
                self.area,
                true,
                hotkeys,
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

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
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
            self.scroll_cursor_into_view(disabled_animation_settings());
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
        if matches!(event, TuiEvent::Yank) {
            ctx.copy_to_clipboard(self.value.clone());
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if self.disabled && self.insert_mode {
            self.insert_mode = false;
            ctx.request_layout();
            ctx.request_redraw();
        }
        if self.scroll_navigation_key(*key) && self.handle_scroll_key(*key, ctx) {
            return EventOutcome::Handled;
        }
        if self.disabled || !self.insert_mode {
            let bindings = keybindings();
            let focus = bindings.focus();
            if focus_navigation_key(*key)
                || focus.next_control_matches(*key)
                || focus.previous_control_matches(*key)
            {
                return EventOutcome::Ignored;
            }
        }
        if self.handle_scroll_key(*key, ctx) {
            return EventOutcome::Handled;
        }
        if self.disabled {
            return EventOutcome::Ignored;
        }
        if self.external_editor_key_matches(*key) {
            if !self.insert_mode {
                if let Some(on_submit) = &self.on_submit {
                    ctx.emit(on_submit(self.value.clone()));
                }
                self.begin_insert_mode();
                ctx.request_layout();
                ctx.request_redraw();
            }
            let (line, col) = self.external_editor_request_position();
            self.request_external_editor(ctx, line, col);
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if delete_forward_key(*key) {
            self.insert_mode = true;
            let previous_value = self.value.clone();
            let outcome = self.on_key(*key);
            self.emit_change_if_needed(&previous_value, ctx);
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
            if KeySpec::key(Key::Enter).matches(*key)
                || matches_any(&self.keys.insert_newline, *key)
            {
                if self.focused
                    && KeySpec::key(Key::Enter).matches(*key)
                    && let Some(on_submit) = &self.on_submit
                {
                    ctx.emit(on_submit(self.value.clone()));
                }
                self.begin_insert_mode();
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
        let scrolled = self.scroll_cursor_into_view(disabled_animation_settings());
        if outcome.submitted {
            self.insert_mode = false;
            self.emit_edit_end(ctx);
            ctx.request_layout();
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

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        if self.start_syntax_job(theme().name()) {
            ctx.request_tick();
        }
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if self.start_syntax_job(theme().name()) {
            ctx.request_tick();
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<M> Animated for TextareaInput<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let syntax_changed = self.poll_syntax_job();
        let syntax_pending = self.start_syntax_job(theme().name());
        let syntax = TickResult {
            changed: syntax_changed,
            layout: false,
            active: false,
            next_tick: syntax_pending.then_some(SYNTAX_POLL_INTERVAL),
        };
        syntax.merge(
            self.cursor_fade
                .tick(self.focused && self.insert_mode, dt, settings)
                .merge(self.scroll.tick(dt, settings))
                .merge(Animated::tick(&mut self.panel, dt, settings)),
        )
    }
}

struct SyntaxCache {
    revision: u64,
    language: Language,
    theme_name: ThemeName,
    source: Vec<char>,
    styles: Vec<Style>,
}

struct SyntaxJob {
    receiver: Receiver<SyntaxCache>,
}

struct SyntaxRequest {
    revision: u64,
    source: String,
    language: Language,
    theme_name: ThemeName,
    sender: mpsc::Sender<SyntaxCache>,
}

fn syntax_worker() -> Option<&'static SyncSender<SyntaxRequest>> {
    SYNTAX_WORKER
        .get_or_init(|| {
            let (sender, receiver) = mpsc::sync_channel::<SyntaxRequest>(SYNTAX_QUEUE_CAPACITY);
            thread::Builder::new()
                .name("tuicore-syntax".into())
                .spawn(move || {
                    while let Ok(request) = receiver.recv() {
                        let highlighted =
                            highlight_text(&request.source, request.language, request.theme_name);
                        let cache = SyntaxCache {
                            revision: request.revision,
                            language: request.language,
                            theme_name: request.theme_name,
                            source: request.source.chars().collect(),
                            styles: syntax_styles(&request.source, &highlighted),
                        };
                        let _ = request.sender.send(cache);
                    }
                })
                .ok()
                .map(|_| sender)
        })
        .as_ref()
}

fn syntax_styles(source: &str, highlighted: &Text<'_>) -> Vec<Style> {
    let mut styles = vec![Style::default(); source.chars().count()];
    let mut line_start = 0;
    for (source_line, highlighted_line) in source.split('\n').zip(&highlighted.lines) {
        let line_end = line_start + source_line.chars().count();
        let mut position = line_start;
        for span in &highlighted_line.spans {
            for _ in span.content.chars() {
                if position >= line_end {
                    break;
                }
                styles[position] = span.style;
                position += 1;
            }
        }
        line_start = line_end.saturating_add(1);
    }
    styles
}

#[derive(Debug, Clone, Copy)]
struct LineRange {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy)]
struct VisualLineRange {
    line_index: usize,
    range: LineRange,
    start_col: usize,
    end_col: usize,
    line_len: usize,
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

impl VisualLineRange {
    fn contains_cursor(self, cursor_line: usize, cursor_col: usize) -> bool {
        if self.line_index != cursor_line
            || cursor_col < self.start_col
            || cursor_col > self.end_col
        {
            return false;
        }
        cursor_col < self.end_col || self.end_col == self.line_len
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

fn syntax_navigation_focus_style(style: Style) -> Style {
    style.bg(theme().highlight_bg())
}

fn language_extension(language: Language) -> Option<String> {
    Language::language_globs(language)
        .into_iter()
        .map(|glob| glob.to_string())
        .find_map(|glob| {
            let extension = glob.strip_prefix("*.")?;
            (!extension.is_empty()
                && extension
                    .chars()
                    .all(|value| value.is_ascii_alphanumeric() || matches!(value, '_' | '+' | '-')))
            .then(|| extension.to_ascii_lowercase())
        })
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

fn visual_char_width(value: char, row_width: usize) -> usize {
    if value == '\t' {
        return TAB_INSERT.len().min(row_width).max(1);
    }
    cell_width(&value.to_string()).min(row_width).max(1)
}

fn wrapped_text_rows(lines: &[String], width: usize) -> usize {
    lines
        .iter()
        .map(|line| wrapped_line_rows(line, width))
        .sum()
}

fn wrapped_line_rows(line: &str, width: usize) -> usize {
    if line.is_empty() || width == 0 {
        return 1;
    }

    let mut rows = 1;
    let mut drawn = 0;
    let chars = line.chars().collect::<Vec<_>>();
    let mut start_col = 0;
    let mut col = 0;
    let mut last_space_col = None;
    while col < chars.len() {
        let value = chars[col];
        let char_width = visual_char_width(value, width);
        if drawn > 0 && drawn + char_width > width {
            if value.is_whitespace() {
                rows += 1;
                col += 1;
                start_col = col;
                drawn = 0;
                last_space_col = None;
                continue;
            }
            if let Some(space_col) = last_space_col
                && space_col >= start_col
            {
                rows += 1;
                start_col = space_col + 1;
                col = start_col;
                drawn = 0;
                last_space_col = None;
                continue;
            }
            rows += 1;
            start_col = col;
            drawn = 0;
            last_space_col = None;
            continue;
        }
        if value.is_whitespace() {
            last_space_col = Some(col);
        }
        col += 1;
        drawn += char_width.min(width).max(1);
    }
    rows
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
