use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::event::{Key, KeyEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ChildKey, Children, ColorTween,
    EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusRequest, FocusTarget, HotkeyEvent,
    HotkeyLabelMode, HotkeyMatch, HotkeySequenceMatcher, LayoutCtx, LayoutProposal, LayoutResult,
    LayoutSizeHint, LifecycleCtx, TabsVariant, TickResult, TreePath, TuiEvent, TuiNode, Tween,
    border_chars, border_set, hotkey_badge_spans, hotkey_badge_width, hotkey_edge_spans,
    hotkey_label_spans, hotkey_sequence_to_event, hotkey_underline_style, keybindings, line_width,
    preset, theme,
};

use super::dialog_layer::DockChrome;

const TABS_FOCUS: &str = "tabs";

pub struct Tab<M = ()> {
    title: String,
    hotkey: Option<String>,
    body: Box<dyn TuiNode<M>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalCloseReason {
    CloseKey,
    Escape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabsSelectionMemory {
    Remember,
    ResetOnOpen,
    ResetOnClose,
}

pub struct Tabs<M = ()> {
    titles: Vec<String>,
    tab_hotkeys: Vec<Option<String>>,
    bodies: Children<M>,
    body_keys: Vec<ChildKey>,
    selected: usize,
    previous_selected: usize,
    allow_looping: bool,
    variant: Option<TabsVariant>,
    border: Option<BorderKind>,
    edge_borders: Option<Borders>,
    bordered: Option<bool>,
    animation: Option<AnimationSpec>,
    focused: bool,
    transition: Tween,
    border_color: ColorTween,
    tab_color: ColorTween,
    selected_color: ColorTween,
    body_area: Rect,
    focus_path: TreePath,
    last_focused_targets: Vec<Option<(TreePath, FocusId)>>,
    body_focus_transfer_pending: bool,
    hotkey: Option<String>,
    hotkey_matcher: HotkeySequenceMatcher,
    pending_hotkey_prefix: Option<String>,
    modal: bool,
    selection_memory: TabsSelectionMemory,
    on_close: Option<Box<dyn Fn(ModalCloseReason) -> M>>,
}

impl<M> Tab<M>
where
    M: 'static,
{
    pub fn new<C>(title: impl Into<String>, body: C) -> Self
    where
        C: TuiNode<M> + 'static,
    {
        Self {
            title: title.into(),
            hotkey: None,
            body: Box::new(body),
        }
    }

    pub fn text(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, TextTabBody::new(body))
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }
}

impl<M> Tabs<M>
where
    M: 'static,
{
    pub fn new(tabs: Vec<Tab<M>>) -> Self {
        let theme = theme();
        let mut titles = Vec::with_capacity(tabs.len());
        let mut tab_hotkeys = Vec::with_capacity(tabs.len());
        let mut body_keys = Vec::with_capacity(tabs.len());
        let mut bodies = Children::new();

        for (index, tab) in tabs.into_iter().enumerate() {
            let key = ChildKey::new(format!("tab-{index}"));
            titles.push(tab.title);
            tab_hotkeys.push(tab.hotkey);
            body_keys.push(key.clone());
            bodies = bodies.child(key, tab.body);
        }

        let hotkey_entries = tab_hotkey_entries(&tab_hotkeys);
        let hotkey_matcher =
            HotkeySequenceMatcher::new(hotkey_entries.iter().map(|(_, hotkey)| hotkey.as_str()));
        let last_focused_targets = vec![None; body_keys.len()];

        Self {
            titles,
            tab_hotkeys,
            bodies,
            body_keys,
            selected: 0,
            previous_selected: 0,
            allow_looping: true,
            variant: None,
            border: None,
            edge_borders: None,
            bordered: None,
            animation: None,
            focused: false,
            transition: Tween::idle(1.0),
            border_color: ColorTween::idle(theme.border_fg()),
            tab_color: ColorTween::idle(theme.border_fg()),
            selected_color: ColorTween::idle(theme.muted_fg()),
            body_area: Rect::default(),
            focus_path: TreePath::default(),
            last_focused_targets,
            body_focus_transfer_pending: false,
            hotkey: None,
            hotkey_matcher,
            pending_hotkey_prefix: None,
            modal: false,
            selection_memory: TabsSelectionMemory::Remember,
            on_close: None,
        }
    }

    /// Creates dialog-styled modal tabs for use inside `DialogLayer`.
    ///
    /// `DialogLayer` still owns placement, docking, backdrop, and focus trapping. Callers may
    /// override variant or edge borders with the normal builders after this shortcut.
    pub fn dialog(tabs: Vec<Tab<M>>) -> Self {
        Self::new(tabs)
            .modal()
            .variant(TabsVariant::Boxed)
            .edge_borders(Borders::ALL)
    }

    pub fn modal(mut self) -> Self {
        self.modal = true;
        self.selection_memory = TabsSelectionMemory::ResetOnClose;
        self
    }

    pub fn selection_memory(mut self, memory: TabsSelectionMemory) -> Self {
        self.selection_memory = memory;
        self
    }

    pub fn set_selection_memory(&mut self, memory: TabsSelectionMemory) {
        self.selection_memory = memory;
    }

    pub fn prepare_modal_open(&mut self, settings: AnimationSettings) {
        if self.modal && self.selection_memory == TabsSelectionMemory::ResetOnOpen {
            self.select_index_with_settings(0, settings);
        }
    }

    pub fn prepare_modal_close(&mut self) {
        if self.modal && self.selection_memory == TabsSelectionMemory::ResetOnClose {
            self.reset_selection();
        }
    }

    pub fn on_close(mut self, handler: impl Fn(ModalCloseReason) -> M + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    pub fn selected(mut self, selected: usize) -> Self {
        let selected = self.clamp_selected(selected);
        self.selected = selected;
        self.previous_selected = selected;
        self
    }

    pub fn allow_looping(mut self, allow_looping: bool) -> Self {
        self.allow_looping = allow_looping;
        self
    }

    pub fn variant(mut self, variant: TabsVariant) -> Self {
        self.variant = Some(variant);
        self
    }

    pub fn set_variant(&mut self, variant: TabsVariant) {
        self.variant = Some(variant);
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn edge_borders(mut self, borders: Borders) -> Self {
        self.edge_borders = Some(borders);
        self
    }

    pub fn set_edge_borders(&mut self, borders: Borders) {
        self.edge_borders = Some(borders);
    }

    pub fn clear_edge_borders(&mut self) {
        self.edge_borders = None;
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = Some(bordered);
        self
    }

    pub fn animation(mut self, animation: AnimationSpec) -> Self {
        self.animation = Some(animation);
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
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self.snap_focus_colors(focused);
        self
    }

    pub fn selected_index(&self) -> usize {
        self.clamp_selected(self.selected)
    }

    pub fn select_index(&mut self, selected: usize) {
        self.select_index_with_settings(selected, crate::animation_settings());
    }

    pub fn select_index_with_settings(&mut self, selected: usize, settings: AnimationSettings) {
        let current = self.clamp_selected(self.selected);
        let selected = self.clamp_selected(selected);
        if selected == current {
            self.selected = current;
            if !self.transition.is_active() {
                self.previous_selected = current;
            }
            return;
        }

        self.previous_selected = current;
        self.selected = selected;
        let animation = settings.resolve(self.animation.unwrap_or_default());
        if animation.enabled {
            self.transition
                .start(0.0, 1.0, animation.duration, animation.easing);
        } else {
            self.transition
                .start(1.0, 1.0, Duration::ZERO, animation.easing);
        }
    }

    fn reset_selection(&mut self) {
        self.selected = self.clamp_selected(0);
        self.previous_selected = self.selected;
        self.transition.snap_to(1.0);
    }

    pub fn next(&mut self) {
        self.select_index(self.next_index());
    }

    pub fn previous(&mut self) {
        self.select_index(self.previous_index());
    }

    fn next_index(&self) -> usize {
        let selected = self.clamp_selected(self.selected);
        let last = self.titles.len().saturating_sub(1);
        if selected >= last && self.allow_looping {
            0
        } else {
            (selected + 1).min(last)
        }
    }

    fn previous_index(&self) -> usize {
        let selected = self.clamp_selected(self.selected);
        if selected == 0 && self.allow_looping {
            self.titles.len().saturating_sub(1)
        } else {
            selected.saturating_sub(1)
        }
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        if !focused {
            self.transition.snap_to_end();
        }
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.start_focus_color_transition(focused, settings);
    }

    fn selected_key(&self) -> Option<&ChildKey> {
        self.body_keys.get(self.selected_index())
    }

    fn target_is_in_selected_body(&self, target: &FocusTarget) -> bool {
        self.selected_key()
            .is_some_and(|key| target.path.first() == Some(key))
    }

    fn clamp_selected(&self, selected: usize) -> usize {
        selected.min(self.titles.len().saturating_sub(1))
    }

    fn snap_focus_colors(&mut self, focused: bool) {
        let theme = theme();
        self.border_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        self.tab_color.snap_to(if focused {
            theme.muted_fg()
        } else {
            theme.border_fg()
        });
        self.selected_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
    }

    fn start_focus_color_transition(&mut self, focused: bool, settings: AnimationSettings) {
        let theme = theme();
        self.border_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.border_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.tab_color.start(
            if focused {
                theme.muted_fg()
            } else {
                theme.border_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.selected_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            },
            settings,
            focus_color_animation(),
        );
    }

    fn calculate_body_area(&self, area: Rect) -> Rect {
        let area = self.modal_render_area(area);
        let variant = self.variant.unwrap_or_else(|| preset().tabs().variant());
        let bordered = self.bordered.unwrap_or_else(|| preset().tabs().bordered());
        if self.titles.is_empty() {
            return if bordered {
                Block::default().borders(Borders::ALL).inner(area)
            } else {
                area
            };
        }

        if variant == TabsVariant::Minimal && bordered {
            return Block::default().borders(Borders::ALL).inner(area);
        }

        let edge_borders = self.resolved_edge_borders(bordered);
        let header_height = match variant {
            TabsVariant::Minimal => 1,
            TabsVariant::Underline if bordered => 1,
            TabsVariant::Underline => 2,
            TabsVariant::Boxed => Self::boxed_header_height(bordered, edge_borders),
        };
        let [_, body] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Fill(1)])
            .areas(area);

        let panel_borders = self.panel_borders(variant, edge_borders);
        if !panel_borders.is_empty() {
            Self::panel_inner_area(body, panel_borders)
        } else {
            body
        }
    }

    fn panel_inner_area(area: Rect, borders: Borders) -> Rect {
        let left_edge_dock = !borders.contains(Borders::TOP)
            && !borders.contains(Borders::BOTTOM)
            && borders.contains(Borders::LEFT);
        let right_edge_dock = !borders.contains(Borders::TOP)
            && !borders.contains(Borders::BOTTOM)
            && borders.contains(Borders::RIGHT);
        let left = if left_edge_dock {
            2
        } else {
            borders.contains(Borders::LEFT) as u16
        };
        let right = if right_edge_dock {
            2
        } else {
            borders.contains(Borders::RIGHT) as u16
        };
        let top = borders.contains(Borders::TOP) as u16;
        let bottom = borders.contains(Borders::BOTTOM) as u16;
        Rect::new(
            area.x.saturating_add(left),
            area.y.saturating_add(top),
            area.width.saturating_sub(left.saturating_add(right)),
            area.height.saturating_sub(top.saturating_add(bottom)),
        )
    }

    fn resolved_edge_borders(&self, bordered: bool) -> Borders {
        if !bordered {
            return Borders::NONE;
        }
        self.edge_borders.unwrap_or(Borders::ALL)
    }

    fn panel_borders(&self, variant: TabsVariant, edge_borders: Borders) -> Borders {
        match variant {
            TabsVariant::Boxed => edge_borders & (Borders::LEFT | Borders::RIGHT | Borders::BOTTOM),
            TabsVariant::Minimal | TabsVariant::Underline => edge_borders,
        }
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let selected = self.selected_index();
        let variant = self.variant.unwrap_or_else(|| preset().tabs().variant());
        let bordered = self.bordered.unwrap_or_else(|| preset().tabs().bordered());
        let border = self.border.unwrap_or_else(|| preset().border());

        if self.titles.is_empty() {
            self.render_empty(frame, area, bordered, border);
            return;
        }

        if variant == TabsVariant::Minimal {
            self.render_minimal(frame, area, selected, bordered, border);
            return;
        }

        let edge_borders = self.resolved_edge_borders(bordered);
        let header_height = match variant {
            TabsVariant::Underline if bordered => 1,
            TabsVariant::Underline => 2,
            TabsVariant::Boxed => Self::boxed_header_height(bordered, edge_borders),
            TabsVariant::Minimal => unreachable!(),
        };
        let [tabs_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Fill(1)])
            .areas(area);

        let panel_borders = self.panel_borders(variant, edge_borders);

        match variant {
            TabsVariant::Boxed => {
                self.render_boxed_header(frame, tabs_area, selected, border, bordered, edge_borders)
            }
            TabsVariant::Underline => {
                self.render_underline_header(frame, tabs_area, selected, bordered)
            }
            TabsVariant::Minimal => unreachable!(),
        }

        if !panel_borders.is_empty() {
            let block = Block::default()
                .borders(panel_borders)
                .border_set(border_set(border))
                .border_style(self.border_style());
            frame.render_widget(block, body_area);
            if panel_borders.contains(Borders::BOTTOM) {
                self.render_hotkey(frame, body_area, border);
            }
            if variant == TabsVariant::Underline {
                frame.render_widget(
                    Paragraph::new(self.underline_panel_top_line(
                        selected,
                        body_area.width,
                        border,
                    )),
                    Rect::new(body_area.x, body_area.y, body_area.width, 1),
                );
            }
        }
    }

