use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::event::{HotkeyEvent, Key, KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ColorTween, ScrollAxes, ScrollBehavior,
    ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize,
    ScrollState, TickResult, border_chars, border_set, hotkey_badge_width, hotkey_edge_spans,
    hotkey_sequence_to_event, hotkey_underline_style, line_width, paragraph_scroll, preset, theme,
};

const PANEL_FOCUS: &str = "panel";
use crate::{
    ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId, FocusTarget, HotkeyMatch,
    HotkeySequenceMatcher, LayoutCtx, LayoutResult, LifecycleCtx, TuiNode,
};

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
}

#[derive(Debug, Clone)]
pub struct Panel {
    top_left: Option<PanelTitle>,
    top_right: Option<PanelTitle>,
    bottom_left: Option<PanelTitle>,
    hotkey: Option<String>,
    hotkey_matcher: HotkeySequenceMatcher,
    border: Option<BorderKind>,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    focused: bool,
    border_color: ColorTween,
    title_color: ColorTween,
    area: Rect,
    pending_hotkey_prefix: Option<String>,
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
            hotkey: None,
            hotkey_matcher: HotkeySequenceMatcher::default(),
            border: None,
            content: Vec::new(),
            scroll: None,
            focused: false,
            border_color: ColorTween::idle(theme.border_fg()),
            title_color: ColorTween::idle(theme.muted_fg()),
            area: Rect::default(),
            pending_hotkey_prefix: None,
        }
    }

    pub fn top_left(mut self, title: impl Into<String>) -> Self {
        self.top_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_top_left(&mut self, title: impl Into<String>) {
        self.top_left = Some(PanelTitle::standard(title));
    }

    pub fn top_right(mut self, title: impl Into<String>) -> Self {
        self.top_right = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_top_right(&mut self, title: impl Into<String>) {
        self.top_right = Some(PanelTitle::standard(title));
    }

    pub fn bottom_left(mut self, title: impl Into<String>) -> Self {
        self.bottom_left = Some(PanelTitle::standard(title));
        self
    }

    pub fn set_bottom_left(&mut self, title: impl Into<String>) {
        self.bottom_left = Some(PanelTitle::standard(title));
    }

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.set_hotkey(title);
        self
    }

    pub fn set_bottom_right(&mut self, title: impl Into<String>) {
        self.set_hotkey(title);
    }

    pub fn title(mut self, position: PanelTitlePosition, title: impl Into<String>) -> Self {
        self.set_title(position, title);
        self
    }

    pub fn set_title(&mut self, position: PanelTitlePosition, title: impl Into<String>) {
        if position == PanelTitlePosition::BottomRight {
            self.set_hotkey(title);
        } else {
            *self.title_slot_mut(position) = Some(PanelTitle { text: title.into() });
        }
    }

    pub fn clear_title(&mut self, position: PanelTitlePosition) {
        if position == PanelTitlePosition::BottomRight {
            self.clear_hotkey();
        } else {
            *self.title_slot_mut(position) = None;
        }
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.set_hotkey(hotkey);
        self
    }

    pub fn set_hotkey(&mut self, hotkey: impl Into<String>) {
        let hotkey = hotkey.into();
        self.hotkey = Some(hotkey.clone());
        self.hotkey_matcher = HotkeySequenceMatcher::new([hotkey]);
    }

    pub fn clear_hotkey(&mut self) {
        self.hotkey = None;
        self.hotkey_matcher = HotkeySequenceMatcher::default();
    }

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
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
        let border_style = Style::default().fg(self.visible_border_color());

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_panel_title(frame, area, border, PanelTitlePosition::TopLeft);
        self.render_panel_title(frame, area, border, PanelTitlePosition::TopRight);
        self.render_panel_title(frame, area, border, PanelTitlePosition::BottomLeft);
        self.render_hotkey(frame, area, border);

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
        match position {
            PanelTitlePosition::TopLeft | PanelTitlePosition::TopRight => {
                self.render_title(frame, area, title, position)
            }
            PanelTitlePosition::BottomLeft | PanelTitlePosition::BottomRight => {
                self.render_inset_title(frame, area, border, title, position)
            }
        }
    }

    fn render_hotkey(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
        let Some(ref hotkey) = self.hotkey else {
            return;
        };
        if area.width <= 4 {
            return;
        }

        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default().fg(self.visible_title_color());
        let width = hotkey_badge_width(hotkey).min(u16::MAX as usize) as u16;
        let x = area.x + area.width.saturating_sub(width);
        let y = title_y(area, PanelTitlePosition::BottomRight);
        let line = Line::from(hotkey_edge_spans(
            hotkey,
            self.pending_hotkey_prefix.as_deref(),
            border,
            border_style,
            title_style,
            hotkey_underline_style(title_style),
        ));

        frame.render_widget(Paragraph::new(line), Rect::new(x, y, width, 1));
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
            .fg(self.visible_title_color())
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
        let border_style = Style::default().fg(self.visible_border_color());
        let title_style = Style::default().fg(self.visible_title_color());
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
        Self { text: title.into() }
    }
}

