use std::time::Duration;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::{Block, Borders, Paragraph};
use tuirealm::state::State;

use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, TabsVariant, TickResult, Tween,
    border_chars, border_set, line_width,
    ui::{animation_settings, keybindings, preset, theme},
};

pub struct Tab<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    pub title: String,
    body: Box<dyn AppComponent<Msg, UserEvent>>,
}

impl<Msg, UserEvent> Tab<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    pub fn new(
        title: impl Into<String>,
        body: impl AppComponent<Msg, UserEvent> + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            body: Box::new(body),
        }
    }

    pub fn text(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, TextPanel::new(body))
    }
}

pub struct Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    tabs: Vec<Tab<Msg, UserEvent>>,
    selected: usize,
    previous_selected: usize,
    allow_looping: bool,
    variant: Option<TabsVariant>,
    border: Option<BorderKind>,
    bordered: Option<bool>,
    animation: Option<AnimationSpec>,
    focused: bool,
    transition: Tween,
}

impl<Msg, UserEvent> Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    pub fn new(tabs: Vec<Tab<Msg, UserEvent>>) -> Self {
        Self {
            tabs,
            selected: 0,
            previous_selected: 0,
            allow_looping: false,
            variant: None,
            border: None,
            bordered: None,
            animation: None,
            focused: false,
            transition: Tween::idle(1.0),
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
        self
    }

    pub fn selected_index(&self) -> usize {
        self.clamp_selected(self.selected)
    }

    pub fn select_index(&mut self, selected: usize) {
        self.select_index_with_settings(selected, animation_settings());
    }

    pub fn select_index_with_settings(&mut self, selected: usize, settings: AnimationSettings) {
        let current = self.clamp_selected(self.selected);
        let selected = self.clamp_selected(selected);
        if selected == current {
            self.selected = current;
            self.previous_selected = current;
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
        let selected = self.clamp_selected(self.selected);
        let last = self.tabs.len().saturating_sub(1);
        let next = if selected >= last && self.allow_looping {
            0
        } else {
            (selected + 1).min(last)
        };
        self.select_index(next);
    }

    pub fn previous(&mut self) {
        let selected = self.clamp_selected(self.selected);
        let previous = if selected == 0 && self.allow_looping {
            self.tabs.len().saturating_sub(1)
        } else {
            selected.saturating_sub(1)
        };
        self.select_index(previous);
    }

    pub fn selected_body_on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        let selected = self.clamp_selected(self.selected);
        self.tabs.get_mut(selected)?.body.on(event)
    }

    fn clamp_selected(&self, selected: usize) -> usize {
        selected.min(self.tabs.len().saturating_sub(1))
    }
}

impl<Msg, UserEvent> Default for Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn default() -> Self {
        Self::new(vec![
            Tab::text("Overview", "Simple tabs component for tuicore."),
            Tab::text(
                "Usage",
                "Use Tab::new(title, component), then Tabs::new(tabs).",
            ),
            Tab::text("State", "The selected tab is a plain index for now."),
        ])
    }
}

