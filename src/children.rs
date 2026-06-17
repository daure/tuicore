use std::{marker::PhantomData, time::Duration};

use ratatui::{Frame, layout::Rect};

use crate::{
    AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRepair, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, TickResult, TuiEvent, TuiNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateChildKey {
    pub key: ChildKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingChildKey {
    pub key: ChildKey,
}

#[derive(Debug, Clone)]
pub struct ChildSlot<C, M = ()> {
    key: ChildKey,
    child: C,
    initialized: bool,
    mounted: bool,
    _message: PhantomData<fn() -> M>,
}

pub struct Children<M = ()> {
    slots: Vec<ChildSlot<Box<dyn TuiNode<M>>, M>>,
    initialized: bool,
    mounted: bool,
}

impl<C, M> ChildSlot<C, M>
where
    C: TuiNode<M>,
{
    pub fn new(key: impl Into<ChildKey>, child: C) -> Self {
        Self {
            key: key.into(),
            child,
            initialized: false,
            mounted: false,
            _message: PhantomData,
        }
    }

    pub fn key(&self) -> &ChildKey {
        &self.key
    }

    pub fn child(&self) -> &C {
        &self.child
    }

    pub fn child_mut(&mut self) -> &mut C {
        &mut self.child
    }

    pub fn into_child(self) -> C {
        self.child
    }

    pub fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        if !self.initialized {
            self.child.init(ctx);
            self.initialized = true;
        }
    }

    pub fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.init(ctx);
        if !self.mounted {
            self.child.mount(ctx);
            self.mounted = true;
        }
    }

    pub fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if self.mounted {
            self.child.unmount(ctx);
            self.mounted = false;
        }
    }

    pub fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.unmount(ctx);
        if self.initialized {
            self.child.destroy(ctx);
            self.initialized = false;
        }
    }

    pub fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        ctx.push_slot(self.key.clone(), area, |ctx| self.child.layout(area, ctx))
    }

    pub fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.child.measure(proposal)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.child.render(frame, area);
    }

    pub fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let Some(child_route) = route.path.without_first_if(&self.key).map(EventRoute::new) else {
            return EventOutcome::Ignored;
        };

        self.child.dispatch_event(&child_route, event, ctx)
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.child.tick(dt, settings)
    }

    pub fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.child.focus(target, focused, ctx);
    }

    pub fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if let Some(target) = target.for_child(&self.key) {
            self.child.dispatch_focus(&target, focused, ctx);
        }
    }
}

impl<M> Default for Children<M> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            initialized: false,
            mounted: false,
        }
    }
}

impl<M> Children<M> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ChildSlot<Box<dyn TuiNode<M>>, M>> {
        self.slots.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ChildSlot<Box<dyn TuiNode<M>>, M>> {
        self.slots.iter_mut()
    }

    pub fn contains_key(&self, key: &ChildKey) -> bool {
        self.slots.iter().any(|slot| slot.key() == key)
    }

    pub fn get(&self, key: &ChildKey) -> Option<&ChildSlot<Box<dyn TuiNode<M>>, M>> {
        self.slots.iter().find(|slot| slot.key() == key)
    }

    pub fn get_mut(&mut self, key: &ChildKey) -> Option<&mut ChildSlot<Box<dyn TuiNode<M>>, M>> {
        self.slots.iter_mut().find(|slot| slot.key() == key)
    }
}

