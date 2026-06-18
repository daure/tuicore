use std::time::Duration;

use crate::{
    AnimationSettings, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusRepair, FocusRequest,
    FocusTarget, Propagation, TickResult, TuiEvent, TuiNode,
};

use super::FocusTransition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEffects<M> {
    pub outcome: EventOutcome,
    pub messages: Vec<M>,
    pub redraw: bool,
    pub layout: bool,
    pub quit: bool,
    pub focus_request: Option<FocusRequest>,
    pub focus_repair: Option<FocusRepair>,
    pub propagation: Propagation,
    pub clear: bool,
}

#[derive(Debug, Default)]
pub struct TreeDispatcher;

impl TreeDispatcher {
    pub fn new() -> Self {
        Self
    }

    pub fn dispatch_event<N, M>(
        &mut self,
        root: &mut N,
        route: &EventRoute,
        event: &TuiEvent,
        settings: AnimationSettings,
    ) -> DispatchEffects<M>
    where
        N: TuiNode<M>,
    {
        let mut ctx = EventCtx::new(settings);
        let outcome = root.dispatch_event(route, event, &mut ctx);
        DispatchEffects::from_event_ctx(outcome, ctx)
    }

    pub fn dispatch_tick<N, M>(
        &mut self,
        root: &mut N,
        dt: Duration,
        settings: AnimationSettings,
    ) -> TickResult
    where
        N: TuiNode<M>,
    {
        root.tick(dt, settings)
    }

    pub fn dispatch_focus<N, M>(
        &mut self,
        root: &mut N,
        transition: FocusTransition,
        settings: AnimationSettings,
    ) -> DispatchEffects<M>
    where
        N: TuiNode<M>,
    {
        let mut ctx = FocusCtx::new(settings);

        if let Some(previous) = transition.previous.as_ref() {
            dispatch_focus_target(root, previous, false, &mut ctx);
        }
        if let Some(current) = transition.current.as_ref() {
            dispatch_focus_target(root, current, true, &mut ctx);
        }

        DispatchEffects {
            outcome: EventOutcome::Handled,
            messages: ctx.drain_messages().collect(),
            redraw: true,
            layout: true,
            quit: false,
            focus_request: ctx.focus_request().cloned(),
            focus_repair: None,
            propagation: Propagation::Continue,
            clear: false,
        }
    }
}

impl<M> DispatchEffects<M> {
    pub fn idle() -> Self {
        Self {
            outcome: EventOutcome::Ignored,
            messages: Vec::new(),
            redraw: false,
            layout: false,
            quit: false,
            focus_request: None,
            focus_repair: None,
            propagation: Propagation::Continue,
            clear: false,
        }
    }

    pub fn from_event_ctx(outcome: EventOutcome, mut ctx: EventCtx<M>) -> Self {
        let redraw = outcome.handled() || ctx.redraw_requested();
        let layout = outcome.handled() || ctx.layout_requested();
        let quit = ctx.quit_requested();
        let focus_request = ctx.focus_request().cloned();
        let focus_repair = ctx.focus_repair();
        let propagation = ctx.propagation();
        let messages = ctx.drain_messages().collect();
        let clear = ctx.clear_requested();

        Self {
            outcome,
            messages,
            redraw,
            layout,
            quit,
            focus_request,
            focus_repair,
            propagation,
            clear,
        }
    }
}

fn dispatch_focus_target<N, M>(
    root: &mut N,
    target: &FocusTarget,
    focused: bool,
    ctx: &mut FocusCtx<M>,
) where
    N: TuiNode<M>,
{
    root.dispatch_focus(target, focused, ctx);
}

#[cfg(test)]
mod tests {
    use ratatui::{Frame, layout::Rect};

    use super::*;
    use crate::{ChildKey, Children, FocusId, Key, KeyEvent, LayoutCtx, LayoutResult, TreePath};

    #[derive(Default)]
    struct EventNode {
        events: usize,
    }

