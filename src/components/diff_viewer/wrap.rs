use unicode_segmentation::UnicodeSegmentation;

use super::{
    DiffRole, DiffStyle, DiffViewer, StyledLine, StyledPart, display_width, measure_parts, part,
    styled_line, styled_line_with_location,
};

impl DiffViewer {
    pub(super) fn refresh_projection(&mut self) {
        if !self.wrap || self.area.width == 0 {
            self.display_parts = self.parts.clone();
        } else {
            let mut width = self.area.width as usize;
            self.display_parts = self.projected_parts(width);
            self.content = measure_parts(&self.display_parts);
            let viewport_width = self.scroll_geometry(self.area).viewport.width;
            if viewport_width != width {
                width = viewport_width;
                self.display_parts = self.projected_parts(width);
            }
        }
        self.content = measure_parts(&self.display_parts);
    }

    fn projected_parts(&self, width: usize) -> Vec<StyledLine> {
        if self.style == DiffStyle::SideBySide {
            side_by_side(&self.parts, width, self.side_divider_column())
        } else {
            lines(&self.parts, width)
        }
    }
}

pub(super) fn lines(lines: &[StyledLine], width: usize) -> Vec<StyledLine> {
    lines
        .iter()
        .flat_map(|source| wrap_line(source, width))
        .collect()
}

pub(super) fn side_by_side(
    source_lines: &[StyledLine],
    width: usize,
    divider_column: usize,
) -> Vec<StyledLine> {
    if width < 3 {
        return lines(source_lines, width);
    }
    let available = width - 3;
    let left_width = available / 2;
    let right_width = available - left_width;
    let mut output = Vec::new();

    for source in source_lines {
        let (left, rest) = split_at_width(&source.parts, divider_column);
        let (divider, right) = split_at_width(&rest, 3);
        if text(&divider) != " │ " {
            output.extend(wrap_line(source, width));
            continue;
        }

        let left_content_width = source
            .side_left_content_width
            .unwrap_or(divider_column)
            .min(divider_column);
        let (left, _) = split_at_width(&left, left_content_width);
        let left = wrap_line(&styled_line(left, source.continuation_indent), left_width);
        let right = wrap_line(&styled_line(right, source.continuation_indent), right_width);
        for index in 0..left.len().max(right.len()) {
            let mut parts = left
                .get(index)
                .map_or_else(Vec::new, |line| line.parts.clone());
            pad(&mut parts, left_width);
            parts.push(part(" │ ", DiffRole::Muted));
            if let Some(right) = right.get(index) {
                parts.extend(right.parts.clone());
            }
            output.push(styled_line_with_location(parts, 0, source.location));
        }
    }
    output
}

fn wrap_line(source: &StyledLine, width: usize) -> Vec<StyledLine> {
    if width == 0 {
        return vec![styled_line_with_location(Vec::new(), 0, source.location)];
    }
    let indent = source.continuation_indent.min(width.saturating_sub(1));
    let (prefix, body) = split_at_width(&source.parts, indent);
    let chunks = chunks(&body, width.saturating_sub(indent).max(1));
    if chunks.is_empty() {
        return vec![styled_line_with_location(prefix, 0, source.location)];
    }

    chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            let mut parts = if index == 0 {
                prefix.clone()
            } else {
                vec![part(" ".repeat(indent), DiffRole::Muted)]
            };
            parts.extend(chunk);
            styled_line_with_location(parts, 0, source.location)
        })
        .collect()
}

fn chunks(parts: &[StyledPart], width: usize) -> Vec<Vec<StyledPart>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;

    for styled in parts {
        for grapheme in styled.text.graphemes(true) {
            let grapheme_width = display_width(grapheme);
            if current_width > 0 && current_width + grapheme_width > width {
                chunks.push(current);
                current = Vec::new();
                current_width = 0;
            }
            push_text(&mut current, grapheme, styled.role);
            current_width += grapheme_width;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn split_at_width(parts: &[StyledPart], width: usize) -> (Vec<StyledPart>, Vec<StyledPart>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut used = 0;
    for styled in parts {
        for grapheme in styled.text.graphemes(true) {
            if used < width {
                push_text(&mut left, grapheme, styled.role);
                used += display_width(grapheme);
            } else {
                push_text(&mut right, grapheme, styled.role);
            }
        }
    }
    (left, right)
}

fn push_text(parts: &mut Vec<StyledPart>, text: &str, role: DiffRole) {
    if let Some(last) = parts.last_mut().filter(|last| last.role == role) {
        last.text.push_str(text);
    } else {
        parts.push(part(text, role));
    }
}

fn pad(parts: &mut Vec<StyledPart>, width: usize) {
    let current = display_width(&text(parts));
    if current < width {
        parts.push(part(" ".repeat(width - current), DiffRole::Normal));
    }
}

fn text(parts: &[StyledPart]) -> String {
    parts.iter().map(|part| part.text.as_str()).collect()
}