    fn render_empty(&self, frame: &mut Frame, area: Rect, bordered: bool, border: BorderKind) {
        if bordered {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border_set(border))
                .border_style(self.border_style());
            frame.render_widget(block, area);
        }
        frame.render_widget(Paragraph::new("No tabs to show."), self.body_area);
    }

    fn render_minimal(
        &self,
        frame: &mut Frame,
        area: Rect,
        selected: usize,
        bordered: bool,
        border: BorderKind,
    ) {
        if bordered {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border_set(border))
                .border_style(self.border_style())
                .title(self.minimal_title_line(selected, area.width.saturating_sub(2)));
            frame.render_widget(block, area);
            self.render_hotkey(frame, area, border);
        } else {
            let [tabs_area, _] = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Fill(1)])
                .areas(area);
            frame.render_widget(
                Paragraph::new(self.minimal_title_line(selected, tabs_area.width)),
                tabs_area,
            );
        }
    }

    fn render_boxed_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        selected: usize,
        border: BorderKind,
        bordered: bool,
        edge_borders: Borders,
    ) {
        let [top, middle, bottom] =
            self.boxed_title_lines(selected, area.width, border, bordered, edge_borders);
        let top_border = bordered && edge_borders.contains(Borders::TOP);
        if top_border {
            frame.render_widget(Paragraph::new(top), area);
        }
        let middle_y = area.y + top_border as u16;
        frame.render_widget(
            Paragraph::new(middle),
            Rect::new(area.x, middle_y, area.width, 1),
        );
        frame.render_widget(
            Paragraph::new(if bordered { bottom } else { bottom }),
            Rect::new(area.x, middle_y + 1, area.width, 1),
        );
    }

    fn boxed_header_height(bordered: bool, edge_borders: Borders) -> u16 {
        2 + (bordered && edge_borders.contains(Borders::TOP)) as u16
    }

    fn render_underline_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        selected: usize,
        bordered: bool,
    ) {
        frame.render_widget(Paragraph::new(self.underline_title_line(selected)), area);
        if area.height > 1 {
            let line = if bordered {
                self.bordered_animated_underline_line(selected, area.width)
            } else {
                self.animated_underline_line(selected, area.width)
            };
            frame.render_widget(
                Paragraph::new(line),
                Rect::new(area.x, area.y + 1, area.width, 1),
            );
        }
    }

    fn render_selected_body<'a>(&'a self, frame: &mut Frame, ctx: &mut crate::RenderCtx<'a>) {
        if let Some(key) = self.selected_key()
            && let Some(body) = self.bodies.get(key)
        {
            body.render(frame, self.body_area, ctx);
        }
    }

    fn title_line(&self, selected: usize, separator: &'static str) -> Line<'static> {
        let mut spans = Vec::new();
        for (index, _) in self.titles.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(separator, self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_label_spans(index, selected, style));
        }
        Line::from(spans)
    }

    fn tab_label_spans(
        &self,
        index: usize,
        selected: usize,
        base_style: Style,
    ) -> Vec<Span<'static>> {
        if self.transition.is_active() && !(index == selected && self.transition.progress() >= 1.0)
        {
            return self.tab_title_spans(index, &self.tab_label(index), selected, base_style);
        }

        let Some(title) = self.titles.get(index) else {
            return Vec::new();
        };
        let active_prefix = self.active_hotkey_prefix(index);
        hotkey_label_spans(
            title,
            self.tab_hotkeys.get(index).and_then(Option::as_deref),
            HotkeyLabelMode::Inline,
            active_prefix.as_deref(),
            base_style,
            hotkey_underline_style(base_style),
        )
    }

    fn tab_title_spans(
        &self,
        index: usize,
        title: &str,
        selected: usize,
        base_style: Style,
    ) -> Vec<Span<'static>> {
        if !self.transition.is_active() || index == selected && self.transition.progress() >= 1.0 {
            return vec![Span::styled(title.to_owned(), base_style)];
        }

        let moving_right = self.selected >= self.previous_selected;
        let Some((path_start, path_end)) = self.transition_path() else {
            return vec![Span::styled(title.to_owned(), base_style)];
        };
        if index < path_start || index > path_end {
            return vec![Span::styled(title.to_owned(), base_style)];
        }

        let progress = self.transition.value().clamp(0.0, 1.0);
        let total_width = self.transition_path_width(path_start, path_end).max(1);
        let offset = self.transition_path_offset(path_start, index);
        let cut = (progress * total_width as f64).round() as usize;
        let left_cutoff = total_width.saturating_sub(cut);
        let mut spans = Vec::new();
        let mut cursor = 0;

        for ch in title.chars() {
            let ch_width = char_width(ch);
            let next = cursor + ch_width;
            let path_cursor = offset + cursor;
            let path_next = offset + next;
            let style = if index == self.previous_selected {
                let stays_highlighted = if moving_right {
                    path_cursor >= cut
                } else {
                    path_next <= left_cutoff
                };
                if stays_highlighted {
                    self.selected_tab_style()
                } else {
                    self.tab_style()
                }
            } else if moving_right {
                let becomes_highlighted = path_next <= cut;
                if becomes_highlighted {
                    self.selected_tab_style()
                } else {
                    self.tab_style()
                }
            } else {
                let becomes_highlighted = path_cursor >= left_cutoff;
                if becomes_highlighted {
                    self.selected_tab_style()
                } else {
                    self.tab_style()
                }
            };
            spans.push(Span::styled(ch.to_string(), style));
            cursor = next;
        }

        spans
    }

    fn transition_path(&self) -> Option<(usize, usize)> {
        if self.previous_selected == self.selected {
            return None;
        }

        Some((
            self.previous_selected.min(self.selected),
            self.previous_selected.max(self.selected),
        ))
    }

    fn transition_path_width(&self, start: usize, end: usize) -> usize {
        (start..=end)
            .map(|index| self.rendered_tab_label_width(index))
            .sum()
    }

    fn transition_path_offset(&self, start: usize, index: usize) -> usize {
        (start..index)
            .map(|index| self.rendered_tab_label_width(index))
            .sum()
    }

    fn underline_title_line(&self, selected: usize) -> Line<'static> {
        let mut line = self.title_line(selected, "  ");
        line.spans.insert(0, Span::raw("  "));
        line
    }

    fn underline_line(&self, selected: usize, width: u16) -> Line<'static> {
        let theme = theme();
        let selected_start = self.underline_start(selected);
        let selected_width = self.rendered_tab_label_width(selected);
        let width = width as usize;
        let mut spans = Vec::new();
        let before = selected_start.min(width);
        let selected_end = selected_start.saturating_add(selected_width).min(width);
        if before > 0 {
            spans.push(Span::styled(
                "─".repeat(before),
                Style::default().fg(theme.border_fg()),
            ));
        }
        if selected_end > before {
            spans.push(Span::styled(
                "─".repeat(selected_end - before),
                self.selected_underline_style(),
            ));
        }
        if width > selected_end {
            spans.push(Span::styled(
                "─".repeat(width - selected_end),
                Style::default().fg(theme.border_fg()),
            ));
        }
        Line::from(spans)
    }

    fn animated_underline_line(&self, selected: usize, width: u16) -> Line<'static> {
        if !self.transition.is_active() || self.previous_selected == selected {
            return self.underline_line(selected, width);
        }

        let start = lerp_usize(
            self.underline_start(self.previous_selected),
            self.underline_start(selected),
            self.transition.value(),
        );
        let previous_width = self.rendered_tab_label_width(self.previous_selected);
        let selected_width = self.rendered_tab_label_width(selected);
        self.underline_segment_line(
            start,
            lerp_usize(previous_width, selected_width, self.transition.value()).max(1),
            width,
        )
    }

    fn bordered_animated_underline_line(&self, selected: usize, width: u16) -> Line<'static> {
        let border = self.border.unwrap_or_else(|| preset().border());
        let chars = crate::border_chars(border);
        let border_style = self.border_style();
        if width < 3 {
            return Line::from(Span::styled(chars.top_left, border_style));
        }

        let inner_width = width.saturating_sub(3);
        let inner = if !self.transition.is_active() || self.previous_selected == selected {
            self.underline_segment_line(
                self.underline_start(selected).saturating_sub(1),
                self.rendered_tab_label_width(selected),
                inner_width,
            )
        } else {
            let start = lerp_usize(
                self.underline_start(self.previous_selected),
                self.underline_start(selected),
                self.transition.value(),
            )
            .saturating_sub(1);
            let previous_width = self.rendered_tab_label_width(self.previous_selected);
            let selected_width = self.rendered_tab_label_width(selected);
            self.underline_segment_line(
                start,
                lerp_usize(previous_width, selected_width, self.transition.value()).max(1),
                inner_width,
            )
        };

        let mut spans = vec![
            Span::styled(chars.top_left, border_style),
            Span::styled(chars.horizontal, border_style),
        ];
        spans.extend(inner.spans);
        if self.focused {
            self.highlight_last_underline_cell(&mut spans);
        }
        spans.push(Span::styled(chars.top_right, border_style));
        Line::from(spans)
    }

    fn underline_panel_top_line(
        &self,
        selected: usize,
        width: u16,
        _border: BorderKind,
    ) -> Line<'static> {
        self.bordered_animated_underline_line(selected, width)
    }

    fn highlight_last_underline_cell(&self, spans: &mut Vec<Span<'static>>) {
        let Some(last) = spans.pop() else {
            return;
        };
        let text = last.content.to_string();
        let Some((split_at, _)) = text.char_indices().last() else {
            spans.push(last);
            return;
        };
        let (prefix, suffix) = text.split_at(split_at);
        if !prefix.is_empty() {
            spans.push(Span::styled(prefix.to_owned(), last.style));
        }
        spans.push(Span::styled(
            suffix.to_owned(),
            self.selected_underline_style(),
        ));
    }

    fn underline_segment_line(
        &self,
        start: usize,
        segment_width: usize,
        width: u16,
    ) -> Line<'static> {
        let theme = theme();
        let width = width as usize;
        let before = start.min(width);
        let segment_end = start.saturating_add(segment_width).min(width);
        let mut spans = Vec::new();
        if before > 0 {
            spans.push(Span::styled(
                "─".repeat(before),
                Style::default().fg(theme.border_fg()),
            ));
        }
        if segment_end > before {
            spans.push(Span::styled(
                "─".repeat(segment_end - before),
                self.selected_underline_style(),
            ));
        }
        if width > segment_end {
            spans.push(Span::styled(
                "─".repeat(width - segment_end),
                Style::default().fg(theme.border_fg()),
            ));
        }
        Line::from(spans)
    }

    fn underline_start(&self, selected: usize) -> usize {
        1 + self
            .titles
            .iter()
            .take(selected)
            .enumerate()
            .map(|(index, _)| self.rendered_tab_label_width(index) + 2)
            .sum::<usize>()
    }

    fn boxed_title_lines(
        &self,
        selected: usize,
        width: u16,
        border: BorderKind,
        bordered: bool,
        edge_borders: Borders,
    ) -> [Line<'static>; 3] {
        let chars = crate::border_chars(border);
        let border_style = self.border_style();
        let tab_count = self.titles.len();
        let top_border = bordered && edge_borders.contains(Borders::TOP);
        let left_border = bordered && edge_borders.contains(Borders::LEFT);
        let right_border = bordered && edge_borders.contains(Borders::RIGHT);
        let widths = self
            .titles
            .iter()
            .enumerate()
            .map(|(index, _)| self.boxed_tab_width(index))
            .collect::<Vec<_>>();
        let used = usize::from(left_border)
            + usize::from(right_border)
            + widths.iter().sum::<usize>()
            + tab_count;
        let fill = (width as usize).saturating_sub(used);
        let mut top = if top_border && left_border {
            vec![Span::styled(chars.top_left, border_style)]
        } else {
            Vec::new()
        };
        let mut middle = if left_border {
            vec![Span::styled(chars.vertical, border_style)]
        } else {
            Vec::new()
        };
        let mut bottom = if left_border {
            vec![Span::styled(chars.left_join, border_style)]
        } else {
            Vec::new()
        };

        for (index, _) in self.titles.iter().enumerate() {
            let label = self.titles[index].clone();
            let cell_width = widths[index];
            if top_border {
                top.push(Span::styled(
                    chars.horizontal.repeat(cell_width),
                    border_style,
                ));
            }
            let title_style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            let has_hotkey = self
                .tab_hotkeys
                .get(index)
                .and_then(Option::as_ref)
                .is_some();
            bottom.extend(self.boxed_tab_bottom_spans(index, cell_width, title_style));
            let left_pad = 1;
            middle.push(Span::raw(" "));
            middle.extend(self.tab_title_spans(index, &label, selected, title_style));
            let right_pad = cell_width.saturating_sub(text_width(&label) + left_pad);
            middle.push(Span::raw(" ".repeat(right_pad)));
            if index + 1 == tab_count {
                if top_border {
                    top.push(Span::styled(chars.top_join, border_style));
                }
                middle.push(Span::styled(chars.vertical, border_style));
                if !has_hotkey {
                    bottom.push(Span::styled(chars.horizontal, border_style));
                }
                if fill > 0 {
                    if top_border {
                        top.push(Span::styled(chars.horizontal.repeat(fill), border_style));
                    }
                    bottom.push(Span::styled(chars.horizontal.repeat(fill), border_style));
                }
                middle.push(Span::raw(" ".repeat(fill)));
                if right_border {
                    if top_border {
                        top.push(Span::styled(chars.top_right, border_style));
                    }
                    middle.push(Span::styled(chars.vertical, border_style));
                    bottom.push(Span::styled(chars.right_join, border_style));
                }
            } else {
                if top_border {
                    top.push(Span::styled(chars.top_join, border_style));
                }
                middle.push(Span::styled(chars.vertical, border_style));
                if !has_hotkey {
                    bottom.push(Span::styled(chars.horizontal, border_style));
                }
            }
        }

        [Line::from(top), Line::from(middle), Line::from(bottom)]
    }

    fn boxed_tab_width(&self, index: usize) -> usize {
        let title_width = self
            .titles
            .get(index)
            .map(|title| text_width(title))
            .unwrap_or_default();
        let hotkey_width = self
            .tab_hotkeys
            .get(index)
            .and_then(Option::as_ref)
            .map(|hotkey| hotkey_badge_width(hotkey))
            .unwrap_or_default();

        let title_cell_width = title_width + 2;
        if hotkey_width > 0 {
            title_cell_width.max(hotkey_width + 1)
        } else {
            title_cell_width
        }
    }

    fn boxed_tab_bottom_spans(
        &self,
        index: usize,
        cell_width: usize,
        title_style: Style,
    ) -> Vec<Span<'static>> {
        let border = self.border.unwrap_or_else(|| preset().border());
        let chars = border_chars(border);
        let border_style = self.border_style();
        let Some(hotkey) = self.tab_hotkeys.get(index).and_then(Option::as_ref) else {
            return vec![Span::styled(
                chars.horizontal.repeat(cell_width),
                border_style,
            )];
        };

        let badge_width = hotkey_badge_width(hotkey);
        let left_width = cell_width.saturating_add(1).saturating_sub(badge_width);
        let mut spans = vec![Span::styled(
            chars.horizontal.repeat(left_width),
            border_style,
        )];
        spans.extend(hotkey_badge_spans(
            hotkey,
            self.active_hotkey_prefix(index).as_deref(),
            border,
            border_style,
            title_style,
            hotkey_underline_style(title_style),
        ));
        spans
    }

    fn minimal_title_line(&self, selected: usize, width: u16) -> Line<'static> {
        let mut spans = vec![Span::styled("─ ", self.border_style())];
        for (index, _) in self.titles.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" · ", self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_label_spans(index, selected, style));
        }
        let used = spans
            .iter()
            .map(|span| text_width(span.content.as_ref()))
            .sum::<usize>();
        let fill = (width as usize).saturating_sub(used).max(1);
        spans.push(Span::styled(
            format!(" {}", "─".repeat(fill.saturating_sub(1))),
            self.border_style(),
        ));
        Line::from(spans)
    }

    fn border_style(&self) -> Style {
        Style::default().fg(self.visible_border_color())
    }

    fn tab_style(&self) -> Style {
        Style::default().fg(self.visible_tab_color())
    }

    fn selected_tab_style(&self) -> Style {
        Style::default()
            .fg(self.visible_selected_color())
            .add_modifier(Modifier::BOLD)
    }

    fn selected_underline_style(&self) -> Style {
        Style::default().fg(self.visible_selected_color())
    }

    fn visible_border_color(&self) -> ratatui::style::Color {
        if self.border_color.is_active() {
            return self.border_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        }
    }

    fn visible_tab_color(&self) -> ratatui::style::Color {
        if self.tab_color.is_active() {
            return self.tab_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.muted_fg()
        } else {
            theme.border_fg()
        }
    }

    fn visible_selected_color(&self) -> ratatui::style::Color {
        if self.selected_color.is_active() {
            return self.selected_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        }
    }

    fn render_hotkey(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(ref hotkey) = self.hotkey else {
            return;
        };
        if area.width <= 4 || area.height == 0 {
            return;
        }

        let border_style = self.border_style();
        let width = hotkey_badge_width(hotkey).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }
        let line = Line::from(hotkey_edge_spans(
            hotkey,
            self.active_component_hotkey_prefix().as_deref(),
            border,
            border_style,
            self.selected_tab_style(),
            hotkey_underline_style(self.selected_tab_style()),
        ));
        let x = area.x + area.width.saturating_sub(width);
        let y = area.y + area.height.saturating_sub(1);
        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_deref().and_then(hotkey_sequence_to_event)
    }

    fn hotkey_sequences(&self) -> Vec<String> {
        self.hotkey
            .iter()
            .chain(self.tab_hotkeys.iter().flatten())
            .cloned()
            .collect()
    }

    fn active_hotkey_prefix(&self, index: usize) -> Option<String> {
        let prefix = if self.hotkey_matcher.prefix().is_empty() {
            self.pending_hotkey_prefix.as_deref().unwrap_or("")
        } else {
            self.hotkey_matcher.prefix()
        };
        if prefix.is_empty() {
            return None;
        }
        self.tab_hotkeys
            .get(index)
            .and_then(Option::as_deref)
            .filter(|hotkey| crate::hotkey::normalize_hotkey(hotkey).starts_with(prefix))
            .map(|_| prefix.to_string())
    }

    fn active_component_hotkey_prefix(&self) -> Option<String> {
        let prefix = if self.hotkey_matcher.prefix().is_empty() {
            self.pending_hotkey_prefix.as_deref().unwrap_or("")
        } else {
            self.hotkey_matcher.prefix()
        };
        if prefix.is_empty() {
            return None;
        }
        self.hotkey
            .as_deref()
            .filter(|hotkey| crate::hotkey::normalize_hotkey(hotkey).starts_with(prefix))
            .map(|_| prefix.to_string())
    }

    fn hotkey_index_for_sequence(&self, sequence: &str) -> Option<usize> {
        self.tab_hotkeys.iter().position(|hotkey| {
            hotkey.as_deref().is_some_and(|hotkey| {
                crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
            })
        })
    }

    fn tab_index_for_hotkey_match(&self, match_index: usize) -> Option<usize> {
        tab_hotkey_entries(&self.tab_hotkeys)
            .get(match_index)
            .map(|(index, _)| *index)
    }

    fn tab_label(&self, index: usize) -> String {
        let Some(title) = self.titles.get(index) else {
            return String::new();
        };
        match self.tab_hotkeys.get(index).and_then(Option::as_ref) {
            Some(hotkey) => format!("{title} |{}|", crate::hotkey::normalize_hotkey(hotkey)),
            None => title.clone(),
        }
    }

    fn tab_label_width(&self, index: usize) -> usize {
        self.titles
            .get(index)
            .map(|_| text_width(&self.tab_label(index)))
            .unwrap_or_default()
    }

    fn rendered_tab_label_width(&self, index: usize) -> usize {
        self.tab_label_width(index)
    }

    fn render_modal_close_label(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        if self.on_close.is_none() {
            return;
        }
        let Some(label) = keybindings().tabs().close_label() else {
            return;
        };
        let bordered = self.bordered.unwrap_or_else(|| preset().tabs().bordered());
        let edge_borders = self.resolved_edge_borders(bordered);
        if !edge_borders.contains(Borders::TOP)
            && !edge_borders.contains(Borders::BOTTOM)
            && edge_borders.contains(Borders::LEFT)
        {
            self.render_modal_vertical_close_label(frame, area, area.x, &label, border);
            return;
        }
        if !edge_borders.contains(Borders::TOP)
            && !edge_borders.contains(Borders::BOTTOM)
            && edge_borders.contains(Borders::RIGHT)
        {
            self.render_modal_vertical_close_label(
                frame,
                area,
                area.right().saturating_sub(1),
                &label,
                border,
            );
            return;
        }

        let open_end = edge_borders == Borders::TOP || edge_borders == Borders::BOTTOM;
        let width = line_width(&Line::from(if open_end {
            format!("┤{label}├")
        } else {
            format!("┤{label}│")
        }))
        .min(u16::MAX as usize) as u16;
        let y = self.modal_close_label_y(area);
        if area.width <= width + 2 || y >= area.bottom() {
            return;
        }
        let style = self.selected_tab_style().add_modifier(Modifier::BOLD);
        let chars = border_chars(border);
        let line = Line::from(vec![
            Span::styled(chars.right_join, self.border_style()),
            Span::styled(label, style),
            Span::styled(
                if open_end {
                    chars.left_join
                } else {
                    chars.vertical
                },
                self.border_style(),
            ),
        ]);
        let x = area.x + area.width.saturating_sub(width);
        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn render_modal_vertical_close_label(
        &self,
        frame: &mut Frame,
        area: Rect,
        x: u16,
        label: &str,
        border: BorderKind,
    ) {
        let label_width = line_width(&Line::from(label)).min(u16::MAX as usize) as u16;
        if area.height < 3 || area.width < label_width {
            return;
        }

        let chars = border_chars(border);
        let border_style = self.border_style();
        let label_style = self.selected_tab_style().add_modifier(Modifier::BOLD);
        let y = area.bottom().saturating_sub(3);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.bottom_join, border_style))),
            Rect::new(x, y, 1, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label.to_owned(), label_style))),
            Rect::new(x, y.saturating_add(1), label_width, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.top_join, border_style))),
            Rect::new(x, y.saturating_add(2), 1, 1),
        );
    }

    fn modal_close_label_y(&self, area: Rect) -> u16 {
        let variant = self.variant.unwrap_or_else(|| preset().tabs().variant());
        let bordered = self.bordered.unwrap_or_else(|| preset().tabs().bordered());
        let edge_borders = self.resolved_edge_borders(bordered);
        if self.modal
            && !edge_borders.contains(Borders::TOP)
            && edge_borders.contains(Borders::BOTTOM)
        {
            return area.y + area.height.saturating_sub(1);
        }
        if self.modal && variant == TabsVariant::Underline {
            return area.y.saturating_add(1);
        }
        area.y
    }

    fn modal_render_area(&self, area: Rect) -> Rect {
        area
    }

    pub(crate) fn select_index_from_event(&mut self, selected: usize, ctx: &mut EventCtx<M>) {
        let previous = self.selected_index();
        self.select_index_with_settings(selected, ctx.animation());
        ctx.request_redraw();
        ctx.request_layout();
        if self.selected_index() != previous {
            self.body_focus_transfer_pending = true;
            if let Some((path, id)) = self
                .last_focused_targets
                .get(self.selected_index())
                .and_then(Clone::clone)
            {
                let path = path
                    .keys()
                    .iter()
                    .cloned()
                    .fold(self.focus_path.clone(), |path, key| path.child(key));
                ctx.focus(FocusRequest::TargetAt { path, id });
            } else {
                ctx.focus(FocusRequest::FirstChildOf {
                    path: self.focus_path.clone(),
                    id: FocusId::new(TABS_FOCUS),
                });
            }
        }
    }

    fn close_from_event(
        &mut self,
        reason: ModalCloseReason,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if self.on_close.is_none() {
            return EventOutcome::Ignored;
        };
        self.prepare_modal_close();
        let Some(handler) = &self.on_close else {
            return EventOutcome::Ignored;
        };
        ctx.emit(handler(reason));
        ctx.stop_propagation();
        ctx.request_redraw();
        EventOutcome::Handled
    }
}

