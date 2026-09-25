use super::*;

impl DiffViewer {
    /// Return one source side's one-based line and zero-based character column under a cell.
    /// Exactly one field of the returned location is set; gutters, headers and padding return None.
    pub fn text_position_at(&self, column: u16, row: u16) -> Option<(DiffLocation, usize)> {
        let viewport = self.scroll_geometry(self.area).layout.viewport;
        if !viewport.contains((column, row).into()) {
            return None;
        }
        let mut y = self.scroll.offset().y + usize::from(row - viewport.y);
        let x = self.scroll.offset().x + usize::from(column - viewport.x);
        let width = usize::from(viewport.width);
        for source in &self.parts {
            let rows = if !self.wrap {
                1
            } else if self.style == DiffStyle::SideBySide {
                wrap::side_by_side(
                    std::slice::from_ref(source),
                    width,
                    self.side_divider_column(),
                )
                .len()
            } else {
                wrap::wrap_line(source, width).len()
            };
            if y >= rows {
                y -= rows;
                continue;
            }
            let location = source.location?;
            if self.style == DiffStyle::SideBySide {
                if self.wrap && width < 3 {
                    return None;
                }
                let left_width = if self.wrap {
                    (width - 3) / 2
                } else {
                    self.side_divider_column()
                };
                let (left, rest) = wrap::split_at_width(&source.parts, self.side_divider_column());
                let (_, right) = wrap::split_at_width(&rest, 3);
                let (left, _) = wrap::split_at_width(
                    &left,
                    source
                        .side_left_content_width
                        .unwrap_or(self.side_divider_column()),
                );
                let (parts, cells, x, location) = if x < left_width {
                    (
                        left,
                        left_width,
                        x,
                        DiffLocation {
                            old_line: Some(location.old_line?),
                            new_line: None,
                        },
                    )
                } else if x >= left_width + 3 {
                    (
                        right,
                        width.saturating_sub(left_width + 3),
                        x - left_width - 3,
                        DiffLocation {
                            old_line: None,
                            new_line: Some(location.new_line?),
                        },
                    )
                } else {
                    return None;
                };
                return source_column(&parts, source.continuation_indent, cells, y, x, self.wrap)
                    .map(|column| (location, column));
            }
            let removed = source
                .parts
                .first()
                .is_some_and(|part| part.role == DiffRole::Removed);
            let location = if removed || location.new_line.is_none() {
                DiffLocation {
                    old_line: Some(location.old_line?),
                    new_line: None,
                }
            } else {
                DiffLocation {
                    old_line: None,
                    new_line: Some(location.new_line?),
                }
            };
            return source_column(
                &source.parts,
                source.continuation_indent,
                width,
                y,
                x,
                self.wrap,
            )
            .map(|column| (location, column));
        }
        None
    }
}

fn source_column(
    parts: &[StyledPart],
    indent: usize,
    width: usize,
    row: usize,
    x: usize,
    wrapped: bool,
) -> Option<usize> {
    if wrapped && width <= indent {
        return None;
    }
    let x = x.checked_sub(indent)?;
    let (_, body) = wrap::split_at_width(parts, indent);
    let chunks = if wrapped {
        wrap::chunks(&body, width.saturating_sub(indent).max(1))
    } else {
        vec![body]
    };
    let previous = chunks
        .iter()
        .take(row)
        .flatten()
        .map(|part| part.text.chars().count())
        .sum::<usize>();
    let text = chunks
        .get(row)?
        .iter()
        .map(|part| part.text.as_str())
        .collect::<String>();
    let mut cells = 0;
    let mut characters = previous;
    for grapheme in text.graphemes(true) {
        let next = cells + display_width(grapheme);
        if (cells..next).contains(&x) {
            return Some(characters);
        }
        cells = next;
        characters += grapheme.chars().count();
    }
    None
}
