use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::event::{KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ColorTween, ScrollAxes, ScrollBehavior,
    ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize,
    ScrollState, TickResult, border_chars, border_set, line_width, paragraph_scroll, preset, theme,
};
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusTarget, LayoutCtx,
    LayoutResult, LifecycleCtx, TuiNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelVariant {
    #[default]
    Standard,
    InsetTitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelTitleStyle {
    #[default]
    Standard,
    Inset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTitlePosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PanelTitle {
    text: String,
    style: PanelTitleStyle,
}

#[derive(Debug, Clone)]
pub struct Panel {
    top_left: Option<PanelTitle>,
    top_right: Option<PanelTitle>,
    bottom_left: Option<PanelTitle>,
    bottom_right: Option<PanelTitle>,
    border: Option<BorderKind>,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    focused: bool,
    border_color: ColorTween,
    title_color: ColorTween,
    area: Rect,
}

pub struct PanelHost<C> {
    panel: Panel,
    child: C,
    child_area: Rect,
}

impl Default for Panel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel {
    pub fn new() -> Self {
        let theme = theme();
        Self {
            top_left: None,
            top_right: None,
            bottom_left: None,
            bottom_right: None,
            border: None,
            content: Vec::new(),
            scroll: None,
            focused: false,
            border_color: ColorTween::idle(theme.border_fg()),
            title_color: ColorTween::idle(theme.muted_fg()),
            area: Rect::default(),
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.top_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_top_left(&mut self, title: impl Into<String>) {
        self.top_left = Some(PanelTitle::standard(title));
    }

    pub fn top_left_style(mut self, style: PanelTitleStyle) -> Self {
        self.set_title_style(PanelTitlePosition::TopLeft, style);
        self
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.top_right = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_top_right(&mut self, title: impl Into<String>) {
        self.top_right = Some(PanelTitle::standard(title));
    }

    pub fn top_right_style(mut self, style: PanelTitleStyle) -> Self {
        self.set_title_style(PanelTitlePosition::TopRight, style);
        self
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.bottom_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_bottom_left(&mut self, title: impl Into<String>) {
        self.bottom_left = Some(PanelTitle::standard(title));
    }

    pub fn bottom_left_style(mut self, style: PanelTitleStyle) -> Self {
        self.set_title_style(PanelTitlePosition::BottomLeft, style);
        self
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.bottom_right = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_bottom_right(&mut self, title: impl Into<String>) {
        self.bottom_right = Some(PanelTitle::standard(title));
    }

    pub fn bottom_right_style(mut self, style: PanelTitleStyle) -> Self {
        self.set_title_style(PanelTitlePosition::BottomRight, style);
        self
    }

    pub fn title(
        mut self,
        position: PanelTitlePosition,
        title: impl Into<String>,
        style: PanelTitleStyle,
    ) -> Self {
        self.set_title(position, title, style);
        self
    }

    pub fn set_title(
        &mut self,
        position: PanelTitlePosition,
        title: impl Into<String>,
        style: PanelTitleStyle,
    ) {
        *self.title_slot_mut(position) = Some(PanelTitle {
            text: title.into(),
            style,
        });
    }

    pub fn clear_title(&mut self, position: PanelTitlePosition) {
        *self.title_slot_mut(position) = None;
    }

    pub fn set_title_style(&mut self, position: PanelTitlePosition, style: PanelTitleStyle) {
        if let Some(title) = self.title_slot_mut(position) {
            title.style = style;
        }
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn variant(mut self, variant: PanelVariant) -> Self {
        if variant == PanelVariant::InsetTitle {
            self.set_title_style(PanelTitlePosition::TopLeft, PanelTitleStyle::Inset);
        }
        self
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines.into_iter().map(Into::into).collect();
        self
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

    pub fn host<C>(self, child: C) -> PanelHost<C> {
        PanelHost {
            panel: self,
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
        let width = self
            .content
            .iter()
            .map(|line| line_width(&Line::from(line.as_str())))
            .max()
            .unwrap_or(0);
        ScrollSize::new(width, self.content.len())
    }

    pub fn scroll_geometry(&self, area: Rect) -> ScrollGeometry {
        let inner = Self::inner_area(area);
        let content = self.content_size();
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

    pub fn clamp_scroll(&mut self, area: Rect, settings: AnimationSettings) -> ScrollOutcome {
        let geometry = self.scroll_geometry(area);
        let Some(scroll) = &mut self.scroll else {
            return ScrollOutcome::idle();
        };
        scroll.clamp_to(geometry.viewport, geometry.content, settings)
    }

    pub fn inner_area(area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(1),
            area.y.saturating_add(1),
            area.width.saturating_sub(2),
            area.height.saturating_sub(2),
        )
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let border = self.border.unwrap_or_else(|| preset().border());
        let border_style = Style::default().fg(self.border_color.value());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_panel_title(frame, area, border, PanelTitlePosition::TopLeft);
        self.render_panel_title(frame, area, border, PanelTitlePosition::TopRight);
        self.render_panel_title(frame, area, border, PanelTitlePosition::BottomLeft);
        self.render_panel_title(frame, area, border, PanelTitlePosition::BottomRight);

        if !inner.is_empty() {
            let lines = self
                .content
                .iter()
                .map(|line| Line::from(line.clone()))
                .collect::<Vec<_>>();
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.content_size());
                let offset = scroll.offset();
                let lines = if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                    visible_lines(&self.content, offset, geometry.viewport)
                } else {
                    lines
                };
                let paragraph = Paragraph::new(lines).alignment(Alignment::Left).scroll(
                    if offset.x > u16::MAX as usize || offset.y > u16::MAX as usize {
                        (0, 0)
                    } else {
                        paragraph_scroll(offset)
                    },
                );
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
            }
        }
    }

    fn render_panel_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        border: BorderKind,
        position: PanelTitlePosition,
    ) {
        let Some(title) = self.title_slot(position) else {
            return;
        };
        match title.style {
            PanelTitleStyle::Standard => self.render_title(frame, area, title, position),
            PanelTitleStyle::Inset => self.render_inset_title(frame, area, border, title, position),
        }
    }

    fn render_title(
        &self,
        frame: &mut Frame,
        area: Rect,
        title: &PanelTitle,
        position: PanelTitlePosition,
    ) {
        if area.width <= 4 {
            return;
        }

        let max_width = area.width.saturating_sub(4) as usize;
        let title = bounded_title(&title.text, max_width);
        let width = line_width(&Line::from(title.as_str())).min(u16::MAX as usize) as u16;
        if width == 0 {
            return;
        }

        let x = match title_alignment(position) {
            Alignment::Left => area.x.saturating_add(2),
            Alignment::Center => area.x + area.width.saturating_sub(width) / 2,
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(2),
        };
        let y = title_y(area, position);
        let style = Style::default()
            .fg(self.title_color.value())
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
        title: &PanelTitle,
        position: PanelTitlePosition,
    ) {
        if area.width <= 4 {
            return;
        }

        let chars = border_chars(border);
        let border_style = Style::default().fg(self.border_color.value());
        let title_style = Style::default().fg(self.title_color.value());
        let title = bounded_title(&title.text, area.width.saturating_sub(5) as usize);
        let title_width = line_width(&Line::from(title.as_str())).min(area.width as usize);
        if title_width == 0 {
            return;
        }

        let line = Line::from(vec![
            Span::styled(chars.right_join, border_style),
            Span::styled(title, title_style),
            Span::styled(chars.left_join, border_style),
        ]);
        let width = (title_width + 2).min(u16::MAX as usize) as u16;
        let x = match title_alignment(position) {
            Alignment::Left | Alignment::Center => area.x.saturating_add(1),
            Alignment::Right => area.x + area.width.saturating_sub(width).saturating_sub(1),
        };
        let y = title_y(area, position);

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
    }
}

impl PanelTitle {
    fn standard(title: impl Into<String>) -> Self {
        Self {
            text: title.into(),
            style: PanelTitleStyle::Standard,
        }
    }
}

impl Panel {
    fn title_slot(&self, position: PanelTitlePosition) -> Option<&PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => self.top_left.as_ref(),
            PanelTitlePosition::TopRight => self.top_right.as_ref(),
            PanelTitlePosition::BottomLeft => self.bottom_left.as_ref(),
            PanelTitlePosition::BottomRight => self.bottom_right.as_ref(),
        }
    }

    fn title_slot_mut(&mut self, position: PanelTitlePosition) -> &mut Option<PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => &mut self.top_left,
            PanelTitlePosition::TopRight => &mut self.top_right,
            PanelTitlePosition::BottomLeft => &mut self.bottom_left,
            PanelTitlePosition::BottomRight => &mut self.bottom_right,
        }
    }
}

fn title_alignment(position: PanelTitlePosition) -> Alignment {
    match position {
        PanelTitlePosition::TopLeft | PanelTitlePosition::BottomLeft => Alignment::Left,
        PanelTitlePosition::TopRight | PanelTitlePosition::BottomRight => Alignment::Right,
    }
}

fn title_y(area: Rect, position: PanelTitlePosition) -> u16 {
    match position {
        PanelTitlePosition::TopLeft | PanelTitlePosition::TopRight => area.y,
        PanelTitlePosition::BottomLeft | PanelTitlePosition::BottomRight => {
            area.y + area.height.saturating_sub(1)
        }
    }
}

impl Animated for Panel {
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

impl<M> TuiNode<M> for Panel {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        ctx.register_focusable(FocusId::new("panel"), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
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

impl<C> PanelHost<C> {
    pub fn panel(&self) -> &Panel {
        &self.panel
    }

    pub fn panel_mut(&mut self) -> &mut Panel {
        &mut self.panel
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }

    pub fn child_area(&self) -> Rect {
        self.child_area
    }
}

impl<C, M> TuiNode<M> for PanelHost<C>
where
    C: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.panel.area = area;
        let inner = Panel::inner_area(area);
        self.child_area = inner;
        ctx.push_slot(ChildKey::body(), inner, |ctx| {
            self.child.layout(inner, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.panel.render(frame, area);
        self.child.render(frame, self.child_area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.panel.event(event, ctx)
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
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.panel, dt, settings).merge(self.child.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let body = ChildKey::body();
        if let Some(child_target) = target.for_child(&body) {
            self.panel.set_focused(focused, ctx.animation());
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

impl Panel {
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

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
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

fn visible_lines(
    lines: &[String],
    offset: ScrollOffset,
    viewport: ScrollSize,
) -> Vec<Line<'static>> {
    lines
        .iter()
        .skip(offset.y)
        .take(viewport.height)
        .map(|line| Line::from(trim_cells(line, offset.x, viewport.width)))
        .collect()
}

fn trim_cells(line: &str, skip: usize, width: usize) -> String {
    let end = skip.saturating_add(width);
    let mut cursor = 0;
    let mut trimmed = String::new();

    for ch in line.chars() {
        let ch_width = char_width(ch);
        let next = cursor + ch_width;
        if ch_width == 0 {
            if cursor >= skip && cursor <= end {
                trimmed.push(ch);
            }
        } else if cursor >= skip && next <= end {
            trimmed.push(ch);
        }
        cursor = next;
        if cursor >= end && ch_width > 0 {
            break;
        }
    }

    trimmed
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
    use ratatui::style::Color;

    use crate::{
        EventCtx, EventRoute, FocusCtx, Key, KeyEvent, LayoutCtx, ScrollbarConfig, ScrollbarGutter,
        ScrollbarStyle, ScrollbarVisibility, TuiEvent, TuiNode, animation_settings,
    };

    use super::super::TextInput;
    use super::*;

    #[test]
    fn empty_scrollable_panel_still_renders_scrollbars() {
        let mut panel = Panel::new();
        panel.scroll = Some(
            ScrollState::new(ScrollAxes::Both).scrollbars(ScrollbarConfig {
                vertical: ScrollbarVisibility::Always,
                horizontal: ScrollbarVisibility::Always,
                gutter: ScrollbarGutter::Reserve,
                style: ScrollbarStyle::ThinTrack,
            }),
        );
        let mut terminal = Terminal::new(TestBackend::new(6, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        assert_ne!(buffer.cell((4, 2)).unwrap().fg, Color::Reset);
    }

    #[test]
    fn clamp_scroll_clamps_offset_after_content_shrinks() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let area = Rect::new(0, 0, 10, 5);
        let mut panel = Panel::new()
            .content((0..20).map(|line| format!("line {line}")))
            .scrollable(ScrollAxes::Vertical);

        panel.scroll_to(ScrollOffset::new(0, 99), area, settings);
        panel.content = vec![String::from("line")];
        let outcome = panel.clamp_scroll(area, settings);

        assert!(outcome.changed);
        assert_eq!(
            panel.scroll.as_ref().unwrap().offset(),
            ScrollOffset::new(0, 0)
        );
    }

    #[test]
    fn handled_scroll_key_stops_propagation() {
        let mut panel = Panel::new()
            .content((0..20).map(|line| format!("line {line}")))
            .scrollable(ScrollAxes::Vertical);
        let area = Rect::new(0, 0, 10, 5);
        let mut layout = LayoutCtx::new();
        <Panel as TuiNode<()>>::layout(&mut panel, area, &mut layout);
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(&TuiEvent::Key(KeyEvent::from(crate::Key::Down)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn focus_changes_start_color_transitions() {
        let mut panel = Panel::new().focused(true);

        panel.set_focused(false, animation_settings());

        assert!(panel.border_color.is_active());
        assert!(panel.title_color.is_active());
    }

    #[test]
    fn focus_changes_snap_when_global_animations_are_disabled() {
        let mut animation = AnimationSettings::default();
        animation.enabled = false;

        let mut panel = Panel::new().focused(true);
        panel.start_focus_color_transition(false, animation);

        let theme = theme();
        assert_eq!(panel.border_color.value(), theme.border_fg());
        assert_eq!(panel.title_color.value(), theme.muted_fg());
        assert!(!panel.border_color.is_active());
        assert!(!panel.title_color.is_active());
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Msg {
        Submit(String),
    }

    #[test]
    fn panel_host_routes_focus_keys_submit_redraw_and_tick() {
        let mut host = Panel::new().top_left("Filter").host(
            TextInput::new()
                .placeholder("Search…")
                .on_submit(Msg::Submit),
        );
        let area = Rect::new(0, 0, 20, 3);
        let mut layout = LayoutCtx::new();

        host.layout(area, &mut layout);
        let target = layout.focus_targets()[0].clone();
        let route = EventRoute::new(target.path.clone());
        let mut focus = FocusCtx::new(AnimationSettings::default());
        host.dispatch_focus(&target, true, &mut focus);

        assert!(focus.redraw_requested());
        assert!(host.panel.border_color.is_active());

        let mut key = EventCtx::new(AnimationSettings::default());
        let outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            &mut key,
        );

        assert!(outcome.handled());
        assert_eq!(key.propagation(), crate::Propagation::Stopped);
        assert!(key.redraw_requested());
        assert_eq!(host.child().current_value(), "x");

        let mut submit = EventCtx::new(AnimationSettings::default());
        host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut submit,
        );

        assert_eq!(
            submit.drain_messages().collect::<Vec<_>>(),
            vec![Msg::Submit("x".into())]
        );
        assert!(submit.redraw_requested());
        assert!(
            TuiNode::tick(
                &mut host,
                Duration::from_millis(16),
                AnimationSettings::default()
            )
            .active
        );
    }

    #[test]
    fn inset_title_variant_embeds_title_in_top_border() {
        let panel = Panel::new()
            .top_left("Processes")
            .border(BorderKind::Plain)
            .variant(PanelVariant::InsetTitle)
            .content(["✖ No processes running"]);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let top = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(top, "┌┤ Processes ├─────────┐");
    }

    #[test]
    fn panel_renders_bottom_titles_with_independent_styles() {
        let panel = Panel::new()
            .bottom_left("Left")
            .bottom_right("Right")
            .bottom_right_style(PanelTitleStyle::Inset)
            .border(BorderKind::Plain);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..24)
            .map(|x| buffer.cell((x, 3)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(bottom, "└─ Left ──────┤ Right ├┘");
    }
}
