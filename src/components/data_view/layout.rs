use std::collections::HashMap;
use std::hash::Hash;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;

use super::{
    CELL_RIGHT_PADDING, CellContext, Column, DataView, DataViewInteraction, DisplayRow,
    FILTER_DROPDOWN_SLOT, SEARCH_SLOT, SelectionMode, SortDirection, VisibleRow, column_key,
};
use crate::{
    ChildKey, LayoutCtx, ScrollGeometry, ScrollOffset, ScrollSize, TuiNode, line_width, preset,
};

pub(super) struct VisibleRowGeometry {
    offsets: Vec<usize>,
}

impl VisibleRowGeometry {
    fn new(heights: impl IntoIterator<Item = u16>) -> Self {
        let mut offsets = vec![0usize];
        for height in heights {
            offsets.push(
                offsets
                    .last()
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(height.max(1) as usize),
            );
        }
        Self { offsets }
    }

    pub(super) fn total_height(&self) -> usize {
        self.offsets.last().copied().unwrap_or(0)
    }

    pub(super) fn span(&self, index: usize) -> Option<(usize, usize)> {
        Some((*self.offsets.get(index)?, *self.offsets.get(index + 1)?))
    }

    pub(super) fn capacity(&self, offset: usize, height: usize) -> usize {
        self.intersecting(offset, offset.saturating_add(height))
            .count()
            .max(1)
    }

