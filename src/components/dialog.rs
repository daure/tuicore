use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::event::{KeyEvent, TuiEvent};
use crate::{
    Animated, AnimationSettings, AnimationSpec, BorderKind, ChildKey, ColorTween, EventCtx,
    EventOutcome, EventRoute, FocusCtx, FocusId, FocusRequest, FocusTarget, HitRegion, KeySpec,
    LayoutCtx, LayoutResult, LifecycleCtx, ScrollAxes, ScrollBehavior, ScrollDelta, ScrollGeometry,
    ScrollLayout, ScrollOffset, ScrollOutcome, ScrollSize, ScrollState, TickResult, TreePath,
    TuiNode, Tween, border_set, hotkey_edge_spans, keybindings, lerp_color, line_width,
    paragraph_scroll, preset, theme,
};

const DIALOG_FOCUS: &str = "dialog";
const BACKDROP_BACKGROUND_DIM_FACTOR: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogTitlePosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogBackdrop {
    enabled: bool,
    amount: f64,
    animation: AnimationSpec,
}

impl DialogBackdrop {
    pub fn none() -> Self {
        Self {
            enabled: false,
            amount: 0.0,
            animation: AnimationSpec::default(),
        }
    }

    pub fn dim() -> Self {
        Self {
            enabled: true,
            amount: 0.45,
            animation: AnimationSpec::default(),
        }
    }

    pub fn amount(mut self, amount: f64) -> Self {
        self.amount = amount.clamp(0.0, 1.0);
        self
    }

    pub fn animation(mut self, animation: AnimationSpec) -> Self {
        self.animation = animation;
        self
    }
}

impl Default for DialogBackdrop {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DialogTitle {
    text: String,
}

pub struct Dialog<M = ()> {
    top_left: Option<DialogTitle>,
    top_right: Option<DialogTitle>,
    bottom_left: Option<DialogTitle>,
    bottom_right: Option<DialogTitle>,
    border: Option<BorderKind>,
    content: Vec<String>,
    scroll: Option<ScrollState>,
    on_close: Option<Box<dyn Fn(DialogCloseReason) -> M>>,
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

pub struct DialogLayer<Base, Layer> {
    base: Base,
    layer: Layer,
    active: bool,
    layer_percent: u16,
    base_rect: Rect,
    layer_rect: Rect,
    backdrop: DialogBackdrop,
    backdrop_tween: Tween,
    restore_focus_on_close: bool,
    layer_focus_origin: Option<(TreePath, FocusId)>,
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
            bottom_right: None,
            border: None,
            content: Vec::new(),
            scroll: None,
            on_close: None,
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

    pub fn bottom_right(mut self, title: impl Into<String>) -> Self {
        self.set_bottom_right(title);
        self
    }

