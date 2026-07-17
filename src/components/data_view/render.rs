use std::collections::HashMap;
use std::hash::Hash;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use super::{
    CellContext, CheckState, DataView, DataViewInteraction, SelectionMode, SortDirection,
    VisibleRow,
};
use crate::search::{SearchMode, search_match};
use crate::{RenderCtx, keybindings, preset, theme};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_with_row_style(frame, area, None);
    }

    pub(crate) fn render_with_row_style(
        &self,
        frame: &mut Frame,
        area: Rect,
        base_row_style: Option<Style>,
    ) {
        let mut ctx = RenderCtx::new();
        self.render_with_row_style_ctx(frame, area, base_row_style, &mut ctx);
        ctx.flush(frame);
    }

    pub(crate) fn render_with_row_style_ctx<'a>(
        &'a self,
        frame: &mut Frame,
        area: Rect,
        base_row_style: Option<Style>,
        ctx: &mut RenderCtx<'a>,
    ) {
        if area.is_empty() || self.columns.is_empty() {
            return;
        }

        let action_height = u16::from(self.action_bar);
        let header_height = u16::from(self.headers);
        let [action_area, header_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(action_height),
                Constraint::Length(header_height),
                Constraint::Fill(1),
            ])
            .areas(area);
        let geometry = self.scroll_geometry(area);
        let visible = self.visible_rows();
        let offset = self.visible_offset(geometry.viewport, geometry.content);
        let column_widths = self.column_widths(geometry.layout.viewport.width as usize);
        let selection_descendants = self.selection_descendants_by_id();

        if self.action_bar {
            self.render_action_bar(frame, action_area);
        }

        if self.headers {
            let header_viewport = Rect::new(
                geometry.layout.viewport.x,
                header_area.y,
                geometry.layout.viewport.width,
                header_area.height,
            );
            self.render_header(frame, header_viewport, &column_widths, offset.x);
        }

        if visible.is_empty() {
            frame.render_widget(
                Paragraph::new("No results found.").style(Style::default().fg(theme().subtle_fg())),
                body_area,
            );
            self.scroll
                .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            self.render_popup(frame, area, ctx);
            return;
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
            let row_style =
                self.row_style(highlighted, row, &selection_descendants, base_row_style);
            frame.render_widget(
                Block::default().style(row_style.unwrap_or_default()),
                row_area,
            );
            self.render_row(
                frame,
                row_area,
                &column_widths,
                offset.x,
                row,
                highlighted,
                row_style,
                &selection_descendants,
            );
        }

        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);

        self.render_popup(frame, area, ctx);
    }

    fn render_action_bar(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let bindings = keybindings();
        let data_keys = bindings.data_view();
        let (search_area, summary_area) = self.action_bar_areas(area);
        self.search_input.render(frame, search_area);
        let filters = self.table_transform_controls_enabled().then(|| {
            if self.transform_state.filters.is_empty() {
                format!("{} filters", data_keys.filter_label())
            } else {
                format!(
                    "{} {} filter(s)",
                    data_keys.filter_label(),
                    self.transform_state.filters.len()
                )
            }
        });
        frame.render_widget(
            Paragraph::new(Line::from(vec![Span::raw(filters.unwrap_or_default())])),
            summary_area,
        );
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

    fn render_popup<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let field_area = self.popup_field_area(area);
        match &self.interaction {
            DataViewInteraction::FilterValues { .. } => {
                if let Some(dropdown) = self.filter_dropdown.as_ref() {
                    dropdown.render(frame, field_area, ctx);
                }
            }
            _ => {}
        }
    }

    fn render_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        column_widths: &[usize],
        offset_x: usize,
        row: &VisibleRow<'_, T, Id>,
        highlighted: bool,
        row_style: Option<Style>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
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
                    highlighted,
                    focused: self.focused,
                },
            );
            if column_index == 0 && (self.tree.is_some() || self.displays_selection_glyphs()) {
                line = self.with_row_prefix(line, row, selection_descendants);
            }
            line = underline_search_matches(line, self.transform_state.search.trim());
            let mut paragraph = Paragraph::new(line).scroll((0, cell_area.scroll_x));
            if let Some(style) = row_style {
                paragraph = paragraph.style(style);
            }
            frame.render_widget(paragraph, cell_area.area);
        }
    }

    fn with_row_prefix(
        &self,
        line: Line<'static>,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
    ) -> Line<'static> {
        let Line {
            spans: original_spans,
            style,
            alignment,
        } = line;
        let mut spans = Vec::new();
        if self.tree.is_some() {
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
        }
        if self.displays_selection_glyphs() {
            let check_state = self.check_state_with_descendants(&row.id, selection_descendants);
            let glyph = self.selection_glyphs.glyph(check_state);
            let content = format!("{glyph} ");
            spans.push(match check_state {
                CheckState::Unchecked => Span::raw(content),
                CheckState::Checked | CheckState::Indeterminate => {
                    Span::styled(content, Style::default().fg(theme().selected_fg()))
                }
            });
        }
        spans.extend(original_spans);
        Line {
            spans,
            style,
            alignment,
        }
    }

    #[cfg(test)]
    pub(super) fn selection_glyph(&self, row: &VisibleRow<'_, T, Id>) -> &'static str {
        let descendants = self.selection_descendants_by_id();
        self.selection_glyph_with_descendants(&row.id, &descendants)
    }

    fn row_style(
        &self,
        highlighted: bool,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        base_row_style: Option<Style>,
    ) -> Option<Style> {
        if highlighted && self.focused {
            Some(self.highlighted_row_style())
        } else if self.row_is_visually_selected(&row.id, selection_descendants) {
            Some(self.selected_row_style())
        } else {
            base_row_style
        }
    }

    fn row_is_visually_selected(
        &self,
        id: &Id,
        selection_descendants: &HashMap<Id, Vec<Id>>,
    ) -> bool {
        self.selection_mode != SelectionMode::None
            && self.check_state_with_descendants(id, selection_descendants) != CheckState::Unchecked
    }

    fn displays_selection_glyphs(&self) -> bool {
        self.selection_mode == SelectionMode::Multi
    }

    fn highlighted_row_style(&self) -> Style {
        let theme = theme();
        Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg())
            .add_modifier(Modifier::BOLD)
    }

    fn selected_row_style(&self) -> Style {
        let theme = theme();
        Style::default()
            .fg(theme.selected_fg())
            .bg(theme.selected_bg())
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

fn underline_search_matches(line: Line<'static>, search: &str) -> Line<'static> {
    if search.is_empty() {
        return line;
    }

    let Line {
        spans,
        style,
        alignment,
    } = line;
    Line {
        spans: spans
            .into_iter()
            .flat_map(|span| underline_span_matches(span, search))
            .collect(),
        style,
        alignment,
    }
}

fn underline_span_matches(span: Span<'static>, search: &str) -> Vec<Span<'static>> {
    let content = span.content.into_owned();
    let mut output = Vec::new();
    let mut cursor = 0;

    while cursor < content.len() {
        let Some(matched) = search_match(search, &content[cursor..], SearchMode::Contains) else {
            break;
        };
        let Some(span_match) = matched.spans.first() else {
            break;
        };
        let start = cursor + span_match.start;
        let end = cursor + span_match.end;
        if start > cursor {
            output.push(Span::styled(content[cursor..start].to_string(), span.style));
        }
        output.push(Span::styled(
            content[start..end].to_string(),
            span.style.add_modifier(Modifier::UNDERLINED),
        ));
        cursor = end;
    }

    if cursor < content.len() {
        output.push(Span::styled(content[cursor..].to_string(), span.style));
    }

    if output.is_empty() {
        output.push(Span::styled(content, span.style));
    }

    output
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
