use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph as RatatuiParagraph, Wrap};

use crate::{LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, TuiNode, line_width, theme};

pub struct Header {
    text: String,
    icon: Option<String>,
}

impl Header {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            icon: None,
        }
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn clear_icon(mut self) -> Self {
        self.icon = None;
        self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(RatatuiParagraph::new(self.line()), area);
    }

    fn line(&self) -> Line<'static> {
        let theme = theme();
        let header_style = Style::default()
            .fg(theme.text_fg())
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        let mut spans = Vec::new();
        if let Some(icon) = &self.icon {
            spans.push(Span::styled(icon.clone(), header_style));
            spans.push(Span::styled(" ", header_style));
        }
        spans.push(Span::styled(self.text.clone(), header_style));
        Line::from(spans)
    }
}

impl<M> TuiNode<M> for Header {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(line_width(&self.line()).min(u16::MAX as usize) as u16, 1)
            .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }
}

pub struct Paragraph {
    text: String,
    wrap: bool,
    overflow: ParagraphOverflow,
    max_lines: Option<usize>,
    style: Option<Style>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParagraphOverflow {
    #[default]
    Clip,
    Ellipsis,
}

impl Paragraph {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            wrap: true,
            overflow: ParagraphOverflow::Clip,
            max_lines: None,
            style: None,
        }
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn overflow(mut self, overflow: ParagraphOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    pub fn max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let area = self.render_area(area);
        if area.is_empty() {
            return;
        }

        let style = self
            .style
            .unwrap_or_else(|| Style::default().fg(theme().text_fg()));
        let max_lines = area.height as usize;
        if self.overflow == ParagraphOverflow::Ellipsis {
            let lines = ellipsized_text_lines(&self.text, area.width, max_lines, self.wrap)
                .into_iter()
                .map(|line| Line::from(Span::styled(line, style)))
                .collect::<Vec<_>>();
            frame.render_widget(RatatuiParagraph::new(lines), area);
            return;
        }

        let mut paragraph = RatatuiParagraph::new(self.text.clone()).style(style);
        if self.wrap {
            paragraph = paragraph.wrap(Wrap { trim: false });
        }
        frame.render_widget(paragraph, area);
    }

    fn render_area(&self, area: Rect) -> Rect {
        let Some(max_lines) = self.max_lines else {
            return area;
        };

        Rect {
            height: area.height.min(max_lines.min(u16::MAX as usize) as u16),
            ..area
        }
    }
}

pub(crate) fn wrapped_text_line_count(text: &str, width: u16, max_lines: usize) -> usize {
    ellipsized_text_lines(text, width, max_lines, true)
        .len()
        .max(1)
}

