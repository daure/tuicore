use ratatui::{buffer::Buffer, layout::Rect, style::Style, text::Line};

use crate::{CopyRegion, line_width, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CellPoint {
    pub x: u16,
    pub y: u16,
}

impl CellPoint {
    pub(crate) const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CellSelection {
    anchor: CellPoint,
    cursor: CellPoint,
    region: Option<CopyRegion>,
}

#[derive(Debug)]
pub(crate) struct MouseCopy {
    enabled: bool,
    anchor: Option<CellPoint>,
    cursor: Option<CellPoint>,
    dragging: bool,
    region: Option<CopyRegion>,
    buffer: Option<Buffer>,
    regions: Vec<CopyRegion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MouseCopyRelease {
    Click(CellPoint),
    Selection(Option<String>),
}

impl Default for MouseCopy {
    fn default() -> Self {
        Self {
            enabled: true,
            anchor: None,
            cursor: None,
            dragging: false,
            region: None,
            buffer: None,
            regions: Vec::new(),
        }
    }
}

impl MouseCopy {
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.cancel();
        }
    }

    pub(crate) fn press(&mut self, point: CellPoint) {
        if !self.enabled {
            return;
        }
        self.anchor = Some(point);
        self.cursor = Some(point);
        self.dragging = false;
        self.region = self
            .regions
            .iter()
            .rev()
            .find(|region| region.area().contains((point.x, point.y).into()))
            .cloned();
    }

    pub(crate) fn drag(&mut self, point: CellPoint) -> bool {
        if !self.enabled || self.anchor.is_none() {
            return false;
        }
        self.cursor = Some(point);
        self.dragging = true;
        true
    }

    pub(crate) fn release(&mut self, point: CellPoint) -> Option<MouseCopyRelease> {
        let anchor = self.anchor?;
        let release = if self.dragging {
            let selection = CellSelection {
                anchor,
                cursor: point,
                region: self.region.clone(),
            };
            let text = self
                .buffer
                .as_ref()
                .and_then(|buffer| selected_text(buffer, selection));
            MouseCopyRelease::Selection(text)
        } else {
            MouseCopyRelease::Click(anchor)
        };
        self.cancel();
        Some(release)
    }

    pub(crate) fn cancel(&mut self) -> bool {
        let changed = self.anchor.is_some();
        self.anchor = None;
        self.cursor = None;
        self.dragging = false;
        self.region = None;
        changed
    }

    pub(crate) fn selection(&self) -> Option<CellSelection> {
        match (self.dragging, self.anchor, self.cursor) {
            (true, Some(anchor), Some(cursor)) => Some(CellSelection {
                anchor,
                cursor,
                region: self.region.clone(),
            }),
            _ => None,
        }
    }

    pub(crate) fn set_buffer(&mut self, buffer: Buffer) {
        self.buffer = Some(buffer);
    }

    pub(crate) fn set_regions(&mut self, regions: &[CopyRegion]) {
        self.regions.clear();
        self.regions.extend_from_slice(regions);
    }
}

pub(crate) fn apply_selection(buffer: &mut Buffer, selection: CellSelection) {
    let Some((start, end, area)) = selection.bounds(buffer) else {
        return;
    };
    let palette = theme();
    let style = Style::default()
        .fg(palette.selected_fg())
        .bg(palette.selected_bg());
    for_each_selected_cell(buffer, area, start, end, |buffer, x, y| {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_style(style);
        }
    });
}