impl<Msg, UserEvent> Component for Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let selected = self.clamp_selected(self.selected);
        let preset = preset();
        let variant = self.variant.unwrap_or_else(|| preset.tabs().variant());
        let bordered = self.bordered.unwrap_or_else(|| preset.tabs().bordered());
        let border = self.border.unwrap_or_else(|| preset.border());
        if self.tabs.is_empty() {
            self.render_empty(frame, area, bordered, border);
            return;
        }

        if variant == TabsVariant::Minimal {
            if !bordered {
                let [tabs_area, body_area] = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(1), Constraint::Fill(1)])
                    .areas(area);
                frame.render_widget(
                    Paragraph::new(self.minimal_title_line(selected, tabs_area.width)),
                    tabs_area,
                );
                self.render_body(frame, body_area, selected);
                return;
            }

            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border_set(border))
                .border_style(self.border_style())
                .title(self.minimal_title_line(selected, area.width.saturating_sub(2)));
            let body_area = block.inner(area);
            frame.render_widget(block, area);
            self.render_body(frame, body_area, selected);
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
                let [top, middle, bottom] =
                    self.boxed_title_lines(selected, tabs_area.width, border);
                frame.render_widget(Paragraph::new(top), tabs_area);
                frame.render_widget(
                    Paragraph::new(middle),
                    Rect::new(tabs_area.x, tabs_area.y + 1, tabs_area.width, 1),
                );
                frame.render_widget(
                    Paragraph::new(if bordered {
                        self.boxed_panel_top_line(tabs_area.width, border)
                    } else {
                        bottom
                    }),
                    Rect::new(tabs_area.x, tabs_area.y + 2, tabs_area.width, 1),
                );
            }
            TabsVariant::Underline => {
                frame.render_widget(
                    Paragraph::new(self.underline_title_line(selected)),
                    tabs_area,
                );
                if !bordered {
                    frame.render_widget(
                        Paragraph::new(self.underline_line(selected, tabs_area.width)),
                        Rect::new(tabs_area.x, tabs_area.y + 1, tabs_area.width, 1),
                    );
                }
            }
            TabsVariant::Minimal => unreachable!(),
        }
        let body_area = if variant == TabsVariant::Underline && bordered {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border_set(border))
                .border_style(self.border_style());
            let inner = block.inner(body_area);
            frame.render_widget(block, body_area);
            frame.render_widget(
                Paragraph::new(self.underline_panel_top_line(selected, body_area.width, border)),
                Rect::new(body_area.x, body_area.y, body_area.width, 1),
            );
            inner
        } else if variant == TabsVariant::Boxed && bordered {
            let block = Block::default()
                .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                .border_set(border_set(border))
                .border_style(self.border_style());
            let inner = block.inner(body_area);
            frame.render_widget(block, body_area);
            inner
        } else {
            body_area
        };
        self.render_body(frame, body_area, selected);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == Attribute::Focus
            && let AttrValue::Flag(focused) = value
        {
            self.focused = focused;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

impl<Msg, UserEvent> Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn render_empty(&mut self, frame: &mut Frame, area: Rect, bordered: bool, border: BorderKind) {
        let body_area = if bordered {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border_set(border))
                .border_style(self.border_style());
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            area
        };
        self.render_body(frame, body_area, 0);
    }

    fn render_body(&mut self, frame: &mut Frame, body_area: Rect, selected: usize) {
        if let Some(tab) = self.tabs.get_mut(selected) {
            tab.body.view(frame, body_area);
        } else {
            frame.render_widget(Paragraph::new("No tabs to show."), body_area);
        }
    }

    fn title_line(&self, selected: usize, separator: &'static str) -> Line<'static> {
        let mut spans = Vec::new();
        for (index, tab) in self.tabs.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(separator, self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_title_spans(index, &tab.title, selected, style));
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

    fn boxed_title_lines(
        &self,
        selected: usize,
        width: u16,
        border: BorderKind,
    ) -> [Line<'static>; 3] {
        let chars = border_chars(border);
        let border_style = self.border_style();
        let tab_count = self.tabs.len();
        let mut widths = self
            .tabs
            .iter()
            .map(|tab| text_width(&tab.title) + 2)
            .collect::<Vec<_>>();
        let used = 2 + widths.iter().sum::<usize>() + tab_count.saturating_sub(1);

        if let Some(last) = widths.last_mut() {
            *last += (width as usize).saturating_sub(used);
        }

        let mut top = vec![Span::styled(chars.top_left, border_style)];
        let mut middle = vec![Span::styled(chars.vertical, border_style)];
        let mut bottom = vec![Span::styled(chars.bottom_left, border_style)];

        for (index, tab) in self.tabs.iter().enumerate() {
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
            let right_pad = cell_width.saturating_sub(text_width(&tab.title) + 1);
            middle.push(Span::raw(" "));
            middle.extend(self.tab_title_spans(index, &tab.title, selected, title_style));
            middle.push(Span::raw(" ".repeat(right_pad)));

            if index + 1 == tab_count {
                top.push(Span::styled(chars.top_right, border_style));
                middle.push(Span::styled(chars.vertical, border_style));
                bottom.push(Span::styled(chars.bottom_right, border_style));
            } else {
                top.push(Span::styled(chars.top_join, border_style));
                middle.push(Span::styled(chars.vertical, border_style));
                bottom.push(Span::styled(chars.bottom_join, border_style));
            }
        }

        [Line::from(top), Line::from(middle), Line::from(bottom)]
    }

    fn boxed_panel_top_line(&self, width: u16, border: BorderKind) -> Line<'static> {
        let chars = border_chars(border);
        let border_style = self.border_style();
        let tab_count = self.tabs.len();
        let mut widths = self
            .tabs
            .iter()
            .map(|tab| text_width(&tab.title) + 2)
            .collect::<Vec<_>>();
        let used = 2 + widths.iter().sum::<usize>() + tab_count.saturating_sub(1);

        if let Some(last) = widths.last_mut() {
            *last += (width as usize).saturating_sub(used);
        }

        let mut spans = vec![Span::styled(chars.left_join, border_style)];
        for (index, cell_width) in widths.iter().copied().enumerate() {
            spans.push(Span::styled(
                chars.horizontal.repeat(cell_width),
                border_style,
            ));
            if index + 1 == tab_count {
                spans.push(Span::styled(chars.right_join, border_style));
            } else {
                spans.push(Span::styled(chars.bottom_join, border_style));
            }
        }

        Line::from(spans)
    }

    fn minimal_title_line(&self, selected: usize, width: u16) -> Line<'static> {
        let mut spans = vec![Span::styled("─ ", self.border_style())];
        for (index, tab) in self.tabs.iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" · ", self.border_style()));
            }
            let style = if index == selected {
                self.selected_tab_style()
            } else {
                self.tab_style()
            };
            spans.extend(self.tab_title_spans(index, &tab.title, selected, style));
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

    fn underline_line(&self, selected: usize, width: u16) -> Line<'static> {
        self.underline_segments_line(selected, width, true, "─")
    }

    fn underline_segments_line(
        &self,
        selected: usize,
        width: u16,
        leading: bool,
        separator: &'static str,
    ) -> Line<'static> {
        let theme = theme();
        let base_style = Style::default().fg(theme.border_fg());
        let width = width as usize;
        let separator_width = text_width(separator);
        let (selected_start, selected_width) =
            self.underline_range(selected, leading, separator_width);
        let (previous_start, previous_width) =
            self.underline_range(self.previous_selected, leading, separator_width);
        let progress = self.transition.value().clamp(0.0, 1.0);
        let start = lerp(previous_start as f64, selected_start as f64, progress).round() as usize;
        let underline_width =
            lerp(previous_width as f64, selected_width as f64, progress).round() as usize;
        let end = (start + underline_width).min(width);

        let mut spans = Vec::new();
        let mut cursor = 0;
        if start > cursor {
            spans.push(Span::styled("─".repeat(start - cursor), base_style));
            cursor = start;
        }
        if end > cursor {
            spans.push(Span::styled(
                "─".repeat(end - cursor),
                self.selected_underline_style(),
            ));
            cursor = end;
        }
        if width > cursor {
            spans.push(Span::styled("─".repeat(width - cursor), base_style));
        }

        Line::from(spans)
    }

    fn underline_range(
        &self,
        index: usize,
        leading: bool,
        separator_width: usize,
    ) -> (usize, usize) {
        let mut start = usize::from(leading);
        for tab in self.tabs.iter().take(index) {
            start += text_width(&tab.title) + separator_width;
        }

        let width = self
            .tabs
            .get(index)
            .map(|tab| text_width(&tab.title))
            .unwrap_or_default();
        (start, width)
    }

    fn underline_panel_top_line(
        &self,
        selected: usize,
        width: u16,
        border: BorderKind,
    ) -> Line<'static> {
        if width < 2 {
            return self.underline_line(selected, width);
        }

        let chars = border_chars(border);
        let mut line = self.underline_segments_line(selected, width.saturating_sub(3), false, "──");
        self.highlight_last_underline_cell(&mut line);
        line.spans
            .insert(0, Span::styled(chars.top_left, self.border_style()));
        line.spans
            .insert(1, Span::styled(chars.horizontal, self.border_style()));
        line.spans
            .push(Span::styled(chars.top_right, self.border_style()));
        line
    }

    fn highlight_last_underline_cell(&self, line: &mut Line<'static>) {
        let Some(last) = line.spans.pop() else {
            return;
        };
        let text = last.content.to_string();
        let Some((split_at, _)) = text.char_indices().last() else {
            line.spans.push(last);
            return;
        };
        let (prefix, suffix) = text.split_at(split_at);
        if !prefix.is_empty() {
            line.spans.push(Span::styled(prefix.to_owned(), last.style));
        }
        line.spans.push(Span::styled(
            suffix.to_owned(),
            self.selected_underline_style(),
        ));
    }

    fn border_style(&self) -> Style {
        let theme = theme();
        Style::default().fg(if self.focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        })
    }

    fn tab_style(&self) -> Style {
        let theme = theme();
        Style::default().fg(if self.focused {
            theme.muted_fg()
        } else {
            theme.border_fg()
        })
    }

    fn selected_tab_style(&self) -> Style {
        let theme = theme();
        Style::default()
            .fg(if self.focused {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            })
            .add_modifier(Modifier::BOLD)
    }

    fn selected_underline_style(&self) -> Style {
        let theme = theme();
        Style::default().fg(if self.focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        })
    }
}