impl<M> DockChrome for Tabs<M>
where
    M: 'static,
{
    fn set_dock_edge_borders(&mut self, borders: Borders) {
        self.set_edge_borders(borders);
    }
}

impl<M> Default for Tabs<M>
where
    M: 'static,
{
    fn default() -> Self {
        Self::new(vec![
            Tab::text("Overview", "Simple tabs component for tuicore."),
            Tab::text("Usage", "Use Tab::new(title, node), then Tabs::new(tabs)."),
            Tab::text("State", "The selected tab is a plain index."),
        ])
    }
}

impl<M> TuiNode<M> for Tabs<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let title_width = self
            .titles
            .iter()
            .enumerate()
            .map(|(index, _)| self.rendered_tab_label_width(index).saturating_add(2))
            .sum::<usize>()
            .min(u16::MAX as usize) as u16;
        let body = self
            .selected_key()
            .and_then(|key| self.bodies.measure_child(key, proposal))
            .unwrap_or_else(LayoutSizeHint::unmeasured);
        let variant = self.variant.unwrap_or_else(|| preset().tabs().variant());
        let bordered = self.bordered.unwrap_or_else(|| preset().tabs().bordered());
        let header_height: u16 = if self.titles.is_empty() {
            0
        } else {
            match variant {
                TabsVariant::Minimal => 1,
                TabsVariant::Underline if bordered => 1,
                TabsVariant::Underline => 2,
                TabsVariant::Boxed => 3,
            }
        };
        let border_pad = (bordered as u16).saturating_mul(2);
        LayoutSizeHint::content(
            title_width
                .max(body.preferred.width)
                .saturating_add(border_pad),
            header_height
                .saturating_add(body.preferred.height)
                .saturating_add(border_pad),
        )
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.focus_path = ctx.current_path();
        self.body_area = self.calculate_body_area(area);
        let first_body_focus_target = ctx.focus_targets().len();
        if let Some(key) = self.selected_key().cloned() {
            self.bodies.layout_child(&key, self.body_area, ctx);
        }
        if self.body_focus_transfer_pending
            && !ctx.focus_targets()[first_body_focus_target..]
                .iter()
                .any(|target| target.enabled)
        {
            self.body_focus_transfer_pending = false;
        }
        let hotkey_sequences = self.hotkey_sequences();
        if hotkey_sequences.is_empty() {
            ctx.register_focusable(FocusId::new(TABS_FOCUS), area, true);
        } else {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(TABS_FOCUS),
                area,
                true,
                hotkey_sequences,
            );
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if self.modal {
            frame.render_widget(Clear, area);
        }
        let tabs_area = self.modal_render_area(area);
        self.render_tabs(frame, tabs_area);
        self.render_selected_body(frame, ctx);
        if self.modal {
            let border = self.border.unwrap_or_else(|| preset().border());
            self.render_modal_close_label(frame, tabs_area, border);
            crate::separator::patch_border_joins(
                frame,
                tabs_area,
                self.body_area,
                border,
                self.border_style(),
            );
        }
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
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
                    if let Some(index) = self.hotkey_index_for_sequence(sequence) {
                        self.select_index_from_event(index, ctx);
                        ctx.stop_propagation();
                        return EventOutcome::Handled;
                    }
                    return EventOutcome::Ignored;
                }
            }
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        match self.hotkey_matcher.on_key(*key) {
            HotkeyMatch::Matched(index) => {
                let Some(index) = self.tab_index_for_hotkey_match(index) else {
                    return EventOutcome::Ignored;
                };
                self.select_index_from_event(index, ctx);
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                ctx.request_redraw();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            HotkeyMatch::Ignored => {}
        }
        if self
            .hotkey_event()
            .is_some_and(|hotkey| tab_hotkey_matches(hotkey, *key))
        {
            ctx.stop_propagation();
            return EventOutcome::Handled;
        }
        let bindings = keybindings();
        if bindings.tabs().previous_matches(*key) {
            self.select_index_from_event(self.previous_index(), ctx);
            ctx.stop_propagation();
            EventOutcome::Handled
        } else if bindings.tabs().next_matches(*key) {
            self.select_index_from_event(self.next_index(), ctx);
            ctx.stop_propagation();
            EventOutcome::Handled
        } else if self.modal && bindings.tabs().close_matches(*key) {
            self.close_from_event(ModalCloseReason::CloseKey, ctx)
        } else if self.modal && keybindings().focus().unfocus_matches(*key) {
            self.close_from_event(ModalCloseReason::Escape, ctx)
        } else {
            EventOutcome::Ignored
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }
        let child = self.bodies.dispatch_routed_child(route, event, ctx);
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        Animated::tick(self, dt, settings)
            .merge(self.bodies.tick(dt, settings))
            .merge(hotkey_tick)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            if !focused && self.body_focus_transfer_pending {
                return;
            }
            if focused {
                self.body_focus_transfer_pending = false;
            }
            self.focus(Some(&target.id), focused, ctx);
        } else {
            if focused
                && let Some(index) = self
                    .body_keys
                    .iter()
                    .position(|key| target.path.first() == Some(key))
            {
                self.last_focused_targets[index] = Some((target.path.clone(), target.id.clone()));
                self.body_focus_transfer_pending = false;
            }
            if focused || self.target_is_in_selected_body(target) {
                self.set_focused(focused, ctx.animation());
            }
            self.bodies.dispatch_focus_target(target, focused, ctx);
            ctx.request_redraw();
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.bodies.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.bodies.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.bodies.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.bodies.destroy(ctx);
    }
}

