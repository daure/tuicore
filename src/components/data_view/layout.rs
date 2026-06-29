use std::collections::HashMap;
use std::hash::Hash;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;

use super::{
    CellContext, Column, DataView, DataViewInteraction, FILTER_DROPDOWN_SLOT, SEARCH_SLOT,
    SelectionMode, SortDirection, VisibleRow, column_key,
};
use crate::{
    ChildKey, LayoutCtx, ScrollGeometry, ScrollOffset, ScrollSize, TuiNode, line_width, preset,
};

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
        let reserved = u16::from(self.action_bar) + u16::from(self.headers);
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

        let selection_descendants = self.selection_descendants_by_id();
        for (row_index, row) in self.visible_rows().into_iter().enumerate() {
            for (index, column) in self.columns.iter().enumerate() {
                widths[index] = widths[index].max(self.rendered_cell_width(
                    index,
                    column,
                    &row,
                    row_index == self.highlighted,
                    &selection_descendants,
                ));
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
                .columns
                .iter()
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
        let prefix_width = if column_index == 0 {
            self.row_prefix_width(row, selection_descendants)
        } else {
            0
        };
        prefix_width + line_width(&line)
    }

    fn row_prefix_width(
        &self,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
    ) -> usize {
        let mut width = 0;
        if self.tree.is_some() {
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
            width += line_width(&Line::from(format!(
                "{} ",
                self.selection_glyph_with_descendants(&row.id, selection_descendants)
            )));
        }
        width
    }
}
