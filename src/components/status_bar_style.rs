use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::{LayoutProposal, TuiNode, line_width, theme};

pub(super) const STATUS_ACTION_TAIL_WIDTH: u16 = 1;

pub(super) fn measured_width<M, N>(node: &N) -> u16
where
    N: TuiNode<M>,
{
    node.measure(LayoutProposal::unbounded()).preferred.width
}

pub(super) fn status_segment_width(label: &str) -> u16 {
    line_width(&Line::from(format!(" {label} "))).min(u16::MAX as usize) as u16
}

pub(super) fn status_action_tail() -> Line<'static> {
    Line::from(Span::styled("", Style::default().fg(theme().surface_bg())))
}

pub(super) fn status_segment_line(
    label_spans: Vec<Span<'static>>,
    focused: bool,
    segment_bg: Color,
    separator_bg: Option<Color>,
) -> Line<'static> {
    let background = if focused {
        theme().highlight_bg()
    } else {
        segment_bg
    };
    let mut separator_style = Style::default().fg(background);
    if let Some(separator_bg) = separator_bg {
        separator_style = separator_style.bg(separator_bg);
    }
    let mut spans = vec![
        Span::styled("", separator_style),
        Span::styled(" ", status_segment_text_style(focused, segment_bg)),
    ];
    spans.extend(label_spans);
    spans.push(Span::styled(
        " ",
        status_segment_text_style(focused, segment_bg),
    ));
    Line::from(spans)
}

pub(super) fn status_segment_text_style(focused: bool, segment_bg: Color) -> Style {
    let theme = theme();
    if focused {
        Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.background_bg()).bg(segment_bg)
    }
}

pub(super) fn centered_field_area(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(1) / 2,
        width,
        1,
    )
}
