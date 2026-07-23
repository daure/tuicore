use std::hash::Hash;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use super::super::text_input::placeholder_line;
use super::util::{bounded_title, connected_popup_border_set};
use super::{
    DROPDOWN_ARROW_DOWN, DROPDOWN_ARROW_UP, Dropdown, DropdownLabelPosition, DropdownVariant,
};
use crate::{
    BorderKind, HotkeyLabelMode, OverlayLayer, border_set, hotkey_badge_width, hotkey_edge_spans,
    hotkey_label_spans, hotkey_underline_style, line_width, preset, theme,
};

impl<T, Id> Dropdown<T, Id>
where
    T: 'static,
    Id: Clone + Eq + Hash + 'static,
{
    pub fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        if area.is_empty() {
            return;
        }

        let field_area = self.field_area(area);
        if !self.open || self.show_field_when_open {
            self.render_field(frame, field_area);
        }
        if self.open {
            let bounds = self.overlay_bounds;
            ctx.push_portal(OverlayLayer::Popover, 0, bounds, |frame, bounds| {
                self.render_portal_popup(frame, bounds);
            });
        }
    }

    fn render_portal_popup(&self, frame: &mut Frame, bounds: Rect) {
        if !self.open || bounds.is_empty() {
            return;
        }

        let popup_area = self.popup_overlay_area(bounds);
        if !popup_area.is_empty() {
            let field_area = self.effective_field_area(bounds);
            let backdrop = self.backdrop_tween.value();
            if backdrop > 0.0 {
                super::super::dialog_layer::dim_backdrop_buffer_except(
                    frame,
                    bounds,
                    backdrop,
                    &[field_area, popup_area],
                );
            }
            if self.show_field_when_open {
                self.render_field(frame, field_area);
            }
            self.render_popup(frame, popup_area);
        }
    }

    pub fn render_field(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        if self.alt_style && self.label_position == DropdownLabelPosition::Top {
            let label_area = Rect::new(area.x, area.y, area.width, area.height.min(1));
            let mut text = String::new();
            if let Some(ref l) = self.label {
                text.push_str(l);
            }
            if let Some(ref h) = self.hotkey {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&format!("|{h}|"));
            }
            if !text.is_empty() && !label_area.is_empty() {
                let theme = theme();
                let style = Style::default()
                    .fg(if self.error {
                        theme.error_fg()
                    } else if self.chrome_is_active() {
                        theme.accent_fg()
                    } else {
                        theme.muted_fg()
                    })
                    .add_modifier(Modifier::BOLD);
                if let Some(ref h) = self.hotkey {
                    let label = self.label.clone().unwrap_or_default();
                    let spans = hotkey_label_spans(
                        &label,
                        Some(h.as_str()),
                        HotkeyLabelMode::Inline,
                        self.pending_hotkey_prefix.as_deref(),
                        style,
                        hotkey_underline_style(style),
                    );
                    frame.render_widget(Paragraph::new(Line::from(spans)), label_area);
                } else {
                    frame.render_widget(Paragraph::new(text).style(style), label_area);
                }
            }
            let dropdown_area = Rect::new(
                area.x,
                area.y.saturating_add(1),
                area.width,
                area.height.saturating_sub(1),
            );
            if !dropdown_area.is_empty() {
                match self.variant {
                    DropdownVariant::Bordered => self.render_bordered_field(frame, dropdown_area),
                    DropdownVariant::Filled => self.render_filled_field(frame, dropdown_area),
                }
            }
        } else {
            match self.variant {
                DropdownVariant::Bordered => self.render_bordered_field(frame, area),
                DropdownVariant::Filled => self.render_filled_field(frame, area),
            }
        }
    }

    fn render_bordered_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let border = preset().border();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(Style::default().fg(if self.error {
                theme.error_fg()
            } else if self.chrome_is_active() {
                theme.accent_fg()
            } else {
                theme.border_fg()
            }));
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let text_area = Rect::new(
            inner.x,
            inner.y,
            inner.width.saturating_sub(3),
            inner.height,
        );
        let text = if self.committed.is_empty() {
            let placeholder_style = Style::default().fg(theme.muted_fg());
            placeholder_line(
                &self.empty_summary(),
                None,
                text_area.width as usize,
                false,
                None,
                placeholder_style,
                placeholder_style,
            )
        } else {
            Line::from(Span::styled(
                self.selected_summary(),
                Style::default().fg(theme.text_fg()),
            ))
        };
        frame.render_widget(Paragraph::new(text), text_area);
        if inner.width > 0 {
            let arrow_area = Rect::new(inner.x + inner.width.saturating_sub(2), inner.y, 1, 1);
            frame.render_widget(
                Paragraph::new(self.dropdown_arrow())
                    .style(Style::default().fg(if self.chrome_is_active() {
                        theme.accent_fg()
                    } else {
                        theme.muted_fg()
                    }))
                    .alignment(Alignment::Right),
                arrow_area,
            );
        }

        if !self.alt_style {
            if let Some(ref label) = self.label {
                self.render_title(frame, area, label, Alignment::Left, area.y);
            }
            if let Some(ref hotkey) = self.hotkey {
                self.render_inset_title(
                    frame,
                    area,
                    border,
                    hotkey,
                    Alignment::Right,
                    area.y + area.height.saturating_sub(1),
                );
            }
        }
    }

    fn render_filled_field(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        let base_style = Style::default()
            .fg(theme.highlight_fg())
            .bg(theme.highlight_bg());
        let text_style = if self.committed.is_empty() {
            base_style.add_modifier(Modifier::DIM)
        } else {
            base_style
        };

        frame.render_widget(Paragraph::new("").style(base_style), area);

        let arrow_x = area.x + area.width.saturating_sub(2);
        let alt_trigger = self.alt_style;
        let inline_trigger = alt_trigger && self.label_position == DropdownLabelPosition::Inline;
        let text_area = if alt_trigger {
            Rect::new(area.x, area.y, area.width.saturating_sub(2), 1)
        } else {
            Rect::new(
                area.x.saturating_add(1),
                area.y,
                area.width.saturating_sub(3),
                1,
            )
        };
        if !text_area.is_empty() {
            let text = if inline_trigger {
                self.inline_filled_line(text_style)
            } else if self.committed.is_empty() && self.no_selection_text.is_some() {
                Line::from(Span::styled(self.empty_summary(), text_style))
            } else if self.committed.is_empty() {
                placeholder_line(
                    &self.empty_summary(),
                    None,
                    text_area.width as usize,
                    false,
                    None,
                    text_style,
                    text_style,
                )
            } else {
                Line::from(Span::styled(self.selected_summary(), text_style))
            };
            frame.render_widget(Paragraph::new(text), text_area);
        }

        let arrow_area = Rect::new(arrow_x, area.y, 1, 1);
        frame.render_widget(
            Paragraph::new(self.dropdown_arrow())
                .style(base_style)
                .alignment(Alignment::Right),
            arrow_area,
        );
    }

    fn dropdown_arrow(&self) -> &'static str {
        if self.open {
            DROPDOWN_ARROW_UP
        } else {
            DROPDOWN_ARROW_DOWN
        }
    }

    pub(super) fn render_popup(&self, frame: &mut Frame, area: Rect) {
        let theme = theme();
        frame.render_widget(Clear, area);
        let popup_content_style = self.popup_content_style();
        let inner = if self.popup_has_border() {
            let border = if self.variant == DropdownVariant::Bordered && !self.centered {
                connected_popup_border_set(preset().border())
            } else {
                border_set(preset().border())
            };
            let block = Block::default()
                .borders(Borders::ALL)
                .border_set(border)
                .border_style(Style::default().fg(if self.is_focused() {
                    theme.accent_fg()
                } else {
                    theme.border_fg()
                }));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            inner
        } else {
            frame.render_widget(
                Paragraph::new("").style(popup_content_style.unwrap_or_default()),
                area,
            );
            area
        };

        let search_height = u16::from(self.search_enabled());
        let [search_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(search_height), Constraint::Fill(1)])
            .areas(inner);
        if self.search_enabled() {
            if let Some(style) = popup_content_style {
                self.search_input
                    .render_with_style(frame, search_area, style);
            } else {
                self.search_input.render(frame, search_area);
            }
        }
        let no_selection_height = u16::from(self.show_no_selection_row());
        let no_selection_area = Rect::new(
            list_area.x,
            list_area.y,
            list_area.width,
            no_selection_height,
        );
        let rows_area = Rect::new(
            list_area.x,
            list_area.y.saturating_add(no_selection_height),
            list_area.width,
            list_area.height.saturating_sub(no_selection_height),
        );
        if self.show_no_selection_row() {
            self.render_no_selection_row(frame, no_selection_area, popup_content_style);
        }

        if self.filtered.is_empty() {
            let line = Line::styled(
                "No results",
                popup_content_style
                    .unwrap_or_default()
                    .fg(theme.muted_fg())
                    .add_modifier(Modifier::ITALIC),
            );
            frame.render_widget(
                Paragraph::new(line).style(popup_content_style.unwrap_or_default()),
                rows_area,
            );
        } else {
            self.data_view
                .render_with_row_style(frame, rows_area, popup_content_style);
        }
    }

    pub(super) fn inline_filled_line(&self, base_style: Style) -> Line<'static> {
        let mut spans = Vec::new();
        if let Some(label) = &self.label {
            spans.push(Span::styled(format!("{label}: "), base_style));
        }

        let value_style = if self.committed.is_empty() {
            base_style.add_modifier(Modifier::DIM)
        } else {
            base_style.add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(self.selected_summary(), value_style));

        if let Some(hotkey) = &self.hotkey {
            spans.extend(hotkey_label_spans(
                "",
                Some(hotkey.as_str()),
                HotkeyLabelMode::Inline,
                self.pending_hotkey_prefix.as_deref(),
                base_style,
                hotkey_underline_style(base_style),
            ));
        }

        Line::from(spans)
    }

    fn render_no_selection_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        popup_content_style: Option<Style>,
    ) {
        let Some(text) = &self.no_selection_text else {
            return;
        };
        if area.is_empty() {
            return;
        }

        let theme = theme();
        let style = if self.no_selection_highlighted {
            Style::default()
                .fg(theme.highlight_fg())
                .bg(theme.highlight_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            popup_content_style.unwrap_or_default().fg(theme.muted_fg())
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(text.clone(), style))).style(style),
            area,
        );
    }

    pub(super) fn popup_content_style(&self) -> Option<Style> {
        (self.variant == DropdownVariant::Filled).then(|| Style::default().bg(theme().surface_bg()))
    }

    fn render_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &str,
        alignment: Alignment,
        y: u16,
    ) {
        if area.width <= 4 {
            return;
        }

        let max_width = area.width.saturating_sub(4) as usize;
        let title = bounded_title(title, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let x = match alignment {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(2),
        };
        let theme = theme();
        let style = Style::default()
            .fg(if self.error {
                theme.error_fg()
            } else if self.chrome_is_active() {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            })
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(title, style))),
            Rect::new(x, y, width, 1),
        );
    }

    fn render_inset_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        border: BorderKind,
        title: &str,
        alignment: Alignment,
        y: u16,
    ) {
        if area.width <= 4 {
            return;
        }

        let theme = theme();
        let border_style = Style::default().fg(if self.error {
            theme.error_fg()
        } else if self.chrome_is_active() {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        let title_style = Style::default().fg(if self.error {
            theme.error_fg()
        } else if self.chrome_is_active() {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
        let width = hotkey_badge_width(title).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let line = Line::from(hotkey_edge_spans(
            title,
            self.pending_hotkey_prefix.as_deref(),
            border,
            border_style,
            title_style,
            hotkey_underline_style(title_style),
        ));
        let x = match alignment {
            Alignment::Left | Alignment::Center => area.x.saturating_add(1),
            Alignment::Right => area.x + area.width.saturating_sub(width),
        };

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }
}
