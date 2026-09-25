use super::*;

impl<M> TextareaInput<M> {
    /// Return the zero-based character index under a terminal cell, excluding chrome and padding.
    pub fn text_index_at(&self, column: u16, row: u16) -> Option<usize> {
        let viewport = self.scroll_geometry(self.area).layout.viewport;
        if !viewport.contains((column, row).into()) {
            return None;
        }
        let y = self.scroll.offset().y + usize::from(row - viewport.y);
        let x = usize::from(column - viewport.x);
        let width = usize::from(viewport.width);
        let ranges = self.line_ranges();
        let range = if self.wrap {
            self.visual_rows(width, &ranges).get(y)?.range
        } else {
            *ranges.get(y)?
        };
        let (cursor_line, cursor_col) = self.cursor_line_col(&ranges);
        let chars = self
            .value
            .chars()
            .skip(range.start)
            .take(range.len())
            .collect::<Vec<_>>();
        let horizontal = if !self.wrap && self.cursor_visible() && y == cursor_line {
            visible_start_for_cursor(&chars, cursor_col, width)
        } else {
            0
        };
        let mut drawn = 0;
        for (index, value) in chars.iter().enumerate().skip(horizontal) {
            let text = display_char(*value, width.saturating_sub(drawn));
            let cells = cell_width(&text);
            if (drawn..drawn + cells).contains(&x) {
                return Some(range.start + index);
            }
            drawn += cells;
        }
        None
    }
}
