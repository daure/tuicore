use std::hash::Hash;

use tuirealm::ratatui::Frame;
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::ratatui::style::{Modifier, Style};
use tuirealm::ratatui::text::{Line, Span};
use tuirealm::ratatui::widgets::{Block, Paragraph};

use super::{CellContext, DataView, SortDirection, VisibleRow};
use crate::{preset, theme};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() || self.columns.is_empty() {
            return;
        }

        let header_height = u16::from(self.headers);
        let [header_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(header_height), Constraint::Fill(1)])
            .areas(area);
        let geometry = self.scroll_geometry(area);
        let visible = self.visible_rows();
        let offset = self.visible_offset(geometry.viewport, geometry.content);
        let column_widths = self.column_widths(geometry.layout.viewport.width as usize);

        if self.headers {
            let header_viewport = Rect::new(
                geometry.layout.viewport.x,
                header_area.y,
                geometry.layout.viewport.width,
                header_area.height,
            );
            self.render_header(frame, header_viewport, &column_widths, offset.x);
        }

        for (line_index, row) in visible
            .iter()
            .enumerate()
            .skip(offset.y)
            .take(geometry.viewport.height)
        {
            let y = body_area.y + (line_index - offset.y) as u16;
            let row_area = Rect::new(
                geometry.layout.viewport.x,
                y,
                geometry.layout.viewport.width,
                1,
            );
            let highlighted = line_index == self.highlighted;
            let selected_style = highlighted.then(|| self.selected_row_style());
            if let Some(style) = selected_style {
                frame.render_widget(Block::default().style(style), row_area);
            }
            self.render_row(
                frame,
                row_area,
                &column_widths,
                offset.x,
                row,
                selected_style,
            );
        }

        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
    }

    fn render_header(
        &self,
        frame: &mut Frame,
        area: Rect,
        column_widths: &[usize],
        offset_x: usize,
    ) {
        if area.is_empty() {
            return;
        }
        let theme = theme();
        let cells = self.column_areas(area, column_widths, offset_x);
        for (column, cell_area) in self.columns.iter().zip(cells) {
            let Some(cell_area) = cell_area else {
                continue;
            };
            let mut header = column.header.clone();
            if let Some(sort) = &self.sort
                && sort.column_id == column.id
            {
                header.push_str(match sort.direction {
                    SortDirection::Ascending => " ↑",
                    SortDirection::Descending => " ↓",
                });
            }
            frame.render_widget(
                Paragraph::new(Line::from(header))
                    .style(
                        Style::default()
                            .fg(theme.accent_fg())
                            .add_modifier(Modifier::BOLD),
                    )
                    .scroll((0, cell_area.scroll_x)),
                cell_area.area,
            );
        }
    }

    fn render_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        column_widths: &[usize],
        offset_x: usize,
        row: &VisibleRow<'_, T, Id>,
        selected_style: Option<Style>,
    ) {
        let cells = self.column_areas(area, column_widths, offset_x);
        for (column_index, (column, cell_area)) in self.columns.iter().zip(cells).enumerate() {
            let Some(cell_area) = cell_area else {
                continue;
            };
            let mut line = (column.renderer)(
                row.row,
                &CellContext {
                    row_id: row.id.clone(),
                    column_id: column.id.clone(),
                    depth: row.depth,
                    has_children: row.has_children,
                    expanded: row.expanded,
                    highlighted: selected_style.is_some(),
                    focused: self.focused,
                },
            );
            if column_index == 0 && self.tree.is_some() {
                line = self.with_tree_prefix(line, row);
            }
            if let Some(style) = selected_style {
                line = apply_line_style(line, style);
            }
            let mut paragraph = Paragraph::new(line).scroll((0, cell_area.scroll_x));
            if let Some(style) = selected_style {
                paragraph = paragraph.style(style);
            }
            frame.render_widget(paragraph, cell_area.area);
        }
    }

    fn with_tree_prefix(&self, line: Line<'static>, row: &VisibleRow<'_, T, Id>) -> Line<'static> {
        let Line {
            spans: original_spans,
            style,
            alignment,
        } = line;
        let mut spans = Vec::new();
        let indent = " ".repeat(
            row.depth
                .saturating_mul(preset().data_view().tree_indent_width()),
        );
        spans.push(Span::raw(indent));
        if row.has_children {
            let glyph = if row.expanded {
                self.tree_glyphs.expanded
            } else {
                self.tree_glyphs.collapsed
            };
            spans.push(Span::raw(format!("{glyph} ")));
        } else {
            spans.push(Span::raw(format!("{} ", self.tree_glyphs.leaf)));
        }
        spans.extend(original_spans);
        Line {
            spans,
            style,
            alignment,
        }
    }

    fn selected_row_style(&self) -> Style {
        let theme = theme();
        if self.focused {
            Style::default()
                .fg(theme.selected_fg())
                .bg(theme.selected_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(theme.muted_fg())
                .bg(theme.selected_bg())
        }
    }

    fn column_areas(
        &self,
        viewport: Rect,
        column_widths: &[usize],
        offset_x: usize,
    ) -> Vec<Option<ViewCellArea>> {
        column_widths
            .iter()
            .scan(0usize, |x, width| {
                let width = (*width).min(u16::MAX as usize);
                let cell = Rect::new(
                    (*x).min(u16::MAX as usize) as u16,
                    viewport.y,
                    width as u16,
                    viewport.height,
                );
                *x = x.saturating_add(width);
                Some(cell)
            })
            .map(|cell| clip_cell(cell, viewport, offset_x))
            .collect()
    }
}

fn apply_line_style(line: Line<'static>, style: Style) -> Line<'static> {
    let Line {
        spans,
        style: line_style,
        alignment,
    } = line;
    Line {
        spans: spans
            .into_iter()
            .map(|span| Span {
                style: span.style.patch(style),
                ..span
            })
            .collect(),
        style: line_style.patch(style),
        alignment,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ViewCellArea {
    area: Rect,
    scroll_x: u16,
}

fn clip_cell(cell: Rect, viewport: Rect, offset_x: usize) -> Option<ViewCellArea> {
    let start = viewport.x as isize + cell.x as isize - offset_x as isize;
    let end = start.saturating_add(cell.width as isize);
    let viewport_start = viewport.x as isize;
    let viewport_end = viewport_start.saturating_add(viewport.width as isize);
    let clipped_start = start.max(viewport_start);
    let clipped_end = end.min(viewport_end);

    if clipped_end <= clipped_start {
        return None;
    }

    Some(ViewCellArea {
        area: Rect::new(
            clipped_start as u16,
            viewport.y,
            (clipped_end - clipped_start) as u16,
            viewport.height,
        ),
        scroll_x: clipped_start.saturating_sub(start).min(u16::MAX as isize) as u16,
    })
}
