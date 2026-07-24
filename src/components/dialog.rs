use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::event::{KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, AxisProposal, BorderKind, ChildKey, ColorTween,
    EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusRequest, FocusTarget, KeySpec,
    LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, Padding, ScrollAxes,
    ScrollBehavior, ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset, ScrollOutcome,
    ScrollSize, ScrollState, TickResult, TuiNode, border_set, keybindings, line_width,
    paragraph_scroll, preset, theme,
};

use super::dialog_layer::DockChrome;
use super::typography::wrapped_text_line_count;

pub(crate) const DIALOG_FOCUS: &str = "dialog";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTitlePosition {
    TopLeft,
    TopRight,
    BottomLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogCloseReason {
    CloseKey,
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogKeyBindings {
    pub close: Vec<KeySpec>,
}

impl Default for DialogKeyBindings {
    fn default() -> Self {
        Self {
            close: vec![KeySpec::plain('x')],
        }
    }
}

impl DialogKeyBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn close_label(&self) -> Option<String> {
        self.close.first().map(|key| key.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DialogTitle {
    text: String,
}

pub struct DialogAction<M = ()> {
    label: String,
    hotkey: Option<KeySpec>,
    on_trigger: Option<Box<dyn Fn() -> M>>,
}

pub struct Dialog<M = ()> {
    top_left: Option<DialogTitle>,
    top_right: Option<DialogTitle>,
    bottom_left: Option<DialogTitle>,
    actions: Vec<DialogAction<M>>,
    border: Option<BorderKind>,
    edge_borders: Option<Borders>,
    content_padding: Padding,
    content: Vec<Line<'static>>,
    scroll: Option<ScrollState>,
    on_close: Option<Box<dyn Fn(DialogCloseReason) -> M>>,
    close_on_unfocus_from_descendants: bool,
    focused: bool,
    border_color: ColorTween,
    title_color: ColorTween,
    area: Rect,
    keys: DialogKeyBindings,
}

pub struct DialogHost<C, M = ()> {
    dialog: Dialog<M>,
    child: C,
    child_area: Rect,
}

impl<M> Default for Dialog<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Dialog<M> {
    pub fn new() -> Self {
        let theme = theme();
        Self {
            top_left: None,
            top_right: None,
            bottom_left: None,
            actions: Vec::new(),
            border: None,
            edge_borders: None,
            content_padding: Padding::default(),
            content: Vec::new(),
            scroll: None,
            on_close: None,
            close_on_unfocus_from_descendants: false,
            focused: false,
            border_color: ColorTween::idle(theme.border_fg()),
            title_color: ColorTween::idle(theme.muted_fg()),
            area: Rect::default(),
            keys: DialogKeyBindings::default(),
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.set_top_left(title);
        self
    }

    pub fn set_top_left(&mut self, title: impl Into<String>) {
        self.top_left = Some(DialogTitle::standard(title));
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.set_top_right(title);
        self
    }

    pub fn set_top_right(&mut self, title: impl Into<String>) {
        self.top_right = Some(DialogTitle::standard(title));
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.set_bottom_left(title);
        self
    }

    pub fn set_bottom_left(&mut self, title: impl Into<String>) {
        self.bottom_left = Some(DialogTitle::standard(title));
    }

    pub fn title(mut self, position: DialogTitlePosition, title: impl Into<String>) -> Self {
        self.set_title(position, title);
        self
    }

    pub fn set_title(&mut self, position: DialogTitlePosition, title: impl Into<String>) {
        *self.title_slot_mut(position) = Some(DialogTitle::standard(title));
    }

    pub fn clear_title(&mut self, position: DialogTitlePosition) {
        *self.title_slot_mut(position) = None;
    }

    pub fn actions(mut self, actions: impl IntoIterator<Item = DialogAction<M>>) -> Self {
        self.set_actions(actions);
        self
    }

    pub fn set_actions(&mut self, actions: impl IntoIterator<Item = DialogAction<M>>) {
        self.actions = actions.into_iter().collect();
    }

    pub fn clear_actions(&mut self) {
        self.actions.clear();
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn edge_borders(mut self, borders: Borders) -> Self {
        self.edge_borders = Some(borders);
        self
    }

    pub fn set_edge_borders(&mut self, borders: Borders) {
        self.edge_borders = Some(borders);
    }

    pub fn clear_edge_borders(&mut self) {
        self.edge_borders = None;
    }

    pub fn content_padding(mut self, padding: Padding) -> Self {
        self.set_content_padding(padding);
        self
    }

    pub fn set_content_padding(&mut self, padding: Padding) {
        self.content_padding = padding;
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines
            .into_iter()
            .map(|line| Line::from(line.into()))
            .collect();
        self
    }

    pub fn set_content(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        self.content = lines
            .into_iter()
            .map(|line| Line::from(line.into()))
            .collect();
    }

    pub(crate) fn set_content_lines(&mut self, lines: impl IntoIterator<Item = Line<'static>>) {
        self.content = lines.into_iter().collect();
    }

    pub fn clear_content(&mut self) {
        self.content.clear();
    }

    pub fn on_close(mut self, handler: impl Fn(DialogCloseReason) -> M + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    pub fn close_on_unfocus_from_descendants(mut self, close: bool) -> Self {
        self.close_on_unfocus_from_descendants = close;
        self
    }

    pub fn keybindings(mut self, keys: DialogKeyBindings) -> Self {
        self.keys = keys;
        self
    }

    pub fn set_keybindings(&mut self, keys: DialogKeyBindings) {
        self.keys = keys;
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        let theme = theme();
        self.border_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        self.title_color.snap_to(if focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
        self
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.start_focus_color_transition(focused, settings);
    }

    pub fn host<C>(self, child: C) -> DialogHost<C, M> {
        DialogHost {
            dialog: self,
            child,
            child_area: Rect::default(),
        }
    }

    pub fn scrollable(mut self, axes: ScrollAxes) -> Self {
        self.scroll = Some(match self.scroll.take() {
            Some(scroll) => scroll.with_axes(axes),
            None => ScrollState::from_preset(axes, preset().scroll()),
        });
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        if let Some(scroll) = self.scroll.take() {
            self.scroll = Some(scroll.behavior(behavior));
        } else {
            self.scroll = Some(
                ScrollState::from_preset(ScrollAxes::Vertical, preset().scroll())
                    .behavior(behavior),
            );
        }
        self
    }

    pub fn content_size(&self) -> ScrollSize {
        let width = self.content.iter().map(line_width).max().unwrap_or(0);
        ScrollSize::new(width, self.content.len())
    }

    fn wrapped_content_size(&self, width: u16) -> ScrollSize {
        if width == 0 {
            return ScrollSize::new(0, 0);
        }
        let height = self
            .content
            .iter()
            .map(|line| wrapped_text_line_count(&line_text(line), width, usize::MAX))
            .sum();
        ScrollSize::new(width as usize, height)
    }

    fn natural_width(&self) -> u16 {
        let borders = self.resolved_edge_borders();
        let full = Rect::new(0, 0, u16::MAX, 1);
        let inner = Self::inner_area_for(full, borders);
        let border_width = full.width.saturating_sub(inner.width);
        let content_width = self.content_size().width.min(u16::MAX as usize) as u16;
        let padding_width = self
            .content_padding
            .left
            .saturating_add(self.content_padding.right);
        let mut width = content_width
            .saturating_add(padding_width)
            .saturating_add(border_width);
        let close_width = self.close_label_width();

        if borders.contains(Borders::TOP) {
            width = width.max(chrome_row_width(
                self.top_left.as_ref().map(title_width),
                self.top_right.as_ref().map(title_width),
                close_width.saturating_add(1),
            ));
        }
        if borders.contains(Borders::BOTTOM) {
            let close_on_bottom = !borders.contains(Borders::TOP);
            width = width.max(chrome_row_width(
                self.bottom_left.as_ref().map(title_width),
                actions_width(&self.actions),
                if close_on_bottom {
                    close_width.saturating_add(1)
                } else {
                    0
                },
            ));
        }
        width
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let inner = self.content_area_for(area, self.resolved_edge_borders());
        let content = self.wrapped_content_size(inner.width);
        if let Some(scroll) = &self.scroll {
            scroll.geometry(inner, content)
        } else {
            let layout = ScrollLayout {
                outer: inner,
                viewport: inner,
                vertical_bar: None,
                horizontal_bar: None,
                corner: None,
            };
            ScrollGeometry {
                layout,
                viewport: ScrollSize::from_area(inner),
                content,
            }
        }
    }

    pub fn on_key(
        &mut self,
        key: impl Into<KeyEvent>,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let key = key.into();
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.on_key(key, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_by(
        &mut self,
        delta: ScrollDelta,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_by(delta, geometry.viewport, geometry.content, settings)
    }

    pub fn scroll_to(
        &mut self,
        offset: ScrollOffset,
        area: Rect,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.scroll_to(offset, geometry.viewport, geometry.content, settings)
    }

    pub fn inner_area(area: Rect) -> Rect {
        Self::inner_area_for(area, Borders::ALL)
    }

    fn resolved_edge_borders(&self) -> Borders {
        self.edge_borders.unwrap_or(Borders::ALL)
    }

    fn inner_area_for(area: Rect, borders: Borders) -> Rect {
        let left_edge_dock = !borders.contains(Borders::TOP)
            && !borders.contains(Borders::BOTTOM)
            && borders.contains(Borders::LEFT);
        let right_edge_dock = !borders.contains(Borders::TOP)
            && !borders.contains(Borders::BOTTOM)
            && borders.contains(Borders::RIGHT);
        let left = if left_edge_dock {
            2
        } else {
            borders.contains(Borders::LEFT) as u16
        };
        let right = if right_edge_dock {
            2
        } else {
            borders.contains(Borders::RIGHT) as u16
        };
        let top = borders.contains(Borders::TOP) as u16;
        let bottom = borders.contains(Borders::BOTTOM) as u16;
        Rect::new(
            area.x.saturating_add(left),
            area.y.saturating_add(top),
            area.width.saturating_sub(left.saturating_add(right)),
            area.height.saturating_sub(top.saturating_add(bottom)),
        )
    }

    fn content_area_for(&self, area: Rect, borders: Borders) -> Rect {
        let inner = Self::inner_area_for(area, borders);
        Rect::new(
            inner.x.saturating_add(self.content_padding.left),
            inner.y.saturating_add(self.content_padding.top),
            inner.width.saturating_sub(
                self.content_padding
                    .left
                    .saturating_add(self.content_padding.right),
            ),
            inner.height.saturating_sub(
                self.content_padding
                    .top
                    .saturating_add(self.content_padding.bottom),
            ),
        )
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.render_with_lines(frame, area, self.content.clone());
    }

    pub(crate) fn render_with_content_lines(
        &self,
        frame: &mut Frame,
        area: Rect,
        lines: Vec<Line<'static>>,
    ) {
        self.render_with_lines(frame, area, lines);
    }

    fn render_with_lines(&self, frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
        if area.is_empty() {
            return;
        }

        frame.render_widget(Clear, area);
        let border = self.border.unwrap_or_else(|| preset().border());
        let edge_borders = self.resolved_edge_borders();
        let border_style = Style::default().fg(self.visible_border_color());
        let block = Block::default()
            .borders(edge_borders)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = self.content_area_for(area, edge_borders);
        frame.render_widget(block, area);

        self.render_titles(frame, area, border);

        if !inner.is_empty() {
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.wrapped_content_size(inner.width));
                let offset = scroll.offset();
                let paragraph = Paragraph::new(lines)
                    .alignment(Alignment::Left)
                    .wrap(Wrap { trim: true })
                    .scroll(paragraph_scroll(offset));
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(
                    Paragraph::new(lines)
                        .alignment(Alignment::Left)
                        .wrap(Wrap { trim: true }),
                    inner,
                );
            }
        }
    }

    fn close(&self, reason: DialogCloseReason, ctx: &mut EventCtx<M>) {
        if let Some(on_close) = &self.on_close {
            ctx.emit(on_close(reason));
        }
        ctx.focus(FocusRequest::Last);
        ctx.stop_propagation();
        ctx.request_redraw();
        ctx.request_layout();
    }

    fn close_reason(&self, key: KeyEvent) -> Option<DialogCloseReason> {
        if matches_any(&self.keys.close, key) {
            Some(DialogCloseReason::CloseKey)
        } else if keybindings().focus().unfocus_matches(key) {
            Some(DialogCloseReason::Escape)
        } else {
            None
        }
    }

    fn render_titles(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let edge_borders = self.resolved_edge_borders();
        let close_on_bottom =
            !edge_borders.contains(Borders::TOP) && edge_borders.contains(Borders::BOTTOM);
        let close_on_left = !edge_borders.contains(Borders::TOP)
            && !edge_borders.contains(Borders::BOTTOM)
            && edge_borders.contains(Borders::LEFT);
        let close_on_right = !edge_borders.contains(Borders::TOP)
            && !edge_borders.contains(Borders::BOTTOM)
            && edge_borders.contains(Borders::RIGHT);
        let horizontal_open_end = edge_borders == Borders::TOP || edge_borders == Borders::BOTTOM;
        if edge_borders.contains(Borders::TOP) {
            self.render_top_left_title(frame, area);
            self.render_top_right_title(frame, area);
            self.render_close_label(frame, area, area.y, border, horizontal_open_end);
        }
        if edge_borders.contains(Borders::BOTTOM) {
            self.render_bottom_title(frame, area, DialogTitlePosition::BottomLeft, 0);
            self.render_bottom_actions(
                frame,
                area,
                if close_on_bottom {
                    self.close_label_width() + 1
                } else {
                    0
                },
            );
            if close_on_bottom {
                self.render_close_label(
                    frame,
                    area,
                    area.y + area.height.saturating_sub(1),
                    border,
                    horizontal_open_end,
                );
            }
        }
        if close_on_left {
            self.render_left_close_label(frame, area, border);
        }
        if close_on_right {
            self.render_right_close_label(frame, area, border);
        }
    }

    fn render_top_left_title(&self, frame: &mut Frame, area: Rect) {
        let Some(title) = self.top_left.as_ref() else {
            return;
        };
        let close_width = self.close_label_width();
        self.render_plain_title(frame, area, title, Alignment::Left, area.y, close_width + 1);
    }

    fn render_top_right_title(&self, frame: &mut Frame, area: Rect) {
        let Some(title) = self.top_right.as_ref() else {
            return;
        };
        let close_width = self.close_label_width();
        self.render_plain_title(
            frame,
            area,
            title,
            Alignment::Right,
            area.y,
            close_width + 1,
        );
    }

    fn close_label_width(&self) -> u16 {
        self.keys
            .close_label()
            .map(|label| close_label_width(&label))
            .unwrap_or_default()
    }

    fn render_bottom_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        position: DialogTitlePosition,
        reserved_right: u16,
    ) {
        let Some(title) = self.title_slot(position) else {
            return;
        };
        let alignment = match position {
            DialogTitlePosition::BottomLeft => Alignment::Left,
            DialogTitlePosition::TopLeft | DialogTitlePosition::TopRight => return,
        };
        self.render_plain_title(
            frame,
            area,
            title,
            alignment,
            area.y + area.height.saturating_sub(1),
            reserved_right,
        );
    }

    fn render_bottom_actions(&self, frame: &mut Frame, area: Rect, reserved_right: u16) {
        if self.actions.is_empty() {
            return;
        }
        let title = DialogTitle {
            text: self
                .actions
                .iter()
                .map(DialogAction::display_label)
                .collect::<Vec<_>>()
                .join(" · "),
        };
        self.render_plain_title(
            frame,
            area,
            &title,
            Alignment::Right,
            area.y + area.height.saturating_sub(1),
            reserved_right,
        );
    }

    fn render_plain_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &DialogTitle,
        alignment: Alignment,
        y: u16,
        reserved_right: u16,
    ) {
        if area.width <= 4 + reserved_right {
            return;
        }
        let max_width = area.width.saturating_sub(4 + reserved_right) as usize;
        let title = bounded_title(&title.text, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }
        let x = match alignment {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x.saturating_add(
                area.width
                    .saturating_sub(width)
                    .saturating_sub(2 + reserved_right),
            ),
        };
        let style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(title, style))),
            Rect::new(x, y, width, 1),
        );
    }

    fn render_close_label(
        &self,
        frame: &mut Frame,
        area: Rect,
        y: u16,
        border: BorderKind,
        open_end: bool,
    ) {
        let Some(label) = self.keys.close_label() else {
            return;
        };
        let width = close_label_width(&label);
        if area.width <= width + 2 {
            return;
        }
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        let chars = crate::border_chars(border);
        let line = Line::from(vec![
            Span::styled(chars.right_join, border_style),
            Span::styled(label, title_style),
            Span::styled(
                if open_end {
                    chars.left_join
                } else {
                    chars.vertical
                },
                border_style,
            ),
        ]);
        let x = area.x + area.width.saturating_sub(width);
        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }

    fn render_left_close_label(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(label) = self.keys.close_label() else {
            return;
        };
        let label_width = line_width(&Line::from(label.as_str())).min(u16::MAX as usize) as u16;
        if area.height < 3 || area.width < label_width {
            return;
        }

        let chars = crate::border_chars(border);
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        let x = area.x;
        let y = area.bottom().saturating_sub(3);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.bottom_join, border_style))),
            Rect::new(x, y, 1, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, title_style))),
            Rect::new(x, y.saturating_add(1), label_width, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.top_join, border_style))),
            Rect::new(x, y.saturating_add(2), 1, 1),
        );
    }

    fn render_right_close_label(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(label) = self.keys.close_label() else {
            return;
        };
        let label_width = line_width(&Line::from(label.as_str())).min(u16::MAX as usize) as u16;
        if area.height < 3 || area.width < label_width {
            return;
        }

        let chars = crate::border_chars(border);
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default()
            .fg(self.visible_title_color())
            .add_modifier(Modifier::BOLD);
        let x = area.right().saturating_sub(1);
        let y = area.bottom().saturating_sub(3);
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.bottom_join, border_style))),
            Rect::new(x, y, 1, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(label, title_style))),
            Rect::new(x, y.saturating_add(1), label_width, 1),
        );
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(chars.top_join, border_style))),
            Rect::new(x, y.saturating_add(2), 1, 1),
        );
    }

    fn visible_border_color(&self) -> ratatui::style::Color {
        if self.border_color.is_active() {
            return self.border_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        }
    }

    fn visible_title_color(&self) -> ratatui::style::Color {
        if self.title_color.is_active() {
            return self.title_color.value();
        }

        let theme = theme();
        if self.focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        }
    }

    fn title_slot(&self, position: DialogTitlePosition) -> Option<&DialogTitle> {
        match position {
            DialogTitlePosition::TopLeft => self.top_left.as_ref(),
            DialogTitlePosition::TopRight => self.top_right.as_ref(),
            DialogTitlePosition::BottomLeft => self.bottom_left.as_ref(),
        }
    }

    fn title_slot_mut(&mut self, position: DialogTitlePosition) -> &mut Option<DialogTitle> {
        match position {
            DialogTitlePosition::TopLeft => &mut self.top_left,
            DialogTitlePosition::TopRight => &mut self.top_right,
            DialogTitlePosition::BottomLeft => &mut self.bottom_left,
        }
    }

    fn start_focus_color_transition(&mut self, focused: bool, settings: AnimationSettings) {
        let theme = theme();
        self.border_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.border_fg()
            },
            settings,
            focus_color_animation(),
        );
        self.title_color.start(
            if focused {
                theme.accent_fg()
            } else {
                theme.muted_fg()
            },
            settings,
            focus_color_animation(),
        );
    }
}

