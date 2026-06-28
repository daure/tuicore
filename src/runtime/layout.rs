use ratatui::layout::Rect;

use crate::{FocusTarget, HitRegion, LayoutCtx, LayoutResult, OverlayLayoutEntry, TuiNode};

#[derive(Debug, Clone)]
pub struct LayoutEngine {
    ctx: LayoutCtx,
    area: Rect,
    result: LayoutResult,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        let area = Rect::default();
        Self {
            ctx: LayoutCtx::new(),
            area,
            result: LayoutResult::new(area),
        }
    }
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn layout<N, M>(&mut self, root: &mut N, area: Rect) -> LayoutResult
    where
        N: TuiNode<M>,
    {
        let mut ctx = LayoutCtx::new();
        let result = ctx.with_overlay_bounds(area, |ctx| root.layout(area, ctx));
        self.ctx = ctx;
        self.area = area;
        self.result = result;
        result
    }

    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn result(&self) -> LayoutResult {
        self.result
    }

    pub fn ctx(&self) -> &LayoutCtx {
        &self.ctx
    }

    pub fn focus_targets(&self) -> &[FocusTarget] {
        self.ctx.focus_targets()
    }

    pub fn hit_regions(&self) -> &[HitRegion] {
        self.ctx.hit_regions()
    }

    pub fn overlays(&self) -> &[OverlayLayoutEntry] {
        self.ctx.overlays()
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Frame;

    use super::*;
    use crate::{FocusId, LayoutCtx};

    struct LayoutNode;

    impl TuiNode<()> for LayoutNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("root"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut crate::RenderCtx<'_>) {}
    }

    #[test]
    fn layout_stores_latest_focus_targets() {
        let mut root = LayoutNode;
        let mut engine = LayoutEngine::new();
        let area = Rect::new(0, 0, 10, 5);

        let result = engine.layout(&mut root, area);

        assert_eq!(result.area, area);
        assert_eq!(engine.focus_targets().len(), 1);
        assert_eq!(engine.focus_targets()[0].id.as_str(), "root");
    }
}