pub(crate) fn ellipsized_text_lines(
    text: &str,
    width: u16,
    max_lines: usize,
    wrap: bool,
) -> Vec<String> {
    if width == 0 || max_lines == 0 {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut clipped = false;

    if wrap {
        for raw_line in text.lines() {
            push_wrapped_line(raw_line, width, max_lines, &mut lines, &mut clipped);
        }
    } else {
        for raw_line in text.lines() {
            push_limited_line(raw_line.to_string(), max_lines, &mut lines, &mut clipped);
        }
    }

    if text.ends_with('\n') {
        push_limited_line(String::new(), max_lines, &mut lines, &mut clipped);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    if clipped || lines.iter().any(|line| text_width(line) > width as usize) {
        if let Some(last) = lines.last_mut() {
            *last = ellipsize_line(last, width);
        }
    }
    for line in &mut lines {
        if text_width(line) > width as usize {
            *line = ellipsize_line(line, width);
        }
    }
    lines
}

fn push_wrapped_line(
    raw_line: &str,
    width: u16,
    max_lines: usize,
    lines: &mut Vec<String>,
    clipped: &mut bool,
) {
    if raw_line.is_empty() {
        push_limited_line(String::new(), max_lines, lines, clipped);
        return;
    }

    let mut current = String::new();
    for word in raw_line.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if text_width(&candidate) <= width as usize {
            current = candidate;
            continue;
        }

        if !current.is_empty() {
            push_limited_line(current, max_lines, lines, clipped);
            current = String::new();
        }

        if text_width(word) <= width as usize {
            current = word.to_string();
        } else {
            push_long_word(word, width, max_lines, lines, clipped, &mut current);
        }
    }
    push_limited_line(current, max_lines, lines, clipped);
}

fn push_long_word(
    word: &str,
    width: u16,
    max_lines: usize,
    lines: &mut Vec<String>,
    clipped: &mut bool,
    current: &mut String,
) {
    for ch in word.chars() {
        let candidate = format!("{current}{ch}");
        if !current.is_empty() && text_width(&candidate) > width as usize {
            push_limited_line(std::mem::take(current), max_lines, lines, clipped);
        }
        current.push(ch);
    }
}

fn push_limited_line(line: String, max_lines: usize, lines: &mut Vec<String>, clipped: &mut bool) {
    if lines.len() < max_lines {
        lines.push(line);
    } else {
        *clipped = true;
    }
}

fn ellipsize_line(line: &str, width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let mut value = line.to_string();
    while text_width(&format!("{value}...")) > width {
        if value.pop().is_none() {
            break;
        }
    }
    format!("{}...", value.trim_end())
}

fn text_width(text: &str) -> usize {
    line_width(&Line::from(text))
}

impl<M> TuiNode<M> for Paragraph {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = self
            .text
            .lines()
            .map(|line| line_width(&Line::from(line)))
            .max()
            .unwrap_or(1)
            .min(u16::MAX as usize) as u16;
        let mut height = self.text.split('\n').count();
        if let Some(max_lines) = self.max_lines {
            height = height.min(max_lines);
        }
        let height = height.min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(width.max(1), height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn header_line_is_bold_and_underlined() {
        let header = Header::new("Deployments");
        let line = header.line();

        assert_eq!(line.spans[0].content.as_ref(), "Deployments");
        assert!(line.spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(
            line.spans[0]
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED)
        );
    }

    #[test]
    fn header_can_render_icon_before_text() {
        let header = Header::new("Settings").icon("");
        let line = header.line();

        assert_eq!(line.spans[0].content.as_ref(), "");
        assert_eq!(line.spans[1].content.as_ref(), " ");
        assert_eq!(line.spans[2].content.as_ref(), "Settings");
    }

    #[test]
    fn ellipsized_lines_adds_dots_when_text_would_clip() {
        let lines = ellipsized_text_lines("alpha beta gamma delta", 13, 1, true);

        assert_eq!(lines, ["alpha beta..."]);
    }

    #[test]
    fn ellipsized_lines_preserves_short_text() {
        let lines = ellipsized_text_lines("short", 12, 1, true);

        assert_eq!(lines, ["short"]);
    }

    #[test]
    fn ellipsized_lines_wraps_on_words() {
        let lines = ellipsized_text_lines("alpha beta gamma", 10, 3, true);

        assert_eq!(lines, ["alpha beta", "gamma"]);
    }

    #[test]
    fn clipped_paragraph_respects_max_lines_when_rendering() {
        let paragraph = Paragraph::new("alpha\nbeta\ngamma").max_lines(2);
        let mut terminal = Terminal::new(TestBackend::new(8, 3)).expect("terminal should build");

        terminal
            .draw(|frame| paragraph.render(frame, frame.area()))
            .expect("paragraph should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "a");
        assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "b");
        assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), " ");
    }

    #[test]
    fn paragraph_measure_caps_height_to_max_lines() {
        let paragraph = Paragraph::new("alpha\nbeta\ngamma").max_lines(2);

        let hint = <Paragraph as TuiNode<()>>::measure(&paragraph, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 2);
    }

    #[test]
    fn paragraph_measure_counts_trailing_blank_line() {
        let paragraph = Paragraph::new("alpha\n");

        let hint = <Paragraph as TuiNode<()>>::measure(&paragraph, LayoutProposal::unbounded());

        assert_eq!(hint.preferred.height, 2);
    }
}