impl<M> Children<M>
where
    M: 'static,
{
    pub fn child<C>(mut self, key: impl Into<ChildKey>, child: C) -> Self
    where
        C: TuiNode<M> + 'static,
    {
        self.push_builder(key, child);
        self
    }

    pub fn try_child<C>(
        mut self,
        key: impl Into<ChildKey>,
        child: C,
    ) -> Result<Self, DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        self.try_push_builder(key, child)?;
        Ok(self)
    }

    fn push_builder<C>(&mut self, key: impl Into<ChildKey>, child: C)
    where
        C: TuiNode<M> + 'static,
    {
        if let Err(error) = self.try_push_builder(key, child) {
            panic!("duplicate child key: {}", error.key.as_str());
        }
    }

    fn try_push_builder<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        if self.contains_key(&key) {
            return Err(DuplicateChildKey { key });
        }
        self.slots.push(ChildSlot::new(key, Box::new(child)));
        Ok(())
    }

    pub fn insert<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        ctx: &mut EventCtx<M>,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        if self.contains_key(&key) {
            return Err(DuplicateChildKey { key });
        }

        let mut slot = ChildSlot::new(key, Box::new(child) as Box<dyn TuiNode<M>>);
        let mut lifecycle = LifecycleCtx::default();
        if self.mounted {
            slot.mount(&mut lifecycle);
        } else if self.initialized {
            slot.init(&mut lifecycle);
        }
        self.slots.push(slot);
        merge_lifecycle_effects(ctx, lifecycle);
        request_tree_update(ctx);
        Ok(())
    }

    pub fn try_insert<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        ctx: &mut EventCtx<M>,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        self.insert(key, child, ctx)
    }

    pub fn replace<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        let Some(index) = self.slots.iter().position(|slot| slot.key() == &key) else {
            return Err(MissingChildKey { key });
        };
        let mut old =
            std::mem::replace(&mut self.slots[index], ChildSlot::new(key, Box::new(child)));
        let was_initialized = old.initialized;
        let was_mounted = old.mounted;
        let mut lifecycle = LifecycleCtx::default();
        old.destroy(&mut lifecycle);
        if was_mounted {
            self.slots[index].mount(&mut lifecycle);
        } else if was_initialized {
            self.slots[index].init(&mut lifecycle);
        }
        merge_lifecycle_effects(ctx, lifecycle);
        request_tree_update(ctx);
        Ok(old.into_child())
    }

    pub fn remove(
        &mut self,
        key: impl Into<ChildKey>,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey> {
        let key = key.into();
        let Some(index) = self.slots.iter().position(|slot| slot.key() == &key) else {
            return Err(MissingChildKey { key });
        };
        let mut slot = self.slots.remove(index);
        ctx.repair_focus(FocusRepair::RemovedChild { index });
        let mut lifecycle = LifecycleCtx::default();
        slot.destroy(&mut lifecycle);
        merge_lifecycle_effects(ctx, lifecycle);
        request_tree_update(ctx);
        Ok(slot.into_child())
    }
}

impl<M> Children<M> {
    pub fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        for slot in &mut self.slots {
            slot.init(ctx);
        }
        self.initialized = true;
    }

    pub fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        for slot in &mut self.slots {
            slot.mount(ctx);
        }
        self.initialized = true;
        self.mounted = true;
    }

    pub fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        for slot in self.slots.iter_mut().rev() {
            slot.unmount(ctx);
        }
        self.mounted = false;
    }

    pub fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        for slot in self.slots.iter_mut().rev() {
            slot.destroy(ctx);
        }
        self.mounted = false;
        self.initialized = false;
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.slots
            .iter_mut()
            .fold(TickResult::IDLE, |result, slot| {
                result.merge(slot.tick(dt, settings))
            })
    }

    pub fn layout_child(
        &mut self,
        key: &ChildKey,
        area: Rect,
        ctx: &mut LayoutCtx,
    ) -> Option<LayoutResult> {
        self.get_mut(key).map(|slot| slot.layout(area, ctx))
    }

    pub fn measure_child(
        &self,
        key: &ChildKey,
        proposal: LayoutProposal,
    ) -> Option<LayoutSizeHint> {
        self.get(key).map(|slot| slot.measure(proposal))
    }

    pub fn dispatch_child(
        &mut self,
        key: &ChildKey,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.get_mut(key)
            .map(|slot| slot.dispatch_event(route, event, ctx))
            .unwrap_or(EventOutcome::Ignored)
    }

    pub fn dispatch_routed_child(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let Some(key) = route.path.first().cloned() else {
            return EventOutcome::Ignored;
        };

        self.dispatch_child(&key, route, event, ctx)
    }

    pub fn dispatch_routed_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
        parent: impl FnOnce(&TuiEvent, &mut EventCtx<M>) -> EventOutcome,
    ) -> EventOutcome {
        self.dispatch_routed_child(route, event, ctx)
            .bubble(ctx, |ctx| parent(event, ctx))
    }

    pub fn dispatch_focus_target(
        &mut self,
        target: &FocusTarget,
        focused: bool,
        ctx: &mut FocusCtx<M>,
    ) {
        let Some(key) = target.path.first().cloned() else {
            return;
        };

        if let Some(slot) = self.get_mut(&key) {
            slot.dispatch_focus(target, focused, ctx);
        }
    }

    pub fn focus_child(
        &mut self,
        key: &ChildKey,
        target: Option<&FocusId>,
        focused: bool,
        ctx: &mut FocusCtx<M>,
    ) {
        if let Some(slot) = self.get_mut(key) {
            slot.focus(target, focused, ctx);
        }
    }
}