    pub fn set_bottom_right(&mut self, title: impl Into<String>) {
        self.bottom_right = Some(DialogTitle::standard(title));
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

    pub fn border(mut self, border: BorderKind) -> Self {
        self.border = Some(border);
        self
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.content = lines.into_iter().map(Into::into).collect();
        self
    }

    pub fn set_content(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        self.content = lines.into_iter().map(Into::into).collect();
    }

    pub fn clear_content(&mut self) {
        self.content.clear();
    }

    pub fn on_close(mut self, handler: impl Fn(DialogCloseReason) -> M + 'static) -> Self {
        self.on_close = Some(Box::new(handler));
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

        frame.render_widget(Clear, area);
        let border = self.border.unwrap_or_else(|| preset().border());
        let border_style = Style::default().fg(self.visible_border_color());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border_set(border))
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        self.render_titles(frame, area, border);

        if !inner.is_empty() {
            let lines = self
                .content
                .iter()
                .map(|line| Line::from(line.clone()))
                .collect::<Vec<_>>();
            if let Some(scroll) = &self.scroll {
                let geometry = scroll.geometry(inner, self.content_size());
                let offset = scroll.offset();
                let paragraph = Paragraph::new(lines)
                    .alignment(Alignment::Left)
                    .scroll(paragraph_scroll(offset));
                frame.render_widget(paragraph, geometry.layout.viewport);
                scroll.render_scrollbars(frame, geometry.layout, geometry.content, self.focused);
            } else {
                frame.render_widget(Paragraph::new(lines).alignment(Alignment::Left), inner);
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
        self.render_top_left_title(frame, area);
        self.render_top_right_title(frame, area);
        self.render_bottom_title(frame, area, DialogTitlePosition::BottomLeft);
        self.render_bottom_title(frame, area, DialogTitlePosition::BottomRight);
        self.render_close_label(frame, area, border);
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

    fn render_bottom_title(&self, frame: &mut Frame, area: Rect, position: DialogTitlePosition) {
        let Some(title) = self.title_slot(position) else {
            return;
        };
        let alignment = match position {
            DialogTitlePosition::BottomLeft => Alignment::Left,
            DialogTitlePosition::BottomRight => Alignment::Right,
            DialogTitlePosition::TopLeft | DialogTitlePosition::TopRight => return,
        };
        self.render_plain_title(
            frame,
            area,
            title,
            alignment,
            area.y + area.height.saturating_sub(1),
            0,
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

    fn render_close_label(&self, frame: &mut Frame, area: Rect, border: BorderKind) {
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
        let line = Line::from(hotkey_edge_spans(
            &label,
            None,
            border,
            border_style,
            title_style,
            title_style,
        ));
        let x = area.x + area.width.saturating_sub(width);
        frame.render_widget(Paragraph::new(line), Rect::new(x, area.y, width, 1));
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
            DialogTitlePosition::BottomRight => self.bottom_right.as_ref(),
        }
    }

    fn title_slot_mut(&mut self, position: DialogTitlePosition) -> &mut Option<DialogTitle> {
        match position {
            DialogTitlePosition::TopLeft => &mut self.top_left,
            DialogTitlePosition::TopRight => &mut self.top_right,
            DialogTitlePosition::BottomLeft => &mut self.bottom_left,
            DialogTitlePosition::BottomRight => &mut self.bottom_right,
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
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.area = area;
        ctx.register_focusable(FocusId::new(DIALOG_FOCUS), area, true);
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        Self::render(self, frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let TuiEvent::Key(key) = event else {
            ctx.stop_propagation();
            return EventOutcome::Handled;
        };
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

impl<C, M> TuiNode<M> for DialogHost<C, M>
where
    C: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dialog.area = area;
        let inner = Dialog::<M>::inner_area(area);
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

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.dialog.render(frame, area);
        self.child.render(frame, self.child_area);
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
        if is_focus_unfocus_event(event) {
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

impl<Base, Layer> DialogLayer<Base, Layer> {
    pub fn new(base: Base, layer: Layer) -> Self {
        Self {
            base,
            layer,
            active: true,
            layer_percent: 100,
            base_rect: Rect::default(),
            layer_rect: Rect::default(),
            backdrop: DialogBackdrop::none(),
            backdrop_tween: Tween::idle(0.0),
            restore_focus_on_close: true,
            layer_focus_origin: None,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self.backdrop_tween
            .snap_to(if active { self.backdrop_target() } else { 0.0 });
        self
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        self.backdrop_tween
            .snap_to(if active { self.backdrop_target() } else { 0.0 });
    }

    pub fn set_active_with_settings(&mut self, active: bool, settings: AnimationSettings) {
        if self.active == active && !self.backdrop_tween.is_active() {
            return;
        }
        self.active = active;
        self.start_backdrop_tween(active, settings);
    }

    pub fn set_active_with_context<M>(&mut self, active: bool, ctx: &mut EventCtx<M>) {
        self.set_active_with_settings(active, ctx.animation());
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(self.focus_request_for_active_change(active));
    }

    pub fn set_active_with_dialog_focus<M>(&mut self, active: bool, ctx: &mut EventCtx<M>) {
        self.set_active_with_settings(active, ctx.animation());
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(if active {
            FocusRequest::Target(FocusId::new(DIALOG_FOCUS))
        } else {
            self.focus_request_for_active_change(false)
        });
    }

    fn focus_request_for_active_change(&mut self, active: bool) -> FocusRequest {
        if active {
            self.restore_focus_on_close = true;
            self.layer_focus_origin = None;
            FocusRequest::Next
        } else if self.restore_focus_on_close {
            FocusRequest::Last
        } else {
            FocusRequest::Next
        }
    }

    fn record_layer_focus(&mut self, target: &FocusTarget, focused: bool) {
        if !focused {
            return;
        }
        let current = (target.path.clone(), target.id.clone());
        if self.layer_focus_origin.is_none() {
            self.layer_focus_origin = Some(current);
        } else if self.layer_focus_origin.as_ref() != Some(&current) {
            self.restore_focus_on_close = false;
        }
    }

    pub fn layer_percent(mut self, percent: u16) -> Self {
        self.layer_percent = percent.clamp(1, 100);
        self
    }

    pub fn set_layer_percent(&mut self, percent: u16) {
        self.layer_percent = percent.clamp(1, 100);
    }

    pub fn backdrop(mut self, backdrop: DialogBackdrop) -> Self {
        self.backdrop = backdrop;
        self.backdrop_tween.snap_to(if self.active {
            self.backdrop_target()
        } else {
            0.0
        });
        self
    }

    pub fn set_backdrop(&mut self, backdrop: DialogBackdrop) {
        self.backdrop = backdrop;
        self.backdrop_tween.snap_to(if self.active {
            self.backdrop_target()
        } else {
            0.0
        });
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn base(&self) -> &Base {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    pub fn layer(&self) -> &Layer {
        &self.layer
    }

    pub fn layer_mut(&mut self) -> &mut Layer {
        &mut self.layer
    }

    fn backdrop_target(&self) -> f64 {
        if self.backdrop.enabled {
            self.backdrop.amount
        } else {
            0.0
        }
    }

    fn start_backdrop_tween(&mut self, active: bool, settings: AnimationSettings) {
        let target = if active { self.backdrop_target() } else { 0.0 };
        let resolved = settings.resolve(self.backdrop.animation);
        if !resolved.enabled || resolved.duration.is_zero() {
            self.backdrop_tween.snap_to(target);
        } else {
            self.backdrop_tween.start(
                self.backdrop_tween.value(),
                target,
                resolved.duration,
                resolved.easing,
            );
        }
    }
}

impl<Base, Layer, M> TuiNode<M> for DialogLayer<Base, Layer>
where
    Base: TuiNode<M>,
    Layer: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.base_rect = area;
        self.layer_rect = centered_percent_rect(area, self.layer_percent);

        if self.active {
            let was_disabled = ctx.focus_disabled();
            ctx.set_focus_disabled(true);
            ctx.push_slot(ChildKey::first(), self.base_rect, |ctx| {
                self.base.layout(self.base_rect, ctx);
            });
            ctx.set_focus_disabled(was_disabled);
            ctx.register_hit_region(HitRegion::new(ctx.current_path(), area));
            ctx.push_slot(ChildKey::second(), self.layer_rect, |ctx| {
                self.layer.layout(self.layer_rect, ctx);
            });
        } else {
            ctx.push_slot(ChildKey::first(), self.base_rect, |ctx| {
                self.base.layout(self.base_rect, ctx);
            });
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        self.base.render(frame, self.base_rect);
        let dim = self.backdrop_tween.value();
        if dim > 0.0 {
            dim_backdrop_buffer(frame, self.base_rect, dim);
        }
        if self.active {
            self.layer.render(frame, self.layer_rect);
        }
    }

    fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.active {
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            EventOutcome::Ignored
        }
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
        let first = ChildKey::first();
        if !self.active {
            if let Some(route) = route.path.without_first_if(&first).map(EventRoute::new) {
                return self.base.dispatch_event(&route, event, ctx);
            }
        }
        let second = ChildKey::second();
        if self.active {
            if let Some(route) = route.path.without_first_if(&second).map(EventRoute::new) {
                return self.layer.dispatch_event(&route, event, ctx);
            }
        }
        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let base = self.base.tick(dt, settings);
        let backdrop = self.backdrop_tween.tick(dt, settings);
        if self.active {
            base.merge(self.layer.tick(dt, settings)).merge(backdrop)
        } else {
            base.merge(backdrop)
        }
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let first = ChildKey::first();
        if let Some(target) = target.for_child(&first) {
            self.base.dispatch_focus(&target, focused, ctx);
            return;
        }
        let second = ChildKey::second();
        if self.active {
            if let Some(target) = target.for_child(&second) {
                self.record_layer_focus(&target, focused);
                self.layer.dispatch_focus(&target, focused, ctx);
            }
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.base.init(ctx);
        self.layer.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.base.mount(ctx);
        self.layer.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.layer.unmount(ctx);
        self.base.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.layer.destroy(ctx);
        self.base.destroy(ctx);
    }
}

fn focus_color_animation() -> AnimationSpec {
    AnimationSpec::default()
}

pub(crate) fn dim_backdrop_buffer(frame: &mut Frame, area: Rect, amount: f64) {
    dim_backdrop_buffer_except(frame, area, amount, &[]);
}

pub(crate) fn dim_backdrop_buffer_except(
    frame: &mut Frame,
    area: Rect,
    amount: f64,
    excluded: &[Rect],
) {
    let theme = theme();
    let fallback_fg = theme.text_fg();
    let fallback_bg = theme.background_bg();
    let amount = amount.clamp(0.0, 1.0);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if excluded.iter().any(|rect| rect_contains(*rect, x, y)) {
                continue;
            }
            let cell = &mut frame.buffer_mut()[(x, y)];
            let bg = if cell.bg == Color::Reset {
                fallback_bg
            } else {
                cell.bg
            };
            let dimmed_bg = blend_cell_color(
                bg,
                fallback_bg,
                fallback_bg,
                amount * BACKDROP_BACKGROUND_DIM_FACTOR,
            );
            let target = if cell.bg == Color::Reset {
                fallback_bg
            } else {
                dimmed_bg
            };
            let fg = blend_cell_color(cell.fg, fallback_fg, target, amount);
            cell.set_fg(fg);
            if cell.bg != Color::Reset {
                cell.set_bg(dimmed_bg);
            }
            cell.modifier.insert(Modifier::DIM);
        }
    }
}

fn rect_contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
}

fn blend_cell_color(color: Color, fallback: Color, backdrop: Color, amount: f64) -> Color {
    let color = if color == Color::Reset {
        fallback
    } else {
        color
    };
    if matches!(color, Color::Rgb(_, _, _)) && matches!(backdrop, Color::Rgb(_, _, _)) {
        lerp_color(color, backdrop, amount)
    } else {
        color
    }
}

fn close_label_width(label: &str) -> u16 {
    line_width(&Line::from(format!("┤{label}│"))).min(u16::MAX as usize) as u16
}

fn centered_percent_rect(area: Rect, percent: u16) -> Rect {
    let percent = percent.clamp(1, 100);
    let width = scaled_dimension(area.width, percent).max((area.width > 0) as u16);
    let height = scaled_dimension(area.height, percent).max((area.height > 0) as u16);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn scaled_dimension(value: u16, percent: u16) -> u16 {
    ((value as u32 * percent as u32) / 100).min(u16::MAX as u32) as u16
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
    use crate::{Button, FocusManager, Key, TextInput, TreePath, animation_settings};

    struct StaticBody;

    struct ColorBody;

    struct TopRightVerticalBody;

    struct BottomRightVerticalBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut ratatui::Frame, _area: Rect) {}
    }

    impl TuiNode<()> for ColorBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
            frame.buffer_mut().set_string(
                area.x,
                area.y,
                "B",
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(10, 20, 30)),
            );
        }
    }

    impl TuiNode<()> for TopRightVerticalBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
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

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
            frame.buffer_mut().set_string(
                area.right().saturating_sub(1),
                area.bottom().saturating_sub(1),
                "│",
                Style::default(),
            );
        }
    }

    #[test]
    fn backdrop_dim_softens_background_and_fades_foreground_to_it() {
        let base = ColorBody;
        let layer = StaticBody;
        let mut dialog_layer =
            DialogLayer::new(base, layer).backdrop(DialogBackdrop::dim().amount(1.0));
        let mut layout = LayoutCtx::new();
        dialog_layer.layout(Rect::new(0, 0, 10, 4), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(10, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog_layer.render(frame, frame.area()))
            .expect("dialog layer should render");

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        let expected_bg = lerp_color(
            Color::Rgb(10, 20, 30),
            theme().background_bg(),
            BACKDROP_BACKGROUND_DIM_FACTOR,
        );
        assert_eq!(cell.bg, expected_bg);
        assert_eq!(cell.fg, expected_bg);
    }

    #[test]
    fn set_backdrop_snaps_active_layer_to_new_amount() {
        let base = StaticBody;
        let layer = StaticBody;
        let mut dialog_layer = DialogLayer::new(base, layer);

        dialog_layer.set_backdrop(DialogBackdrop::dim().amount(0.7));

        assert_eq!(dialog_layer.backdrop_tween.value(), 0.7);
    }

    #[test]
    fn set_active_with_context_requests_layout_redraw_and_focus() {
        let mut dialog_layer = DialogLayer::new(StaticBody, StaticBody).active(false);
        let mut ctx = EventCtx::<()>::default();

        dialog_layer.set_active_with_context(true, &mut ctx);

        assert!(dialog_layer.is_active());
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Next));
    }

    #[test]
    fn set_active_with_dialog_focus_targets_dialog_chrome() {
        let mut dialog_layer = DialogLayer::new(StaticBody, StaticBody).active(false);
        let mut ctx = EventCtx::<()>::default();

        dialog_layer.set_active_with_dialog_focus(true, &mut ctx);

        assert_eq!(
            ctx.focus_request(),
            Some(&FocusRequest::Target(FocusId::new(DIALOG_FOCUS)))
        );
    }

    #[test]
    fn closing_immediately_restores_previous_focus() {
        let mut dialog_layer = DialogLayer::new(StaticBody, StaticBody).active(false);
        let mut ctx = EventCtx::<()>::default();

        dialog_layer.set_active_with_context(true, &mut ctx);
        dialog_layer.set_active_with_context(false, &mut ctx);

        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Last));
    }

