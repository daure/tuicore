use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use crate::{
    AnimationSettings, AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx,
    FocusTarget, HintSource, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx,
    TickResult, TuiEvent, TuiNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    #[default]
    Center,
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlaySize {
    #[default]
    FitContent,
    Fixed {
        width: u16,
        height: u16,
    },
    Fill,
}

pub struct Overlay<Base, Layer> {
    base: Base,
    layer: Layer,
    anchor: OverlayAnchor,
    layer_size: OverlaySize,
    base_rect: Rect,
    layer_rect: Rect,
}

impl<Base, Layer> Overlay<Base, Layer> {
    pub fn new(base: Base, layer: Layer) -> Self {
        Self {
            base,
            layer,
            anchor: OverlayAnchor::Center,
            layer_size: OverlaySize::FitContent,
            base_rect: Rect::default(),
            layer_rect: Rect::default(),
        }
    }

    pub fn anchor(mut self, anchor: OverlayAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn layer_size(mut self, layer_size: OverlaySize) -> Self {
        self.layer_size = layer_size;
        self
    }

    pub fn fixed_layer(mut self, width: u16, height: u16) -> Self {
        self.layer_size = OverlaySize::Fixed { width, height };
        self
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

    pub fn base_rect(&self) -> Rect {
        self.base_rect
    }

    pub fn layer_rect(&self) -> Rect {
        self.layer_rect
    }
}

impl<Base, Layer, M> TuiNode<M> for Overlay<Base, Layer>
where
    Base: TuiNode<M>,
    Layer: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.base.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.base_rect = area;
        self.layer_rect = self.place_layer::<M>(area);
        ctx.push_slot(ChildKey::first(), self.base_rect, |ctx| {
            self.base.layout(self.base_rect, ctx);
        });
        ctx.push_slot(ChildKey::second(), self.layer_rect, |ctx| {
            self.layer.layout(self.layer_rect, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        self.base.render(frame, self.base_rect);
        self.layer.render(frame, self.layer_rect);
    }

    fn render_overlay(&self, frame: &mut Frame, area: Rect) {
        self.base.render_overlay(frame, area);
        self.layer.render_overlay(frame, area);
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
        if let Some(route) = route.path.without_first_if(&first).map(EventRoute::new) {
            return self
                .base
                .dispatch_event(&route, event, ctx)
                .bubble(ctx, |ctx| self.event(event, ctx));
        }

        let second = ChildKey::second();
        if let Some(route) = route.path.without_first_if(&second).map(EventRoute::new) {
            return self
                .layer
                .dispatch_event(&route, event, ctx)
                .bubble(ctx, |ctx| self.event(event, ctx));
        }

        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.base
            .tick(dt, settings)
            .merge(self.layer.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let first = ChildKey::first();
        if let Some(target) = target.for_child(&first) {
            self.base.dispatch_focus(&target, focused, ctx);
            return;
        }

        let second = ChildKey::second();
        if let Some(target) = target.for_child(&second) {
            self.layer.dispatch_focus(&target, focused, ctx);
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

impl<Base, Layer> Overlay<Base, Layer> {
    fn place_layer<M>(&self, area: Rect) -> Rect
    where
        Layer: TuiNode<M>,
    {
        let (width, height) = self.resolve_layer_size::<M>(area);
        let (x, y) = match self.anchor {
            OverlayAnchor::TopLeft => (area.x, area.y),
            OverlayAnchor::TopRight => (right_aligned(area, width), area.y),
            OverlayAnchor::BottomLeft => (area.x, bottom_aligned(area, height)),
            OverlayAnchor::BottomRight => {
                (right_aligned(area, width), bottom_aligned(area, height))
            }
            OverlayAnchor::Center => (
                area.x.saturating_add(area.width.saturating_sub(width) / 2),
                area.y
                    .saturating_add(area.height.saturating_sub(height) / 2),
            ),
            OverlayAnchor::Above => (
                area.x.saturating_add(area.width.saturating_sub(width) / 2),
                area.y,
            ),
            OverlayAnchor::Below => (
                area.x.saturating_add(area.width.saturating_sub(width) / 2),
                bottom_aligned(area, height),
            ),
        };
        Rect::new(x, y, width, height)
    }

    fn resolve_layer_size<M>(&self, area: Rect) -> (u16, u16)
    where
        Layer: TuiNode<M>,
    {
        match self.layer_size {
            OverlaySize::Fill => (area.width, area.height),
            OverlaySize::Fixed { width, height } => {
                (width.min(area.width), height.min(area.height))
            }
            OverlaySize::FitContent => {
                let hint = self.layer.measure(LayoutProposal {
                    width: AxisProposal::AtMost(area.width),
                    height: AxisProposal::AtMost(area.height),
                });
                if hint.source == HintSource::LegacyUnmeasured {
                    ((area.width > 0) as u16, (area.height > 0) as u16)
                } else {
                    (
                        hint.preferred.width.min(area.width),
                        hint.preferred.height.min(area.height),
                    )
                }
            }
        }
    }
}

fn right_aligned(area: Rect, width: u16) -> u16 {
    area.x.saturating_add(area.width.saturating_sub(width))
}

fn bottom_aligned(area: Rect, height: u16) -> u16 {
    area.y.saturating_add(area.height.saturating_sub(height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayoutSize;

    struct Probe {
        size: LayoutSize,
    }

    impl TuiNode<()> for Probe {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint::content(self.size.width, self.size.height).normalized(proposal)
        }

        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    #[test]
    fn overlay_measure_excludes_layer_and_layouts_base_full_area() {
        let mut overlay = Overlay::new(
            Probe {
                size: LayoutSize::new(10, 4),
            },
            Probe {
                size: LayoutSize::new(6, 2),
            },
        )
        .anchor(OverlayAnchor::TopRight);
        let mut ctx = LayoutCtx::new();

        let hint = overlay.measure(LayoutProposal::unbounded());
        overlay.layout(Rect::new(2, 3, 20, 10), &mut ctx);

        assert_eq!(hint.preferred, crate::LayoutSize::new(10, 4));
        assert_eq!(overlay.base_rect(), Rect::new(2, 3, 20, 10));
        assert_eq!(overlay.layer_rect(), Rect::new(16, 3, 6, 2));
    }
}
