use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;

use crate::line_width;

#[derive(Clone)]
struct StyledGrapheme {
    content: String,
    style: Style,
    width: usize,
}

pub(super) fn wrap_event_spans(
    spans: &[Span<'static>],
    width: usize,
    max_lines: usize,
    line_style: Style,
) -> Vec<Vec<Span<'static>>> {
    if width == 0 || max_lines == 0 {
        return Vec::new();
    }

    let graphemes = styled_graphemes(spans);
    let mut lines: Vec<Vec<StyledGrapheme>> = vec![Vec::new()];
    let mut line_widths = vec![0usize];
    let mut overflowed = false;

    for grapheme in graphemes {
        let line_index = lines.len() - 1;
        if grapheme.width > width {
            overflowed = true;
            break;
        }
        if line_widths[line_index] + grapheme.width > width {
            if lines.len() == max_lines {
                overflowed = true;
                break;
            }
            if let Some(space_index) = single_wrap_space(&lines[line_index]) {
                let mut continuation = lines[line_index].split_off(space_index + 1);
                line_widths[line_index] = lines[line_index]
                    .iter()
                    .map(|grapheme| grapheme.width)
                    .sum();
                let continuation_width: usize =
                    continuation.iter().map(|grapheme| grapheme.width).sum();
                let grapheme_width = grapheme.width;
                continuation.push(grapheme);
                lines.push(continuation);
                line_widths.push(continuation_width + grapheme_width);
                continue;
            } else {
                lines.push(Vec::new());
                line_widths.push(0);
            }
        }
        let line_index = lines.len() - 1;
        line_widths[line_index] += grapheme.width;
        lines[line_index].push(grapheme);
    }

    if overflowed {
        let line = lines.last_mut().expect("event wrapper always has a line");
        let line_width = line_widths.last_mut().expect("event wrapper tracks widths");
        let ellipsis_style = line.last().map_or(line_style, |grapheme| grapheme.style);
        while *line_width + 3 > width {
            let Some(removed) = line.pop() else {
                break;
            };
            *line_width = line_width.saturating_sub(removed.width);
        }
        if *line_width + 3 <= width {
            line.push(StyledGrapheme {
                content: "...".to_string(),
                style: ellipsis_style,
                width: 3,
            });
        }
    }

    lines
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|grapheme| Span::styled(grapheme.content, grapheme.style))
                .collect()
        })
        .collect()
}

fn single_wrap_space(line: &[StyledGrapheme]) -> Option<usize> {
    if line.last().is_some_and(|grapheme| grapheme.content == " ") {
        return None;
    }
    line.iter().enumerate().rev().find_map(|(index, grapheme)| {
        (grapheme.content == " "
            && index > 0
            && index + 1 < line.len()
            && line[index - 1].content != " "
            && line[index + 1].content != " ")
            .then_some(index)
    })
}

fn styled_graphemes(spans: &[Span<'static>]) -> Vec<StyledGrapheme> {
    let content = spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let mut span_index = 0;
    let mut span_end = spans.first().map_or(0, |span| span.content.len());

    content
        .grapheme_indices(true)
        .map(|(start, grapheme)| {
            while start >= span_end && span_index + 1 < spans.len() {
                span_index += 1;
                span_end += spans[span_index].content.len();
            }
            StyledGrapheme {
                content: grapheme.to_string(),
                style: spans
                    .get(span_index)
                    .map_or(Style::default(), |span| span.style),
                width: line_width(&Line::from(grapheme)),
            }
        })
        .collect()
}