impl DialogTitle {
    fn standard(title: impl Into<String>) -> Self {
        Self { text: title.into() }
    }
}

impl<M> DialogAction<M> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hotkey: None,
            on_trigger: None,
        }
    }

    pub fn hotkey(mut self, hotkey: KeySpec) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    pub fn on_trigger(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.on_trigger = Some(Box::new(handler));
        self
    }

    fn display_label(&self) -> String {
        match self.hotkey {
            Some(hotkey) => format!("{} ({})", self.label, hotkey.label()),
            None => self.label.clone(),
        }
    }

    fn matches(&self, key: KeyEvent) -> bool {
        self.hotkey.is_some_and(|hotkey| hotkey.matches(key))
    }
}

impl<M> Animated for Dialog<M> {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let scroll = self
            .scroll
            .as_mut()
            .map(|scroll| scroll.tick(dt, settings))
            .unwrap_or(TickResult::IDLE);
        scroll
            .merge(self.border_color.tick(dt, settings))
            .merge(self.title_color.tick(dt, settings))
    }
}

impl<M> TuiNode<M> for Dialog<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let natural_width = self.natural_width();
        let measured_width = match proposal.width {
            AxisProposal::Unbounded => natural_width,
            AxisProposal::AtMost(max) => natural_width.min(max),
            AxisProposal::Exact(exact) => exact,
        };
        let borders = self.resolved_edge_borders();
        let inner = self.content_area_for(Rect::new(0, 0, measured_width, u16::MAX), borders);
        let content_height = self.wrapped_content_size(inner.width).height;
        let border_height =
            borders.contains(Borders::TOP) as usize + borders.contains(Borders::BOTTOM) as usize;
        let measured_height = content_height
            .saturating_add(border_height)
            .saturating_add(self.content_padding.top as usize)
            .saturating_add(self.content_padding.bottom as usize)
            .min(u16::MAX as usize) as u16;

        LayoutSizeHint::content(measured_width, measured_height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let width_changed = self.area.width != 0 && self.area.width != area.width;
        self.area = area;
        if width_changed && let Some(scroll) = &mut self.scroll {
            scroll.snap_horizontal_to_start();
        }
        ctx.register_focusable(FocusId::new(DIALOG_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            ctx.stop_propagation();
            return EventOutcome::Handled;
        };
        if let Some(action) = self.actions.iter().find(|action| action.matches(*key)) {
            if let Some(on_trigger) = &action.on_trigger {
                ctx.emit(on_trigger());
            }
            ctx.stop_propagation();
            ctx.request_redraw();
            return EventOutcome::Handled;
        }
        if let Some(reason) = self.close_reason(*key) {
            self.close(reason, ctx);
            return EventOutcome::Handled;
        }
        let outcome = self.on_key(*key, self.area, ctx.animation());
        if outcome.needs_redraw() {
            ctx.request_redraw();
        }
        if outcome.handled {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }
}

impl<C, M> DialogHost<C, M> {
    pub fn dialog(&self) -> &Dialog<M> {
        &self.dialog
    }

    pub fn dialog_mut(&mut self) -> &mut Dialog<M> {
        &mut self.dialog
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }
}

impl<M> DockChrome for Dialog<M> {
    fn set_dock_edge_borders(&mut self, borders: Borders) {
        self.set_edge_borders(borders);
    }
}

impl<C, M> DockChrome for DialogHost<C, M> {
    fn set_dock_edge_borders(&mut self, borders: Borders) {
        self.dialog.set_edge_borders(borders);
    }
}

impl<C, M> TuiNode<M> for DialogHost<C, M>
where
    C: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let borders = self.dialog.resolved_edge_borders();
        let full = Rect::new(0, 0, u16::MAX, u16::MAX);
        let content = self.dialog.content_area_for(full, borders);
        let horizontal_inset = full.width.saturating_sub(content.width);
        let vertical_inset = full.height.saturating_sub(content.height);
        let child = self.child.measure(LayoutProposal {
            width: inset_axis_proposal(proposal.width, horizontal_inset),
            height: inset_axis_proposal(proposal.height, vertical_inset),
        });
        let width = child
            .preferred
            .width
            .saturating_add(horizontal_inset)
            .max(self.dialog.natural_width());
        let height = child.preferred.height.saturating_add(vertical_inset);

        LayoutSizeHint::content(width, height).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let width_changed = self.dialog.area.width != 0 && self.dialog.area.width != area.width;
        self.dialog.area = area;
        if width_changed && let Some(scroll) = &mut self.dialog.scroll {
            scroll.snap_horizontal_to_start();
        }
        let inner = self
            .dialog
            .content_area_for(area, self.dialog.resolved_edge_borders());
        self.child_area = inner;
        let fallback_inserted = ctx
            .with_focus_fallback_status(FocusId::new(DIALOG_FOCUS), area, |ctx| {
                ctx.push_slot(ChildKey::body(), inner, |ctx| {
                    self.child.layout(inner, ctx);
                });
            })
            .1;
        if !fallback_inserted {
            ctx.register_focusable(FocusId::new(DIALOG_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        self.dialog.render(frame, area);
        self.child.render(frame, self.child_area, ctx);
        crate::separator::patch_border_joins(
            frame,
            area,
            self.child_area,
            self.dialog.border.unwrap_or_else(|| preset().border()),
            Style::default().fg(self.dialog.visible_border_color()),
        );
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.dialog.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            return self.event(event, ctx);
        }
        let body = ChildKey::body();
        let child = route
            .path
            .without_first_if(&body)
            .map(EventRoute::new)
            .map(|route| self.child.dispatch_event(&route, event, ctx))
            .unwrap_or(EventOutcome::Ignored);
        if is_focus_unfocus_event(event) && ctx.propagation() == crate::Propagation::Stopped {
            return child;
        }
        if is_focus_unfocus_event(event)
            && route.path.keys().len() > 1
            && !self.dialog.close_on_unfocus_from_descendants
        {
            return child;
        }
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.dialog, dt, settings).merge(self.child.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() && target.id.as_str() == DIALOG_FOCUS {
            self.dialog.set_focused(focused, ctx.animation());
            ctx.request_redraw();
            return;
        }
        let body = ChildKey::body();
        if let Some(child_target) = target.for_child(&body) {
            self.dialog.set_focused(focused, ctx.animation());
            self.child.dispatch_focus(&child_target, focused, ctx);
            ctx.request_redraw();
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.destroy(ctx);
    }
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn close_label_width(label: &str) -> u16 {
    line_width(&Line::from(format!("┤{label}├"))).min(u16::MAX as usize) as u16
}

fn inset_axis_proposal(proposal: AxisProposal, inset: u16) -> AxisProposal {
    match proposal {
        AxisProposal::Unbounded => AxisProposal::Unbounded,
        AxisProposal::AtMost(value) => AxisProposal::AtMost(value.saturating_sub(inset)),
        AxisProposal::Exact(value) => AxisProposal::Exact(value.saturating_sub(inset)),
    }
}

fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

fn title_width(title: &DialogTitle) -> u16 {
    line_width(&Line::from(format!(" {} ", title.text))).min(u16::MAX as usize) as u16
}

fn actions_width<M>(actions: &[DialogAction<M>]) -> Option<u16> {
    (!actions.is_empty()).then(|| {
        let labels = actions
            .iter()
            .map(DialogAction::display_label)
            .collect::<Vec<_>>()
            .join(" · ");
        line_width(&Line::from(format!(" {labels} "))).min(u16::MAX as usize) as u16
    })
}

fn chrome_row_width(left: Option<u16>, right: Option<u16>, reserved_right: u16) -> u16 {
    match (left, right) {
        (Some(left), Some(right)) => left
            .saturating_add(right)
            .saturating_add(reserved_right)
            .saturating_add(5),
        (Some(width), None) | (None, Some(width)) => {
            width.saturating_add(reserved_right).saturating_add(4)
        }
        (None, None) if reserved_right > 0 => reserved_right.saturating_add(3),
        (None, None) => 0,
    }
}

fn is_focus_unfocus_event(event: &TuiEvent) -> bool {
    let TuiEvent::Key(key) = event else {
        return false;
    };
    keybindings().focus().unfocus_matches(*key)
}

fn matches_any(bindings: &[KeySpec], key: KeyEvent) -> bool {
    bindings.iter().any(|binding| binding.matches(key))
}

fn bounded_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let mut value = format!(" {title} ");
    if line_width(&Line::from(value.as_str())) > max_width {
        value = truncate_cells(&value, max_width);
    }
    value
}

fn truncate_cells(value: &str, max_width: usize) -> String {
    let mut width = 0;
    let mut truncated = String::new();
    for ch in value.chars() {
        let ch_width = char_width(ch);
        if ch_width > 0 && width + ch_width > max_width {
            break;
        }
        width += ch_width;
        truncated.push(ch);
    }
    truncated
}

fn char_width(ch: char) -> usize {
    let mut value = String::new();
    value.push(ch);
    line_width(&Line::from(value))
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::{Key, TextInput, animation_settings};

    fn render_node<M>(node: &impl TuiNode<M>, frame: &mut ratatui::Frame, area: Rect) {
        let mut ctx = crate::RenderCtx::new();
        TuiNode::render(node, frame, area, &mut ctx);
        ctx.flush(frame);
    }

    struct StaticBody;

    struct TopRightVerticalBody;

    struct BottomRightVerticalBody;

    struct NestedFocusableBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(
            &self,
            _frame: &mut ratatui::Frame,
            _area: Rect,
            _ctx: &mut crate::RenderCtx<'_>,
        ) {
        }
    }

    impl TuiNode<()> for TopRightVerticalBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
            frame.buffer_mut().set_string(
                area.right().saturating_sub(1),
                area.y,
                "│",
                Style::default(),
            );
        }
    }

    impl TuiNode<()> for BottomRightVerticalBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
            frame.buffer_mut().set_string(
                area.right().saturating_sub(1),
                area.bottom().saturating_sub(1),
                "│",
                Style::default(),
            );
        }
    }

    impl NestedFocusableBody {
        fn new() -> Self {
            Self
        }
    }

    impl TuiNode<DialogCloseReason> for NestedFocusableBody {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("nested"), area, true);
            ctx.push_slot(ChildKey::new("input"), area, |ctx| {
                ctx.register_focusable(FocusId::new("input"), area, true);
            });
            LayoutResult::new(area)
        }

        fn render(
            &self,
            _frame: &mut ratatui::Frame,
            _area: Rect,
            _ctx: &mut crate::RenderCtx<'_>,
        ) {
        }

        fn dispatch_event(
            &mut self,
            _route: &EventRoute,
            _event: &TuiEvent,
            _ctx: &mut EventCtx<DialogCloseReason>,
        ) -> EventOutcome {
            EventOutcome::Ignored
        }

        fn focus(
            &mut self,
            _target: Option<&FocusId>,
            _focused: bool,
            _ctx: &mut FocusCtx<DialogCloseReason>,
        ) {
        }

        fn dispatch_focus(
            &mut self,
            target: &FocusTarget,
            focused: bool,
            ctx: &mut FocusCtx<DialogCloseReason>,
        ) {
            if target.path.is_empty() {
                self.focus(Some(&target.id), focused, ctx);
            }
        }
    }

    #[test]
    fn dialog_renders_titles_actions_and_fixed_close_label() {
        let dialog = Dialog::<()>::new()
            .top_left("Title")
            .top_right("State")
            .bottom_left("Help")
            .actions([
                DialogAction::new("Delete").hotkey(KeySpec::plain('d')),
                DialogAction::new("Keep").hotkey(KeySpec::plain('k')),
            ])
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .flat_map(|y| (0..40).map(move |x| buffer.cell((x, y)).unwrap().symbol()))
            .collect::<String>();
        assert!(rendered.contains("Title"));
        assert!(rendered.contains("State"));
        assert!(rendered.contains("Help"));
        assert!(rendered.contains("Delete (d) · Keep (k)"));
        assert!(rendered.contains("┤x│"));
    }

    #[test]
    fn long_dialog_content_wraps_at_word_boundaries() {
        let dialog = Dialog::<()>::new().content(["one two three four five six"]);
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..20)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect()
        };
        assert!(row(1).contains("one two three four"), "{}", row(1));
        assert!(row(2).contains("five six"), "{}", row(2));
    }

    #[test]
    fn content_padding_affects_measurement_and_rendered_body_area() {
        let dialog = Dialog::<()>::new()
            .content(["Body"])
            .content_padding(Padding::all(1));
        let size = dialog.measure(LayoutProposal::unbounded()).preferred;
        assert_eq!(size, crate::LayoutSize::new(8, 5));
        let mut terminal = Terminal::new(TestBackend::new(size.width, size.height))
            .expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((1, 1)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((1, 2)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((2, 2)).unwrap().symbol(), "B");
        assert_eq!(buffer.cell((1, 3)).unwrap().symbol(), " ");
    }

    #[test]
    fn narrower_dialog_proposals_increase_wrapped_height() {
        let dialog = Dialog::<()>::new().content(["one two three four five six"]);
        let wide = dialog.measure(LayoutProposal {
            width: AxisProposal::AtMost(30),
            height: AxisProposal::Unbounded,
        });
        let narrow = dialog.measure(LayoutProposal {
            width: AxisProposal::AtMost(14),
            height: AxisProposal::Unbounded,
        });

        assert_eq!(wide.preferred.height, 3);
        assert_eq!(narrow.preferred.height, 5);
    }

    #[test]
    fn dialog_measure_includes_content_titles_actions_and_borders() {
        let body = Dialog::<()>::new()
            .keybindings(DialogKeyBindings { close: Vec::new() })
            .content(["Body"]);
        let decorated = Dialog::<()>::new()
            .keybindings(DialogKeyBindings { close: Vec::new() })
            .top_left("Title")
            .actions([DialogAction::new("Confirm").hotkey(KeySpec::plain('c'))])
            .content(["Body"]);

        let body_hint = body.measure(LayoutProposal::unbounded());
        let decorated_hint = decorated.measure(LayoutProposal::unbounded());

        assert_eq!(body_hint.preferred, crate::LayoutSize::new(6, 3));
        assert_eq!(decorated_hint.preferred, crate::LayoutSize::new(17, 3));
    }

    #[test]
    fn dialog_measure_clamps_both_dimensions_to_proposal() {
        let dialog = Dialog::<()>::new().content([
            "This description is deliberately long enough to wrap over several narrow lines",
        ]);
        let hint = dialog.measure(LayoutProposal {
            width: AxisProposal::AtMost(12),
            height: AxisProposal::AtMost(4),
        });

        assert_eq!(hint.preferred, crate::LayoutSize::new(12, 4));
    }

    #[test]
    fn dialog_action_hotkey_is_optional() {
        let dialog = Dialog::<()>::new().actions([DialogAction::new("Dismiss")]);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..24)
            .map(|x| buffer.cell((x, 3)).unwrap().symbol())
            .collect::<String>();
        assert!(bottom.contains("Dismiss"), "{bottom}");
        assert!(!bottom.contains("Dismiss ("), "{bottom}");
    }

    #[test]
    fn dialog_top_right_close_hotkey_aligns_with_border_snapshot() {
        let dialog = Dialog::<()>::new()
            .top_left("Prompt")
            .top_right("Ready")
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(40, 5)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .map(|y| {
                (0..40)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "╭─ Prompt ───────────────── Ready ───┤x│",
            "│Body                                  │",
            "│                                      │",
            "│                                      │",
            "╰──────────────────────────────────────╯",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn dialog_can_hide_docked_touching_edges() {
        let dialog = Dialog::<()>::new()
            .edge_borders(Borders::BOTTOM)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(20, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let row = |y| -> String {
            (0..20)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert!(row(0).starts_with("Body"), "{}", row(0));
        assert!(!row(0).contains('─'), "{}", row(0));
        assert!(!row(0).starts_with('│'), "{}", row(0));
        assert!(!row(0).ends_with('│'), "{}", row(0));
        assert!(row(3).contains('─'), "{}", row(3));
    }

    #[test]
    fn dialog_left_edge_dock_renders_vertical_close_label() {
        let dialog = Dialog::<()>::new()
            .edge_borders(Borders::LEFT)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(8, 6)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let column = (0..6)
            .map(|y| buffer.cell((0, y)).unwrap().symbol())
            .collect::<String>();
        let close_row = (0..8)
            .map(|x| buffer.cell((x, 4)).unwrap().symbol())
            .collect::<String>();
        let content_row = (0..8)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();

        assert_eq!(column, "│││┴x┬");
        assert!(close_row.starts_with("x"), "{close_row}");
        assert!(content_row.starts_with("│ Body"), "{content_row}");
    }

    #[test]
    fn dialog_right_edge_dock_renders_vertical_close_label() {
        let dialog = Dialog::<()>::new()
            .edge_borders(Borders::RIGHT)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(8, 6)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let column = (0..6)
            .map(|y| buffer.cell((7, y)).unwrap().symbol())
            .collect::<String>();
        let content_row = (0..8)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();

        assert_eq!(column, "│││┴x┬");
        assert!(content_row.starts_with("Body"), "{content_row}");
        assert!(content_row.ends_with(" │"), "{content_row}");
    }

    #[test]
    fn partial_width_dialog_snackbar_uses_closed_close_label_end() {
        let dialog = Dialog::<()>::new()
            .edge_borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .top_left("Snackbar")
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(32, 6)).expect("terminal should build");

        terminal
            .draw(|frame| dialog.render(frame, frame.area()))
            .expect("dialog should render");

        let buffer = terminal.backend().buffer();
        let top = (0..32)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();

        assert!(top.ends_with("┤x│"), "{top}");
    }

    #[test]
    fn dialog_host_join_patch_does_not_overwrite_close_label() {
        let mut host = Dialog::<()>::new().host(TopRightVerticalBody);
        let mut layout = LayoutCtx::new();
        host.layout(Rect::new(0, 0, 20, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&host, frame, frame.area()))
            .expect("dialog host should render");

        let buffer = terminal.backend().buffer();
        let top_line = (0..20)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert!(top_line.ends_with("┤x│"), "{top_line}");
    }

    #[test]
    fn dialog_host_join_patch_does_not_join_scrollbar_to_bottom_border() {
        let mut host = Dialog::<()>::new().host(BottomRightVerticalBody);
        let mut layout = LayoutCtx::new();
        host.layout(Rect::new(0, 0, 20, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).expect("terminal should build");

        terminal
            .draw(|frame| render_node(&host, frame, frame.area()))
            .expect("dialog host should render");

        let buffer = terminal.backend().buffer();
        let bottom_line = (0..20)
            .map(|x| buffer.cell((x, 5)).unwrap().symbol())
            .collect::<String>();
        assert!(!bottom_line.contains('┴'), "{bottom_line}");
    }

    #[test]
    fn close_key_emits_close_message_and_stops_propagation() {
        let mut dialog = Dialog::new().on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = dialog.event(&TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::CloseKey]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn action_hotkey_emits_action_message_and_stops_propagation() {
        let mut dialog = Dialog::new().actions([
            DialogAction::new("Delete")
                .hotkey(KeySpec::plain('d'))
                .on_trigger(|| "delete"),
            DialogAction::new("Keep").on_trigger(|| "keep"),
        ]);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = dialog.event(&TuiEvent::Key(Key::Char('d').into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["delete"]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn custom_close_key_replaces_default_close_key() {
        let mut dialog = Dialog::new()
            .keybindings(DialogKeyBindings {
                close: vec![KeySpec::plain('q')],
            })
            .on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        assert_eq!(
            dialog.event(&TuiEvent::Key(Key::Char('x').into()), &mut ctx),
            EventOutcome::Ignored
        );
        assert_eq!(
            dialog.event(&TuiEvent::Key(Key::Char('q').into()), &mut ctx),
            EventOutcome::Handled
        );
        assert_eq!(ctx.messages(), &[DialogCloseReason::CloseKey]);
    }

    #[test]
    fn unfocus_key_closes_dialog_when_dialog_shell_is_focused() {
        let mut dialog = Dialog::new().on_close(|reason| reason);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = dialog.event(&TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn focused_text_input_receives_x_after_enter_before_dialog_close_policy() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let enter = host.dispatch_event(&route, &TuiEvent::Key(Key::Enter.into()), &mut ctx);
        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(enter, EventOutcome::Handled);
        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "x");
        assert!(ctx.messages().is_empty());
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn focused_text_input_bubbles_x_before_insert_mode_for_dialog_close_policy() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Char('x').into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(host.child().current_value(), "");
        assert_eq!(ctx.messages(), &[DialogCloseReason::CloseKey]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn escape_bubbles_from_child_and_closes_dialog() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn escape_from_nested_child_blurs_instead_of_closing_dialog() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(NestedFocusableBody::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route_path = layout
            .focus_targets()
            .iter()
            .find(|target| target.id.as_str() == "input")
            .expect("nested input should be focusable")
            .path
            .clone();
        assert_eq!(route_path.keys().len(), 2);
        let route = EventRoute::new(route_path);
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert!(ctx.messages().is_empty());
        assert_eq!(ctx.propagation(), crate::Propagation::Continue);
    }

    #[test]
    fn opted_in_dialog_closes_from_nested_focus_mode_on_unfocus_keys() {
        let keys = [
            KeyEvent::from(Key::Esc),
            KeyEvent {
                code: Key::Char('['),
                modifiers: crate::KeyModifiers::CONTROL,
            },
        ];

        for key in keys {
            let mut host = Dialog::new()
                .close_on_unfocus_from_descendants(true)
                .on_close(|reason| reason)
                .host(NestedFocusableBody::new());
            let mut layout = LayoutCtx::new();
            host.layout(Rect::new(0, 0, 24, 5), &mut layout);
            let route_path = layout
                .focus_targets()
                .iter()
                .find(|target| target.id.as_str() == "input")
                .expect("nested input should be focusable")
                .path
                .clone();
            let mut ctx = EventCtx::new(animation_settings());

            let outcome =
                host.dispatch_event(&EventRoute::new(route_path), &TuiEvent::Key(key), &mut ctx);

            assert_eq!(outcome, EventOutcome::Handled);
            assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
            assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        }
    }

    #[test]
    fn dialog_host_registers_single_fallback_focus_when_child_has_none() {
        let mut host = Dialog::<()>::new().host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 24, 5), &mut layout);

        assert_eq!(layout.focus_targets().len(), 1);
        assert_eq!(layout.focus_targets()[0].id.as_str(), "dialog");
        assert!(layout.focus_targets()[0].path.is_empty());
    }
}