fn selected_text(buffer: &Buffer, selection: CellSelection) -> Option<String> {
    let (start, end, area) = selection.bounds(buffer)?;
    let mut text = String::new();
    let mut previous_wraps = false;
    for y in start.y..=end.y {
        let left = if y == start.y { start.x } else { area.x };
        let right = if y == end.y {
            end.x
        } else {
            area.right().saturating_sub(1)
        };
        let mut line = String::new();
        let mut x = u32::from(left);
        while x <= u32::from(right) {
            if let Some(cell) = buffer.cell((x as u16, y)) {
                line.push_str(cell.symbol());
                x += line_width(&Line::from(cell.symbol())).max(1) as u32;
            } else {
                x += 1;
            }
        }
        let line = line.trim_end();
        if y != start.y {
            if previous_wraps {
                let line = line.trim_start();
                if !text.is_empty() && !line.is_empty() {
                    text.push(' ');
                }
                text.push_str(line);
            } else {
                text.push('\n');
                text.push_str(line);
            }
        } else {
            text.push_str(line);
        }
        previous_wraps = y < end.y
            && (selection
                .region
                .as_ref()
                .is_some_and(|region| region.joins_after(y))
                || (right == area.right().saturating_sub(1)
                    && row_reaches_right_edge(buffer, area, y)));
    }
    (!text.is_empty()).then_some(text)
}

fn row_reaches_right_edge(buffer: &Buffer, area: Rect, y: u16) -> bool {
    let x = area.right().saturating_sub(1);
    let Some(cell) = buffer.cell((x, y)) else {
        return false;
    };
    if !cell.symbol().trim().is_empty() {
        return true;
    }
    let Some(previous_x) = x.checked_sub(1).filter(|x| *x >= area.x) else {
        return false;
    };
    buffer.cell((previous_x, y)).is_some_and(|cell| {
        line_width(&Line::from(cell.symbol())) > 1 && !cell.symbol().trim().is_empty()
    })
}

impl CellSelection {
    fn bounds(&self, buffer: &Buffer) -> Option<(CellPoint, CellPoint, Rect)> {
        let area = self
            .region
            .as_ref()
            .map(CopyRegion::area)
            .unwrap_or(buffer.area)
            .intersection(buffer.area);
        if area.is_empty() {
            return None;
        }
        let anchor = clamp_to_area(self.anchor, area);
        let cursor = clamp_to_area(self.cursor, area);
        if (anchor.y, anchor.x) <= (cursor.y, cursor.x) {
            Some((anchor, cursor, area))
        } else {
            Some((cursor, anchor, area))
        }
    }
}

fn clamp_to_area(point: CellPoint, area: Rect) -> CellPoint {
    CellPoint {
        x: point.x.clamp(area.x, area.right().saturating_sub(1)),
        y: point.y.clamp(area.y, area.bottom().saturating_sub(1)),
    }
}

