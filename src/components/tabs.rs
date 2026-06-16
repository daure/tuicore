use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ChildKey, Children, ColorTween,
    EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusTarget, LayoutCtx, LayoutResult,
    LifecycleCtx, TabsVariant, TickResult, TuiEvent, TuiNode, Tween, border_set, keybindings,
    line_width, preset, theme,
};

const TABS_FOCUS: &str = "tabs";

pub struct Tab<M = ()> {
    title: String,
    body: Box<dyn TuiNode<M>>,
}

pub struct Tabs<M = ()> {
    titles: Vec<String>,
    bodies: Children<M>,
    body_keys: Vec<ChildKey>,
    selected: usize,
    previous_selected: usize,
    allow_looping: bool,
    variant: Option<TabsVariant>,
    border: Option<BorderKind>,
    bordered: Option<bool>,
    animation: Option<AnimationSpec>,
    focused: bool,
    transition: Tween,
    border_color: ColorTween,
    tab_color: ColorTween,
    selected_color: ColorTween,
    body_area: Rect,
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
            body: Box::new(body),
        }
    }

    pub fn text(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, TextTabBody::new(body))
    }
}

impl<M> Tabs<M>
where
    M: 'static,
{
    pub fn new(tabs: Vec<Tab<M>>) -> Self {
        let theme = theme();
        let mut titles = Vec::with_capacity(tabs.len());
        let mut body_keys = Vec::with_capacity(tabs.len());
        let mut bodies = Children::new();

        for (index, tab) in tabs.into_iter().enumerate() {
            let key = ChildKey::new(format!("tab-{index}"));
            titles.push(tab.title);
            body_keys.push(key.clone());
            bodies = bodies.child(key, tab.body);
        }

        Self {
            titles,
            bodies,
            body_keys,
            selected: 0,
            previous_selected: 0,
            allow_looping: false,
            variant: None,
            border: None,
            bordered: None,
            animation: None,
            focused: false,
            transition: Tween::idle(1.0),
            border_color: ColorTween::idle(theme.border_fg()),
            tab_color: ColorTween::idle(theme.border_fg()),
            selected_color: ColorTween::idle(theme.muted_fg()),
            body_area: Rect::default(),
        }
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

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = Some(bordered);
        self
    }

    pub fn animation(mut self, animation: AnimationSpec) -> Self {
        self.animation = Some(animation);
        self
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

        let header_height = match variant {
            TabsVariant::Minimal => 1,
            TabsVariant::Underline if bordered => 1,
            TabsVariant::Underline => 2,
            TabsVariant::Boxed => 3,
        };
        let [_, body] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Fill(1)])
            .areas(area);

        if bordered {
            let borders = if variant == TabsVariant::Boxed {
                Borders::LEFT | Borders::RIGHT | Borders::BOTTOM
            } else {
                Borders::ALL
            };
            Block::default().borders(borders).inner(body)
        } else {
            body
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

        let header_height = match variant {
            TabsVariant::Underline if bordered => 1,
            TabsVariant::Underline => 2,
            TabsVariant::Boxed => 3,
            TabsVariant::Minimal => unreachable!(),
        };
        let [tabs_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Fill(1)])
            .areas(area);

        match variant {
            TabsVariant::Boxed => {
                self.render_boxed_header(frame, tabs_area, selected, border, bordered)
            }
            TabsVariant::Underline => {
                self.render_underline_header(frame, tabs_area, selected, bordered)
            }
            TabsVariant::Minimal => unreachable!(),
        }

        if bordered {
            let block = Block::default()
                .borders(if variant == TabsVariant::Boxed {
                    Borders::LEFT | Borders::RIGHT | Borders::BOTTOM
                } else {
                    Borders::ALL
                })
                .border_set(border_set(border))
                .border_style(self.border_style());
            frame.render_widget(block, body_area);
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
    ) {
        let [top, middle, bottom] = self.boxed_title_lines(selected, area.width, border);
        frame.render_widget(Paragraph::new(top), area);
        frame.render_widget(
            Paragraph::new(middle),
            Rect::new(area.x, area.y + 1, area.width, 1),
        );
        frame.render_widget(
            Paragraph::new(if bordered { bottom } else { bottom }),
            Rect::new(area.x, area.y + 2, area.width, 1),
        );
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

    fn render_selected_body(&self, frame: &mut Frame) {
        if let Some(key) = self.selected_key()
            && let Some(body) = self.bodies.get(key)
        {
            body.render(frame, self.body_area);
        }
    }

    fn title_line(&self, selected: usize, separator: &'static str) -> Line<'static> {
        let mut spans = Vec::new();
        for (index, title) in self.titles.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(separator, self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_title_spans(index, title, selected, style));
        }
        Line::from(spans)
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

        let progress = self.transition.value().clamp(0.0, 1.0);
        let width = text_width(title).max(1);
        let cut = (progress * width as f64).round() as usize;
        let moving_right = self.selected >= self.previous_selected;
        let mut spans = Vec::new();
        let mut cursor = 0;

        for ch in title.chars() {
            let ch_width = char_width(ch);
            let next = cursor + ch_width;
            let style = if index == self.previous_selected {
                let stays_highlighted = if moving_right {
                    cursor >= cut
                } else {
                    next <= width.saturating_sub(cut)
                };
                if stays_highlighted {
                    self.selected_tab_style()
                } else {
                    self.tab_style()
                }
            } else if index == selected {
                let becomes_highlighted = if moving_right {
                    next <= cut
                } else {
                    cursor >= width.saturating_sub(cut)
                };
                if becomes_highlighted {
                    self.selected_tab_style()
                } else {
                    self.tab_style()
                }
            } else {
                base_style
            };
            spans.push(Span::styled(ch.to_string(), style));
            cursor = next;
        }

        spans
    }

    fn underline_title_line(&self, selected: usize) -> Line<'static> {
        let mut line = self.title_line(selected, "  ");
        line.spans.insert(0, Span::raw("  "));
        line
    }

    fn underline_line(&self, selected: usize, width: u16) -> Line<'static> {
        let theme = theme();
        let selected_start = self.underline_start(selected);
        let selected_width = self
            .titles
            .get(selected)
            .map(|title| text_width(title))
            .unwrap_or_default();
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
        let previous_width = self
            .titles
            .get(self.previous_selected)
            .map(|title| text_width(title))
            .unwrap_or_default();
        let selected_width = self
            .titles
            .get(selected)
            .map(|title| text_width(title))
            .unwrap_or_default();
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
                self.titles
                    .get(selected)
                    .map(|title| text_width(title))
                    .unwrap_or_default(),
                inner_width,
            )
        } else {
            let start = lerp_usize(
                self.underline_start(self.previous_selected),
                self.underline_start(selected),
                self.transition.value(),
            )
            .saturating_sub(1);
            let previous_width = self
                .titles
                .get(self.previous_selected)
                .map(|title| text_width(title))
                .unwrap_or_default();
            let selected_width = self
                .titles
                .get(selected)
                .map(|title| text_width(title))
                .unwrap_or_default();
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
            .map(|title| text_width(title) + 2)
            .sum::<usize>()
    }

    fn boxed_title_lines(
        &self,
        selected: usize,
        width: u16,
        border: BorderKind,
    ) -> [Line<'static>; 3] {
        let chars = crate::border_chars(border);
        let border_style = self.border_style();
        let tab_count = self.titles.len();
        let mut widths = self
            .titles
            .iter()
            .map(|title| text_width(title) + 2)
            .collect::<Vec<_>>();
        let used = 2 + widths.iter().sum::<usize>() + tab_count.saturating_sub(1);
        if let Some(last) = widths.last_mut() {
            *last += (width as usize).saturating_sub(used);
        }

        let mut top = vec![Span::styled(chars.top_left, border_style)];
        let mut middle = vec![Span::styled(chars.vertical, border_style)];
        let mut bottom = vec![Span::styled(chars.left_join, border_style)];

        for (index, title) in self.titles.iter().enumerate() {
            let cell_width = widths[index];
            top.push(Span::styled(
                chars.horizontal.repeat(cell_width),
                border_style,
            ));
            bottom.push(Span::styled(
                chars.horizontal.repeat(cell_width),
                border_style,
            ));
            let title_style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            let right_pad = cell_width.saturating_sub(text_width(title) + 1);
            middle.push(Span::raw(" "));
            middle.extend(self.tab_title_spans(index, title, selected, title_style));
            middle.push(Span::raw(" ".repeat(right_pad)));
            if index + 1 == tab_count {
                top.push(Span::styled(chars.top_right, border_style));
                middle.push(Span::styled(chars.vertical, border_style));
                bottom.push(Span::styled(chars.right_join, border_style));
            } else {
                top.push(Span::styled(chars.top_join, border_style));
                middle.push(Span::styled(chars.vertical, border_style));
                bottom.push(Span::styled(chars.bottom_join, border_style));
            }
        }

        [Line::from(top), Line::from(middle), Line::from(bottom)]
    }

    fn minimal_title_line(&self, selected: usize, width: u16) -> Line<'static> {
        let mut spans = vec![Span::styled("─ ", self.border_style())];
        for (index, title) in self.titles.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" · ", self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_title_spans(index, title, selected, style));
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
        Style::default().fg(self.border_color.value())
    }

    fn tab_style(&self) -> Style {
        Style::default().fg(self.tab_color.value())
    }

    fn selected_tab_style(&self) -> Style {
        Style::default()
            .fg(self.selected_color.value())
            .add_modifier(Modifier::BOLD)
    }

    fn selected_underline_style(&self) -> Style {
        Style::default().fg(self.selected_color.value())
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
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.body_area = self.calculate_body_area(area);
        ctx.register_focusable(FocusId::new(TABS_FOCUS), area, true);
        if let Some(key) = self.selected_key().cloned() {
            self.bodies.layout_child(&key, self.body_area, ctx);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_tabs(frame, area);
        self.render_selected_body(frame);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        let bindings = keybindings();
        if bindings.tabs().previous_matches(*key) {
            self.select_index_with_settings(self.previous_index(), ctx.animation());
            ctx.request_redraw();
            ctx.request_layout();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else if bindings.tabs().next_matches(*key) {
            self.select_index_with_settings(self.next_index(), ctx.animation());
            ctx.request_redraw();
            ctx.request_layout();
            ctx.stop_propagation();
            EventOutcome::Handled
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
        Animated::tick(self, dt, settings).merge(self.bodies.tick(dt, settings))
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        } else {
            self.set_focused(focused, ctx.animation());
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

    fn render(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(self.body.as_str()).wrap(Wrap { trim: true }),
            area,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{Key, KeyEvent, Propagation, TreePath};

    struct TickProbe {
        ticks: Rc<RefCell<usize>>,
    }

    impl TuiNode<()> for TickProbe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("body"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

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
    fn tabs_layout_uses_children_for_selected_body_path() {
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

        assert_eq!(ctx.focus_targets()[0].path, TreePath::new());
        assert_eq!(
            ctx.focus_targets()[1].path,
            TreePath::from_keys([ChildKey::new("tab-0")])
        );
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