    impl TuiNode<&'static str> for EventNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<&'static str>) -> EventOutcome {
            self.events += 1;
            ctx.emit("event");
            ctx.request_redraw();
            EventOutcome::Handled
        }
    }

    #[derive(Default)]
    struct FocusLeaf {
        focused: Vec<(String, bool)>,
    }

    impl TuiNode<&'static str> for FocusLeaf {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn focus(
            &mut self,
            target: Option<&FocusId>,
            focused: bool,
            ctx: &mut FocusCtx<&'static str>,
        ) {
            self.focused
                .push((target.unwrap().as_str().to_owned(), focused));
            ctx.emit("focus");
        }
    }

    struct FocusContainer {
        children: Children<&'static str>,
    }

    #[derive(Default)]
    struct AnimationFocusLeaf {
        animation_enabled: Option<bool>,
    }

    impl TuiNode<&'static str> for FocusContainer {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn dispatch_focus(
            &mut self,
            target: &FocusTarget,
            focused: bool,
            ctx: &mut FocusCtx<&'static str>,
        ) {
            self.children.dispatch_focus_target(target, focused, ctx);
        }
    }

    impl TuiNode<()> for AnimationFocusLeaf {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn focus(&mut self, _target: Option<&FocusId>, _focused: bool, ctx: &mut FocusCtx<()>) {
            self.animation_enabled = Some(ctx.animation().enabled);
        }
    }

    #[test]
    fn dispatcher_calls_root_and_drains_messages() {
        let mut root = EventNode::default();
        let mut dispatcher = TreeDispatcher::new();
        let route = EventRoute::new(Default::default());
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let effects =
            dispatcher.dispatch_event(&mut root, &route, &event, AnimationSettings::default());

        assert_eq!(root.events, 1);
        assert!(effects.redraw);
        assert!(effects.layout);
        assert_eq!(effects.messages, vec!["event"]);
    }

    #[test]
    fn dispatcher_does_not_deliver_unpeeled_routes_to_leaf_nodes() {
        let mut root = EventNode::default();
        let mut dispatcher = TreeDispatcher::new();
        let route = EventRoute::new(TreePath::from_keys([ChildKey::new("child")]));
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let effects =
            dispatcher.dispatch_event(&mut root, &route, &event, AnimationSettings::default());

        assert_eq!(root.events, 0);
        assert_eq!(effects.outcome, EventOutcome::Ignored);
    }

    #[test]
    fn dispatcher_routes_focus_by_full_tree_path() {
        let leaf = FocusLeaf::default();
        let child = FocusContainer {
            children: Children::new().child(ChildKey::new("leaf"), leaf),
        };
        let mut root = FocusContainer {
            children: Children::new().child(ChildKey::new("child"), child),
        };
        let target = FocusTarget {
            id: FocusId::new("input"),
            path: TreePath::from_keys([ChildKey::new("child"), ChildKey::new("leaf")]),
            area: Rect::default(),
            enabled: true,
        };
        let transition = FocusTransition {
            previous: None,
            current: Some(target),
        };
        let mut dispatcher = TreeDispatcher::new();

        let effects =
            dispatcher.dispatch_focus(&mut root, transition, AnimationSettings::default());

        assert_eq!(effects.outcome, EventOutcome::Handled);
        assert!(effects.redraw);
        assert!(effects.layout);
        assert_eq!(effects.messages, vec!["focus"]);
    }

    #[test]
    fn dispatcher_passes_animation_settings_to_focus_context() {
        let mut root = AnimationFocusLeaf::default();
        let transition = FocusTransition {
            previous: None,
            current: Some(FocusTarget {
                id: FocusId::new("input"),
                path: TreePath::new(),
                area: Rect::default(),
                enabled: true,
            }),
        };
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut dispatcher = TreeDispatcher::new();

        dispatcher.dispatch_focus(&mut root, transition, settings);

        assert_eq!(root.animation_enabled, Some(false));
    }
}
