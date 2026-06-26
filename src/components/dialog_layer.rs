use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::widgets::Borders;

use crate::event::TuiEvent;
use crate::{
    AnimationSettings, AnimationSpec, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusId, FocusRequest, FocusTarget, HitRegion, LayoutCtx, LayoutResult, LifecycleCtx,
    TickResult, TreePath, TuiNode, Tween, lerp_color, theme,
};

use super::dialog::DIALOG_FOCUS;

const BACKDROP_BACKGROUND_DIM_FACTOR: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DialogBackdrop {
    enabled: bool,
    amount: f64,
    animation: AnimationSpec,
}

pub struct DialogLayer<Base, Layer> {
    base: Base,
    layer: Layer,
    active: bool,
    layer_percent: u16,
    layer_cross_percent: u16,
    layer_edge_offset: u16,
    placement: DialogLayerPlacement,
    base_rect: Rect,
    layer_rect: Rect,
    backdrop: DialogBackdrop,
    backdrop_tween: Tween,
    restore_focus_on_close: bool,
    layer_focus_origin: Option<(TreePath, FocusId)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogLayerPlacement {
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSide {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DockSpec {
    side: DockSide,
    main_percent: u16,
    cross_percent: u16,
    edge_offset: u16,
}

pub trait DockChrome {
    fn set_dock_edge_borders(&mut self, borders: Borders);
}

impl DockSide {
    pub fn placement(self) -> DialogLayerPlacement {
        match self {
            Self::Top => DialogLayerPlacement::Top,
            Self::Bottom => DialogLayerPlacement::Bottom,
            Self::Left => DialogLayerPlacement::Left,
            Self::Right => DialogLayerPlacement::Right,
        }
    }
}

impl DockSpec {
    pub fn new(side: DockSide, main_percent: u16) -> Self {
        Self {
            side,
            main_percent: main_percent.clamp(1, 100),
            cross_percent: 100,
            edge_offset: 0,
        }
    }

    pub fn top(percent: u16) -> Self {
        Self::new(DockSide::Top, percent)
    }

    pub fn bottom(percent: u16) -> Self {
        Self::new(DockSide::Bottom, percent)
    }

    pub fn left(percent: u16) -> Self {
        Self::new(DockSide::Left, percent)
    }

    pub fn right(percent: u16) -> Self {
        Self::new(DockSide::Right, percent)
    }

    pub fn side(self) -> DockSide {
        self.side
    }

    pub fn main_percent(self) -> u16 {
        self.main_percent
    }

    pub fn cross_percent_value(self) -> u16 {
        self.cross_percent
    }

    pub fn edge_offset_value(self) -> u16 {
        self.edge_offset
    }

    pub fn cross_percent(mut self, percent: u16) -> Self {
        self.cross_percent = percent.clamp(1, 100);
        self
    }

    pub fn edge_offset(mut self, offset: u16) -> Self {
        self.edge_offset = offset;
        self
    }

    pub fn placement(self) -> DialogLayerPlacement {
        self.side.placement()
    }

    pub fn edge_borders(self) -> Borders {
        dock_edge_borders(self.side, self.cross_percent)
    }
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

impl<Base, Layer> DialogLayer<Base, Layer> {
    pub fn new(base: Base, layer: Layer) -> Self {
        Self {
            base,
            layer,
            active: true,
            layer_percent: 100,
            layer_cross_percent: 100,
            layer_edge_offset: 0,
            placement: DialogLayerPlacement::Center,
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

    #[deprecated(
        since = "0.1.0",
        note = "Use `set_active_with_context` instead to let the focus manager resolve focus natively"
    )]
    pub fn set_active_with_dialog_focus<M>(&mut self, active: bool, ctx: &mut EventCtx<M>) {
        self.set_active_with_settings(active, ctx.animation());
        ctx.request_layout();
        ctx.request_redraw();
        ctx.focus(if active {
            self.reset_open_focus_bookkeeping();
            FocusRequest::Target(FocusId::new(DIALOG_FOCUS))
        } else {
            self.focus_request_for_active_change(false)
        });
    }

    fn focus_request_for_active_change(&mut self, active: bool) -> FocusRequest {
        if active {
            self.reset_open_focus_bookkeeping();
            FocusRequest::Next
        } else if self.restore_focus_on_close {
            FocusRequest::Last
        } else {
            FocusRequest::Next
        }
    }

    fn reset_open_focus_bookkeeping(&mut self) {
        self.restore_focus_on_close = true;
        self.layer_focus_origin = None;
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

    pub fn layer_cross_percent(mut self, percent: u16) -> Self {
        self.layer_cross_percent = percent.clamp(1, 100);
        self
    }

    pub fn set_layer_cross_percent(&mut self, percent: u16) {
        self.layer_cross_percent = percent.clamp(1, 100);
    }

    pub fn placement(mut self, placement: DialogLayerPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn set_placement(&mut self, placement: DialogLayerPlacement) {
        self.placement = placement;
        self.layer_edge_offset = 0;
    }

    pub fn dock(mut self, spec: DockSpec) -> Self {
        self.set_dock(spec);
        self
    }

    pub fn set_dock(&mut self, spec: DockSpec) {
        self.placement = spec.placement();
        self.layer_percent = spec.main_percent();
        self.layer_cross_percent = spec.cross_percent_value();
        self.layer_edge_offset = spec.edge_offset_value();
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

impl<Base, Layer> DialogLayer<Base, Layer>
where
    Layer: DockChrome,
{
    pub fn docked(mut self, spec: DockSpec) -> Self {
        self.set_docked(spec);
        self
    }

    pub fn set_docked(&mut self, spec: DockSpec) {
        self.set_dock(spec);
        self.layer.set_dock_edge_borders(spec.edge_borders());
    }
}

impl<Base, Layer, M> TuiNode<M> for DialogLayer<Base, Layer>
where
    Base: TuiNode<M>,
    Layer: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.base_rect = area;
        self.layer_rect = layer_rect(
            area,
            self.layer_percent,
            self.layer_cross_percent,
            self.placement,
            self.layer_edge_offset,
        );

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

    fn render_overlay(&self, frame: &mut Frame, area: Rect) {
        if self.active {
            self.layer.render_overlay(frame, area);
        } else {
            self.base.render_overlay(frame, area);
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
            let fg = if is_powerline_fill(cell.symbol()) {
                blend_cell_color(
                    cell.fg,
                    fallback_fg,
                    fallback_bg,
                    amount * BACKDROP_BACKGROUND_DIM_FACTOR,
                )
            } else {
                blend_cell_color(cell.fg, fallback_fg, target, amount)
            };
            cell.set_fg(fg);
            if cell.bg != Color::Reset {
                cell.set_bg(dimmed_bg);
            }
            cell.modifier.insert(Modifier::DIM);
        }
    }
}

fn is_powerline_fill(symbol: &str) -> bool {
    matches!(symbol, "" | "" | "" | "")
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

fn layer_rect(
    area: Rect,
    percent: u16,
    cross_percent: u16,
    placement: DialogLayerPlacement,
    edge_offset: u16,
) -> Rect {
    match placement {
        DialogLayerPlacement::Center => centered_percent_rect(area, percent),
        DialogLayerPlacement::Top => horizontal_dock_rect(
            area,
            percent,
            cross_percent,
            area.y.saturating_add(edge_offset),
        ),
        DialogLayerPlacement::Bottom => {
            let height = scaled_dimension(area.height, percent).max((area.height > 0) as u16);
            horizontal_dock_rect(
                area,
                percent,
                cross_percent,
                area.bottom()
                    .saturating_sub(height)
                    .saturating_sub(edge_offset)
                    .max(area.y),
            )
        }
        DialogLayerPlacement::Left => vertical_dock_rect(
            area,
            percent,
            cross_percent,
            area.x.saturating_add(edge_offset),
        ),
        DialogLayerPlacement::Right => {
            let width = scaled_dimension(area.width, percent).max((area.width > 0) as u16);
            vertical_dock_rect(
                area,
                percent,
                cross_percent,
                area.right()
                    .saturating_sub(width)
                    .saturating_sub(edge_offset)
                    .max(area.x),
            )
        }
    }
}

fn horizontal_dock_rect(area: Rect, percent: u16, cross_percent: u16, y: u16) -> Rect {
    let percent = percent.clamp(1, 100);
    let cross_percent = cross_percent.clamp(1, 100);
    let width = scaled_dimension(area.width, cross_percent).max((area.width > 0) as u16);
    let height = scaled_dimension(area.height, percent).max((area.height > 0) as u16);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        y,
        width,
        height,
    )
}

fn vertical_dock_rect(area: Rect, percent: u16, cross_percent: u16, x: u16) -> Rect {
    let percent = percent.clamp(1, 100);
    let cross_percent = cross_percent.clamp(1, 100);
    let width = scaled_dimension(area.width, percent).max((area.width > 0) as u16);
    let height = scaled_dimension(area.height, cross_percent).max((area.height > 0) as u16);
    Rect::new(
        x,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn dock_edge_borders(side: DockSide, cross_percent: u16) -> Borders {
    match side {
        DockSide::Top if cross_percent == 100 => Borders::BOTTOM,
        DockSide::Top => Borders::LEFT | Borders::RIGHT | Borders::BOTTOM,
        DockSide::Bottom if cross_percent == 100 => Borders::TOP,
        DockSide::Bottom => Borders::TOP | Borders::LEFT | Borders::RIGHT,
        DockSide::Left if cross_percent == 100 => Borders::RIGHT,
        DockSide::Left => Borders::TOP | Borders::RIGHT | Borders::BOTTOM,
        DockSide::Right if cross_percent == 100 => Borders::LEFT,
        DockSide::Right => Borders::TOP | Borders::LEFT | Borders::BOTTOM,
    }
}

fn scaled_dimension(value: u16, percent: u16) -> u16 {
    ((value as u32 * percent as u32) / 100).min(u16::MAX as u32) as u16
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Style;

    use super::*;
    use crate::{
        Button, Dialog, DialogCloseReason, EventRoute, FocusManager, Key, TextInput, TreePath,
        animation_settings,
    };

    struct StaticBody;

    #[derive(Default)]
    struct ChromeBody {
        borders: Option<Borders>,
    }

    struct ColorBody;

    struct PowerlineBody;

    impl TuiNode<()> for StaticBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut ratatui::Frame, _area: Rect) {}
    }

    impl DockChrome for ChromeBody {
        fn set_dock_edge_borders(&mut self, borders: Borders) {
            self.borders = Some(borders);
        }
    }

    impl TuiNode<()> for ChromeBody {
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

    impl TuiNode<()> for PowerlineBody {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
            let fill = Color::Rgb(10, 120, 200);
            let previous_fill = Color::Rgb(240, 200, 40);
            frame
                .buffer_mut()
                .set_string(area.x, area.y, "", Style::default().fg(fill));
            frame.buffer_mut().set_string(
                area.x + 1,
                area.y,
                "A",
                Style::default().fg(Color::Rgb(255, 255, 255)).bg(fill),
            );
            frame.buffer_mut().set_string(
                area.x,
                area.y + 1,
                "",
                Style::default().fg(fill).bg(previous_fill),
            );
            frame.buffer_mut().set_string(
                area.x + 1,
                area.y + 1,
                "B",
                Style::default().fg(Color::Rgb(255, 255, 255)).bg(fill),
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
    fn backdrop_dim_keeps_powerline_fill_aligned_with_adjacent_background() {
        let base = PowerlineBody;
        let layer = StaticBody;
        let mut dialog_layer =
            DialogLayer::new(base, layer).backdrop(DialogBackdrop::dim().amount(1.0));
        let mut layout = LayoutCtx::new();
        dialog_layer.layout(Rect::new(0, 0, 10, 4), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(10, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog_layer.render(frame, frame.area()))
            .expect("dialog layer should render");

        let cap = terminal.backend().buffer().cell((0, 0)).unwrap();
        let content = terminal.backend().buffer().cell((1, 0)).unwrap();
        assert_eq!(cap.fg, content.bg);
    }

    #[test]
    fn backdrop_dim_keeps_powerline_separator_aligned_with_next_background() {
        let base = PowerlineBody;
        let layer = StaticBody;
        let mut dialog_layer =
            DialogLayer::new(base, layer).backdrop(DialogBackdrop::dim().amount(1.0));
        let mut layout = LayoutCtx::new();
        dialog_layer.layout(Rect::new(0, 0, 10, 4), &mut layout);
        let mut terminal = Terminal::new(TestBackend::new(10, 4)).expect("terminal should build");

        terminal
            .draw(|frame| dialog_layer.render(frame, frame.area()))
            .expect("dialog layer should render");

        let separator = terminal.backend().buffer().cell((0, 1)).unwrap();
        let content = terminal.backend().buffer().cell((1, 1)).unwrap();
        assert_eq!(separator.fg, content.bg);
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
    fn set_active_with_dialog_focus_reopen_restores_previous_focus() {
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
            focused_events_before_global_hotkeys: false,
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
            focused_events_before_global_hotkeys: false,
        };

        dialog_layer.set_active_with_dialog_focus(true, &mut ctx);
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.dispatch_focus(&other_focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.set_active_with_dialog_focus(false, &mut ctx);
        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Next));

        dialog_layer.set_active_with_dialog_focus(true, &mut ctx);
        dialog_layer.set_active_with_dialog_focus(false, &mut ctx);

        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Last));
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
            focused_events_before_global_hotkeys: false,
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
            focused_events_before_global_hotkeys: false,
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
            focused_events_before_global_hotkeys: false,
        };

        dialog_layer.set_active_with_context(true, &mut ctx);
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.dispatch_focus(&focus_target, true, &mut FocusCtx::<()>::default());
        dialog_layer.set_active_with_context(false, &mut ctx);

        assert_eq!(ctx.focus_request(), Some(&FocusRequest::Last));
    }

    #[test]
    fn layer_rect_places_edges_and_scales_cross_axis() {
        let area = Rect::new(10, 20, 100, 50);

        assert_eq!(
            layer_rect(area, 40, 80, DialogLayerPlacement::Top, 0),
            Rect::new(20, 20, 80, 20)
        );
        assert_eq!(
            layer_rect(area, 40, 80, DialogLayerPlacement::Bottom, 0),
            Rect::new(20, 50, 80, 20)
        );
        assert_eq!(
            layer_rect(area, 30, 80, DialogLayerPlacement::Left, 0),
            Rect::new(10, 25, 30, 40)
        );
        assert_eq!(
            layer_rect(area, 30, 80, DialogLayerPlacement::Right, 0),
            Rect::new(80, 25, 30, 40)
        );
    }

    #[test]
    fn dock_spec_sets_layer_placement_size_and_edge_borders() {
        let mut dialog_layer = DialogLayer::new(StaticBody, ChromeBody::default())
            .docked(DockSpec::bottom(40).cross_percent(80));

        assert_eq!(dialog_layer.placement, DialogLayerPlacement::Bottom);
        assert_eq!(dialog_layer.layer_percent, 40);
        assert_eq!(dialog_layer.layer_cross_percent, 80);
        assert_eq!(dialog_layer.layer_edge_offset, 0);
        assert_eq!(
            dialog_layer.layer.borders,
            Some(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        );

        dialog_layer.layout(Rect::new(10, 20, 100, 50), &mut LayoutCtx::new());
        assert_eq!(dialog_layer.layer_rect, Rect::new(20, 50, 80, 20));

        dialog_layer.set_docked(DockSpec::left(30));

        assert_eq!(dialog_layer.placement, DialogLayerPlacement::Left);
        assert_eq!(dialog_layer.layer_percent, 30);
        assert_eq!(dialog_layer.layer_cross_percent, 100);
        assert_eq!(dialog_layer.layer_edge_offset, 0);
        assert_eq!(dialog_layer.layer.borders, Some(Borders::RIGHT));
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
    fn active_layer_closes_dialog_host_on_escape_from_child_route() {
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

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), crate::Propagation::Stopped);
        assert_eq!(ctx.messages(), &[DialogCloseReason::Escape]);
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
}