impl Panel {
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

    fn title_slot(&self, position: PanelTitlePosition) -> Option<&PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => self.top_left.as_ref(),
            PanelTitlePosition::TopRight => self.top_right.as_ref(),
            PanelTitlePosition::BottomLeft => self.bottom_left.as_ref(),
            PanelTitlePosition::BottomRight => None,
        }
    }

    fn title_slot_mut(&mut self, position: PanelTitlePosition) -> &mut Option<PanelTitle> {
        match position {
            PanelTitlePosition::TopLeft => &mut self.top_left,
            PanelTitlePosition::TopRight => &mut self.top_right,
            PanelTitlePosition::BottomLeft => &mut self.bottom_left,
            PanelTitlePosition::BottomRight => {
                panic!("bottom-right panel slot is reserved for hotkeys")
            }
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
        let hotkey_tick = if self.hotkey_matcher.tick(dt) {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        let scroll = self
            .scroll
            .as_mut()
            .map(|scroll| scroll.tick(dt, settings))
            .unwrap_or(TickResult::IDLE);

        scroll
            .merge(self.border_color.tick(dt, settings))
            .merge(self.title_color.tick(dt, settings))
            .merge(hotkey_tick)
    }
}

impl<M> TuiNode<M> for Panel {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        if let Some(hotkey) = self.hotkey.clone() {
            ctx.register_focusable_with_hotkey_sequences(
                FocusId::new(PANEL_FOCUS),
                area,
                true,
                vec![hotkey],
            );
        } else {
            ctx.register_focusable(FocusId::new(PANEL_FOCUS), area, true);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if let TuiEvent::Hotkey(hotkey) = event {
            return self.on_hotkey_event(hotkey, ctx);
        }
        let TuiEvent::Key(key) = event else {
            return EventOutcome::Ignored;
        };
        if let Some(outcome) = self.handle_hotkey_key(*key, ctx) {
            return outcome;
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
        let fallback_inserted = if let Some(hotkey) = self.panel.hotkey.clone() {
            ctx.with_focus_fallback_hotkey_sequence_status(
                FocusId::new(PANEL_FOCUS),
                area,
                hotkey,
                |ctx| {
                    ctx.push_slot(ChildKey::body(), inner, |ctx| {
                        self.child.layout(inner, ctx);
                    });
                },
            )
            .1
        } else {
            ctx.with_focus_fallback_status(FocusId::new(PANEL_FOCUS), area, |ctx| {
                ctx.push_slot(ChildKey::body(), inner, |ctx| {
                    self.child.layout(inner, ctx);
                });
            })
            .1
        };
        if !fallback_inserted {
            ctx.register_focusable(FocusId::new(PANEL_FOCUS), area, true);
        }
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

        if let TuiEvent::Key(key) = event {
            if let Some(outcome) = self.panel.handle_hotkey_key(*key, ctx) {
                return outcome;
            }
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
        if target.path.is_empty() && target.id.as_str() == PANEL_FOCUS {
            self.panel.set_focused(focused, ctx.animation());
            ctx.request_redraw();
            return;
        }

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

    fn hotkey_event(&self) -> Option<KeyEvent> {
        self.hotkey.as_deref().and_then(hotkey_sequence_to_event)
    }

    fn hotkey_matches(&self, key: KeyEvent) -> bool {
        self.hotkey_event()
            .is_some_and(|hotkey| panel_hotkey_matches(hotkey, key))
    }

    fn handle_hotkey_key<M>(
        &mut self,
        key: KeyEvent,
        ctx: &mut EventCtx<M>,
    ) -> Option<EventOutcome> {
        match self.hotkey_matcher.on_key(key) {
            HotkeyMatch::Matched(_) | HotkeyMatch::Pending | HotkeyMatch::Canceled => {
                ctx.stop_propagation();
                return Some(EventOutcome::Handled);
            }
            HotkeyMatch::Ignored => {}
        }

        if self.hotkey_matches(key) {
            ctx.stop_propagation();
            Some(EventOutcome::Handled)
        } else {
            None
        }
    }

    fn on_hotkey_event<M>(&mut self, hotkey: &HotkeyEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        match hotkey {
            HotkeyEvent::Pending(prefix) => {
                if self.hotkey_has_prefix(prefix) {
                    self.pending_hotkey_prefix = Some(prefix.clone());
                    ctx.request_redraw();
                }
                EventOutcome::Ignored
            }
            HotkeyEvent::Canceled => {
                if self.pending_hotkey_prefix.take().is_some() {
                    ctx.request_redraw();
                }
                EventOutcome::Ignored
            }
            HotkeyEvent::Commit(sequence) => {
                self.pending_hotkey_prefix = None;
                if self.hotkey_matches_sequence(sequence) {
                    ctx.request_redraw();
                    ctx.stop_propagation();
                    EventOutcome::Handled
                } else {
                    EventOutcome::Ignored
                }
            }
        }
    }

    fn hotkey_has_prefix(&self, prefix: &str) -> bool {
        self.hotkey.as_deref().is_some_and(|hotkey| {
            crate::hotkey::normalize_hotkey(hotkey)
                .starts_with(&crate::hotkey::normalize_hotkey(prefix))
        })
    }

    fn hotkey_matches_sequence(&self, sequence: &str) -> bool {
        self.hotkey.as_deref().is_some_and(|hotkey| {
            crate::hotkey::normalize_hotkey(hotkey) == crate::hotkey::normalize_hotkey(sequence)
        })
    }
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

fn panel_hotkey_matches(hotkey: KeyEvent, key: KeyEvent) -> bool {
    if hotkey.modifiers != key.modifiers {
        return false;
    }
    match (hotkey.code, key.code) {
        (Key::Char(a), Key::Char(b)) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
        (a, b) => a == b,
    }
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
        EventCtx, EventRoute, Flex, FlexItem, FocusCtx, FocusManager, Key, KeyEvent, LayoutCtx,
        ScrollbarConfig, ScrollbarGutter, ScrollbarStyle, ScrollbarVisibility, TreePath, TuiEvent,
        TuiNode, animation_settings,
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

    #[test]
    fn render_uses_current_theme_even_without_focus_change() {
        let _lock = crate::ENV_LOCK.lock().expect("test env lock should lock");
        let original = theme();
        crate::set_theme(crate::Theme::named(crate::ThemeName::Vercel));
        let panel = Panel::new();
        crate::set_theme(crate::Theme::named(crate::ThemeName::Dracula));
        let expected = theme().border_fg();
        let mut terminal = Terminal::new(TestBackend::new(12, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        assert_eq!(
            terminal.backend().buffer().cell((0, 0)).unwrap().fg,
            expected
        );
        crate::set_theme(original);
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Msg {
        Submit(String),
    }

    struct StaticBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut ratatui::Frame, _area: Rect) {}
    }

    #[test]
    fn panel_host_registers_fallback_focus_when_child_has_none() {
        let mut host = Panel::new().top_left("Preview").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 1);
        let target = layout.focus_targets()[0].clone();
        assert_eq!(target.id.as_str(), "panel");
        assert!(target.path.is_empty());

        let mut focus = FocusCtx::new(AnimationSettings::default());
        host.dispatch_focus(&target, true, &mut focus);

        assert!(host.panel().is_focused());
        assert!(focus.redraw_requested());
    }

    #[test]
    fn panel_host_preserves_hotkey_on_fallback_focus() {
        let mut host = Panel::new().hotkey("p").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(
            layout.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
    }

    #[test]
    fn panel_host_attaches_hotkey_to_child_focus_target() {
        let mut host = Panel::new().hotkey("p").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 2);
        assert_eq!(
            layout.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
        assert_eq!(
            layout.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::body()])
        );
        assert_eq!(layout.focus_targets()[1].id.as_str(), "panel");
        assert!(layout.focus_targets()[1].path.is_empty());
    }

    #[test]
    fn panel_host_fallback_siblings_traverse_once_each() {
        let mut flex = Flex::row()
            .child("first", Panel::new().host(StaticBody), FlexItem::fixed(10))
            .child("second", Panel::new().host(StaticBody), FlexItem::fixed(10));
        let mut layout = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 20, 4), &mut layout);

        assert_eq!(layout.focus_targets().len(), 2);
        let mut focus = FocusManager::new();
        focus.validate(layout.focus_targets());
        assert_eq!(
            focus.current().unwrap().path,
            TreePath::from_keys([ChildKey::new("first")])
        );

        focus.next(layout.focus_targets());

        assert_eq!(
            focus.current().unwrap().path,
            TreePath::from_keys([ChildKey::new("second")])
        );
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
    fn panel_host_hotkey_is_consumed_before_child_input() {
        let mut host = Panel::new().hotkey("p").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::<()>::default();

        let outcome = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('p'))),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(host.child().current_value(), "");
    }

    #[test]
    fn panel_host_multiletter_hotkey_is_consumed_before_child_input() {
        let mut host = Panel::new().hotkey("pa").host(TextInput::<()>::new());
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::<()>::default();

        let pending = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('p'))),
            &mut ctx,
        );
        let matched = host.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('a'))),
            &mut ctx,
        );

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(host.child().current_value(), "");
    }

    #[test]
    fn focused_panel_multiletter_hotkey_is_consumed_from_key_events() {
        let mut panel = Panel::new().hotkey("pa");
        let mut ctx = EventCtx::<()>::default();

        let pending = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('p'))), &mut ctx);
        let matched = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('a'))), &mut ctx);

        assert_eq!(pending, EventOutcome::Handled);
        assert_eq!(matched, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn top_titles_always_render_standard() {
        let panel = Panel::new()
            .top_left("Processes")
            .border(BorderKind::Plain)
            .content(["✖ No processes running"]);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let top = (0..24)
            .map(|x| buffer.cell((x, 0)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(top, "┌─ Processes ──────────┐");
    }

    #[test]
    fn bottom_left_title_and_bottom_right_hotkey_render_inset() {
        let panel = Panel::new()
            .bottom_left("Left")
            .hotkey("r")
            .border(BorderKind::Plain);
        let mut terminal = Terminal::new(TestBackend::new(24, 4)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let bottom = (0..24)
            .map(|x| buffer.cell((x, 3)).unwrap().symbol())
            .collect::<String>();
        assert_eq!(bottom, "└┤ Left ├────────────┤r│");
    }

    #[test]
    fn panel_bottom_right_hotkey_aligns_with_border_snapshot() {
        let panel = Panel::new()
            .top_left("Services")
            .bottom_right("Ready")
            .hotkey("run")
            .border(BorderKind::Plain)
            .content(["Body"]);
        let mut terminal = Terminal::new(TestBackend::new(36, 5)).expect("terminal should build");

        terminal
            .draw(|frame| panel.render(frame, frame.area()))
            .expect("panel should render");

        let buffer = terminal.backend().buffer();
        let rendered = (0..5)
            .map(|y| {
                (0..36)
                    .map(|x| buffer.cell((x, y)).unwrap().symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        let expected = [
            "┌─ Services ───────────────────────┐",
            "│Body                              │",
            "│                                  │",
            "│                                  │",
            "└──────────────────────────────┤run│",
        ]
        .join("\n");
        assert_eq!(rendered, expected);
    }

    #[test]
    fn panel_registers_hotkey_with_focus_target() {
        let mut panel = Panel::new().hotkey("p");
        let mut ctx = LayoutCtx::new();

        <Panel as TuiNode<()>>::layout(&mut panel, Rect::new(0, 0, 20, 4), &mut ctx);

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(Key::Char('p')))
        );
    }

    #[test]
    fn panel_hotkey_event_is_consumed_when_focused() {
        let mut panel = Panel::new().hotkey("p");
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(&TuiEvent::Key(KeyEvent::from(Key::Char('p'))), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn panel_hotkey_pending_prefix_is_render_state() {
        let mut panel = Panel::new().hotkey("pa");
        let mut ctx = EventCtx::<()>::default();

        let outcome = panel.event(
            &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
            &mut ctx,
        );

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(panel.pending_hotkey_prefix.as_deref(), Some("p"));
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn panel_host_hotkey_commit_clears_pending_prefix_and_stops_propagation() {
        let mut host = Panel::new().hotkey("pa").host(StaticBody);
        let mut layout = LayoutCtx::new();

        host.layout(Rect::new(0, 0, 20, 3), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut pending = EventCtx::<()>::default();
        host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Pending("p".into())),
            &mut pending,
        );

        let mut commit = EventCtx::<()>::default();
        let outcome = host.dispatch_event(
            &route,
            &TuiEvent::Hotkey(HotkeyEvent::Commit("pa".into())),
            &mut commit,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(host.panel().pending_hotkey_prefix, None);
        assert_eq!(commit.propagation(), crate::Propagation::Stopped);
        assert!(commit.redraw_requested());
    }
}
