use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::{
    AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget,
    LayoutCtx, LayoutResult, LifecycleCtx, TickResult, TuiEvent, TuiNode,
};
use crate::{Separator, separator};

pub struct Split<L, R> {
    direction: Direction,
    first: L,
    second: R,
    first_constraint: Constraint,
    second_constraint: Constraint,
    first_area: Rect,
    second_area: Rect,
    gap: u16,
    separator: Option<Separator>,
    separator_area: Option<Rect>,
}

impl<L, R> Split<L, R> {
    pub fn horizontal(left: L, right: R) -> Self {
        Self::new(Direction::Horizontal, left, right)
    }

    pub fn vertical(top: L, bottom: R) -> Self {
        Self::new(Direction::Vertical, top, bottom)
    }

    pub fn ratio(mut self, first: u16, second: u16) -> Self {
        let denominator = (u32::from(first) + u32::from(second)).max(1);
        self.first_constraint = Constraint::Ratio(first.into(), denominator);
        self.second_constraint = Constraint::Ratio(second.into(), denominator);
        self
    }

    pub fn constraints(mut self, first: Constraint, second: Constraint) -> Self {
        self.first_constraint = first;
        self.second_constraint = second;
        self
    }

    pub fn first_constraint(mut self, constraint: Constraint) -> Self {
        self.first_constraint = constraint;
        self
    }

