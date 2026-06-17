use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use crate::{
    AnimationSettings, AxisProposal, ChildKey, Children, DuplicateChildKey, EventCtx, EventOutcome,
    EventRoute, FocusCtx, FocusTarget, HintSource, LayoutCtx, LayoutProposal, LayoutResult,
    LayoutSize, LayoutSizeHint, LifecycleCtx, MissingChildKey, Padding, TickResult, TuiEvent,
    TuiNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackAlign {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StackSize {
    #[default]
    Fill,
    Fixed(u16),
    FitContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackItem {
    pub horizontal: StackAlign,
    pub vertical: StackAlign,
    pub inset: Padding,
    pub width: StackSize,
    pub height: StackSize,
}

pub struct Stack<M = ()> {
    children: Children<M>,
    items: Vec<StackChild>,
    rects: Vec<(ChildKey, Rect)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StackChild {
    key: ChildKey,
    item: StackItem,
}

impl Default for StackItem {
    fn default() -> Self {
        Self {
            horizontal: StackAlign::Stretch,
            vertical: StackAlign::Stretch,
            inset: Padding::default(),
            width: StackSize::Fill,
            height: StackSize::Fill,
        }
    }
}

impl StackItem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn align(mut self, horizontal: StackAlign, vertical: StackAlign) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }

    pub fn inset(mut self, inset: Padding) -> Self {
        self.inset = inset;
        self
    }

    pub fn fixed(mut self, width: u16, height: u16) -> Self {
        self.width = StackSize::Fixed(width);
        self.height = StackSize::Fixed(height);
        self
    }

    pub fn fit_content(mut self) -> Self {
        self.width = StackSize::FitContent;
        self.height = StackSize::FitContent;
        self
    }
}

impl<M> Default for Stack<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Stack<M> {
    pub fn new() -> Self {
        Self {
            children: Children::new(),
            items: Vec::new(),
            rects: Vec::new(),
        }
    }

    pub fn children(&self) -> &Children<M> {
        &self.children
    }

    pub fn child_rect(&self, key: &ChildKey) -> Option<Rect> {
        self.rects
            .iter()
            .find_map(|(child_key, rect)| (child_key == key).then_some(*rect))
    }
}

impl<M> Stack<M>
where
    M: 'static,
{
    pub fn child<C>(mut self, key: impl Into<ChildKey>, child: C, item: StackItem) -> Self
    where
        C: TuiNode<M> + 'static,
    {
        if let Err(error) = self.try_push(key, child, item) {
            panic!("duplicate child key: {}", error.key.as_str());
        }
        self
    }

    pub fn try_child<C>(
        mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: StackItem,
    ) -> Result<Self, DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        self.try_push(key, child, item)?;
        Ok(self)
    }

    fn try_push<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: StackItem,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children = std::mem::take(&mut self.children).try_child(key.clone(), child)?;
        self.items.push(StackChild { key, item });
        Ok(())
    }

    pub fn insert<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: StackItem,
        ctx: &mut EventCtx<M>,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children.insert(key.clone(), child, ctx)?;
        self.items.push(StackChild { key, item });
        Ok(())
    }

    pub fn remove(
        &mut self,
        key: impl Into<ChildKey>,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey> {
        let key = key.into();
        let old = self.children.remove(key.clone(), ctx)?;
        self.items.retain(|child| child.key != key);
        Ok(old)
    }
}

