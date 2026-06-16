use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Direction, Rect};

use crate::{
    AnimationSettings, ChildKey, Children, DuplicateChildKey, EventCtx, EventOutcome, EventRoute,
    FocusCtx, FocusTarget, LayoutCtx, LayoutResult, LifecycleCtx, MissingChildKey, TickResult,
    TuiEvent, TuiNode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainAlign {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAlign {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSize {
    Auto,
    Fixed(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlexItem {
    main: FlexMain,
    cross: CrossSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlexMain {
    Fixed(u16),
    Fill(u16),
    Percent(u16),
}

pub struct Flex<M = ()> {
    direction: Direction,
    children: Children<M>,
    items: Vec<FlexChild>,
    rects: Vec<(ChildKey, Rect)>,
    gap: u16,
    padding: Padding,
    justify: MainAlign,
    align: CrossAlign,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlexChild {
    key: ChildKey,
    item: FlexItem,
}

impl Padding {
    pub fn all(value: u16) -> Self {
        Self {
            left: value,
            right: value,
            top: value,
            bottom: value,
        }
    }

    pub fn horizontal_vertical(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }
}

impl FlexItem {
    pub fn fixed(size: u16) -> Self {
        Self {
            main: FlexMain::Fixed(size),
            cross: CrossSize::Auto,
        }
    }

    pub fn fill(weight: u16) -> Self {
        Self {
            main: FlexMain::Fill(weight.max(1)),
            cross: CrossSize::Auto,
        }
    }

    pub fn percent(percent: u16) -> Self {
        Self {
            main: FlexMain::Percent(percent.min(100)),
            cross: CrossSize::Auto,
        }
    }

    pub fn cross_size(mut self, cross: CrossSize) -> Self {
        self.cross = cross;
        self
    }
}

impl<M> Flex<M> {
    pub fn row() -> Self {
        Self::new(Direction::Horizontal)
    }

    pub fn column() -> Self {
        Self::new(Direction::Vertical)
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn justify(mut self, justify: MainAlign) -> Self {
        self.justify = justify;
        self
    }

    pub fn align(mut self, align: CrossAlign) -> Self {
        self.align = align;
        self
    }

    pub fn children(&self) -> &Children<M> {
        &self.children
    }

    pub fn child_rect(&self, key: &ChildKey) -> Option<Rect> {
        self.rects
            .iter()
            .find_map(|(child_key, rect)| (child_key == key).then_some(*rect))
    }

    fn new(direction: Direction) -> Self {
        Self {
            direction,
            children: Children::new(),
            items: Vec::new(),
            rects: Vec::new(),
            gap: 0,
            padding: Padding::default(),
            justify: MainAlign::Start,
            align: CrossAlign::Stretch,
        }
    }
}

impl<M> Flex<M>
where
    M: 'static,
{
    pub fn child<C>(mut self, key: impl Into<ChildKey>, child: C, item: FlexItem) -> Self
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
        item: FlexItem,
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
        item: FlexItem,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children = std::mem::take(&mut self.children).try_child(key.clone(), child)?;
        self.items.push(FlexChild { key, item });
        Ok(())
    }

    pub fn insert<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
        ctx: &mut EventCtx<M>,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children.insert(key.clone(), child, ctx)?;
        self.items.push(FlexChild { key, item });
        Ok(())
    }

    pub fn replace<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        let old = self.children.replace(key.clone(), child, ctx)?;
        if let Some(flex_child) = self.items.iter_mut().find(|child| child.key == key) {
            flex_child.item = item;
        }
        Ok(old)
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

impl<M> TuiNode<M> for Flex<M> {
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
        let child = self.children.dispatch_routed_child(route, event, ctx);
        child.bubble(ctx, |ctx| self.event(event, ctx))
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

impl<M> Flex<M> {
    fn calculate_rects(&self, area: Rect) -> Vec<(ChildKey, Rect)> {
        let inner = self.inner_area(area);
        let main_available = self.main_len(inner);
        let count = self.items.len();
        if count == 0 {
            return Vec::new();
        }

        let gap_count = u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX);
        let base_gap = u32::from(self.gap).saturating_mul(gap_count);
        let main_without_gap = u32::from(main_available).saturating_sub(base_gap);
        let lengths = self.main_lengths(main_without_gap);
        let used = lengths
            .iter()
            .fold(0u32, |used, length| used.saturating_add(u32::from(*length)));
        let spare = main_without_gap.saturating_sub(used);
        let justify_gap = match self.justify {
            MainAlign::SpaceBetween if count > 1 => {
                u32::from(self.gap).saturating_add(spare / gap_count)
            }
            _ => u32::from(self.gap),
        };
        let offset = match self.justify {
            MainAlign::Start | MainAlign::SpaceBetween => 0,
            MainAlign::Center => spare / 2,
            MainAlign::End => spare,
        };

        let mut cursor = offset;
        self.items
            .iter()
            .zip(lengths)
            .map(|(child, main)| {
                let rect = self.rect_for(inner, cursor, main, child.item.cross);
                cursor = cursor
                    .saturating_add(u32::from(main))
                    .saturating_add(justify_gap);
                (child.key.clone(), rect)
            })
            .collect()
    }

    fn inner_area(&self, area: Rect) -> Rect {
        let x = area.x.saturating_add(self.padding.left);
        let y = area.y.saturating_add(self.padding.top);
        let width = area
            .width
            .saturating_sub(self.padding.left.saturating_add(self.padding.right));
        let height = area
            .height
            .saturating_sub(self.padding.top.saturating_add(self.padding.bottom));
        Rect::new(x, y, width, height)
    }

    fn main_lengths(&self, available: u32) -> Vec<u16> {
        let fixed = self
            .items
            .iter()
            .map(|child| match child.item.main {
                FlexMain::Fixed(value) => u32::from(value),
                FlexMain::Percent(percent) => available.saturating_mul(u32::from(percent)) / 100,
                FlexMain::Fill(_) => 0,
            })
            .fold(0u32, |sum, length| sum.saturating_add(length));
        let fill_space = available.saturating_sub(fixed);
        let fill_weight = self
            .items
            .iter()
            .map(|child| match child.item.main {
                FlexMain::Fill(weight) => u32::from(weight),
                _ => 0,
            })
            .fold(0u32, |sum, weight| sum.saturating_add(weight));

        self.items
            .iter()
            .map(|child| match child.item.main {
                FlexMain::Fixed(value) => clamp_u32_to_u16(u32::from(value).min(available)),
                FlexMain::Percent(percent) => {
                    clamp_u32_to_u16(available.saturating_mul(u32::from(percent)) / 100)
                }
                FlexMain::Fill(weight) if fill_weight > 0 => {
                    clamp_u32_to_u16(fill_space.saturating_mul(u32::from(weight)) / fill_weight)
                }
                FlexMain::Fill(_) => 0,
            })
            .collect()
    }

    fn rect_for(&self, area: Rect, main_offset: u32, main: u16, cross: CrossSize) -> Rect {
        let cross_available = self.cross_len(area);
        let cross_len = match (cross, self.align) {
            (CrossSize::Fixed(size), _) => size.min(cross_available),
            (CrossSize::Auto, CrossAlign::Stretch) => cross_available,
            (CrossSize::Auto, _) => cross_available,
        };
        let cross_offset = match self.align {
            CrossAlign::Start | CrossAlign::Stretch => 0,
            CrossAlign::Center => cross_available.saturating_sub(cross_len) / 2,
            CrossAlign::End => cross_available.saturating_sub(cross_len),
        };
        let main_remaining = self
            .main_len(area)
            .saturating_sub(clamp_u32_to_u16(main_offset));
        let main_offset = clamp_u32_to_u16(main_offset);
        let main_len = main.min(main_remaining);

        match self.direction {
            Direction::Horizontal => Rect::new(
                area.x.saturating_add(main_offset),
                area.y.saturating_add(cross_offset),
                main_len,
                cross_len,
            ),
            Direction::Vertical => Rect::new(
                area.x.saturating_add(cross_offset),
                area.y.saturating_add(main_offset),
                cross_len,
                main_len,
            ),
        }
    }

    fn main_len(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        }
    }

    fn cross_len(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.height,
            Direction::Vertical => area.width,
        }
    }
}

fn clamp_u32_to_u16(value: u32) -> u16 {
    value.min(u32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{FocusId, Key, KeyEvent, Propagation, TreePath};

    #[derive(Default)]
    struct Probe {
        ticks: Rc<RefCell<usize>>,
    }

    impl TuiNode<()> for Probe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("probe"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            ctx.stop_propagation();
            EventOutcome::Handled
        }

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            *self.ticks.borrow_mut() += 1;
            TickResult::IDLE
        }
    }

    #[test]
    fn flex_row_calculates_fixed_percent_fill_gap_and_padding() {
        let mut flex = Flex::row()
            .padding(Padding::horizontal_vertical(2, 1))
            .gap(1)
            .child("fixed", Probe::default(), FlexItem::fixed(10))
            .child("percent", Probe::default(), FlexItem::percent(50))
            .child("fill", Probe::default(), FlexItem::fill(1));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 100, 5), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("fixed")).unwrap(),
            Rect::new(2, 1, 10, 3)
        );
        assert_eq!(
            flex.child_rect(&ChildKey::from("percent")).unwrap().width,
            47
        );
        assert_eq!(flex.child_rect(&ChildKey::from("fill")).unwrap().width, 37);
        assert_eq!(
            ctx.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::from("fixed")])
        );
    }

    #[test]
    fn flex_ticks_each_child_once() {
        let ticks = Rc::new(RefCell::new(0));
        let mut flex = Flex::column()
            .child(
                "one",
                Probe {
                    ticks: Rc::clone(&ticks),
                },
                FlexItem::fixed(1),
            )
            .child(
                "two",
                Probe {
                    ticks: Rc::clone(&ticks),
                },
                FlexItem::fill(1),
            );

        flex.tick(Duration::from_millis(16), AnimationSettings::default());

        assert_eq!(*ticks.borrow(), 2);
    }

    #[test]
    fn flex_routes_child_events_and_preserves_stop_propagation() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fill(1));
        let route = EventRoute::new(TreePath::from_keys([ChildKey::from("one")]));
        let mut ctx = EventCtx::default();

        let outcome =
            flex.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn flex_insert_updates_layout_items_and_children_together() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fixed(10));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        flex.insert("two", Probe::default(), FlexItem::fixed(20), &mut ctx)
            .unwrap();
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(flex.children().contains_key(&ChildKey::from("two")));
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().width, 20);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_replace_updates_layout_item_for_existing_child() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fixed(10));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        let old = flex
            .replace("one", Probe::default(), FlexItem::fixed(25), &mut ctx)
            .unwrap();
        drop(old);
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(flex.children().contains_key(&ChildKey::from("one")));
        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().width, 25);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_remove_updates_layout_items_and_children_together() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fixed(10))
            .child("two", Probe::default(), FlexItem::fixed(20));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        let old = flex.remove("one", &mut ctx).unwrap();
        drop(old);
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(!flex.children().contains_key(&ChildKey::from("one")));
        assert!(flex.child_rect(&ChildKey::from("one")).is_none());
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 0);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_large_fixed_lengths_do_not_overflow() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fixed(50_000))
            .child("two", Probe::default(), FlexItem::fixed(50_000));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, u16::MAX, 1), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("one")).unwrap().width,
            50_000
        );
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 50_000);
        assert_eq!(
            flex.child_rect(&ChildKey::from("two")).unwrap().width,
            15_535
        );
    }

    #[test]
    fn flex_large_fill_weights_do_not_overflow() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fill(u16::MAX))
            .child("two", Probe::default(), FlexItem::fill(u16::MAX))
            .child("three", Probe::default(), FlexItem::fill(u16::MAX));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 99, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().width, 33);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().width, 33);
        assert_eq!(flex.child_rect(&ChildKey::from("three")).unwrap().width, 33);
    }
}