impl<Msg, UserEvent> AppComponent<Msg, UserEvent> for Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Tick => {
                self.tick(animation_settings().frame_duration(), animation_settings());
                self.selected_body_on(event)
            }
            Event::Keyboard(key) if keybindings().tabs().previous_matches(*key) => {
                self.previous();
                None
            }
            Event::Keyboard(key) if keybindings().tabs().next_matches(*key) => {
                self.next();
                None
            }
            _ => self.selected_body_on(event),
        }
    }
}

impl<Msg, UserEvent> Animated for Tabs<Msg, UserEvent>
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn tick(&mut self, dt: Duration, settings: crate::AnimationSettings) -> TickResult {
        self.transition.tick(dt, settings)
    }
}

fn lerp(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress
}

fn text_width(value: &str) -> usize {
    line_width(&Line::from(value))
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    text_width(&value)
}

struct TextPanel {
    body: String,
}

impl TextPanel {
    fn new(body: impl Into<String>) -> Self {
        Self { body: body.into() }
    }
}

impl Component for TextPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let panel = Paragraph::new(self.body.as_str())
            .wrap(tuirealm::ratatui::widgets::Wrap { trim: true });

        frame.render_widget(panel, area);
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

impl<Msg, UserEvent> AppComponent<Msg, UserEvent> for TextPanel
where
    Msg: PartialEq + 'static,
    UserEvent: Eq + PartialEq + Clone + 'static,
{
    fn on(&mut self, _event: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_index_with_settings_uses_component_animation_spec() {
        let mut tabs = Tabs::<(), ()>::new(vec![Tab::text("One", ""), Tab::text("Two", "")])
            .animation(AnimationSpec {
                enabled: Some(true),
                duration: Some(Duration::from_millis(42)),
                easing: None,
            });

        tabs.select_index_with_settings(1, AnimationSettings::default());

        assert_eq!(tabs.selected_index(), 1);
        assert!(tabs.transition.is_active());
        assert_eq!(tabs.transition.duration(), Duration::from_millis(42));
    }
}