impl<M> Animated for Tabs<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.transition
            .tick(dt, settings)
            .merge(self.border_color.tick(dt, settings))
            .merge(self.tab_color.tick(dt, settings))
            .merge(self.selected_color.tick(dt, settings))
    }
}

fn lerp_usize(from: usize, to: usize, t: f64) -> usize {
    (from as f64 + (to as f64 - from as f64) * t)
        .round()
        .max(0.0) as usize
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn tab_hotkey_matches(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
        (a, b) => a == b,
    }
}

fn tab_hotkey_entries(tab_hotkeys: &[Option<String>]) -> Vec<(usize, String)> {
    tab_hotkeys
        .iter()
        .enumerate()
        .filter_map(|(index, hotkey)| {
            let hotkey = crate::hotkey::normalize_hotkey(hotkey.as_deref()?);
            (!hotkey.is_empty()).then_some((index, hotkey))
        })
        .collect()
}

fn text_width(value: &str) -> usize {
    line_width(&Line::from(value))
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    text_width(&value)
}

struct TextTabBody {
    body: String,
}

impl TextTabBody {
    fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl<M> TuiNode<M> for TextTabBody {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        frame.render_widget(
            Paragraph::new(self.body.as_str()).wrap(Wrap { trim: true }),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{Key, KeyEvent, Propagation, TreePath};

    fn render_node<M>(node: &impl TuiNode<M>, frame: &mut Frame, area: Rect) {
        let mut ctx = crate::RenderCtx::new();
        TuiNode::render(node, frame, area, &mut ctx);
        ctx.flush(frame);
    }

    fn char_positions(value: &str, needle: char) -> Vec<usize> {
        value
            .chars()
            .enumerate()
            .filter_map(|(index, ch)| (ch == needle).then_some(index))
            .collect()
    }

    struct TickProbe {
        ticks: Rc<RefCell<usize>>,
    }

    impl TuiNode<()> for TickProbe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("body"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut crate::RenderCtx<'_>) {}

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            *self.ticks.borrow_mut() += 1;
            TickResult::IDLE
        }
    }

    #[test]
    fn select_index_with_settings_uses_component_animation_spec() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]).animation(
            AnimationSpec {
                enabled: Some(true),
                duration: Some(Duration::from_millis(42)),
                easing: None,
            },
        );

        tabs.select_index_with_settings(1, AnimationSettings::default());

        assert_eq!(tabs.selected_index(), 1);
        assert!(tabs.transition.is_active());
        assert_eq!(tabs.transition.duration(), Duration::from_millis(42));
    }

    #[test]
    fn losing_focus_finishes_active_transition() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new(
                "One",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
            Tab::text("Two", ""),
        ]);

        tabs.select_index_with_settings(1, AnimationSettings::default());
        tabs.set_focused(false, AnimationSettings::default());

        assert_eq!(tabs.selected_index(), 1);
        assert!(!tabs.transition.is_active());
        assert_eq!(tabs.transition.progress(), 1.0);
    }

    #[test]
    fn blurring_previous_body_during_tab_switch_keeps_transition_active() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new(
                "One",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
            Tab::text("Two", ""),
        ]);
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 20, 5), &mut layout);
        let previous_body = layout.focus_targets()[0].clone();

        tabs.select_index_with_settings(1, AnimationSettings::default());
        let mut focus = FocusCtx::default();
        tabs.dispatch_focus(&previous_body, false, &mut focus);

        assert_eq!(tabs.selected_index(), 1);
        assert!(tabs.transition.is_active());
    }

    #[test]
    fn blurring_tabs_shell_during_switch_to_body_keeps_transition_active() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new("One", TickProbe { ticks }),
            Tab::text("Two", ""),
        ])
        .selected(1)
        .focused(true);
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 20, 5), &mut layout);
        let shell = layout
            .focus_targets()
            .iter()
            .find(|target| target.id == FocusId::new(TABS_FOCUS))
            .expect("tabs shell should be focusable")
            .clone();

        tabs.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('['))),
            &mut EventCtx::default(),
        );
        tabs.dispatch_focus(&shell, false, &mut FocusCtx::default());

        assert_eq!(tabs.selected_index(), 0);
        assert!(tabs.transition.is_active());
    }

    #[test]
    fn tabs_without_focusable_body_blur_after_selection_change() {
        let mut tabs =
            Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]).focused(true);
        let area = Rect::new(0, 0, 20, 5);
        let mut initial_layout = LayoutCtx::new();
        tabs.layout(area, &mut initial_layout);

        tabs.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char(']'))),
            &mut EventCtx::default(),
        );
        let mut updated_layout = LayoutCtx::new();
        tabs.layout(area, &mut updated_layout);
        let shell = updated_layout
            .focus_targets()
            .iter()
            .find(|target| target.id == FocusId::new(TABS_FOCUS))
            .expect("tabs shell should be focusable")
            .clone();

        tabs.dispatch_focus(&shell, false, &mut FocusCtx::default());

        assert!(!tabs.focused);
        assert!(!tabs.transition.is_active());
    }

    #[test]
    fn tabs_layout_registers_selected_body_before_shell_focus() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new(
                "One",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
            Tab::text("Two", ""),
        ]);
        let mut ctx = LayoutCtx::new();

        tabs.layout(Rect::new(0, 0, 20, 5), &mut ctx);

        assert_eq!(
            ctx.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::new("tab-0")])
        );
        assert_eq!(ctx.focus_targets()[1].path, TreePath::new());
    }

    #[test]
    fn tabs_key_switches_selection_and_stops_propagation() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char(']'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.layout_requested());
        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::FirstChildOf {
                path: TreePath::default(),
                id: FocusId::new(TABS_FOCUS),
            })
        );
    }

    #[test]
    fn returning_to_tab_restores_its_last_focused_child() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new(
                "One",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
            Tab::new("Two", TickProbe { ticks }),
        ]);
        let area = Rect::new(0, 0, 20, 5);
        let tabs_key = ChildKey::new("tabs-root");

        let mut first_layout = LayoutCtx::new();
        first_layout.push_slot(tabs_key.clone(), area, |ctx| {
            tabs.layout(area, ctx);
        });
        let first_body = first_layout.focus_targets()[0].clone();
        tabs.dispatch_focus(
            &first_body.for_child(&tabs_key).unwrap(),
            true,
            &mut FocusCtx::default(),
        );

        tabs.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char(']'))),
            &mut EventCtx::default(),
        );
        let mut second_layout = LayoutCtx::new();
        second_layout.push_slot(tabs_key.clone(), area, |ctx| {
            tabs.layout(area, ctx);
        });
        let second_body = second_layout.focus_targets()[0].clone();
        tabs.dispatch_focus(
            &second_body.for_child(&tabs_key).unwrap(),
            true,
            &mut FocusCtx::default(),
        );

        let mut return_ctx = EventCtx::default();
        tabs.event(
            &TuiEvent::Key(KeyEvent::from(Key::Char('['))),
            &mut return_ctx,
        );

        assert_eq!(
            return_ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: first_body.path,
                id: first_body.id,
            })
        );
    }

    #[test]
    fn tabs_key_does_not_request_child_focus_when_selection_stays_put() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .selected(1)
            .allow_looping(false);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char(']'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.focus_request(), None);
    }

    #[test]
    fn modal_tabs_reset_selection_on_close_by_default() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .modal()
            .selected(1);

        tabs.prepare_modal_close();

        assert_eq!(tabs.selected_index(), 0);
    }

    #[test]
    fn modal_tabs_do_not_reset_selection_on_open_by_default() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .modal()
            .selected(1);

        tabs.prepare_modal_open(AnimationSettings::default());

        assert_eq!(tabs.selected_index(), 1);
    }

    #[test]
    fn modal_tabs_can_remember_selection_on_close() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .modal()
            .selection_memory(TabsSelectionMemory::Remember)
            .selected(1);

        tabs.prepare_modal_close();

        assert_eq!(tabs.selected_index(), 1);
    }

    #[test]
    fn modal_tabs_can_reset_selection_on_open() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .modal()
            .selection_memory(TabsSelectionMemory::ResetOnOpen)
            .selected(1);

        tabs.prepare_modal_open(AnimationSettings::default());

        assert_eq!(tabs.selected_index(), 0);
    }

    #[test]
    fn modal_tabs_close_event_resets_selection_before_reopen() {
        let mut tabs = Tabs::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .modal()
            .selected(1)
            .on_close(|reason| reason);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('x'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 0);
        assert_eq!(ctx.messages(), &[ModalCloseReason::CloseKey]);
    }

    #[test]
    fn dialog_tabs_use_modal_boxed_all_borders_and_close_handler() {
        let mut tabs = Tabs::dialog(vec![Tab::text("One", "")]).on_close(|reason| reason);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);

        assert_eq!(tabs.variant, Some(TabsVariant::Boxed));
        assert_eq!(tabs.edge_borders, Some(Borders::ALL));
        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[ModalCloseReason::Escape]);
    }

    #[test]
    fn dialog_tabs_allow_style_overrides() {
        let tabs = Tabs::<()>::dialog(vec![Tab::text("One", "")])
            .variant(TabsVariant::Minimal)
            .edge_borders(Borders::BOTTOM);

        assert_eq!(tabs.variant, Some(TabsVariant::Minimal));
        assert_eq!(tabs.edge_borders, Some(Borders::BOTTOM));
    }

    #[test]
    fn tabs_key_wraps_selection_by_default() {
        let mut tabs =
            Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]).selected(1);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char(']'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 0);
        assert_eq!(ctx.propagation(), Propagation::Stopped);

        let mut ctx = EventCtx::default();
        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('['))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn tabs_key_can_disable_wrapping() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .selected(1)
            .allow_looping(false);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char(']'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);

        let mut tabs =
            Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]).allow_looping(false);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('['))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 0);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn tabs_key_selection_uses_event_context_animation_settings() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")]);
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut ctx = EventCtx::new(settings);

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char(']'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert!(!tabs.transition.is_active());
        assert_eq!(tabs.transition.progress(), 1.0);
    }

    #[test]
    fn tabs_registers_hotkey_with_focus_target() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", "")]).hotkey("m");
        let mut ctx = LayoutCtx::new();

        tabs.layout(Rect::new(0, 0, 20, 5), &mut ctx);

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('m')))
        );
        assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["m"]);
    }

    #[test]
    fn tabs_registers_tab_hotkey_sequences_with_focus_target() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("One", "").hotkey("o"),
            Tab::text("Two", "").hotkey("t"),
        ]);
        let mut ctx = LayoutCtx::new();

        tabs.layout(Rect::new(0, 0, 20, 5), &mut ctx);

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('o')))
        );
        assert_eq!(
            ctx.focus_targets()[0].hotkeys,
            vec![
                KeyEvent::from(Key::Char('o')),
                KeyEvent::from(Key::Char('t'))
            ]
        );
        assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["o", "t"]);
    }

    #[test]
    fn tabs_hotkey_event_is_consumed_when_focused() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", "")]).hotkey("m");
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('m'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn tab_hotkey_switches_to_matching_tab() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "").hotkey("o"),
            Tab::text("Usage", "").hotkey("u"),
        ]);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('u'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn tab_hotkey_match_uses_tab_index_when_earlier_tab_has_no_hotkey() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", ""),
            Tab::text("Usage", "").hotkey("u"),
        ]);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('u'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn normalized_tab_hotkey_commit_switches_tab() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "").hotkey("O"),
            Tab::text("Go", "").hotkey("g g"),
        ]);
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(
            &TuiEvent::Hotkey(HotkeyEvent::Commit("gg".to_string())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn tab_hotkey_renders_inline_in_title_variants() {
        let tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "").hotkey("o"),
            Tab::text("Usage", "").hotkey("u"),
        ]);

        let title = tabs
            .underline_title_line(0)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert!(title.contains("Overview |o|"));
        assert!(title.contains("Usage |u|"));
    }

    #[test]
    fn multiletter_tab_hotkey_waits_for_completion() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Open", "").hotkey("op"),
            Tab::text("Overview", "").hotkey("ov"),
        ]);
        let mut ctx = EventCtx::default();

        let pending = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('o'))), &mut ctx);

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 0);
        assert_eq!(tabs.hotkey_matcher.prefix(), "o");

        let mut ctx = EventCtx::default();
        let matched = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('v'))), &mut ctx);

        assert_eq!(matched, EventOutcome::Handled);
        assert_eq!(tabs.selected_index(), 1);
        assert_eq!(tabs.hotkey_matcher.prefix(), "");
    }

    #[test]
    fn multiletter_tab_hotkey_can_timeout_or_cancel() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("Overview", "").hotkey("ov")]);
        let mut ctx = EventCtx::default();

        tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('o'))), &mut ctx);
        assert_eq!(tabs.hotkey_matcher.prefix(), "o");

        <Tabs<()> as TuiNode<()>>::tick(
            &mut tabs,
            Duration::from_secs(2),
            AnimationSettings::default(),
        );
        assert_eq!(tabs.hotkey_matcher.prefix(), "");

        let mut ctx = EventCtx::default();
        tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('o'))), &mut ctx);
        let mut ctx = EventCtx::default();
        tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Esc)), &mut ctx);
        assert_eq!(tabs.hotkey_matcher.prefix(), "");
    }

    #[test]
    fn tab_hotkey_jump_animates_intermediate_tab_title() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("AA", "").hotkey("a"),
            Tab::text("BB", "").hotkey("b"),
            Tab::text("CC", "").hotkey("c"),
        ])
        .animation(AnimationSpec {
            enabled: Some(true),
            duration: Some(Duration::from_millis(100)),
            easing: Some(crate::Easing::Linear),
        });
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('c'))), &mut ctx);
        Animated::tick(
            &mut tabs,
            Duration::from_millis(50),
            AnimationSettings::default(),
        );

        let spans = tabs.tab_title_spans(1, "BB |b|", tabs.selected_index(), tabs.tab_style());
        assert_eq!(outcome, EventOutcome::Handled);
        assert!(
            spans
                .iter()
                .any(|span| span.style == tabs.selected_tab_style())
        );
    }

    #[test]
    fn tab_hotkey_jump_left_animates_intermediate_tab_title() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("AA", "").hotkey("a"),
            Tab::text("BB", "").hotkey("b"),
            Tab::text("CC", "").hotkey("c"),
        ])
        .selected(2)
        .animation(AnimationSpec {
            enabled: Some(true),
            duration: Some(Duration::from_millis(100)),
            easing: Some(crate::Easing::Linear),
        });
        let mut ctx = EventCtx::default();

        let outcome = tabs.event(&TuiEvent::Key(KeyEvent::from(Key::Char('a'))), &mut ctx);
        Animated::tick(
            &mut tabs,
            Duration::from_millis(50),
            AnimationSettings::default(),
        );

        let spans = tabs.tab_title_spans(1, "BB |b|", tabs.selected_index(), tabs.tab_style());
        assert_eq!(outcome, EventOutcome::Handled);
        assert!(
            spans
                .iter()
                .any(|span| span.style == tabs.selected_tab_style())
        );
    }

    #[test]
    fn tab_hotkey_renders_on_boxed_tab_bottom_border() {
        let tabs = Tabs::<()>::new(vec![Tab::text("Overview", "Body").hotkey("o")]);
        let mut terminal = Terminal::new(TestBackend::new(24, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..6)
            .map(|y| {
                (0..24)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("Overview"));
        assert!(rendered.contains("┤o├"));
        assert!(!rendered.contains("Overview |o|"));
    }

    #[test]
    fn boxed_tab_hotkeys_align_with_tab_boundaries() {
        let tabs = Tabs::<()>::new(vec![
            Tab::text("Intro", "Body").hotkey("i"),
            Tab::text("Overview", "Body").hotkey("w"),
            Tab::text("Usage", "Body").hotkey("e"),
            Tab::text("State", "Body").hotkey("tat"),
            Tab::text("Logs", "Body").hotkey("l"),
        ]);
        let mut terminal = Terminal::new(TestBackend::new(80, 4)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..3)
            .map(|y| {
                (0..80)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "╭───────┬──────────┬───────┬───────┬──────┬────────────────────────────────────╮",
            "│ Intro │ Overview │ Usage │ State │ Logs │                                    │",
            "├─────┤i├────────┤w├─────┤e├───┤tat├────┤l├────────────────────────────────────┤",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn boxed_last_tab_title_keeps_right_border_at_minimum_width() {
        let tabs = Tabs::<()>::new(vec![Tab::text("State", "Body").hotkey("tat")]);
        let mut terminal = Terminal::new(TestBackend::new(16, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let title = (0..16)
            .map(|x| buffer.cell((x, 1)).unwrap().symbol())
            .collect::<String>();
        let bottom = (0..16)
            .map(|x| buffer.cell((x, 2)).unwrap().symbol())
            .collect::<String>();

        assert_eq!(title, "│ State │      │");
        assert!(bottom.contains("┤tat├─"), "{bottom}");
        assert!(!bottom.contains("┤tat├┴"), "{bottom}");
    }

    #[test]
    fn boxed_tabs_can_hide_edge_borders() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", "Body")])
            .variant(TabsVariant::Boxed)
            .modal()
            .edge_borders(Borders::NONE);
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 18, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(18, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..18)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(!row(1).starts_with('│'), "{}", row(1));
        assert!(!row(1).ends_with('│'), "{}", row(1));
        assert!(row(2).starts_with("Body"), "{}", row(2));
        assert!(!row(5).contains('─'), "{}", row(5));
    }

    #[test]
    fn boxed_tabs_can_keep_side_borders_without_bottom_border() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", "Body")])
            .variant(TabsVariant::Boxed)
            .modal()
            .edge_borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 18, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(18, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..18)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(row(3).starts_with('│'), "{}", row(3));
        assert!(row(3).ends_with('│'), "{}", row(3));
        assert!(row(3).contains("Body"), "{}", row(3));
        assert!(!row(5).contains('─'), "{}", row(5));
    }

    #[test]
    fn boxed_tabs_can_hide_top_and_side_borders() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("One", "Body")])
            .variant(TabsVariant::Boxed)
            .modal()
            .edge_borders(Borders::BOTTOM);
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 18, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(18, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..18)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(!row(0).contains('─'), "{}", row(0));
        assert!(!row(1).starts_with('│'), "{}", row(1));
        assert!(!row(1).ends_with('│'), "{}", row(1));
        assert!(row(5).contains('─'), "{}", row(5));
    }

    #[test]
    fn boxed_tabs_without_side_borders_align_header_separators() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "Body").hotkey("o"),
            Tab::text("Behavior", "Body").hotkey("b"),
            Tab::text("Close", "Body").hotkey("c"),
        ])
        .variant(TabsVariant::Boxed)
        .modal()
        .edge_borders(Borders::TOP)
        .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 64, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(64, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..64)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };
        let top = row(0);
        let middle = row(1);
        let bottom = row(2);

        let top_joins = char_positions(&top, '┬');
        let middle_joins = char_positions(&middle, '│');

        assert!(top.starts_with("──────────┬"), "{top}");
        assert!(middle.starts_with(" Overview │"), "{middle}");
        assert!(bottom.starts_with("────────┤o├"), "{bottom}");
        assert_eq!(&top_joins[..3], &middle_joins[..3], "{top}\n{middle}");
    }

    #[test]
    fn bordered_tabs_render_bottom_right_hotkey() {
        let tabs = Tabs::<()>::new(vec![Tab::text("One", "Body")]).hotkey("m");
        let mut terminal = Terminal::new(TestBackend::new(24, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..24)
            .map(|x| buffer.cell((x, 5)).unwrap().symbol())
            .collect::<String>();
        assert!(bottom.contains("┤m│"));
    }

    #[test]
    fn whole_tabs_bottom_right_hotkey_aligns_with_border_snapshot() {
        let tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "Body"),
            Tab::text("Usage", "Body"),
        ])
        .hotkey("nav");
        let mut terminal = Terminal::new(TestBackend::new(40, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..6)
            .map(|y| {
                (0..40)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "╭──────────┬───────┬───────────────────╮",
            "│ Overview │ Usage │                   │",
            "├──────────────────────────────────────┤",
            "│                                      │",
            "│                                      │",
            "╰──────────────────────────────────┤nav│",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn underline_modal_tabs_put_close_label_on_body_border_without_extra_padding() {
        let mut tabs = Tabs::<()>::new(vec![
            Tab::text("Overview", "Body").hotkey("o"),
            Tab::text("Usage", "Body").hotkey("u"),
        ])
        .variant(TabsVariant::Underline)
        .modal()
        .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 32, 8), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(32, 8)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..32)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(!row(0).contains('x'), "{}", row(0));
        assert!(row(1).ends_with("┤x│"), "{}", row(1));
    }

    #[test]
    fn docked_modal_tabs_put_close_label_on_bottom_border_with_open_join() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("Overview", "Body")])
            .variant(TabsVariant::Underline)
            .edge_borders(Borders::BOTTOM)
            .modal()
            .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 32, 8), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(32, 8)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..32)
            .map(|x| buffer.cell((x, 7)).unwrap().symbol())
            .collect::<String>();

        assert!(bottom.ends_with("┤x├"), "{bottom}");
    }

    #[test]
    fn partial_width_tabs_snackbar_uses_closed_close_label_end() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("Overview", "Body")])
            .variant(TabsVariant::Boxed)
            .edge_borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .modal()
            .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 32, 8), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(32, 8)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let top = (0..32)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();

        assert!(top.ends_with("┤x│"), "{top}");
    }

    #[test]
    fn docked_modal_tabs_put_vertical_close_label_on_left_edge_bottom() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("Overview", "Body")])
            .variant(TabsVariant::Underline)
            .edge_borders(Borders::LEFT)
            .modal()
            .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 8, 8), &mut layout);
        assert_eq!(tabs.body_area.x, 2);
        let mut terminal = Terminal::new(TestBackend::new(8, 8)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let column = (0..8)
            .map(|y| buffer.cell((0, y)).unwrap().symbol())
            .collect::<String>();

        assert!(column.ends_with("┴x┬"), "{column}");
    }

    #[test]
    fn docked_modal_tabs_put_vertical_close_label_on_right_edge_bottom() {
        let mut tabs = Tabs::<()>::new(vec![Tab::text("Overview", "Body")])
            .variant(TabsVariant::Underline)
            .edge_borders(Borders::RIGHT)
            .modal()
            .on_close(|_| ());
        let mut layout = LayoutCtx::new();
        tabs.layout(Rect::new(0, 0, 8, 8), &mut layout);
        assert_eq!(tabs.body_area.right(), 6);
        let mut terminal = Terminal::new(TestBackend::new(8, 8)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&tabs, frame, frame.area()))
            .expect("tabs should render");

        let buffer = terminal.backend().buffer();
        let column = (0..8)
            .map(|y| buffer.cell((7, y)).unwrap().symbol())
            .collect::<String>();

        assert!(column.ends_with("┴x┬"), "{column}");
    }

    #[test]
    fn tabs_tick_propagates_to_all_bodies_once() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tabs = Tabs::<()>::new(vec![
            Tab::new(
                "One",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
            Tab::new(
                "Two",
                TickProbe {
                    ticks: Rc::clone(&ticks),
                },
            ),
        ]);

        TuiNode::tick(
            &mut tabs,
            Duration::from_millis(16),
            AnimationSettings::default(),
        );

        assert_eq!(*ticks.borrow(), 2);
    }
}
