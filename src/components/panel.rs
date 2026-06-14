use std::time::Duration;

use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::Component;
use tuirealm::event::KeyEvent;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Alignment, Rect};
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::{Block, Borders, Paragraph};
use tuirealm::state::State;

use crate::{
    Animated, AnimationSettings, BorderKind, ScrollAxes, ScrollBehavior, ScrollDelta,
    ScrollGeometry, ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize, ScrollState, TickResult,
    border_set, line_width, paragraph_scroll, preset, theme,
};

#[derive(Debug, Clone)]
pub struct Panel {
    top_left: Option<String>,
    top_right: Option<String>,
    border: Option<BorderKind>,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    focused: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel {
    pub fn new() -> Self {
        Self {
            top_left: None,
            top_right: None,
            border: None,
            content: Vec::new(),
            scroll: None,
            focused: false,
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.top_left = Some(title.into());
        self
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.top_right = Some(title.into());
        self
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines.into_iter().map(Into::into).collect();
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn scrollable(mut self, axes: ScrollAxes) -> Self {
        self.scroll = Some(match self.scroll.take() {
            Some(scroll) => scroll.with_axes(axes),
            None => ScrollState::from_preset(axes, preset().scroll()),
        });
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        if let Some(scroll) = self.scroll.take() {
            self.scroll = Some(scroll.behavior(behavior));
        } else {
            self.scroll = Some(
                ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll())
                    .behavior(behavior),
            );
        }
        self
    }

    pub fn content_size(&self) -> ScrollSize {
        let width = self
            .content
            .iter()
            .map(|line| line_width(&Line::from(line.as_str())))
            .max()
            .unwrap_or(0);
        ScrollSize::new(width, self.content.len())
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let inner = Self::inner_area(area);
        let content = self.content_size();
        if let Some(scroll) = &self.scroll {
            scroll.geometry(inner, content)
        } else {
            let layout = ScrollLayout {
                outer: inner,
                viewport: inner,
                vertical_bar: None,
                horizontal_bar: None,
                corner: None,
            };
            ScrollGeometry {
                layout,
                viewport: ScrollSize::from_area(inner),
                content,
            }
        }
    }

    pub fn on_key(
        &mut self,
        key: KeyEvent,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.on_key(key, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_by(
        &mut self,
        delta: ScrollDelta,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_by(delta, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_to(
        &mut self,
        offset: ScrollOffset,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_to(offset, geometry.viewport, geometry.content, settings)
    }

    pub fn clamp_scroll(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.clamp_to(geometry.viewport, geometry.content, settings)
    }

    pub fn inner_area(area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let border = self.border.unwrap_or_else(|| preset().border());
        let theme = theme();
        let border_style = Style::default().fg(if self.focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_title(frame, area, self.top_left.as_deref(), Alignment::Left);
        self.render_title(frame, area, self.top_right.as_deref(), Alignment::Right);

        if !inner.is_empty() {
            let lines = self
                .content
                .iter()
                .map(|line| Line::from(line.clone()))
                .collect::<Vec<_>>();
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.content_size());
                let offset = scroll.offset();
                let lines = if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                    visible_lines(&self.content, offset, geometry.viewport)
                } else {
                    lines
                };
                let paragraph = Paragraph::new(lines).alignment(Alignment::Left).scroll(
                    if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                        (0, 0)
                    } else {
                        paragraph_scroll(offset)
                    },
                );
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
            }
        }
    }

    fn render_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: Option<&str>,
        alignment: Alignment,
    ) {
        let Some(title) = title else {
            return;
        };
        if area.width <= 4 {
            return;
        }

        let max_width = area.width.saturating_sub(4) as usize;
        let title = bounded_title(title, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let x = match alignment {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(2),
        };
        let theme = theme();
        let style = Style::default()
            .fg(if self.focused {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            })
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(title, style))),
            Rect::new(x, area.y, width, 1),
        );
    }
}

impl Animated for Panel {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.scroll
            .as_mut()
            .map(|scroll| scroll.tick(dt, settings))
            .unwrap_or(TickResult::IDLE)
    }
}

impl Component for Panel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.render(frame, area);
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        match _attr {
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

fn bounded_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut value = format!(" {title} ");
    if line_width(&Line::from(value.as_str())) > max_width {
        value = truncate_cells(&value, max_width);
    }
    value
}

fn visible_lines(
    lines: &[String],
    offset: ScrollOffset,
    viewport: ScrollSize,
) -> Vec<Line<'static>> {
    lines
        .iter()
        .skip(offset.y)
        .take(viewport.height)
        .map(|line| Line::from(trim_cells(line, offset.x, viewport.width)))
        .collect()
}

fn trim_cells(line: &str, skip: usize, width: usize) -> String {
    let end = skip.saturating_add(width);
    let mut cursor = 0;
    let mut trimmed = String::new();

    for ch in line.chars() {
        let ch_width = char_width(ch);
        let next = cursor + ch_width;
        if ch_width == 0 {
            if cursor >= skip && cursor <= end {
                trimmed.push(ch);
            }
        } else if cursor >= skip && next <= end {
            trimmed.push(ch);
        }
        cursor = next;
        if cursor >= end && ch_width > 0 {
            break;
        }
    }

    trimmed
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();

    for ch in value.chars() {
        let ch_width = char_width(ch);
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }

    truncated
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    line_width(&Line::from(value))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use crate::{ScrollbarConfig, ScrollbarGutter, ScrollbarVisibility};

    use super::*;

    #[test]
    fn empty_scrollable_panel_still_renders_scrollbars() {
        let mut panel = Panel::new();
        panel.scroll = Some(
            ScrollState::new(ScrollAxes::Both).scrollbars(ScrollbarConfig {
                vertical: ScrollbarVisibility::Always,
                horizontal: ScrollbarVisibility::Always,
                gutter: ScrollbarGutter::Reserve,
            }),
        );
        let mut terminal = Terminal::new(TestBackend::new(6, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        assert_ne!(buffer.cell((4, 2)).unwrap().fg, Color::Reset);
    }

    #[test]
    fn clamp_scroll_clamps_offset_after_content_shrinks() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let area = Rect::new(0, 0, 10, 5);
        let mut panel = Panel::new()
            .content((0..20).map(|line| format!("line {line}")))
            .scrollable(ScrollAxes::Vertical);

        panel.scroll_to(ScrollOffset::new(0, 99), area, settings);
        panel.content = vec![String::from("line")];
        let outcome = panel.clamp_scroll(area, settings);

        assert!(outcome.changed);
        assert_eq!(
            panel.scroll.as_ref().unwrap().offset(),
            ScrollOffset::new(0, 0)
        );
    }
}