    #[test]
    fn closing_after_navigation_does_not_restore_previous_focus() {
        let mut dialog_layer = DialogLayer::new(StaticBody, StaticBody).active(false);
        let mut ctx = EventCtx::<()>::default();
        let focus_target = FocusTarget {
            id: FocusId::new("child"),
            path: TreePath::from_keys([ChildKey::second(), ChildKey::new("child")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
        };
        let other_focus_target = FocusTarget {
            id: FocusId::new("other"),
            path: TreePath::from_keys([ChildKey::second(), ChildKey::new("other")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
        };

        dialog_layer.set_active_with_context(true, &mut ctx);
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.dispatch_focus(&other_focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.set_active_with_context(false, &mut ctx);

        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Next));
    }

    #[test]
    fn repeated_initial_focus_still_restores_previous_focus_on_close() {
        let mut dialog_layer = DialogLayer::new(StaticBody, StaticBody).active(false);
        let mut ctx = EventCtx::<()>::default();
        let focus_target = FocusTarget {
            id: FocusId::new("child"),
            path: TreePath::from_keys([ChildKey::second(), ChildKey::new("child")]),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
        };

        dialog_layer.set_active_with_context(true, &mut ctx);
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.set_active_with_context(false, &mut ctx);

        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Last));
    }

    #[test]
    fn dialog_renders_all_title_slots_and_fixed_close_label() {
        let dialog = Dialog::<()>::new()
            .top_left("Title")
            .top_right("State")
            .bottom_left("Help")
            .bottom_right("Enter OK")
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
        assert!(rendered.contains("Enter OK"));
        assert!(rendered.contains("┤x│"));
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
    fn dialog_host_join_patch_does_not_overwrite_close_label() {
        let mut host = Dialog::<()>::new().host(TopRightVerticalBody);
        let mut layout = LayoutCtx::new();
        host.layout(Rect::new(0, 0, 20, 6), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(20, 6)).expect("terminal should build");

        terminal
            .draw(|frame| host.render(frame, frame.area()))
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
            .draw(|frame| host.render(frame, frame.area()))
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
    fn focused_text_input_receives_x_before_dialog_close_policy() {
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
        assert_eq!(host.child().current_value(), "x");
        assert!(ctx.messages().is_empty());
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
    }

    #[test]
    fn escape_bubbles_from_child_without_closing_dialog() {
        let mut host = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut layout = LayoutCtx::new();
        let area = Rect::new(0, 0, 24, 5);
        host.layout(area, &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome = host.dispatch_event(&route, &TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert!(ctx.messages().is_empty());
        assert_eq!(ctx.propagation(), crate::Propagation::Continue);
    }

    #[test]
    fn unfocus_from_direct_text_input_focuses_dialog_parent() {
        let base = Button::<()>::new("Base");
        let layer = Dialog::<()>::new().host(TextInput::<()>::new());
        let mut dialog_layer = DialogLayer::new(base, layer);
        let mut layout = LayoutCtx::new();
        let mut focus = FocusManager::new();

        dialog_layer.layout(Rect::new(0, 0, 24, 5), &mut layout);
        focus.validate(layout.focus_targets());
        assert_eq!(focus.current().unwrap().id.as_str(), "input");

        focus.apply_request(&FocusRequest::Unfocus, layout.focus_targets());

        let current = focus.current().unwrap();
        assert_eq!(current.id.as_str(), "dialog");
        assert_eq!(current.path, TreePath::default().child(ChildKey::second()));
    }

    #[test]
    fn active_dialog_layer_does_not_stop_focus_key_from_child_route() {
        let base = Button::<DialogCloseReason>::new("Base");
        let layer = Dialog::new()
            .on_close(|reason| reason)
            .host(TextInput::<DialogCloseReason>::new());
        let mut dialog_layer = DialogLayer::new(base, layer);
        let mut layout = LayoutCtx::new();
        dialog_layer.layout(Rect::new(0, 0, 24, 5), &mut layout);
        let route = EventRoute::new(layout.focus_targets()[0].path.clone());
        let mut ctx = EventCtx::new(animation_settings());

        let outcome =
            dialog_layer.dispatch_event(&route, &TuiEvent::Key(Key::Esc.into()), &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert_eq!(ctx.propagation(), crate::Propagation::Continue);
        assert!(ctx.messages().is_empty());
    }

    #[test]
    fn active_dialog_layer_disables_base_focus_targets() {
        let base = Button::<()>::new("Base").hotkey("b");
        let layer = Dialog::<()>::new().host(Button::new("Dialog"));
        let mut dialog_layer = DialogLayer::new(base, layer);
        let mut ctx = LayoutCtx::new();

        dialog_layer.layout(Rect::new(0, 0, 20, 5), &mut ctx);

        let targets = ctx.focus_targets();
        assert_eq!(targets.len(), 2);
        assert_eq!(
            targets[0].path,
            TreePath::default()
                .child(ChildKey::second())
                .child(ChildKey::body())
        );
        assert_eq!(targets[1].id.as_str(), "dialog");
        assert_eq!(
            targets[1].path,
            TreePath::default().child(ChildKey::second())
        );
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
