use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::event::{
    HotkeyEvent, Key, KeyEvent, KeyModifiers, MouseButton, MouseEventKind, TuiEvent,
};
use crate::{
    Animated, AnimationSettings, AxisProposal, EventCtx, EventOutcome, FocusCtx, FocusId,
    FocusRequest, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, OverlayLayer,
    TickResult, TuiNode, border_set, keybindings, line_width, preset, theme,
};

use super::Panel;
use super::text_input::{CursorFade, InputChrome, placeholder_line, text_char};

const TAG_INPUT_FOCUS: &str = "tag-input";
const LEFT_CAP: &str = "";
const RIGHT_CAP: &str = "";
const MAX_POPUP_ROWS: u16 = 6;
const POPUP_MIN_WIDTH: u16 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagInputEvent<Id = String> {
    AddedExisting { id: Id, label: String },
    CreateRequested { label: String },
    RemovedExisting { id: Id, label: String },
    RemovedCustom { label: String },
    QueryChanged { query: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedTag<Id = String> {
    Existing { id: Id, label: String },
    Custom { label: String },
}

impl<Id> SelectedTag<Id> {
    pub fn label(&self) -> &str {
        match self {
            Self::Existing { label, .. } | Self::Custom { label } => label,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TagOption<Id> {
    id: Id,
    label: String,
}

pub struct TagInput<Id = String> {
    options: Vec<TagOption<Id>>,
    selected: Vec<SelectedTag<Id>>,
    query: String,
    highlighted_option: usize,
    highlighted_tag: Option<usize>,
    placeholder: String,
    hotkey: Option<String>,
    pending_hotkey_prefix: Option<String>,
    chrome: InputChrome,
    panel: Panel,
    focused: bool,
    popup_open: bool,
    area: Rect,
    outer_area: Rect,
    overlay_bounds: Rect,
    cursor_fade: CursorFade,
    events: Vec<TagInputEvent<Id>>,
}

impl TagInput<String> {
    pub fn new(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let labels = labels.into_iter().map(Into::into).collect::<Vec<String>>();
        Self::with_options(labels, |label| label.clone(), |label| label.clone())
    }

    pub fn selected(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let labels = unique_strings(labels);
        self.selected = labels
            .into_iter()
            .map(|label| match self.option_for_label(&label) {
                Some(option) => SelectedTag::Existing {
                    id: option.id.clone(),
                    label: option.label.clone(),
                },
                None => SelectedTag::Custom { label },
            })
            .collect();
        self
    }
}

impl<Id> TagInput<Id>
where
    Id: Clone + Eq + 'static,
{
    pub fn with_options<T>(
        options: impl IntoIterator<Item = T>,
        id: impl Fn(&T) -> Id,
        label: impl Fn(&T) -> String,
    ) -> Self {
        Self {
            options: unique_options(options, id, label),
            selected: Vec::new(),
            query: String::new(),
            highlighted_option: 0,
            highlighted_tag: None,
            placeholder: String::from("add tags"),
            hotkey: None,
            pending_hotkey_prefix: None,
            chrome: InputChrome::Plain,
            panel: Panel::new(),
            focused: false,
            popup_open: false,
            area: Rect::default(),
            outer_area: Rect::default(),
            overlay_bounds: Rect::default(),
            cursor_fade: CursorFade::default(),
            events: Vec::new(),
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self.sync_panel();
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

    pub fn selected_existing(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.selected.clear();
        for id in ids {
            if let Some(option) = self.option_for_id(&id) {
                self.selected.push(SelectedTag::Existing {
                    id: option.id.clone(),
                    label: option.label.clone(),
                });
            }
        }
        self
    }

    pub fn selected_custom(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.selected.extend(
            unique_strings(labels)
                .into_iter()
                .map(|label| SelectedTag::Custom { label }),
        );
        self
    }

    pub fn selected_tags(&self) -> &[SelectedTag<Id>] {
        &self.selected
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn take_events(&mut self) -> Vec<TagInputEvent<Id>> {
        std::mem::take(&mut self.events)
    }

    pub fn height_for_width(&self, width: u16) -> u16 {
        self.outer_height_for_width(width)
    }

    fn content_height_for_width(&self, width: u16) -> u16 {
        self.field_layout(width)
            .lines
            .len()
            .min(u16::MAX as usize)
            .max(1) as u16
    }

    fn outer_height_for_width(&self, width: u16) -> u16 {
        let content = self.content_height_for_width(self.inner_width(width));
        match self.chrome {
            InputChrome::Plain => content,
            InputChrome::Panel(_) => content.saturating_add(2),
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        self.sync_panel();
        if !focused {
            self.highlighted_tag = None;
            self.popup_open = false;
        }
        self.cursor_fade.reset();
    }

    pub fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if area.is_empty() {
            return;
        }

        let area = self.render_chrome(frame, area);
        let layout = self.field_layout(area.width);
        frame.render_widget(Paragraph::new(Text::from(layout.lines.clone())), area);
        if self.popup_open && self.highlighted_tag.is_none() {
            let input_row = layout.input_row;
            ctx.push_portal(
                OverlayLayer::Popover,
                0,
                self.overlay_bounds,
                move |frame, bounds| {
                    self.render_popup(frame, self.popup_area(input_row, bounds));
                },
            );
        }
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

    fn content_area(&self, area: Rect) -> Rect {
        let height = self.outer_height_for_width(area.width).min(area.height);
        let area = Rect::new(area.x, area.y, area.width, height);
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => Panel::inner_area(area),
        }
    }

    fn render_chrome(&self, frame: &mut Frame, area: Rect) -> Rect {
        let height = self.outer_height_for_width(area.width).min(area.height);
        let area = Rect::new(area.x, area.y, area.width, height);
        match self.chrome {
            InputChrome::Plain => area,
            InputChrome::Panel(_) => {
                self.panel.render(frame, area);
                Panel::inner_area(area)
            }
        }
    }

    fn inner_width(&self, width: u16) -> u16 {
        match self.chrome {
            InputChrome::Plain => width,
            InputChrome::Panel(_) => width.saturating_sub(2),
        }
    }

    fn chrome_measure(&self, width: u16, height: u16, proposal: LayoutProposal) -> LayoutSizeHint {
        let (width, height) = match self.chrome {
            InputChrome::Plain => (width, height),
            InputChrome::Panel(_) => (width.saturating_add(2), height.saturating_add(2)),
        };
        LayoutSizeHint::content(width, height).normalized(proposal)
    }

    fn panel_click_focus<M>(&self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> bool {
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
            id: FocusId::new(TAG_INPUT_FOCUS),
        });
        ctx.stop_propagation();
        true
    }

    fn on_key(&mut self, key: KeyEvent) -> bool {
        if self.highlighted_tag.is_some() {
            return self.on_tag_key(key);
        }

        if ctrl_char(key, 'h') {
            return self.focus_previous_tag();
        }
        if ctrl_char(key, 'l') {
            return self.focus_next_tag();
        }
        if key.modifiers == KeyModifiers::CONTROL && matches!(key.code, Key::Enter) {
            return self.request_create_exact_query();
        }
        if cancel_key(key) {
            return self.clear_query_or_close();
        }

        let dropdown_keys = keybindings().dropdown().clone();
        if dropdown_keys.next_matches(key) {
            self.popup_open = true;
            return self.move_option(1);
        }
        if dropdown_keys.previous_matches(key) {
            self.popup_open = true;
            return self.move_option(-1);
        }
        if dropdown_keys.page_next_matches(key) {
            self.popup_open = true;
            return self.move_option(MAX_POPUP_ROWS as isize);
        }
        if dropdown_keys.page_previous_matches(key) {
            self.popup_open = true;
            return self.move_option(-(MAX_POPUP_ROWS as isize));
        }
        if self.query.is_empty()
            && !self.popup_open
            && (dropdown_keys.select_matches(key)
                || matches!(key.code, Key::Enter | Key::Char(' ')))
        {
            self.popup_open = true;
            self.highlighted_option = 0;
            return true;
        }
        if dropdown_keys.select_matches(key) || matches!(key.code, Key::Enter) {
            return self.add_highlighted_existing();
        }

        match key.code {
            Key::Backspace => self.backspace_query(),
            Key::Esc => self.clear_query_or_close(),
            Key::Char(value) if text_char(key) => {
                self.query.push(value);
                self.highlighted_option = 0;
                self.popup_open = true;
                self.cursor_fade.reset();
                self.events.push(TagInputEvent::QueryChanged {
                    query: self.query.clone(),
                });
                true
            }
            _ => false,
        }
    }

    fn on_tag_key(&mut self, key: KeyEvent) -> bool {
        if ctrl_char(key, 'h') {
            return self.move_tag(-1);
        }
        if ctrl_char(key, 'l') {
            return self.move_tag(1);
        }
        if matches!(key.code, Key::Enter) {
            self.focus_search();
            self.popup_open = true;
            self.highlighted_option = 0;
            return true;
        }
        if matches!(key.code, Key::Char(' ')) && text_char(key) {
            self.focus_search();
            self.popup_open = true;
            if !self.query.is_empty() {
                self.query.push(' ');
                self.events.push(TagInputEvent::QueryChanged {
                    query: self.query.clone(),
                });
            }
            self.highlighted_option = 0;
            self.cursor_fade.reset();
            return true;
        }
        if let Key::Char(value) = key.code
            && text_char(key)
        {
            self.focus_search();
            self.query.push(value);
            self.popup_open = true;
            self.highlighted_option = 0;
            self.cursor_fade.reset();
            self.events.push(TagInputEvent::QueryChanged {
                query: self.query.clone(),
            });
            return true;
        }
        if matches!(key.code, Key::Backspace | Key::Delete) {
            return self.remove_highlighted_tag();
        }
        if matches!(key.code, Key::Esc) || ctrl_char(key, '[') {
            self.focus_search();
            return true;
        }
        false
    }

    fn render_popup(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let theme = theme();
        frame.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(preset().border()))
            .border_style(Style::default().fg(if self.focused {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let options = self.filtered_options();
        let visible_height = inner.height as usize;
        let (rows, start, option_count) = if options.is_empty() {
            (
                vec![Line::from(Span::styled(
                    "No tags available",
                    Style::default()
                        .fg(theme.muted_fg())
                        .add_modifier(Modifier::ITALIC),
                ))],
                0,
                0,
            )
        } else {
            let start = self.visible_option_start(visible_height, options.len());
            let option_count = options.len();
            let rows = options
                .into_iter()
                .enumerate()
                .skip(start)
                .take(visible_height)
                .map(|(index, option)| {
                    self.option_line(&option.label, index == self.highlighted_option)
                })
                .collect::<Vec<_>>();
            (rows, start, option_count)
        };
        let rows_area = if option_count > visible_height && inner.width > 1 {
            Rect::new(
                inner.x,
                inner.y,
                inner.width.saturating_sub(1),
                inner.height,
            )
        } else {
            inner
        };
        frame.render_widget(Paragraph::new(Text::from(rows)), rows_area);
        if option_count > visible_height && inner.width > 1 {
            self.render_scrollbar(frame, inner, start, visible_height, option_count);
        }
    }

    fn render_scrollbar(
        &self,
        frame: &mut Frame,
        area: Rect,
        start: usize,
        visible_height: usize,
        option_count: usize,
    ) {
        if area.height == 0 || visible_height == 0 || option_count <= visible_height {
            return;
        }
        let theme = theme();
        let track_style = Style::default().fg(theme.border_fg());
        let thumb_style = Style::default().fg(theme.accent_fg());
        let height = area.height as usize;
        let thumb_height = ((visible_height * height) / option_count)
            .max(1)
            .min(height);
        let max_start = option_count.saturating_sub(visible_height).max(1);
        let thumb_top = ((height.saturating_sub(thumb_height)) * start) / max_start;
        let x = area.x + area.width.saturating_sub(1);
        for row in 0..height {
            let glyph = if row >= thumb_top && row < thumb_top + thumb_height {
                "┃"
            } else {
                "│"
            };
            let style = if glyph == "┃" {
                thumb_style
            } else {
                track_style
            };
            frame.render_widget(
                Paragraph::new(glyph).style(style),
                Rect::new(x, area.y + row as u16, 1, 1),
            );
        }
    }

    fn visible_option_start(&self, visible_height: usize, option_count: usize) -> usize {
        if visible_height == 0 || self.highlighted_option < visible_height {
            return 0;
        }
        self.highlighted_option
            .saturating_add(1)
            .saturating_sub(visible_height)
            .min(option_count.saturating_sub(visible_height))
    }

    fn option_line(&self, label: &str, highlighted: bool) -> Line<'static> {
        let theme = theme();
        let style = if highlighted {
            Style::default()
                .fg(theme.selected_fg())
                .bg(theme.selected_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_fg())
        };
        Line::from(Span::styled(label.to_owned(), style))
    }

    fn field_layout(&self, width: u16) -> FieldLayout {
        let width = width as usize;
        let mut lines = Vec::new();
        let mut spans = Vec::new();
        let mut used = 0;

        for (index, tag) in self.selected.iter().enumerate() {
            let chip = self.chip_spans(tag.label(), self.highlighted_tag == Some(index));
            let chip_width = line_width(&Line::from(chip.clone()));
            if used > 0 && used + chip_width > width {
                lines.push(Line::from(std::mem::take(&mut spans)));
                used = 0;
            }
            spans.extend(chip);
            used += chip_width;
        }

        let input = self.input_spans(width.saturating_sub(used));
        let input_width = line_width(&Line::from(input.clone()));
        if used > 0 && used + input_width > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
        }
        let input_row = lines.len() as u16;
        spans.extend(input);
        lines.push(Line::from(spans));

        FieldLayout { lines, input_row }
    }

    fn chip_spans(&self, label: &str, highlighted: bool) -> Vec<Span<'static>> {
        let theme = theme();
        let background = if highlighted {
            theme.selected_bg()
        } else {
            theme.highlight_bg()
        };
        let foreground = if highlighted {
            theme.selected_fg()
        } else {
            theme.highlight_fg()
        };
        let cap_style = Style::default().fg(background);
        let chip_style = Style::default().fg(foreground).bg(background);
        vec![
            Span::styled(LEFT_CAP, cap_style),
            Span::styled(format!("{label} ×"), chip_style),
            Span::styled(RIGHT_CAP, cap_style),
            Span::raw(" "),
        ]
    }

    fn input_spans(&self, width: usize) -> Vec<Span<'static>> {
        let theme = theme();
        let width = width.max(1);
        if self.query.is_empty() {
            return placeholder_line(
                &self.placeholder,
                self.inline_hotkey(),
                width,
                self.focused && self.highlighted_tag.is_none(),
                self.pending_hotkey_prefix.as_deref(),
                self.cursor_fade
                    .style(Style::default().fg(theme.muted_fg())),
                Style::default().fg(theme.muted_fg()),
            )
            .spans;
        }

        let value_style = Style::default().fg(theme.text_fg());
        let mut spans = self
            .query
            .chars()
            .map(|value| Span::styled(value.to_string(), value_style))
            .collect::<Vec<_>>();
        if self.focused && self.highlighted_tag.is_none() {
            spans.push(Span::styled(" ", self.cursor_fade.style(value_style)));
        }
        spans
    }

    fn popup_area(&self, input_row: u16, bounds: Rect) -> Rect {
        let y = self.area.y.saturating_add(input_row).saturating_add(1);
        let available = bounds.y.saturating_add(bounds.height).saturating_sub(y);
        let height = self.popup_height().min(available);
        let width = self
            .popup_width()
            .min(bounds.right().saturating_sub(self.area.x));
        Rect::new(self.area.x, y, width, height)
    }

    fn popup_height(&self) -> u16 {
        let rows = self
            .filtered_options()
            .len()
            .max(1)
            .min(MAX_POPUP_ROWS as usize) as u16;
        rows.saturating_add(2)
    }

    fn popup_width(&self) -> u16 {
        let option_width = self
            .filtered_options()
            .iter()
            .map(|option| line_width(&Line::from(option.label.as_str())))
            .max()
            .unwrap_or_else(|| line_width(&Line::from("No tags available")));
        option_width
            .saturating_add(4)
            .max(POPUP_MIN_WIDTH as usize)
            .min(u16::MAX as usize) as u16
    }

    fn filtered_options(&self) -> Vec<TagOption<Id>> {
        let query = self.query.to_ascii_lowercase();
        self.options
            .iter()
            .filter(|option| !self.is_selected_id(&option.id))
            .filter(|option| query.is_empty() || option.label.to_ascii_lowercase().contains(&query))
            .cloned()
            .collect()
    }

    fn unwrapped_width(&self) -> u16 {
        let tags_width = self
            .selected
            .iter()
            .map(|tag| line_width(&Line::from(self.chip_spans(tag.label(), false))))
            .sum::<usize>();
        let input_width = if self.query.is_empty() {
            line_width(&placeholder_line(
                &self.placeholder,
                self.inline_hotkey(),
                usize::MAX,
                false,
                self.pending_hotkey_prefix.as_deref(),
                Style::default(),
                Style::default(),
            ))
        } else {
            line_width(&Line::from(self.query.as_str())).saturating_add(1)
        };
        tags_width
            .saturating_add(input_width)
            .min(u16::MAX as usize) as u16
    }

    fn move_option(&mut self, delta: isize) -> bool {
        let len = self.filtered_options().len();
        if len == 0 {
            self.highlighted_option = 0;
            return true;
        }
        self.highlighted_option = move_index(self.highlighted_option, delta, len);
        true
    }

    fn add_highlighted_existing(&mut self) -> bool {
        let options = self.filtered_options();
        let Some(option) = options.get(self.highlighted_option).cloned() else {
            return false;
        };
        self.selected.push(SelectedTag::Existing {
            id: option.id.clone(),
            label: option.label.clone(),
        });
        self.events.push(TagInputEvent::AddedExisting {
            id: option.id,
            label: option.label,
        });
        self.query.clear();
        self.highlighted_option = 0;
        self.popup_open = false;
        self.cursor_fade.reset();
        true
    }

    fn request_create_exact_query(&mut self) -> bool {
        let label = self.query.trim().to_owned();
        if label.is_empty() {
            return false;
        }
        self.selected.push(SelectedTag::Custom {
            label: label.clone(),
        });
        self.events.push(TagInputEvent::CreateRequested { label });
        self.query.clear();
        self.highlighted_option = 0;
        self.popup_open = false;
        self.cursor_fade.reset();
        true
    }

    fn backspace_query(&mut self) -> bool {
        if self.query.pop().is_some() {
            self.highlighted_option = 0;
            self.popup_open = !self.query.is_empty();
            self.cursor_fade.reset();
            self.events.push(TagInputEvent::QueryChanged {
                query: self.query.clone(),
            });
            return true;
        }
        if self.selected.is_empty() {
            return false;
        }
        self.highlighted_tag = Some(self.selected.len() - 1);
        self.popup_open = false;
        true
    }

    fn clear_query_or_close(&mut self) -> bool {
        let changed = !self.query.is_empty() || self.popup_open;
        self.query.clear();
        self.highlighted_option = 0;
        self.popup_open = false;
        if changed {
            self.events.push(TagInputEvent::QueryChanged {
                query: self.query.clone(),
            });
        }
        changed
    }

    fn handle_hotkey<M>(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                self.pending_hotkey_prefix = Some(prefix.clone());
                self.sync_panel();
                ctx.request_redraw();
                EventOutcome::Ignored
            }
            HotkeyEvent::Canceled => {
                if self.pending_hotkey_prefix.take().is_some() {
                    self.sync_panel();
                    ctx.request_redraw();
                }
                EventOutcome::Ignored
            }
            HotkeyEvent::Commit(_) => {
                self.pending_hotkey_prefix = None;
                self.sync_panel();
                self.focus_search();
                ctx.focus(FocusRequest::TargetAt {
                    path: ctx.current_path(),
                    id: FocusId::new(TAG_INPUT_FOCUS),
                });
                ctx.request_redraw();
                ctx.stop_propagation();
                EventOutcome::Handled
            }
        }
    }

    fn focus_previous_tag(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        self.highlighted_tag = Some(self.selected.len() - 1);
        self.popup_open = false;
        true
    }

    fn focus_next_tag(&mut self) -> bool {
        if self.selected.is_empty() {
            return false;
        }
        self.highlighted_tag = Some(0);
        self.popup_open = false;
        true
    }

    fn focus_search(&mut self) {
        self.highlighted_tag = None;
        self.cursor_fade.reset();
    }

    fn move_tag(&mut self, delta: isize) -> bool {
        let Some(index) = self.highlighted_tag else {
            return false;
        };
        if delta.is_negative() && index == 0 {
            self.focus_search();
            return true;
        }
        if delta.is_positive() && index + 1 >= self.selected.len() {
            self.focus_search();
            return true;
        }
        self.highlighted_tag = Some(move_index(index, delta, self.selected.len()));
        true
    }

    fn remove_highlighted_tag(&mut self) -> bool {
        let Some(index) = self.highlighted_tag else {
            return false;
        };
        if index >= self.selected.len() {
            self.focus_search();
            return true;
        }
        let removed = self.selected.remove(index);
        match removed {
            SelectedTag::Existing { id, label } => {
                self.events
                    .push(TagInputEvent::RemovedExisting { id, label });
            }
            SelectedTag::Custom { label } => {
                self.events.push(TagInputEvent::RemovedCustom { label });
            }
        }
        if self.selected.is_empty() {
            self.focus_search();
        } else if index < self.selected.len() {
            self.highlighted_tag = Some(index);
        } else {
            self.highlighted_tag = Some(self.selected.len() - 1);
        }
        true
    }

    fn option_for_id(&self, id: &Id) -> Option<&TagOption<Id>> {
        self.options.iter().find(|option| &option.id == id)
    }

    fn option_for_label(&self, label: &str) -> Option<&TagOption<Id>> {
        self.options
            .iter()
            .find(|option| option.label.eq_ignore_ascii_case(label))
    }

    fn is_selected_id(&self, id: &Id) -> bool {
        self.selected.iter().any(|tag| match tag {
            SelectedTag::Existing { id: selected, .. } => selected == id,
            SelectedTag::Custom { .. } => false,
        })
    }
}

impl<Id, M> TuiNode<M> for TagInput<Id>
where
    Id: Clone + Eq + 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = match proposal.width {
            AxisProposal::Exact(width) | AxisProposal::AtMost(width) => {
                self.inner_width(width).max(1)
            }
            AxisProposal::Unbounded => self.unwrapped_width().max(32),
        };
        self.chrome_measure(width, self.content_height_for_width(width), proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.outer_area = area;
        self.area = self.content_area(area);
        self.overlay_bounds = ctx.overlay_bounds();
        let focus_area = if self.is_panel_mode() {
            self.outer_area
        } else {
            self.area
        };
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_text_entry_focusable_with_hotkey_sequences(
                FocusId::new(TAG_INPUT_FOCUS),
                focus_area,
                true,
                vec![hotkey],
                true,
            );
        } else {
            ctx.register_text_entry_focusable(
                FocusId::new(TAG_INPUT_FOCUS),
                focus_area,
                true,
                true,
            );
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.panel_click_focus(event, ctx) {
            return EventOutcome::Handled;
        }
        if let TuiEvent::Hotkey(hotkey) = event {
            return self.handle_hotkey(hotkey, ctx);
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if cancel_key(*key)
            && self.query.is_empty()
            && !self.popup_open
            && self.highlighted_tag.is_none()
        {
            ctx.focus(FocusRequest::Unfocus);
            ctx.request_redraw();
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        if self.on_key(*key) {
            ctx.request_redraw();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused);
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

impl<Id> Animated for TagInput<Id>
where
    Id: Clone + Eq,
{
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.cursor_fade
            .tick(self.focused && self.highlighted_tag.is_none(), dt, settings)
    }
}

#[derive(Debug, Clone)]
struct FieldLayout {
    lines: Vec<Line<'static>>,
    input_row: u16,
}

fn unique_options<T, Id>(
    options: impl IntoIterator<Item = T>,
    id: impl Fn(&T) -> Id,
    label: impl Fn(&T) -> String,
) -> Vec<TagOption<Id>>
where
    Id: Clone + Eq,
{
    let mut unique: Vec<TagOption<Id>> = Vec::new();
    for option in options {
        let id = id(&option);
        let label = label(&option);
        if !label.trim().is_empty() && !unique.iter().any(|existing| existing.id == id) {
            unique.push(TagOption { id, label });
        }
    }
    unique
}

fn unique_strings(labels: impl IntoIterator<Item = impl Into<String>>) -> Vec<String> {
    let mut unique = Vec::new();
    for label in labels {
        let label = label.into();
        if !label.trim().is_empty()
            && !unique
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(&label))
        {
            unique.push(label);
        }
    }
    unique
}

fn move_index(index: usize, delta: isize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if delta.is_negative() {
        index.saturating_sub(delta.unsigned_abs())
    } else {
        index.saturating_add(delta as usize).min(len - 1)
    }
}

fn ctrl_char(key: KeyEvent, value: char) -> bool {
    key.modifiers == KeyModifiers::CONTROL
        && matches!(key.code, Key::Char(actual) if actual == value)
}

fn cancel_key(key: KeyEvent) -> bool {
    matches!(key.code, Key::Esc) || ctrl_char(key, '[')
}

fn rect_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x && x < area.x + area.width && y >= area.y && y < area.y + area.height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Propagation, TreePath};

    #[test]
    fn panel_height_for_width_includes_border_chrome() {
        let plain = TagInput::new(["alpha", "beta"]).placeholder("tag");
        let panel = TagInput::new(["alpha", "beta"])
            .placeholder("tag")
            .panel("Tags");

        assert_eq!(plain.height_for_width(8), 1);
        assert_eq!(panel.height_for_width(10), 3);
    }

    #[test]
    fn panel_registers_outer_area_for_hotkey_focus() {
        let mut input = TagInput::new(["alpha"]).hotkey("t").panel("Tags");
        let mut ctx = LayoutCtx::new();

        <TagInput as TuiNode<()>>::layout(&mut input, Rect::new(2, 3, 12, 4), &mut ctx);

        let target = ctx.focus_targets().first().unwrap();
        assert_eq!(target.area, Rect::new(2, 3, 12, 4));
        assert_eq!(target.hotkey_sequences, vec!["t"]);
    }

    #[test]
    fn panel_hotkey_commit_targets_current_panel_route() {
        let mut input = TagInput::new(["alpha"]).hotkey("t").panel("Tags");
        let mut ctx = EventCtx::<()>::default();

        let outcome = input.event(&TuiEvent::Hotkey(HotkeyEvent::Commit("t".into())), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::new(),
                id: FocusId::new(TAG_INPUT_FOCUS),
            })
        );
    }
}