fn for_each_selected_cell(
    buffer: &mut Buffer,
    area: Rect,
    start: CellPoint,
    end: CellPoint,
    mut visit: impl FnMut(&mut Buffer, u16, u16),
) {
    for y in start.y..=end.y {
        let left = if y == start.y { start.x } else { area.x };
        let right = if y == end.y {
            end.x
        } else {
            area.right().saturating_sub(1)
        };
        for x in left..=right {
            visit(buffer, x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect};

    use super::*;

    fn buffer_with_text(lines: &[&str], width: u16) -> Buffer {
        let mut buffer = Buffer::empty(Rect::new(0, 0, width, lines.len() as u16));
        for (row, line) in lines.iter().enumerate() {
            buffer.set_string(0, row as u16, *line, Style::default());
        }
        buffer
    }

    #[test]
    fn drag_release_copies_the_rendered_range() {
        let mut copy = MouseCopy::default();
        copy.set_buffer(buffer_with_text(&["alpha beta"], 10));
        copy.press(CellPoint::new(2, 0));

        assert!(copy.drag(CellPoint::new(6, 0)));
        let release = copy
            .release(CellPoint::new(6, 0))
            .expect("a drag should consume the release");

        assert_eq!(release, MouseCopyRelease::Selection(Some("pha b".into())));
        assert_eq!(copy.selection(), None);
    }

    #[test]
    fn click_release_preserves_the_press_position() {
        let mut copy = MouseCopy::default();
        copy.press(CellPoint::new(3, 2));

        assert_eq!(
            copy.release(CellPoint::new(4, 2)),
            Some(MouseCopyRelease::Click(CellPoint::new(3, 2)))
        );
    }

    #[test]
    fn multiline_copy_trims_terminal_padding() {
        let buffer = buffer_with_text(&["first", "second", "third"], 8);
        let selection = CellSelection {
            anchor: CellPoint::new(2, 0),
            cursor: CellPoint::new(2, 2),
            region: None,
        };

        assert_eq!(
            selected_text(&buffer, selection).as_deref(),
            Some("rst\nsecond\nthi")
        );
    }

    #[test]
    fn reverse_drag_uses_document_order() {
        let buffer = buffer_with_text(&["alpha", "bravo"], 6);
        let selection = CellSelection {
            anchor: CellPoint::new(2, 1),
            cursor: CellPoint::new(1, 0),
            region: None,
        };

        assert_eq!(
            selected_text(&buffer, selection).as_deref(),
            Some("lpha\nbra")
        );
    }

    #[test]
    fn copy_skips_the_continuation_cell_of_a_wide_symbol() {
        let buffer = buffer_with_text(&["界a"], 3);
        let selection = CellSelection {
            anchor: CellPoint::new(0, 0),
            cursor: CellPoint::new(2, 0),
            region: None,
        };

        assert_eq!(selected_text(&buffer, selection).as_deref(), Some("界a"));
    }

    #[test]
    fn wrapped_rows_join_with_one_space() {
        let buffer = buffer_with_text(&["wrapped", "  text"], 7);
        let selection = CellSelection {
            anchor: CellPoint::new(0, 0),
            cursor: CellPoint::new(5, 1),
            region: None,
        };

        assert_eq!(
            selected_text(&buffer, selection).as_deref(),
            Some("wrapped text")
        );
    }

    #[test]
    fn word_wrapped_rows_join_when_the_break_precedes_the_right_edge() {
        let buffer = buffer_with_text(
            &[
                "This uses the Dialog chrome only, with text content",
                "inside.",
            ],
            56,
        );
        let selection = CellSelection {
            anchor: CellPoint::new(3, 0),
            cursor: CellPoint::new(6, 1),
            region: Some(CopyRegion::new(Rect::new(0, 0, 56, 2)).soft_wrap_rows([0])),
        };

        assert_eq!(
            selected_text(&buffer, selection).as_deref(),
            Some("s uses the Dialog chrome only, with text content inside.")
        );
    }

    #[test]
    fn selection_highlight_uses_semantic_theme_colors() {
        let mut buffer = buffer_with_text(&["text"], 4);
        let selection = CellSelection {
            anchor: CellPoint::new(1, 0),
            cursor: CellPoint::new(2, 0),
            region: None,
        };

        apply_selection(&mut buffer, selection);

        assert_eq!(
            buffer.cell((0, 0)).unwrap().bg,
            ratatui::style::Color::Reset
        );
        assert_eq!(buffer.cell((1, 0)).unwrap().fg, theme().selected_fg());
        assert_eq!(buffer.cell((2, 0)).unwrap().bg, theme().selected_bg());
    }

    #[test]
    fn multiline_copy_wraps_within_the_deepest_registered_region() {
        let first = "Open bottom tabs dialog |tb|";
        let second = "Open left tabs dialog |tl|";
        let content_width = first.len().max(second.len()) as u16 + 1;
        let lines = [
            format!(
                "│ {first:<content_width$} │",
                content_width = content_width as usize
            ),
            format!(
                "│ {second:<content_width$} │",
                content_width = content_width as usize
            ),
        ];
        let mut copy = MouseCopy::default();
        copy.set_buffer(buffer_with_text(
            &[lines[0].as_str(), lines[1].as_str()],
            content_width + 4,
        ));
        copy.set_regions(&[
            Rect::new(1, 0, content_width + 2, 2).into(),
            Rect::new(2, 0, content_width, 2).into(),
        ]);
        copy.press(CellPoint::new(2, 0));
        copy.drag(CellPoint::new(2 + second.len() as u16 - 1, 1));

        assert_eq!(
            copy.release(CellPoint::new(2 + second.len() as u16 - 1, 1)),
            Some(MouseCopyRelease::Selection(Some(format!(
                "{first}\n{second}"
            ))))
        );
    }
}