impl<M> TuiNode<M> for Stack<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let mut min = LayoutSize::default();
        let mut preferred = LayoutSize::default();
        for child in &self.items {
            let width_available = inset_axis_bound(
                proposal.width,
                child.item.inset.left,
                child.item.inset.right,
            );
            let height_available = inset_axis_bound(
                proposal.height,
                child.item.inset.top,
                child.item.inset.bottom,
            );
            let Some(hint) = self.children.measure_child(&child.key, proposal) else {
                continue;
            };
            let extra_width = child.item.inset.left.saturating_add(child.item.inset.right);
            let extra_height = child.item.inset.top.saturating_add(child.item.inset.bottom);
            let child_min_width =
                measure_stack_axis(child.item.width, width_available, hint, true, true);
            let child_min_height =
                measure_stack_axis(child.item.height, height_available, hint, false, true);
            let child_preferred_width =
                measure_stack_axis(child.item.width, width_available, hint, true, false);
            let child_preferred_height =
                measure_stack_axis(child.item.height, height_available, hint, false, false);
            min.width = min.width.max(child_min_width.saturating_add(extra_width));
            min.height = min
                .height
                .max(child_min_height.saturating_add(extra_height));
            preferred.width = preferred
                .width
                .max(child_preferred_width.saturating_add(extra_width));
            preferred.height = preferred
                .height
                .max(child_preferred_height.saturating_add(extra_height));
        }

        LayoutSizeHint {
            source: HintSource::Measured,
            min,
            preferred,
            expand: crate::AxisExpand {
                width: true,
                height: true,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.rects = self.calculate_rects(area);
        for (key, rect) in &self.rects {
            self.children.layout_child(key, *rect, ctx);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        for (key, rect) in &self.rects {
            if let Some(child) = self.children.get(key) {
                child.render(frame, *rect);
            }
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.children
            .dispatch_routed_child(route, event, ctx)
            .bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.children.tick(dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.children.dispatch_focus_target(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.destroy(ctx);
    }
}

impl<M> Stack<M> {
    fn calculate_rects(&self, area: Rect) -> Vec<(ChildKey, Rect)> {
        self.items
            .iter()
            .map(|child| {
                let inner = inset_area(area, child.item.inset);
                let hint = self
                    .children
                    .measure_child(&child.key, LayoutProposal::at_most_area(inner));
                let width = resolve_size(child.item.width, inner.width, hint, true);
                let height = resolve_size(child.item.height, inner.height, hint, false);
                let x = align_offset(inner.x, inner.width, width, child.item.horizontal);
                let y = align_offset(inner.y, inner.height, height, child.item.vertical);
                (child.key.clone(), Rect::new(x, y, width, height))
            })
            .collect()
    }
}

fn inset_area(area: Rect, inset: Padding) -> Rect {
    let x = area.x.saturating_add(inset.left);
    let y = area.y.saturating_add(inset.top);
    let width = area
        .width
        .saturating_sub(inset.left.saturating_add(inset.right));
    let height = area
        .height
        .saturating_sub(inset.top.saturating_add(inset.bottom));
    Rect::new(x, y, width, height)
}

fn resolve_size(size: StackSize, available: u16, hint: Option<LayoutSizeHint>, width: bool) -> u16 {
    match size {
        StackSize::Fill => available,
        StackSize::Fixed(value) => value.min(available),
        StackSize::FitContent => match hint {
            Some(hint) if hint.source == HintSource::Measured => {
                let preferred = if width {
                    hint.preferred.width
                } else {
                    hint.preferred.height
                };
                preferred.min(available)
            }
            _ => ((available > 0) as u16).min(available),
        },
    }
}

fn measure_stack_axis(
    size: StackSize,
    available: Option<u16>,
    hint: LayoutSizeHint,
    width: bool,
    min: bool,
) -> u16 {
    let hint_size = if min { hint.min } else { hint.preferred };
    let hinted = if width {
        hint_size.width
    } else {
        hint_size.height
    };
    match size {
        StackSize::Fill => available.unwrap_or(hinted),
        StackSize::Fixed(value) => available
            .map(|available| value.min(available))
            .unwrap_or(value),
        StackSize::FitContent if hint.source == HintSource::Measured => available
            .map(|available| hinted.min(available))
            .unwrap_or(hinted),
        StackSize::FitContent => {
            if min {
                0
            } else {
                available
                    .map(|available| (available > 0) as u16)
                    .unwrap_or(1)
            }
        }
    }
}

fn inset_axis_bound(proposal: AxisProposal, start: u16, end: u16) -> Option<u16> {
    match proposal {
        AxisProposal::AtMost(value) | AxisProposal::Exact(value) => {
            Some(value.saturating_sub(start.saturating_add(end)))
        }
        AxisProposal::Unbounded => None,
    }
}

fn align_offset(origin: u16, available: u16, size: u16, align: StackAlign) -> u16 {
    let slack = available.saturating_sub(size);
    origin.saturating_add(match align {
        StackAlign::Stretch | StackAlign::Start => 0,
        StackAlign::Center => slack / 2,
        StackAlign::End => slack,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FocusId, HintSource};

    struct Probe {
        size: LayoutSize,
    }

    impl TuiNode<()> for Probe {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint::content(self.size.width, self.size.height).normalized(proposal)
        }

        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("probe"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    struct LegacyProbe;

    impl TuiNode<()> for LegacyProbe {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    #[test]
    fn stack_layers_children_with_alignment_and_inset() {
        let mut stack = Stack::new()
            .child(
                "base",
                Probe {
                    size: LayoutSize::new(1, 1),
                },
                StackItem::new(),
            )
            .child(
                "badge",
                Probe {
                    size: LayoutSize::new(4, 2),
                },
                StackItem::new()
                    .fit_content()
                    .align(StackAlign::End, StackAlign::Start)
                    .inset(Padding::all(1)),
            );
        let mut ctx = LayoutCtx::new();

        stack.layout(Rect::new(0, 0, 20, 10), &mut ctx);

        assert_eq!(
            stack.child_rect(&ChildKey::from("base")),
            Some(Rect::new(0, 0, 20, 10))
        );
        assert_eq!(
            stack.child_rect(&ChildKey::from("badge")),
            Some(Rect::new(15, 1, 4, 2))
        );
        assert_eq!(ctx.focus_targets().len(), 2);
    }

    #[test]
    fn stack_measure_uses_max_child_hint() {
        let stack = Stack::new()
            .child(
                "small",
                Probe {
                    size: LayoutSize::new(4, 2),
                },
                StackItem::new(),
            )
            .child(
                "large",
                Probe {
                    size: LayoutSize::new(8, 3),
                },
                StackItem::new(),
            );

        let hint = stack.measure(LayoutProposal::unbounded());

        assert_eq!(hint.source, HintSource::Measured);
        assert_eq!(hint.preferred, LayoutSize::new(8, 3));
    }

    #[test]
    fn stack_measure_applies_item_fixed_and_fit_content_sizes() {
        let stack = Stack::new()
            .child(
                "fixed",
                Probe {
                    size: LayoutSize::new(1, 1),
                },
                StackItem::new().fixed(10, 5),
            )
            .child(
                "content",
                Probe {
                    size: LayoutSize::new(4, 2),
                },
                StackItem::new().fit_content().inset(Padding::all(1)),
            );

        let hint = stack.measure(LayoutProposal::unbounded());

        assert_eq!(hint.preferred, LayoutSize::new(10, 5));
    }

    #[test]
    fn stack_measure_applies_fill_available_and_legacy_fallback() {
        let fill = Stack::new().child(
            "fill",
            Probe {
                size: LayoutSize::new(1, 1),
            },
            StackItem::new(),
        );
        let legacy = Stack::new().child("legacy", LegacyProbe, StackItem::new().fit_content());

        let fill_hint = fill.measure(LayoutProposal::at_most(20, 10));
        let legacy_hint = legacy.measure(LayoutProposal::unbounded());

        assert_eq!(fill_hint.preferred, LayoutSize::new(20, 10));
        assert_eq!(legacy_hint.preferred, LayoutSize::new(1, 1));
    }
}
