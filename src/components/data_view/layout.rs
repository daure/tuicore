use std::borrow::Cow;
use std::collections::HashMap;
use std::hash::Hash;

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span, Text};

use super::{
    CELL_RIGHT_PADDING, CellContext, Column, DataView, DataViewInteraction, DisplayRow,
    FILTER_DROPDOWN_SLOT, SEARCH_SLOT, SelectionMode, SortDirection, VisibleRow, column_key,
};
use crate::{
    ChildKey, LayoutCtx, ScrollGeometry, ScrollOffset, ScrollSize, TuiNode, line_width, preset,
};

pub(super) struct VisibleRowGeometry {
    kind: VisibleRowGeometryKind,
}

enum VisibleRowGeometryKind {
    Uniform { count: usize, height: usize },
    Variable { offsets: Vec<usize> },
}

pub(super) enum IntersectingRows<'a> {
    Empty,
    Uniform {
        next: usize,
        end: usize,
        height: usize,
    },
    Variable {
        offsets: &'a [usize],
        next: usize,
        end: usize,
    },
}

impl Iterator for IntersectingRows<'_> {
    type Item = (usize, usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Empty => None,
            Self::Uniform { next, end, height } if *next < *end => {
                let index = *next;
                *next += 1;
                let row_start = index.saturating_mul(*height);
                Some((index, row_start, row_start.saturating_add(*height)))
            }
            Self::Variable { offsets, next, end } if *next < *end => {
                let index = *next;
                *next += 1;
                Some((index, offsets[index], offsets[index + 1]))
            }
            _ => None,
        }
    }
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
        Self {
            kind: VisibleRowGeometryKind::Variable { offsets },
        }
    }

    fn uniform(count: usize, height: u16) -> Self {
        Self {
            kind: VisibleRowGeometryKind::Uniform {
                count,
                height: height.max(1) as usize,
            },
        }
    }

    pub(super) fn total_height(&self) -> usize {
        match &self.kind {
            VisibleRowGeometryKind::Uniform { count, height } => count.saturating_mul(*height),
            VisibleRowGeometryKind::Variable { offsets } => offsets.last().copied().unwrap_or(0),
        }
    }

    pub(super) fn span(&self, index: usize) -> Option<(usize, usize)> {
        match &self.kind {
            VisibleRowGeometryKind::Uniform { count, height } if index < *count => {
                let start = index.saturating_mul(*height);
                Some((start, start.saturating_add(*height)))
            }
            VisibleRowGeometryKind::Uniform { .. } => None,
            VisibleRowGeometryKind::Variable { offsets } => {
                Some((*offsets.get(index)?, *offsets.get(index + 1)?))
            }
        }
    }

    pub(super) fn capacity(&self, offset: usize, height: usize) -> usize {
        self.intersecting(offset, offset.saturating_add(height))
            .count()
            .max(1)
    }

    fn height_through(&self, row_count: usize) -> usize {
        match &self.kind {
            VisibleRowGeometryKind::Uniform { count, height } => {
                row_count.min(*count).saturating_mul(*height)
            }
            VisibleRowGeometryKind::Variable { offsets } => offsets
                .get(row_count.min(offsets.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0),
        }
    }

    pub(super) fn intersecting(&self, start: usize, end: usize) -> IntersectingRows<'_> {
        if start >= end {
            return IntersectingRows::Empty;
        }
        match &self.kind {
            VisibleRowGeometryKind::Uniform { count, height } => {
                let first = (start / *height).min(*count);
                let last = end
                    .saturating_sub(1)
                    .saturating_div(*height)
                    .saturating_add(1)
                    .min(*count);
                IntersectingRows::Uniform {
                    next: first,
                    end: last,
                    height: *height,
                }
            }
            VisibleRowGeometryKind::Variable { offsets } => {
                let first = offsets[1..].partition_point(|row_end| *row_end <= start);
                let last = offsets[..offsets.len().saturating_sub(1)]
                    .partition_point(|row_start| *row_start < end);
                IntersectingRows::Variable {
                    offsets,
                    next: first,
                    end: last,
                }
            }
        }
    }
}

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(crate) fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let rendered_widths = self.rendered_column_widths_for_viewport(area.width as usize);
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

    pub(super) fn content_size(
        &self,
        viewport_width: usize,
        rendered_widths: &[usize],
    ) -> ScrollSize {
        let column_widths = self.column_widths_with_rendered(viewport_width, rendered_widths);
        let width = column_widths.iter().sum();
        ScrollSize::new(
            width,
            self.visible_row_geometry_for_viewport(&column_widths)
                .total_height(),
        )
    }

    pub(super) fn visible_row_geometry(&self) -> VisibleRowGeometry {
        if !self.wrap_cells {
            return self.configured_visible_row_geometry();
        }
        let rendered_widths = self.rendered_column_widths_for_viewport(self.area.width as usize);
        let geometry = self.scroll_geometry_with_rendered_widths(self.area, &rendered_widths);
        let column_widths = self
            .column_widths_with_rendered(geometry.layout.viewport.width as usize, &rendered_widths);
        self.visible_row_geometry_for_viewport(&column_widths)
    }

    pub(super) fn scroll_geometry_and_row_geometry(
        &self,
        area: Rect,
    ) -> (ScrollGeometry, VisibleRowGeometry) {
        let rendered_widths = self.rendered_column_widths_for_viewport(area.width as usize);
        let geometry = self.scroll_geometry_with_rendered_widths(area, &rendered_widths);
        let column_widths = self
            .column_widths_with_rendered(geometry.layout.viewport.width as usize, &rendered_widths);
        let rows = self.visible_row_geometry_for_viewport(&column_widths);
        (geometry, rows)
    }

    pub(super) fn visible_row_geometry_for_viewport(
        &self,
        column_widths: &[usize],
    ) -> VisibleRowGeometry {
        if !self.wrap_cells {
            return self.configured_visible_row_geometry();
        }
        let selection_descendants = self.selection_descendants_by_id();
        let show_tree_gutter = self.shows_tree_gutter();
        let highlighted_id = self.highlighted_id();
        VisibleRowGeometry::new(self.display_rows().into_iter().map(|row| match row {
            DisplayRow::Data(row) => self.wrapped_row_height(
                &row,
                column_widths,
                &selection_descendants,
                show_tree_gutter,
                highlighted_id.as_ref(),
            ),
            DisplayRow::SelectionPlaceholder { .. } => self.row_height,
        }))
    }

    fn configured_visible_row_geometry(&self) -> VisibleRowGeometry {
        if self.row_height_by.is_none() {
            return VisibleRowGeometry::uniform(self.display_rows().len(), self.row_height);
        }
        VisibleRowGeometry::new(self.display_rows().into_iter().map(|row| match row {
            DisplayRow::Data(row) => self.row_height_for(row.row),
            DisplayRow::SelectionPlaceholder { .. } => self.row_height,
        }))
    }

    fn wrapped_row_height(
        &self,
        row: &VisibleRow<'_, T, Id>,
        column_widths: &[usize],
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
        highlighted_id: Option<&Id>,
    ) -> u16 {
        let minimum = self.row_height_for(row.row);
        if !self.wrap_cells {
            return minimum;
        }
        self.visible_columns()
            .zip(column_widths)
            .enumerate()
            .map(|(index, (column, _width))| {
                let text = (column.renderer)(
                    row.row,
                    &CellContext {
                        row_id: row.id.clone(),
                        column_id: column.id.clone(),
                        depth: row.depth,
                        has_children: row.has_children,
                        expanded: row.expanded,
                        highlighted: highlighted_id == Some(&row.id),
                        focused: self.focused,
                    },
                );
                let text = self.wrapped_cell_text(
                    index,
                    column,
                    text,
                    self.cell_content_width(index, column_widths),
                    row,
                    selection_descendants,
                    show_tree_gutter,
                );
                wrapped_text_height(&text, self.cell_content_width(index, column_widths))
            })
            .max()
            .unwrap_or(1)
            .max(minimum)
    }

    pub(super) fn wrapped_cell_text(
        &self,
        column_index: usize,
        column: &Column<T, Id>,
        mut text: Text<'static>,
        width: u16,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
    ) -> Text<'static> {
        let default_continuation_indent = if column_index == 0 {
            if self.left_gutter_marker_by.is_some()
                || show_tree_gutter
                || self.selection_mode == SelectionMode::Multi
            {
                text = self.with_row_prefix(text, row, selection_descendants, show_tree_gutter);
            }
            self.row_prefix_width(row, selection_descendants, show_tree_gutter)
                .saturating_add(2)
        } else {
            2
        };
        let continuation_indent = column
            .continuation_indent
            .as_ref()
            .map(|indent| indent(row.row))
            .unwrap_or(default_continuation_indent);
        if self.wrap_cells {
            self.repeat_left_gutter_marker_on_wrapped_lines(
                wrap_text(text, width, continuation_indent),
                row.row,
            )
        } else {
            text
        }
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
                if self.wrap_cells || columns[index].sizing == super::ColumnSizing::Constrained {
                    configured
                } else {
                    configured.max(rendered.saturating_add(padding))
                }
            })
            .collect()
    }

    pub(super) fn cell_content_width(&self, index: usize, column_widths: &[usize]) -> u16 {
        let padding = usize::from(index + 1 < column_widths.len()) * CELL_RIGHT_PADDING;
        column_widths
            .get(index)
            .copied()
            .unwrap_or(0)
            .saturating_sub(padding)
            .min(u16::MAX as usize) as u16
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

        if self.wrap_cells
            || !columns
                .iter()
                .any(|column| column.sizing == super::ColumnSizing::Intrinsic)
        {
            return widths;
        }

        if self.shows_headers() {
            for (index, column) in columns.iter().enumerate() {
                if column.sizing == super::ColumnSizing::Intrinsic {
                    widths[index] = widths[index].max(self.header_width(column));
                }
            }
        }

        let selection_descendants = self.selection_descendants_by_id();
        let show_tree_gutter = self.shows_tree_gutter();
        let highlighted_id = self.highlighted_id();
        for row in self.display_rows() {
            match row {
                DisplayRow::Data(row) => {
                    for (index, column) in columns.iter().enumerate() {
                        if column.sizing == super::ColumnSizing::Intrinsic {
                            widths[index] = widths[index].max(self.rendered_cell_width(
                                index,
                                column,
                                &row,
                                highlighted_id.as_ref() == Some(&row.id),
                                &selection_descendants,
                                show_tree_gutter,
                            ));
                        }
                    }
                }
                DisplayRow::SelectionPlaceholder {
                    count,
                    depth,
                    focused,
                } => {
                    if columns
                        .first()
                        .is_some_and(|column| column.sizing == super::ColumnSizing::Intrinsic)
                        && let Some(width) = widths.first_mut()
                    {
                        let label = if focused {
                            format!("Moving {count} tasks")
                        } else {
                            format!("{count} items selected")
                        };
                        *width = (*width).max(line_width(&Line::from(label)).saturating_add(
                            self.selection_placeholder_prefix_width(depth, show_tree_gutter),
                        ));
                    }
                }
            }
        }

        widths
    }

    pub(super) fn prepare_metrics(&mut self, viewport_width: usize) {
        if !self.metrics_cacheable() {
            self.metric_cache = None;
            return;
        }
        if self.metric_cache.as_ref().is_some_and(|cache| {
            cache.revision == self.metric_revision && cache.viewport_width == viewport_width
        }) {
            return;
        }
        self.metric_cache = Some(super::DataViewMetricCache {
            revision: self.metric_revision,
            viewport_width,
            rendered_column_widths: self.rendered_column_widths(),
        });
    }

    pub(super) fn rendered_column_widths_for_viewport(
        &self,
        viewport_width: usize,
    ) -> Cow<'_, [usize]> {
        if let Some(cache) = self.metric_cache.as_ref().filter(|cache| {
            self.metrics_cacheable()
                && cache.revision == self.metric_revision
                && cache.viewport_width == viewport_width
        }) {
            return Cow::Borrowed(&cache.rendered_column_widths);
        }
        Cow::Owned(self.rendered_column_widths())
    }

    fn metrics_cacheable(&self) -> bool {
        !self.wrap_cells
            && self.row_height_by.is_none()
            && self.tree.is_none()
            && self.pagination.is_none()
            && self.selection_mode == SelectionMode::None
            && self.selection_overlay.is_none()
            && matches!(self.interaction, DataViewInteraction::Grid)
            && self
                .visible_columns()
                .any(|column| column.sizing == super::ColumnSizing::Intrinsic)
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
        let mut width = self
            .left_gutter_marker_by
            .as_ref()
            .is_some_and(|marker| marker(row.row).is_some())
            .into();
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
            let glyph = if self.is_selection_disabled_for_row(row.row) {
                self.selection_disabled_glyph
            } else {
                self.selection_glyph_for_row_with_descendants(
                    row.row,
                    &row.id,
                    selection_descendants,
                )
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

fn wrapped_text_height(text: &ratatui::text::Text<'_>, width: u16) -> u16 {
    let _ = width;
    text.lines.len().max(1).min(u16::MAX as usize) as u16
}

fn wrap_text(text: Text<'static>, width: u16, continuation_indent: usize) -> Text<'static> {
    let style = text.style;
    let lines: Vec<_> = text
        .lines
        .into_iter()
        .flat_map(|line| wrap_line(line, width, continuation_indent))
        .collect();
    let mut wrapped = Text::from(lines);
    wrapped.style = style;
    wrapped
}

fn wrap_line(line: Line<'static>, width: u16, continuation_indent: usize) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let line_style = line.style;
    let alignment = line.alignment;
    let original = line.clone();
    let mut lines = Vec::new();
    let mut spans = Vec::new();
    let mut pending_whitespace = Vec::new();
    let mut used_width = 0usize;
    let mut has_content = false;

    for span in line.spans {
        for token in span.content.split_inclusive(char::is_whitespace) {
            let token_width = line_width(&Line::from(token));
            let whitespace = token.chars().all(char::is_whitespace);
            if whitespace {
                if has_content {
                    pending_whitespace.push(Span::styled(token.to_owned(), span.style));
                } else {
                    spans.push(Span::styled(token.to_owned(), span.style));
                    used_width = used_width.saturating_add(token_width);
                }
                continue;
            }

            let pending_width = pending_whitespace
                .iter()
                .map(|span| line_width(&Line::from(span.clone())))
                .sum::<usize>();
            if has_content
                && used_width
                    .saturating_add(pending_width)
                    .saturating_add(token_width)
                    > width
            {
                lines.push(Line {
                    spans,
                    style: line_style,
                    alignment,
                });
                spans = vec![Span::raw(" ".repeat(continuation_indent))];
                used_width = continuation_indent;
                pending_whitespace.clear();
            } else {
                used_width = used_width.saturating_add(pending_width);
                spans.append(&mut pending_whitespace);
            }
            spans.push(Span::styled(token.to_owned(), span.style));
            used_width = used_width.saturating_add(token_width);
            has_content = true;
        }
    }

    if !has_content {
        return vec![original];
    }
    lines.push(Line {
        spans,
        style: line_style,
        alignment,
    });
    lines
}