    fn height_through(&self, row_count: usize) -> usize {
        self.offsets
            .get(row_count.min(self.offsets.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    }

    pub(super) fn intersecting(
        &self,
        start: usize,
        end: usize,
    ) -> impl Iterator<Item = (usize, usize, usize)> + '_ {
        self.offsets
            .windows(2)
            .enumerate()
            .filter_map(move |(index, span)| {
                (span[0] < end && span[1] > start).then_some((index, span[0], span[1]))
            })
    }
}

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(crate) fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let rendered_widths = self.rendered_column_widths();
        self.scroll_geometry_with_rendered_widths(area, &rendered_widths)
    }

    pub(super) fn scroll_geometry_with_rendered_widths(
        &self,
        area: Rect,
        rendered_widths: &[usize],
    ) -> ScrollGeometry {
        let body_area = self.body_area(area);
        let mut content = self.content_size(body_area.width as usize, rendered_widths);
        let mut geometry = self.scroll.geometry(body_area, content);

        for _ in 0..3 {
            let next_content =
                self.content_size(geometry.layout.viewport.width as usize, rendered_widths);
            if next_content == content {
                return geometry;
            }
            content = next_content;
            geometry = self.scroll.geometry(body_area, content);
        }

        geometry
    }

    pub(super) fn body_area(&self, area: Rect) -> Rect {
        let reserved = u16::from(self.action_bar) + u16::from(self.shows_headers());
        if reserved > 0 {
            Rect::new(
                area.x,
                area.y.saturating_add(reserved),
                area.width,
                area.height.saturating_sub(reserved),
            )
        } else {
            area
        }
    }

    pub(super) fn action_bar_areas(&self, area: Rect) -> (Rect, Rect) {
        if !self.action_bar || area.is_empty() {
            return (Rect::default(), Rect::default());
        }
        let search_width = area.width.min(28);
        let [search_area, summary_area] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(search_width), Constraint::Fill(1)])
            .areas(Rect::new(area.x, area.y, area.width, 1));
        (search_area, summary_area)
    }

    pub(super) fn popup_field_area(&self, area: Rect) -> Rect {
        if area.width == 0 || area.height == 0 {
            return Rect::default();
        }
        let width = area.width.min(40);
        Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(1) / 2,
            width,
            1,
        )
    }

    pub(super) fn layout_children<M>(&mut self, area: Rect, ctx: &mut LayoutCtx) {
        if self.action_bar {
            let (search_area, _) = self.action_bar_areas(area);
            ctx.push_slot(ChildKey::new(SEARCH_SLOT), search_area, |ctx| {
                self.search_input.layout(search_area, ctx);
                ctx.set_focus_tab_stop(super::search_focus_id(), false);
            });
        }

        let popup_area = self.popup_field_area(area);
        if let Some(dropdown) = self.filter_dropdown.as_mut() {
            ctx.push_slot(ChildKey::new(FILTER_DROPDOWN_SLOT), popup_area, |ctx| {
                <Box<super::ChoiceDropdown> as TuiNode<M>>::layout(dropdown, popup_area, ctx);
            });
        }
    }

    fn content_size(&self, viewport_width: usize, rendered_widths: &[usize]) -> ScrollSize {
        let width = self
            .column_widths_with_rendered(viewport_width, rendered_widths)
            .into_iter()
            .sum();
        ScrollSize::new(width, self.visible_row_geometry().total_height())
    }

    pub(super) fn visible_row_geometry(&self) -> VisibleRowGeometry {
        VisibleRowGeometry::new(self.display_rows().into_iter().map(|row| match row {
            DisplayRow::Data(row) => self.row_height_for(row.row),
            DisplayRow::SelectionPlaceholder { .. } => self.row_height,
        }))
    }

    pub(super) fn highlighted_row_area(&self) -> Rect {
        let body = self.body_area(self.area);
        let (start, end) = self
            .visible_row_geometry()
            .span(self.highlighted)
            .unwrap_or((0, 1));
        Rect::new(
            body.x,
            body.y.saturating_add(start.min(u16::MAX as usize) as u16),
            body.width,
            end.saturating_sub(start).min(u16::MAX as usize) as u16,
        )
    }

    pub(crate) fn measured_rows_height(&self, max_rows: usize) -> u16 {
        let total = self.visible_row_geometry().height_through(max_rows);
        total.min(u16::MAX as usize) as u16
    }

    pub(super) fn visible_offset(&self, viewport: ScrollSize, content: ScrollSize) -> ScrollOffset {
        let offset = self.scroll.offset();
        ScrollOffset::new(
            offset.x.min(content.width.saturating_sub(viewport.width)),
            offset.y.min(content.height.saturating_sub(viewport.height)),
        )
    }

    #[cfg(test)]
    pub(super) fn column_widths(&self, viewport_width: usize) -> Vec<usize> {
        let rendered = self.rendered_column_widths();
        self.column_widths_with_rendered(viewport_width, &rendered)
    }

    pub(super) fn column_widths_with_rendered(
        &self,
        viewport_width: usize,
        rendered: &[usize],
    ) -> Vec<usize> {
        let columns = self.visible_columns().collect::<Vec<_>>();
        let configured = self.configured_column_widths(viewport_width);

        configured
            .into_iter()
            .zip(rendered.iter().copied())
            .enumerate()
            .map(|(index, (configured, rendered))| {
                let padding = if index + 1 == columns.len() {
                    0
                } else {
                    CELL_RIGHT_PADDING
                };
                if columns[index].sizing == super::ColumnSizing::Constrained {
                    configured
                } else {
                    configured.max(rendered.saturating_add(padding))
                }
            })
            .collect()
    }

    fn configured_column_widths(&self, viewport_width: usize) -> Vec<usize> {
        let columns = self.visible_columns().collect::<Vec<_>>();
        if columns.is_empty() {
            return Vec::new();
        }

        let content_width = self
            .configured_content_width(viewport_width)
            .min(u16::MAX as usize);
        let column_padding = self.column_padding_width();
        let area = Rect::new(0, 0, content_width.saturating_sub(column_padding) as u16, 1);
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                columns
                    .iter()
                    .map(|column| column.width)
                    .collect::<Vec<_>>(),
            )
            .split(area)
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                let padding = if index + 1 == columns.len() {
                    0
                } else {
                    CELL_RIGHT_PADDING
                };
                cell.width as usize + padding
            })
            .collect()
    }

    fn configured_content_width(&self, viewport_width: usize) -> usize {
        let minimum_width = self
            .configured_minimum_column_widths()
            .into_iter()
            .sum::<usize>()
            .saturating_add(self.column_padding_width());
        viewport_width.max(minimum_width)
    }

    fn column_padding_width(&self) -> usize {
        self.visible_column_count()
            .saturating_sub(1)
            .saturating_mul(CELL_RIGHT_PADDING)
    }

    fn configured_minimum_column_widths(&self) -> Vec<usize> {
        self.visible_columns()
            .map(|column| match column.width {
                Constraint::Length(width) | Constraint::Min(width) => width as usize,
                _ => 0,
            })
            .collect()
    }

    pub(super) fn rendered_column_widths(&self) -> Vec<usize> {
        let columns = self.visible_columns().collect::<Vec<_>>();
        let mut widths = vec![0; columns.len()];

        if self.shows_headers() {
            for (index, column) in columns.iter().enumerate() {
                widths[index] = widths[index].max(self.header_width(column));
            }
        }

        let selection_descendants = self.selection_descendants_by_id();
        let show_tree_gutter = self.shows_tree_gutter();
        for row in self.display_rows() {
            match row {
                DisplayRow::Data(row) => {
                    for (index, column) in columns.iter().enumerate() {
                        widths[index] = widths[index].max(self.rendered_cell_width(
                            index,
                            column,
                            &row,
                            self.highlighted_id().as_ref() == Some(&row.id),
                            &selection_descendants,
                            show_tree_gutter,
                        ));
                    }
                }
                DisplayRow::SelectionPlaceholder { count, depth, .. } => {
                    if let Some(width) = widths.first_mut() {
                        *width =
                            (*width).max(
                                line_width(&Line::from(format!("{count} items selected")))
                                    .saturating_add(self.selection_placeholder_prefix_width(
                                        depth,
                                        show_tree_gutter,
                                    )),
                            );
                    }
                }
            }
        }

        widths
    }

    fn header_width(&self, column: &Column<T, Id>) -> usize {
        let mut header = self.header_label(column);
        if self.filter_active(&column.id) {
            header.push_str(" ");
        }
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

    pub(crate) fn header_label(&self, column: &Column<T, Id>) -> String {
        if self.interaction == DataViewInteraction::HeaderFilter
            && column.filter_key.is_some()
            && let Some(index) = self
                .visible_columns()
                .position(|candidate| candidate.id == column.id)
            && let Some(key) = column_key(index)
        {
            return format!("{key} {}", column.header);
        }
        column.header.clone()
    }

    fn rendered_cell_width(
        &self,
        column_index: usize,
        column: &Column<T, Id>,
        row: &VisibleRow<'_, T, Id>,
        highlighted: bool,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
    ) -> usize {
        let text = (column.renderer)(
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
        let prefix_width = if column_index == 0 {
            self.row_prefix_width(row, selection_descendants, show_tree_gutter)
        } else {
            0
        };
        text.lines
            .iter()
            .take(self.row_height_for(row.row) as usize)
            .map(line_width)
            .max()
            .unwrap_or(0)
            .saturating_add(prefix_width)
    }

    fn row_prefix_width(
        &self,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
    ) -> usize {
        let mut width = 0;
        if show_tree_gutter {
            width += row
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
            width += line_width(&Line::from(format!("{glyph} ")));
        }
        if self.selection_mode == SelectionMode::Multi {
            let glyph = if self.is_selection_disabled(&row.id) {
                self.selection_disabled_glyph
            } else {
                self.selection_glyph_with_descendants(&row.id, selection_descendants)
            };
            width += line_width(&Line::from(format!("{glyph} ")));
        }
        width
    }

    fn selection_placeholder_prefix_width(&self, depth: usize, show_tree_gutter: bool) -> usize {
        show_tree_gutter
            .then(|| {
                depth
                    .saturating_mul(preset().data_view().tree_indent_width())
                    .saturating_add(line_width(&Line::from(format!(
                        "{} ",
                        self.tree_glyphs.leaf
                    ))))
            })
            .unwrap_or(0)
    }
}