    pub fn second_constraint(mut self, constraint: Constraint) -> Self {
        self.second_constraint = constraint;
        self
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn separator(mut self, separator: Separator) -> Self {
        self.separator = Some(separator);
        self
    }

    pub fn first(&self) -> &L {
        &self.first
    }

    pub fn first_mut(&mut self) -> &mut L {
        &mut self.first
    }

    pub fn second(&self) -> &R {
        &self.second
    }

    pub fn second_mut(&mut self) -> &mut R {
        &mut self.second
    }

    pub fn child_areas(&self) -> (Rect, Rect) {
        (self.first_area, self.second_area)
    }

    pub fn separator_area(&self) -> Option<Rect> {
        self.separator_area
    }

    fn new(direction: Direction, first: L, second: R) -> Self {
        Self {
            direction,
            first,
            second,
            first_constraint: Constraint::Percentage(50),
            second_constraint: Constraint::Percentage(50),
            first_area: Rect::default(),
            second_area: Rect::default(),
            gap: 0,
            separator: None,
            separator_area: None,
        }
    }
}

impl<L, R, M> TuiNode<M> for Split<L, R>
where
    L: TuiNode<M>,
    R: TuiNode<M>,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let separator_cell = (self.separator.is_some() && self.main_len(area) > 0) as u16;
        let gap = self
            .gap
            .min(self.main_len(area).saturating_sub(separator_cell));
        let reserved = gap.saturating_add(separator_cell);
        let child_area = self.child_layout_area(area, reserved);
        let [first, second] = Layout::default()
            .direction(self.direction)
            .constraints([self.first_constraint, self.second_constraint])
            .areas(child_area);
        self.first_area = first;
        self.second_area = self.shift_second(second, reserved);
        self.separator_area = self.separator_rect(first, gap, separator_cell);
        ctx.push_slot(ChildKey::first(), first, |ctx| {
            self.first.layout(first, ctx);
        });
        ctx.push_slot(ChildKey::second(), self.second_area, |ctx| {
            self.second.layout(self.second_area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        self.first.render(frame, self.first_area);
        self.second.render(frame, self.second_area);
        if let (Some(separator), Some(area)) = (self.separator, self.separator_area) {
            match self.direction {
                Direction::Horizontal => separator::draw_vertical(frame, area, separator),
                Direction::Vertical => separator::draw_horizontal(frame, area, separator),
            }
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
        if let Some(route) = route.path.without_first_if(&first).map(EventRoute::new) {
            return self
                .first
                .dispatch_event(&route, event, ctx)
                .bubble(ctx, |ctx| self.event(event, ctx));
        }

        let second = ChildKey::second();
        if let Some(route) = route.path.without_first_if(&second).map(EventRoute::new) {
            return self
                .second
                .dispatch_event(&route, event, ctx)
                .bubble(ctx, |ctx| self.event(event, ctx));
        }

        EventOutcome::Ignored
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.first
            .tick(dt, settings)
            .merge(self.second.tick(dt, settings))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        let first = ChildKey::first();
        if let Some(target) = target.for_child(&first) {
            self.first.dispatch_focus(&target, focused, ctx);
            return;
        }

        let second = ChildKey::second();
        if let Some(target) = target.for_child(&second) {
            self.second.dispatch_focus(&target, focused, ctx);
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.first.init(ctx);
        self.second.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.first.mount(ctx);
        self.second.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.second.unmount(ctx);
        self.first.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.second.destroy(ctx);
        self.first.destroy(ctx);
    }
}

impl<L, R> Split<L, R> {
    fn main_len(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        }
    }

    fn child_layout_area(&self, area: Rect, reserved: u16) -> Rect {
        match self.direction {
            Direction::Horizontal => Rect::new(
                area.x,
                area.y,
                area.width.saturating_sub(reserved),
                area.height,
            ),
            Direction::Vertical => Rect::new(
                area.x,
                area.y,
                area.width,
                area.height.saturating_sub(reserved),
            ),
        }
    }

    fn shift_second(&self, rect: Rect, reserved: u16) -> Rect {
        match self.direction {
            Direction::Horizontal => Rect::new(
                rect.x.saturating_add(reserved),
                rect.y,
                rect.width,
                rect.height,
            ),
            Direction::Vertical => Rect::new(
                rect.x,
                rect.y.saturating_add(reserved),
                rect.width,
                rect.height,
            ),
        }
    }

    fn separator_rect(&self, first: Rect, gap: u16, separator_cell: u16) -> Option<Rect> {
        if separator_cell == 0 {
            return None;
        }
        let offset = gap / 2;
        match self.direction {
            Direction::Horizontal => Some(Rect::new(
                first.x.saturating_add(first.width).saturating_add(offset),
                first.y,
                1,
                first.height,
            )),
            Direction::Vertical => Some(Rect::new(
                first.x,
                first.y.saturating_add(first.height).saturating_add(offset),
                first.width,
                1,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{FocusId, Key, KeyEvent, Panel, TreePath};

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

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            *self.ticks.borrow_mut() += 1;
            TickResult::IDLE
        }
    }

    #[test]
    fn split_ratio_lays_out_children_with_reserved_paths() {
        let mut split = Split::horizontal(Probe::default(), Probe::default()).ratio(30, 70);
        let mut ctx = LayoutCtx::new();

        split.layout(Rect::new(0, 0, 100, 10), &mut ctx);

        assert_eq!(split.child_areas().0.width, 30);
        assert_eq!(split.child_areas().1.width, 70);
        assert_eq!(
            ctx.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::first()])
        );
        assert_eq!(
            ctx.focus_targets()[1].path,
            TreePath::from_keys([ChildKey::second()])
        );
    }

    #[test]
    fn split_ratio_handles_max_u16_parts_without_overflow() {
        let mut split =
            Split::horizontal(Probe::default(), Probe::default()).ratio(u16::MAX, u16::MAX);
        let mut ctx = LayoutCtx::new();

        split.layout(Rect::new(0, 0, 100, 10), &mut ctx);

        assert_eq!(split.child_areas().0.width, 50);
        assert_eq!(split.child_areas().1.width, 50);
    }

    #[test]
    fn split_horizontal_separator_reserves_space_and_records_area() {
        let mut split = Split::horizontal(Probe::default(), Probe::default())
            .ratio(1, 1)
            .gap(2)
            .separator(Separator::new());
        let mut ctx = LayoutCtx::new();

        split.layout(Rect::new(0, 0, 11, 3), &mut ctx);

        assert_eq!(split.child_areas().0, Rect::new(0, 0, 4, 3));
        assert_eq!(split.child_areas().1, Rect::new(7, 0, 4, 3));
        assert_eq!(split.separator_area(), Some(Rect::new(5, 0, 1, 3)));
    }

    #[test]
    fn split_vertical_separator_reserves_space_and_records_area() {
        let mut split = Split::vertical(Probe::default(), Probe::default())
            .ratio(1, 1)
            .separator(Separator::new());
        let mut ctx = LayoutCtx::new();

        split.layout(Rect::new(0, 0, 8, 5), &mut ctx);

        assert_eq!(split.child_areas().0, Rect::new(0, 0, 8, 2));
        assert_eq!(split.child_areas().1, Rect::new(0, 3, 8, 2));
        assert_eq!(split.separator_area(), Some(Rect::new(0, 2, 8, 1)));
    }

    #[test]
    fn split_separator_handles_tiny_areas() {
        let mut split = Split::horizontal(Probe::default(), Probe::default())
            .gap(10)
            .separator(Separator::new());
        let mut ctx = LayoutCtx::new();

        split.layout(Rect::new(0, 0, 1, 1), &mut ctx);

        assert_eq!(split.separator_area(), Some(Rect::new(0, 0, 1, 1)));
    }

    #[test]
    fn split_ticks_each_child_once() {
        let ticks = Rc::new(RefCell::new(0));
        let mut split = Split::horizontal(
            Probe {
                ticks: Rc::clone(&ticks),
            },
            Probe {
                ticks: Rc::clone(&ticks),
            },
        );

        split.tick(Duration::from_millis(16), AnimationSettings::default());

        assert_eq!(*ticks.borrow(), 2);
    }

    #[test]
    fn split_bubbles_routed_events_to_parent() {
        let mut split = Split::horizontal(Probe::default(), Probe::default());
        let route = EventRoute::new(TreePath::from_keys([ChildKey::first()]));
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));
        let mut ctx = EventCtx::<()>::default();

        assert_eq!(
            split.dispatch_event(&route, &event, &mut ctx),
            EventOutcome::Ignored
        );
    }

    #[test]
    fn nested_panel_split_input_tree_ticks_each_leaf_once() {
        let ticks = Rc::new(RefCell::new(0));
        let mut tree = Panel::new().host(Split::horizontal(
            Panel::new().host(Probe {
                ticks: Rc::clone(&ticks),
            }),
            Panel::new().host(Probe {
                ticks: Rc::clone(&ticks),
            }),
        ));

        tree.tick(Duration::from_millis(16), AnimationSettings::default());

        assert_eq!(*ticks.borrow(), 2);
    }
}
