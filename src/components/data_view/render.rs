use std::collections::HashMap;
use std::hash::Hash;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Paragraph};

use super::{
    CELL_RIGHT_PADDING, CellContext, CheckState, DataView, DataViewInteraction, DisplayRow,
    SelectionMode, SortDirection, VisibleRow,
};
use crate::search::{MatchSpan, SearchMode, search_match};
use crate::{RenderCtx, keybindings, lerp_color, line_width, preset, theme};

impl<T, Id> DataView<T, Id>
where
    Id: Clone + Eq + Hash,
{
    pub(crate) fn visible_column_rects(
        &self,
        area: Rect,
        row_y: u16,
        row_height: u16,
    ) -> Vec<Rect> {
        let rendered_widths = self.rendered_column_widths();
        let geometry = self.scroll_geometry_with_rendered_widths(area, &rendered_widths);
        let offset = self.visible_offset(geometry.viewport, geometry.content);
        let viewport = Rect::new(
            geometry.layout.viewport.x,
            row_y,
            geometry.layout.viewport.width,
            row_height,
        );
        let column_widths = self
            .column_widths_with_rendered(geometry.layout.viewport.width as usize, &rendered_widths);
        self.column_areas(viewport, &column_widths, offset.x)
            .into_iter()
            .map(|cell| {
                cell.map(|cell| cell.area)
                    .unwrap_or(Rect::new(viewport.x, row_y, 0, row_height))
            })
            .collect()
    }

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
        if area.is_empty() {
            return;
        }

        let action_height = u16::from(self.action_bar);
        let header_height = u16::from(self.shows_headers());
        let [action_area, header_area, body_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(action_height),
                Constraint::Length(header_height),
                Constraint::Fill(1),
            ])
            .areas(area);
        if self.action_bar {
            self.render_action_bar(frame, action_area);
        }

        if self.visible_column_count() == 0 {
            self.render_popup(frame, area, ctx);
            return;
        }

        let rendered_widths = self.rendered_column_widths();
        let geometry = self.scroll_geometry_with_rendered_widths(area, &rendered_widths);
        let visible = self.display_rows();
        let offset = self.visible_offset(geometry.viewport, geometry.content);
        let column_widths = self
            .column_widths_with_rendered(geometry.layout.viewport.width as usize, &rendered_widths);
        let selection_descendants = self.selection_descendants_by_id();
        let show_tree_gutter = self.shows_tree_gutter();
        let highlighted_id = self.highlighted_id();

        if self.shows_headers() {
            let header_viewport = Rect::new(
                geometry.layout.viewport.x,
                header_area.y,
                geometry.layout.viewport.width,
                header_area.height,
            );
            self.render_header(frame, header_viewport, &column_widths, offset.x);
        }

        if visible.is_empty() {
            self.render_empty_state(frame, body_area);
            self.scroll
                .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            self.render_popup(frame, area, ctx);
            return;
        }

        let last_line = offset.y.saturating_add(geometry.viewport.height);
        let row_geometry = self.visible_row_geometry_for_viewport(&column_widths);
        for (line_index, row_start, row_end) in row_geometry.intersecting(offset.y, last_line) {
            let row = &visible[line_index];
            let clipped_start = row_start.max(offset.y);
            let clipped_end = row_end.min(last_line);
            if clipped_start >= clipped_end {
                continue;
            }
            let y = body_area.y + clipped_start.saturating_sub(offset.y) as u16;
            let row_area = Rect::new(
                geometry.layout.viewport.x,
                y,
                geometry.layout.viewport.width,
                clipped_end.saturating_sub(clipped_start) as u16,
            );
            let highlighted =
                matches!(row, DisplayRow::Data(row) if highlighted_id.as_ref() == Some(&row.id));
            let row_style = match row {
                DisplayRow::Data(row) => {
                    self.row_style(highlighted, row, &selection_descendants, base_row_style)
                }
                DisplayRow::SelectionPlaceholder { focused, .. } => Some(if *focused {
                    self.reorder_placeholder_style()
                } else {
                    self.selected_row_style()
                }),
            };
            frame.render_widget(
                Block::default().style(row_style.unwrap_or_default()),
                row_area,
            );
            match row {
                DisplayRow::Data(row) => self.render_row(
                    frame,
                    row_area,
                    &column_widths,
                    offset.x,
                    clipped_start.saturating_sub(row_start) as u16,
                    row,
                    highlighted,
                    row_style,
                    &selection_descendants,
                    show_tree_gutter,
                ),
                DisplayRow::SelectionPlaceholder {
                    count,
                    depth,
                    focused,
                } => self.render_selection_placeholder(
                    frame,
                    row_area,
                    &column_widths,
                    offset.x,
                    *count,
                    *depth,
                    *focused,
                    row_style,
                    show_tree_gutter,
                ),
            }
        }

        self.scroll
            .render_scrollbars(frame, geometry.layout, geometry.content, self.focused);

        self.render_popup(frame, area, ctx);
    }

    fn render_empty_state(&self, frame: &mut Frame, body_area: Rect) {
        let style = Style::default().fg(theme().muted_fg());
        let Some(empty_state) = self.empty_state.as_ref() else {
            frame.render_widget(
                Paragraph::new(self.empty_message.as_str()).style(style),
                body_area,
            );
            return;
        };
        empty_state.render_state(frame, body_area);
    }

    fn render_action_bar(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let bindings = keybindings();
        let data_keys = bindings.data_view();
        let (search_area, summary_area) = self.action_bar_areas(area);
        self.search_input.render(frame, search_area);
        let filters = self.filter_controls_enabled().then(|| {
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
        for (column, cell_area) in self.visible_columns().zip(cells) {
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
        clip_y: u16,
        row: &VisibleRow<'_, T, Id>,
        highlighted: bool,
        row_style: Option<Style>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
    ) {
        let cells = self.column_areas(area, column_widths, offset_x);
        for (column_index, (column, cell_area)) in self.visible_columns().zip(cells).enumerate() {
            let Some(cell_area) = cell_area else {
                continue;
            };
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
            let mut text = self.wrapped_cell_text(
                column_index,
                text,
                self.cell_content_width(column_index, column_widths),
                row,
                selection_descendants,
                show_tree_gutter,
            );
            text.lines = text
                .lines
                .into_iter()
                .map(|line| {
                    underline_search_matches(
                        line,
                        self.transform_state.search.trim(),
                        self.search_mode,
                    )
                })
                .collect();
            if (self.row_has_reorder_highlight(&row.id) || highlighted && self.focused)
                && !self.is_selection_disabled_for_row(row.row)
            {
                if let Some(foreground) = row_style.and_then(|style| style.fg) {
                    for line in &mut text.lines {
                        for span in &mut line.spans {
                            span.style = span.style.fg(foreground);
                        }
                    }
                }
            }
            let mut paragraph = Paragraph::new(text).scroll((clip_y, cell_area.scroll_x));
            if let Some(style) = row_style {
                paragraph = paragraph.style(style);
            }
            frame.render_widget(paragraph, cell_area.area);
        }
    }

    fn render_selection_placeholder(
        &self,
        frame: &mut Frame,
        area: Rect,
        column_widths: &[usize],
        offset_x: usize,
        count: usize,
        depth: usize,
        focused: bool,
        style: Option<Style>,
        show_tree_gutter: bool,
    ) {
        let cells = self.column_areas(area, column_widths, offset_x);
        let Some(Some(cell)) = cells.first() else {
            return;
        };
        let placeholder_area = Rect::new(
            cell.area.x,
            area.y,
            area.right().saturating_sub(cell.area.x),
            area.height,
        );
        let label = if focused {
            format!("Moving {count} tasks")
        } else {
            format!("{count} items selected")
        };
        let text =
            self.with_selection_placeholder_prefix(Text::from(label), depth, show_tree_gutter);
        let mut paragraph = Paragraph::new(text).scroll((0, cell.scroll_x));
        if let Some(style) = style {
            paragraph = paragraph.style(style);
        }
        frame.render_widget(paragraph, placeholder_area);
    }

    pub(super) fn with_row_prefix(
        &self,
        mut text: Text<'static>,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        show_tree_gutter: bool,
    ) -> Text<'static> {
        if text.lines.is_empty() {
            text.lines.push(Line::default());
        }
        let mut prefix = Vec::new();
        if show_tree_gutter {
            let indent = " ".repeat(
                row.depth
                    .saturating_mul(preset().data_view().tree_indent_width()),
            );
            prefix.push(Span::raw(indent));
            if row.has_children {
                let glyph = if row.expanded {
                    self.tree_glyphs.expanded
                } else {
                    self.tree_glyphs.collapsed
                };
                prefix.push(Span::raw(format!("{glyph} ")));
            } else {
                prefix.push(Span::raw(format!("{} ", self.tree_glyphs.leaf)));
            }
        }
        if self.displays_selection_glyphs() {
            let disabled = self.is_selection_disabled_for_row(row.row);
            let check_state =
                self.check_state_for_row_with_descendants(row.row, &row.id, selection_descendants);
            let glyph = if disabled {
                self.selection_disabled_glyph
            } else {
                self.selection_glyphs.glyph(check_state)
            };
            let content = format!("{glyph} ");
            prefix.push(if disabled {
                Span::styled(content, Style::default().fg(theme().muted_fg()))
            } else {
                Span::raw(content)
            });
        }
        let gutter_width = prefix
            .iter()
            .map(|span| line_width(&Line::from(span.clone())))
            .sum();
        for (index, line) in text.lines.iter_mut().enumerate() {
            let mut spans = if index == 0 {
                prefix.clone()
            } else {
                vec![Span::raw(" ".repeat(gutter_width))]
            };
            spans.append(&mut line.spans);
            line.spans = spans;
        }
        text
    }

    fn with_selection_placeholder_prefix(
        &self,
        mut text: Text<'static>,
        depth: usize,
        show_tree_gutter: bool,
    ) -> Text<'static> {
        if !show_tree_gutter {
            return text;
        }
        if text.lines.is_empty() {
            text.lines.push(Line::default());
        }
        let prefix = format!(
            "{}{} ",
            " ".repeat(depth.saturating_mul(preset().data_view().tree_indent_width())),
            self.tree_glyphs.leaf
        );
        let prefix_width = line_width(&Line::from(prefix.as_str()));
        for (index, line) in text.lines.iter_mut().enumerate() {
            let mut spans = if index == 0 {
                vec![Span::raw(prefix.clone())]
            } else {
                vec![Span::raw(" ".repeat(prefix_width))]
            };
            spans.append(&mut line.spans);
            line.spans = spans;
        }
        text
    }

    #[cfg(test)]
    pub(super) fn selection_glyph(&self, row: &VisibleRow<'_, T, Id>) -> &'static str {
        let descendants = self.selection_descendants_by_id();
        self.selection_glyph_for_row_with_descendants(row.row, &row.id, &descendants)
    }

    fn row_style(
        &self,
        highlighted: bool,
        row: &VisibleRow<'_, T, Id>,
        selection_descendants: &HashMap<Id, Vec<Id>>,
        base_row_style: Option<Style>,
    ) -> Option<Style> {
        let custom_style = self
            .row_style_by
            .as_ref()
            .and_then(|style_fn| style_fn(row.row));

        let effective_base = match (base_row_style, custom_style) {
            (Some(b), Some(c)) => Some(b.patch(c)),
            (None, Some(c)) => Some(c),
            (b, None) => b,
        };

        if self.row_has_reorder_highlight(&row.id) {
            Some(self.reorder_highlighted_row_style())
        } else if self
            .selection_overlay
            .as_ref()
            .is_some_and(|overlay| overlay.selected.contains(&row.id))
        {
            Some(self.selected_row_style())
        } else if highlighted && self.focused {
            Some(self.highlighted_row_style())
        } else if !self.displays_selection_glyphs()
            && self.row_is_visually_selected(&row.id, selection_descendants)
        {
            Some(self.selected_row_style())
        } else {
            effective_base
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

    fn reorder_highlighted_row_style(&self) -> Style {
        let theme = theme();
        self.reorder_highlighted_row_style_with_colors(theme.highlight_fg(), theme.highlight_bg())
    }

    fn reorder_placeholder_style(&self) -> Style {
        let theme = theme();
        Style::default()
            .fg(theme.highlight_bg())
            .bg(theme.highlight_fg())
            .add_modifier(Modifier::BOLD)
    }

    pub(super) fn reorder_highlighted_row_style_with_colors(
        &self,
        base_foreground: ratatui::style::Color,
        base_background: ratatui::style::Color,
    ) -> Style {
        let progress = self.reorder_highlight_progress();
        let foreground = lerp_color(base_foreground, base_background, progress);
        let background = lerp_color(base_background, base_foreground, progress);
        Style::default()
            .fg(foreground)
            .bg(background)
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
            .enumerate()
            .scan(0usize, |x, width| {
                let (index, width) = width;
                let width = (*width).min(u16::MAX as usize);
                let padding = if index + 1 == column_widths.len() {
                    0
                } else {
                    CELL_RIGHT_PADDING
                };
                let cell = Rect::new(
                    (*x).min(u16::MAX as usize) as u16,
                    viewport.y,
                    width.saturating_sub(padding) as u16,
                    viewport.height,
                );
                *x = x.saturating_add(width);
                Some(cell)
            })
            .map(|cell| clip_cell(cell, viewport, offset_x))
            .collect()
    }
}

fn underline_search_matches(
    line: Line<'static>,
    search: &str,
    search_mode: SearchMode,
) -> Line<'static> {
    if search.is_empty() {
        return line;
    }

    let Line {
        spans,
        style,
        alignment,
    } = line;
    let content = spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    let matches = search_highlight_spans(search, &content, search_mode);
    let mut span_offset = 0;
    Line {
        spans: spans
            .into_iter()
            .flat_map(|span| {
                let underlined = underline_span_matches(span, span_offset, &matches);
                span_offset += underlined
                    .iter()
                    .map(|span| span.content.len())
                    .sum::<usize>();
                underlined
            })
            .collect(),
        style,
        alignment,
    }
}

fn search_highlight_spans(search: &str, content: &str, search_mode: SearchMode) -> Vec<MatchSpan> {
    if search_mode == SearchMode::Fuzzy {
        return search_match(search, content, search_mode)
            .map(|matched| matched.spans)
            .unwrap_or_default();
    }

    let mut matches = Vec::new();
    let mut cursor = 0;
    while cursor < content.len() {
        let Some(matched) = search_match(search, &content[cursor..], search_mode) else {
            break;
        };
        let Some(span) = matched.spans.first() else {
            break;
        };
        matches.push(MatchSpan {
            start: cursor + span.start,
            end: cursor + span.end,
        });
        cursor += span.end;
    }
    matches
}

fn underline_span_matches(
    span: Span<'static>,
    span_offset: usize,
    matches: &[MatchSpan],
) -> Vec<Span<'static>> {
    let content = span.content.into_owned();
    let mut output = Vec::new();
    let mut cursor = 0;
    let span_end = span_offset + content.len();

    for matched in matches {
        let start = matched.start.max(span_offset);
        let end = matched.end.min(span_end);
        if start >= end {
            continue;
        }
        let start = start - span_offset;
        let end = end - span_offset;
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
