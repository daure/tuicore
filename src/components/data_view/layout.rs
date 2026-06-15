use std::hash::Hash;

use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::text::Line;

use super::{CellContext, Column, DataView, SortDirection, VisibleRow};
use crate::{ScrollGeometry, ScrollOffset, ScrollSize, line_width, preset};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(super) fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let body_area = self.body_area(area);
        let mut content = self.content_size(body_area.width as usize);
        let mut geometry = self.scroll.geometry(body_area, content);

        for _ in 0..3 {
            let next_content = self.content_size(geometry.layout.viewport.width as usize);
            if next_content == content {
                return geometry;
            }
            content = next_content;
            geometry = self.scroll.geometry(body_area, content);
        }

        geometry
    }

    pub(super) fn body_area(&self, area: Rect) -> Rect {
        if self.headers {
            Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            )
        } else {
            area
        }
    }

    fn content_size(&self, viewport_width: usize) -> ScrollSize {
        let width = self.column_widths(viewport_width).into_iter().sum();
        ScrollSize::new(width, self.visible_len())
    }

    pub(super) fn visible_offset(&self, viewport: ScrollSize, content: ScrollSize) -> ScrollOffset {
        let offset = self.scroll.offset();
        ScrollOffset::new(
            offset.x.min(content.width.saturating_sub(viewport.width)),
            offset
                .y
                .min(self.visible_len().saturating_sub(viewport.height)),
        )
    }

    pub(super) fn column_widths(&self, viewport_width: usize) -> Vec<usize> {
        let configured = self.configured_column_widths(viewport_width);
        let rendered = self.rendered_column_widths();

        configured
            .into_iter()
            .zip(rendered)
            .map(|(configured, rendered)| configured.max(rendered))
            .collect()
    }

    fn configured_column_widths(&self, viewport_width: usize) -> Vec<usize> {
        if self.columns.is_empty() {
            return Vec::new();
        }

        let content_width = self
            .configured_content_width(viewport_width)
            .min(u16::MAX as usize);
        let area = Rect::new(0, 0, content_width as u16, 1);
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                self.columns
                    .iter()
                    .map(|column| column.width)
                    .collect::<Vec<_>>(),
            )
            .split(area)
            .iter()
            .map(|cell| cell.width as usize)
            .collect()
    }

    fn configured_content_width(&self, viewport_width: usize) -> usize {
        let minimum_width = self.configured_minimum_column_widths().into_iter().sum();
        viewport_width.max(minimum_width)
    }

    fn configured_minimum_column_widths(&self) -> Vec<usize> {
        self.columns
            .iter()
            .map(|column| match column.width {
                Constraint::Length(width) | Constraint::Min(width) => width as usize,
                _ => 0,
            })
            .collect()
    }

    fn rendered_column_widths(&self) -> Vec<usize> {
        let mut widths = vec![0; self.columns.len()];

        if self.headers {
            for (index, column) in self.columns.iter().enumerate() {
                widths[index] = widths[index].max(self.header_width(column));
            }
        }

        for (row_index, row) in self.visible_rows().into_iter().enumerate() {
            for (index, column) in self.columns.iter().enumerate() {
                widths[index] = widths[index].max(self.rendered_cell_width(
                    index,
                    column,
                    &row,
                    row_index == self.highlighted,
                ));
            }
        }

        widths
    }

    fn header_width(&self, column: &Column<T, Id>) -> usize {
        let mut header = column.header.clone();
        if let Some(sort) = &self.sort
            && sort.column_id == column.id
        {
            header.push_str(match sort.direction {
                SortDirection::Ascending => " ↑",
                SortDirection::Descending => " ↓",
            });
        }
        line_width(&Line::from(header))
    }

    fn rendered_cell_width(
        &self,
        column_index: usize,
        column: &Column<T, Id>,
        row: &VisibleRow<'_, T, Id>,
        highlighted: bool,
    ) -> usize {
        let line = (column.renderer)(
            row.row,
            &CellContext {
                row_id: row.id.clone(),
                column_id: column.id.clone(),
                depth: row.depth,
                has_children: row.has_children,
                expanded: row.expanded,
                highlighted,
                focused: self.focused,
            },
        );
        let prefix_width = if column_index == 0 && self.tree.is_some() {
            self.tree_prefix_width(row)
        } else {
            0
        };
        prefix_width + line_width(&line)
    }

    fn tree_prefix_width(&self, row: &VisibleRow<'_, T, Id>) -> usize {
        let indent_width = row
            .depth
            .saturating_mul(preset().data_view().tree_indent_width());
        let glyph = if row.has_children {
            if row.expanded {
                self.tree_glyphs.expanded
            } else {
                self.tree_glyphs.collapsed
            }
        } else {
            self.tree_glyphs.leaf
        };
        indent_width + line_width(&Line::from(format!("{glyph} ")))
    }
}