fn request_tree_update<M>(ctx: &mut EventCtx<M>) {
    ctx.request_layout();
    ctx.request_redraw();
}

fn merge_lifecycle_effects<M>(ctx: &mut EventCtx<M>, mut lifecycle: LifecycleCtx<M>) {
    if lifecycle.layout_requested() {
        ctx.request_layout();
    }
    if lifecycle.redraw_requested() {
        ctx.request_redraw();
    }
    for message in lifecycle.drain_messages() {
        ctx.emit(message);
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::TreePath;

    #[derive(Default)]
    struct TestNode {
        init_count: usize,
        mount_count: usize,
        unmount_count: usize,
        destroy_count: usize,
        tick_count: usize,
    }

    impl TuiNode<()> for TestNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            self.tick_count += 1;
            TickResult::CHANGED
        }

        fn init(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.init_count += 1;
        }

        fn mount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.mount_count += 1;
        }

        fn unmount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.unmount_count += 1;
        }

        fn destroy(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.destroy_count += 1;
        }
    }

    struct EventTestNode {
        message: &'static str,
        outcome: EventOutcome,
        stop: bool,
    }

    impl TuiNode<&'static str> for EventTestNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<&'static str>) -> EventOutcome {
            ctx.emit(self.message);
            if self.stop {
                ctx.stop_propagation();
            }
            self.outcome
        }
    }

    struct EventContainer {
        children: Children<&'static str>,
        message: &'static str,
        outcome: EventOutcome,
    }

    struct FocusTestNode;

    struct LifecycleEmitter;

    struct OrderedNode {
        name: &'static str,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    impl TuiNode<&'static str> for EventContainer {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<&'static str>) -> EventOutcome {
            ctx.emit(self.message);
            self.outcome
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<&'static str>,
        ) -> EventOutcome {
            let message = self.message;
            let outcome = self.outcome;
            self.children
                .dispatch_routed_event(route, event, ctx, move |_event, ctx| {
                    ctx.emit(message);
                    outcome
                })
        }
    }

    impl TuiNode<&'static str> for FocusTestNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn focus(
            &mut self,
            _target: Option<&FocusId>,
            _focused: bool,
            ctx: &mut FocusCtx<&'static str>,
        ) {
            ctx.emit("leaf focus");
        }
    }

    impl TuiNode<&'static str> for LifecycleEmitter {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn init(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("init");
        }

        fn mount(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("mount");
        }
    }

    impl TuiNode<()> for OrderedNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn init(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.log.borrow_mut().push(match self.name {
                "old" => "old init",
                "new" => "new init",
                _ => "unknown init",
            });
        }

        fn mount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.log.borrow_mut().push(match self.name {
                "old" => "old mount",
                "new" => "new mount",
                _ => "unknown mount",
            });
        }

        fn unmount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.log.borrow_mut().push(match self.name {
                "old" => "old unmount",
                "new" => "new unmount",
                _ => "unknown unmount",
            });
        }

        fn destroy(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.log.borrow_mut().push(match self.name {
                "old" => "old destroy",
                "new" => "new destroy",
                _ => "unknown destroy",
            });
        }
    }

    #[test]
    fn children_reject_duplicate_keys() {
        let result = Children::new()
            .try_child(ChildKey::new("body"), TestNode::default())
            .and_then(|children| children.try_child(ChildKey::new("body"), TestNode::default()));

        match result {
            Ok(_) => panic!("duplicate child key accepted"),
            Err(error) => assert_eq!(error.key.as_str(), "body"),
        }
    }

    #[test]
    fn child_accepts_string_keys() {
        let children = Children::new().child("body", TestNode::default());

        assert!(children.contains_key(&ChildKey::from("body")));
    }

    #[test]
    fn dynamic_insert_rejects_duplicate_keys() {
        let mut children = Children::new().child("body", TestNode::default());
        let mut ctx = EventCtx::default();

        let error = children
            .insert("body", TestNode::default(), &mut ctx)
            .unwrap_err();

        assert_eq!(error.key, ChildKey::from("body"));
    }

    #[test]
    fn child_slot_runs_lifecycle_once() {
        let mut slot = ChildSlot::new(ChildKey::new("body"), TestNode::default());
        let mut ctx = LifecycleCtx::default();

        slot.mount(&mut ctx);
        slot.mount(&mut ctx);
        slot.destroy(&mut ctx);
        slot.destroy(&mut ctx);

        let child = slot.child();
        assert_eq!(child.init_count, 1);
        assert_eq!(child.mount_count, 1);
        assert_eq!(child.unmount_count, 1);
        assert_eq!(child.destroy_count, 1);
    }

    #[test]
    fn replace_mounts_live_replacement_after_destroying_old_child() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let old = OrderedNode {
            name: "old",
            log: Rc::clone(&log),
        };
        let new = OrderedNode {
            name: "new",
            log: Rc::clone(&log),
        };
        let mut children = Children::new().child(ChildKey::new("body"), old);
        let mut lifecycle = LifecycleCtx::default();
        let mut ctx = EventCtx::default();

        children.mount(&mut lifecycle);
        children
            .replace(ChildKey::new("body"), new, &mut ctx)
            .unwrap();

        assert_eq!(
            log.borrow().as_slice(),
            &[
                "old init",
                "old mount",
                "old unmount",
                "old destroy",
                "new init",
                "new mount"
            ]
        );
    }

    #[test]
    fn live_replace_requests_layout_and_redraw() {
        let mut children = Children::new().child(ChildKey::new("body"), TestNode::default());
        let mut ctx = EventCtx::default();

        children
            .replace(ChildKey::new("body"), TestNode::default(), &mut ctx)
            .unwrap();

        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn live_insert_merges_lifecycle_effects_into_event_context() {
        let mut children = Children::new();
        let mut lifecycle = LifecycleCtx::default();
        let mut ctx = EventCtx::default();

        children.mount(&mut lifecycle);
        children.insert("body", LifecycleEmitter, &mut ctx).unwrap();

        assert_eq!(ctx.messages(), &["init", "mount"]);
        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn live_remove_requests_layout_and_redraw() {
        let mut children = Children::new().child(ChildKey::new("body"), TestNode::default());
        let mut ctx = EventCtx::default();

        children.remove(ChildKey::new("body"), &mut ctx).unwrap();

        assert!(ctx.layout_requested());
        assert!(ctx.redraw_requested());
    }

    #[test]
    fn live_remove_requests_focus_repair_at_removed_index() {
        let mut children = Children::new()
            .child(ChildKey::new("one"), TestNode::default())
            .child(ChildKey::new("two"), TestNode::default())
            .child(ChildKey::new("three"), TestNode::default());
        let mut ctx = EventCtx::default();

        children.remove(ChildKey::new("two"), &mut ctx).unwrap();

        assert_eq!(
            ctx.focus_repair(),
            Some(FocusRepair::RemovedChild { index: 1 })
        );
    }

    #[test]
    fn children_tick_merges_child_results() {
        let mut children = Children::new().child(ChildKey::new("body"), TestNode::default());

        let result = children.tick(Duration::from_millis(16), AnimationSettings::default());

        assert_eq!(result, TickResult::CHANGED);
    }

    #[test]
    fn routed_dispatch_peels_nested_paths_and_bubbles_handled_events() {
        let leaf = EventTestNode {
            message: "leaf",
            outcome: EventOutcome::Handled,
            stop: false,
        };
        let mid = EventContainer {
            children: Children::new().child(ChildKey::new("leaf"), leaf),
            message: "mid",
            outcome: EventOutcome::Ignored,
        };
        let mut root = EventContainer {
            children: Children::new().child(ChildKey::new("mid"), mid),
            message: "root",
            outcome: EventOutcome::Ignored,
        };
        let route = EventRoute::new(TreePath::from_keys([
            ChildKey::new("mid"),
            ChildKey::new("leaf"),
        ]));
        let event = TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter));
        let mut ctx = EventCtx::default();

        let outcome = root.dispatch_event(&route, &event, &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["leaf", "mid", "root"]);
    }

    #[test]
    fn routed_dispatch_bubbles_ignored_children_to_parent() {
        let leaf = EventTestNode {
            message: "leaf",
            outcome: EventOutcome::Ignored,
            stop: false,
        };
        let mut root = EventContainer {
            children: Children::new().child(ChildKey::new("leaf"), leaf),
            message: "root",
            outcome: EventOutcome::Handled,
        };
        let route = EventRoute::new(TreePath::from_keys([ChildKey::new("leaf")]));
        let event = TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter));
        let mut ctx = EventCtx::default();

        let outcome = root.dispatch_event(&route, &event, &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["leaf", "root"]);
    }

    #[test]
    fn routed_dispatch_honors_stopped_propagation() {
        let leaf = EventTestNode {
            message: "leaf",
            outcome: EventOutcome::Handled,
            stop: true,
        };
        let mut root = EventContainer {
            children: Children::new().child(ChildKey::new("leaf"), leaf),
            message: "root",
            outcome: EventOutcome::Handled,
        };
        let route = EventRoute::new(TreePath::from_keys([ChildKey::new("leaf")]));
        let event = TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter));
        let mut ctx = EventCtx::default();

        let outcome = root.dispatch_event(&route, &event, &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["leaf"]);
    }

    #[test]
    fn child_slot_dispatch_peels_own_route_key() {
        let leaf = EventTestNode {
            message: "leaf",
            outcome: EventOutcome::Handled,
            stop: false,
        };
        let mut slot = ChildSlot::new(ChildKey::new("leaf"), leaf);
        let route = EventRoute::new(TreePath::from_keys([ChildKey::new("leaf")]));
        let event = TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter));
        let mut ctx = EventCtx::default();

        let outcome = slot.dispatch_event(&route, &event, &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.messages(), &["leaf"]);
    }

    #[test]
    fn empty_event_route_does_not_reach_child_slot() {
        let leaf = EventTestNode {
            message: "leaf",
            outcome: EventOutcome::Handled,
            stop: false,
        };
        let mut children = Children::new().child(ChildKey::new("leaf"), leaf);
        let route = EventRoute::new(TreePath::new());
        let event = TuiEvent::Key(crate::KeyEvent::from(crate::Key::Enter));
        let mut ctx = EventCtx::default();

        let outcome = children.dispatch_child(&ChildKey::new("leaf"), &route, &event, &mut ctx);

        assert_eq!(outcome, EventOutcome::Ignored);
        assert!(ctx.messages().is_empty());
    }

    #[test]
    fn empty_focus_path_does_not_reach_child_slot() {
        let mut children = Children::new().child(ChildKey::new("leaf"), FocusTestNode);
        let target = FocusTarget {
            id: FocusId::new("input"),
            path: TreePath::new(),
            area: Rect::default(),
            enabled: true,
        };
        let mut ctx = FocusCtx::default();

        children.dispatch_focus_target(&target, true, &mut ctx);

        assert!(ctx.messages().is_empty());
    }
}
