use std::collections::VecDeque;

use crate::{
    AnimationSettings, EventCtx, EventRoute, FocusKeyBindings, FocusRepair, FocusRequest,
    FocusTarget, HitRegion, Key, KeyEvent, LifecycleCtx, Propagation, TreePath, TuiEvent, TuiNode,
    animation_settings, keybindings,
};

use super::{
    DispatchEffects, EventSource, FocusManager, LayoutEngine, Renderer, Result, Scheduler,
    TerminalGuard, TreeDispatcher,
};

type MessageHandler<N, M> = dyn FnMut(&mut N, M, &mut EventCtx<M>);

pub struct TreeApp<N, M = ()> {
    root: N,
    animation_settings: AnimationSettings,
    on_message: Option<Box<MessageHandler<N, M>>>,
}

#[derive(Debug, Default)]
struct RuntimeFlags {
    redraw: bool,
    layout: bool,
    quit: bool,
    focus_request: Option<FocusRequest>,
    focus_repair: Option<FocusRepair>,
    clear: bool,
}

pub fn run<N>(root: N) -> Result<()>
where
    N: TuiNode<()>,
{
    TreeApp::new(root).run()
}

impl<N, M> TreeApp<N, M> {
    pub fn new(root: N) -> Self {
        Self {
            root,
            animation_settings: animation_settings(),
            on_message: None,
        }
    }

    pub fn animation_settings(mut self, settings: AnimationSettings) -> Self {
        self.animation_settings = settings;
        self
    }

    pub fn on_message(
        mut self,
        handler: impl FnMut(&mut N, M, &mut EventCtx<M>) + 'static,
    ) -> Self {
        self.on_message = Some(Box::new(handler));
        self
    }
}

impl<N, M> TreeApp<N, M>
where
    N: TuiNode<M>,
{
    pub fn run(mut self) -> Result<()> {
        let mut terminal = TerminalGuard::new()?;
        let mut event_source = EventSource::new();
        let mut scheduler = Scheduler::new(self.animation_settings);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut renderer = Renderer::new();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
        };

        flags.merge(self.mount_root());

        let run_result = self.run_loop(
            &mut terminal,
            &mut event_source,
            &mut scheduler,
            &mut layout_engine,
            &mut focus_manager,
            &mut dispatcher,
            &mut renderer,
            &mut flags,
        );

        flags.merge(self.unmount_root());
        let restore_result = terminal.restore();

        if let Err(error) = run_result {
            return Err(error);
        }
        restore_result
    }

    fn run_loop(
        &mut self,
        terminal: &mut TerminalGuard,
        event_source: &mut EventSource,
        scheduler: &mut Scheduler,
        layout_engine: &mut LayoutEngine,
        focus_manager: &mut FocusManager,
        dispatcher: &mut TreeDispatcher,
        renderer: &mut Renderer,
        flags: &mut RuntimeFlags,
    ) -> Result<()> {
        while !flags.quit {
            if flags.clear {
                terminal.terminal_mut().clear()?;
                flags.clear = false;
                flags.layout = true;
                flags.redraw = true;
            }

            self.layout_if_pending(flags, focus_manager, layout_engine, dispatcher, terminal)?;

            self.apply_pending_focus(flags, focus_manager, layout_engine, dispatcher);

            self.layout_if_pending(flags, focus_manager, layout_engine, dispatcher, terminal)?;

            if flags.redraw {
                let area = terminal.terminal_mut().size()?.into();
                if flags.layout || layout_engine.area() != area {
                    self.layout_root(flags, focus_manager, layout_engine, dispatcher, area);
                }
                renderer.render(terminal.terminal_mut(), &self.root, area)?;
                flags.redraw = false;
            }

            if let Some(event) = event_source.poll(scheduler.timeout())? {
                self.dispatch_runtime_event(flags, focus_manager, layout_engine, dispatcher, event);
            }

            if let Some(dt) = scheduler.tick(self.animation_settings.max_dt) {
                let tick = dispatcher.dispatch_tick(&mut self.root, dt, self.animation_settings);
                flags.redraw |= tick.changed || tick.active;
                flags.layout |= tick.changed || tick.active;
            }
        }

        Ok(())
    }

    fn mount_root(&mut self) -> RuntimeFlags {
        let mut ctx = LifecycleCtx::default();
        self.root.init(&mut ctx);
        self.root.mount(&mut ctx);
        self.handle_lifecycle(ctx)
    }

    fn unmount_root(&mut self) -> RuntimeFlags {
        let mut ctx = LifecycleCtx::default();
        self.root.unmount(&mut ctx);
        self.root.destroy(&mut ctx);
        self.handle_lifecycle(ctx)
    }

    fn handle_lifecycle(&mut self, mut ctx: LifecycleCtx<M>) -> RuntimeFlags {
        let mut flags = RuntimeFlags {
            redraw: ctx.redraw_requested(),
            layout: ctx.layout_requested(),
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
        };
        let messages = ctx.drain_messages().collect();
        self.handle_messages(&mut flags, messages);
        flags
    }

    fn handle_effects(&mut self, effects: DispatchEffects<M>) -> RuntimeFlags {
        let mut flags = RuntimeFlags::from_effects(&effects);
        self.handle_messages(&mut flags, VecDeque::from(effects.messages));
        flags
    }

    fn handle_messages(&mut self, flags: &mut RuntimeFlags, mut messages: VecDeque<M>) {
        while let Some(message) = messages.pop_front() {
            let Some(handler) = self.on_message.as_mut() else {
                continue;
            };

            let mut ctx = EventCtx::new(self.animation_settings);
            handler(&mut self.root, message, &mut ctx);

            flags.redraw |= ctx.redraw_requested();
            flags.layout |= ctx.layout_requested();
            flags.quit |= ctx.quit_requested();
            if let Some(request) = ctx.focus_request().cloned() {
                flags.focus_request = Some(request);
            }
            if let Some(repair) = ctx.focus_repair() {
                flags.focus_repair = Some(repair);
            }
            messages.extend(ctx.drain_messages());
        }
    }

    fn layout_if_pending(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &mut LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        terminal: &mut TerminalGuard,
    ) -> Result<()> {
        if flags.layout {
            let area = terminal.terminal_mut().size()?.into();
            self.layout_root(flags, focus_manager, layout_engine, dispatcher, area);
        }
        Ok(())
    }

    fn layout_root(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &mut LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        area: ratatui::layout::Rect,
    ) {
        layout_engine.layout(&mut self.root, area);
        flags.layout = false;
        flags.redraw = true;
        let transition = if flags.focus_request.is_some() {
            None
        } else if let Some(repair) = flags.focus_repair.take() {
            focus_manager.repair(&repair, layout_engine.focus_targets())
        } else {
            focus_manager.validate(layout_engine.focus_targets())
        };
        if let Some(transition) = transition {
            let effects =
                dispatcher.dispatch_focus(&mut self.root, transition, self.animation_settings);
            flags.merge(self.handle_effects(effects));
        }
    }

    fn apply_pending_focus(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &LayoutEngine,
        dispatcher: &mut TreeDispatcher,
    ) {
        let Some(request) = flags.focus_request.take() else {
            return;
        };

        if let Some(transition) =
            focus_manager.apply_request(&request, layout_engine.focus_targets())
        {
            let effects =
                dispatcher.dispatch_focus(&mut self.root, transition, self.animation_settings);
            flags.merge(self.handle_effects(effects));
        }
    }

    fn dispatch_runtime_event(
        &mut self,
        flags: &mut RuntimeFlags,
        focus_manager: &mut FocusManager,
        layout_engine: &LayoutEngine,
        dispatcher: &mut TreeDispatcher,
        event: TuiEvent,
    ) {
        let event = event;
        if let TuiEvent::Key(key) = &event {
            let current_is_input = focus_manager
                .current()
                .map(|t| t.id.as_str() == "input" || t.id.as_str() == "textarea")
                .unwrap_or(false);
            if !current_is_input {
                let mut hotkey_target = None;
                for target in layout_engine.focus_targets() {
                    if target.enabled && focus_target_matches_hotkey(target, key) {
                        hotkey_target = Some(target.clone());
                        break;
                    }
                }
                if let Some(target) = hotkey_target {
                    flags.focus_request = Some(FocusRequest::TargetAt {
                        path: target.path.clone(),
                        id: target.id.clone(),
                    });
                    self.apply_pending_focus(flags, focus_manager, layout_engine, dispatcher);
                }
            }
        }

        let route = EventRoute::new(route_path_for_event(
            &event,
            layout_engine.hit_regions(),
            focus_manager.current_path(),
        ));
        if matches!(event, TuiEvent::Resize(_, _)) {
            flags.layout = true;
        }
        let effects =
            dispatcher.dispatch_event(&mut self.root, &route, &event, self.animation_settings);
        let focus_request = focus_request_from_event(&event, &effects);
        flags.merge(self.handle_effects(effects));
        if flags.focus_request.is_none() {
            flags.focus_request = focus_request;
        }
    }

    #[cfg(test)]
    fn run_test_events(
        mut self,
        events: impl IntoIterator<Item = TuiEvent>,
        area: ratatui::layout::Rect,
    ) -> Self {
        let mut scheduler = Scheduler::new(self.animation_settings);
        let mut layout_engine = LayoutEngine::new();
        let mut focus_manager = FocusManager::new();
        let mut dispatcher = TreeDispatcher::new();
        let mut flags = RuntimeFlags {
            redraw: true,
            layout: true,
            quit: false,
            focus_request: None,
            focus_repair: None,
            clear: false,
        };

        flags.merge(self.mount_root());
        self.layout_root(
            &mut flags,
            &mut focus_manager,
            &mut layout_engine,
            &mut dispatcher,
            area,
        );

        for event in events {
            if flags.quit {
                break;
            }
            if flags.layout {
                self.layout_root(
                    &mut flags,
                    &mut focus_manager,
                    &mut layout_engine,
                    &mut dispatcher,
                    area,
                );
            }
            self.apply_pending_focus(
                &mut flags,
                &mut focus_manager,
                &layout_engine,
                &mut dispatcher,
            );
            self.dispatch_runtime_event(
                &mut flags,
                &mut focus_manager,
                &layout_engine,
                &mut dispatcher,
                event,
            );
            if let Some(dt) = scheduler.tick(self.animation_settings.max_dt) {
                let tick = dispatcher.dispatch_tick(&mut self.root, dt, self.animation_settings);
                flags.redraw |= tick.changed || tick.active;
                flags.layout |= tick.changed || tick.active;
            }
        }

        flags.merge(self.unmount_root());
        self
    }
}

fn route_path_for_event(
    event: &TuiEvent,
    hit_regions: &[HitRegion],
    focused_path: TreePath,
) -> TreePath {
    let TuiEvent::Mouse(mouse) = event else {
        return focused_path;
    };

    hit_regions
        .iter()
        .rev()
        .find(|region| region.contains(mouse.column, mouse.row))
        .map(|region| region.path.clone())
        .unwrap_or(focused_path)
}

fn focus_request_from_event<M>(
    event: &TuiEvent,
    effects: &DispatchEffects<M>,
) -> Option<FocusRequest> {
    let bindings = keybindings();
    focus_request_from_event_with_bindings(event, effects, bindings.focus())
}

fn focus_request_from_event_with_bindings<M>(
    event: &TuiEvent,
    effects: &DispatchEffects<M>,
    focus: &FocusKeyBindings,
) -> Option<FocusRequest> {
    if effects.propagation == Propagation::Stopped || effects.focus_request.is_some() {
        return None;
    }

    let TuiEvent::Key(key) = event else {
        return None;
    };
    if focus.next_matches(*key) {
        Some(FocusRequest::Next)
    } else if focus.previous_matches(*key) {
        Some(FocusRequest::Previous)
    } else if focus.unfocus_matches(*key) {
        Some(FocusRequest::Unfocus)
    } else {
        None
    }
}

impl RuntimeFlags {
    fn from_effects<M>(effects: &DispatchEffects<M>) -> Self {
        Self {
            redraw: effects.redraw,
            layout: effects.layout,
            quit: effects.quit,
            focus_request: effects.focus_request.clone(),
            focus_repair: effects.focus_repair,
            clear: effects.clear,
        }
    }

    fn merge(&mut self, other: Self) {
        self.redraw |= other.redraw;
        self.layout |= other.layout;
        self.quit |= other.quit;
        self.clear |= other.clear;
        if other.focus_request.is_some() {
            self.focus_request = other.focus_request;
        }
        if other.focus_repair.is_some() {
            self.focus_repair = other.focus_repair;
        }
    }
}

fn keys_match(a: &KeyEvent, b: &KeyEvent) -> bool {
    if a.modifiers != b.modifiers {
        return false;
    }
    match (a.code, b.code) {
        (Key::Char(c1), Key::Char(c2)) => c1.to_ascii_lowercase() == c2.to_ascii_lowercase(),
        (code_a, code_b) => code_a == code_b,
    }
}

fn focus_target_matches_hotkey(target: &FocusTarget, key: &KeyEvent) -> bool {
    target
        .hotkey
        .as_ref()
        .is_some_and(|hotkey| keys_match(hotkey, key))
        || target.hotkeys.iter().any(|hotkey| keys_match(hotkey, key))
}

#[cfg(test)]
mod tests {
    use ratatui::{Frame, layout::Rect};

    use super::*;
    use crate::{
        ChildKey, EventOutcome, Flex, FlexItem, FocusCtx, FocusId, FocusTarget, Key, KeyEvent,
        KeyModifiers, KeySpec, LayoutCtx, LayoutResult, MouseButton, MouseEvent, MouseEventKind,
        Preset, TreePath, preset, set_preset,
    };

    #[derive(Default)]
    struct QuitNode {
        mounted: bool,
        destroyed: bool,
        events: usize,
    }

    #[derive(Default)]
    struct LifecycleMessageNode {
        messages: Vec<&'static str>,
    }

    #[derive(Default)]
    struct DynamicFocusNode {
        show_new: bool,
        events: usize,
        focused: Option<String>,
    }

    struct RemoveFocusedMiddleNode {
        flex: Flex,
        focus_log: std::rc::Rc<std::cell::RefCell<Vec<(String, bool)>>>,
        removed: bool,
    }

    #[derive(Default)]
    struct ModalRestoreNode {
        active: bool,
        focused: Vec<String>,
    }

    struct FocusProbe {
        name: &'static str,
        focus_log: std::rc::Rc<std::cell::RefCell<Vec<(String, bool)>>>,
    }

    struct MouseProbe {
        name: &'static str,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    struct MouseRouteNode {
        flex: Flex,
        event_log: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl TuiNode<()> for QuitNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            self.events += 1;
            ctx.request_quit();
            EventOutcome::Handled
        }

        fn mount(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.mounted = true;
        }

        fn destroy(&mut self, _ctx: &mut LifecycleCtx<()>) {
            self.destroyed = true;
        }
    }

    impl TuiNode<&'static str> for LifecycleMessageNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn mount(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("mounted");
        }

        fn unmount(&mut self, ctx: &mut LifecycleCtx<&'static str>) {
            ctx.emit("unmounted");
        }
    }

    impl TuiNode<()> for DynamicFocusNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("old"), area, true);
            if self.show_new {
                ctx.register_focusable(FocusId::new("new"), area, true);
            }
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            self.events += 1;
            if self.events == 1 {
                self.show_new = true;
                ctx.request_layout();
                ctx.focus(FocusRequest::TargetAt {
                    path: TreePath::new(),
                    id: FocusId::new("new"),
                });
            } else {
                ctx.request_quit();
            }
            EventOutcome::Handled
        }

        fn focus(&mut self, target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            if focused {
                self.focused = target.map(|target| target.as_str().to_owned());
            }
        }
    }

    impl RemoveFocusedMiddleNode {
        fn new() -> Self {
            let focus_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let flex = Flex::row()
                .child(
                    "one",
                    FocusProbe {
                        name: "one",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                )
                .child(
                    "two",
                    FocusProbe {
                        name: "two",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                )
                .child(
                    "three",
                    FocusProbe {
                        name: "three",
                        focus_log: std::rc::Rc::clone(&focus_log),
                    },
                    FlexItem::fixed(1),
                );

            Self {
                flex,
                focus_log,
                removed: false,
            }
        }
    }

    impl TuiNode<()> for RemoveFocusedMiddleNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.flex.layout(area, ctx)
        }

        fn render(&self, frame: &mut Frame, area: Rect) {
            self.flex.render(frame, area);
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            if route.path.first() == Some(&ChildKey::from("two")) && !self.removed {
                self.removed = true;
                self.flex.remove("two", ctx).unwrap();
                EventOutcome::Handled
            } else {
                self.flex.dispatch_event(route, event, ctx)
            }
        }

        fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<()>) {
            self.flex.dispatch_focus(target, focused, ctx);
        }
    }

    impl TuiNode<()> for FocusProbe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new(self.name), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, _event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            EventOutcome::Ignored
        }

        fn focus(&mut self, _target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            self.focus_log
                .borrow_mut()
                .push((self.name.to_owned(), focused));
        }
    }

    impl TuiNode<()> for ModalRestoreNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            if self.active {
                ctx.register_focusable(FocusId::new("dialog"), area, true);
            } else {
                let left = Rect::new(area.x, area.y, 1, area.height);
                let right = Rect::new(area.x.saturating_add(1), area.y, 1, area.height);
                ctx.register_focusable(FocusId::new("one"), left, true);
                ctx.register_focusable(FocusId::new("two"), right, true);
            }
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn dispatch_event(
            &mut self,
            _route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            let TuiEvent::Key(key) = event else {
                return EventOutcome::Ignored;
            };
            if !self.active && key.code == Key::Enter {
                self.active = true;
                ctx.request_layout();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            if self.active && key.code == Key::Char('x') {
                self.active = false;
                ctx.focus(FocusRequest::Last);
                ctx.request_layout();
                ctx.stop_propagation();
                return EventOutcome::Handled;
            }
            EventOutcome::Ignored
        }

        fn focus(&mut self, target: Option<&FocusId>, focused: bool, _ctx: &mut FocusCtx<()>) {
            if focused {
                if let Some(target) = target {
                    self.focused.push(target.as_str().to_owned());
                }
            }
        }
    }

    impl MouseRouteNode {
        fn new() -> Self {
            let event_log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
            let flex = Flex::row()
                .child(
                    "left",
                    MouseProbe {
                        name: "left",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                )
                .child(
                    "right",
                    MouseProbe {
                        name: "right",
                        event_log: std::rc::Rc::clone(&event_log),
                    },
                    FlexItem::fixed(5),
                );

            Self { flex, event_log }
        }
    }

    impl TuiNode<()> for MouseRouteNode {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            self.flex.layout(area, ctx)
        }

        fn render(&self, frame: &mut Frame, area: Rect) {
            self.flex.render(frame, area);
        }

        fn dispatch_event(
            &mut self,
            route: &EventRoute,
            event: &TuiEvent,
            ctx: &mut EventCtx<()>,
        ) -> EventOutcome {
            self.flex.dispatch_event(route, event, ctx)
        }
    }

    impl TuiNode<()> for MouseProbe {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}

        fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            if matches!(event, TuiEvent::Mouse(_)) {
                self.event_log.borrow_mut().push(self.name.to_owned());
                ctx.request_quit();
                EventOutcome::Handled
            } else {
                EventOutcome::Ignored
            }
        }
    }

    fn effects(propagation: Propagation) -> DispatchEffects<()> {
        DispatchEffects {
            outcome: EventOutcome::Ignored,
            messages: Vec::new(),
            redraw: false,
            layout: false,
            quit: false,
            focus_request: None,
            focus_repair: None,
            propagation,
            clear: false,
        }
    }

    #[test]
    fn test_loop_seam_mounts_dispatches_quit_and_unmounts() {
        let app = TreeApp::new(QuitNode::default());
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event], Rect::new(0, 0, 20, 5));

        assert!(app.root.mounted);
        assert!(app.root.destroyed);
        assert_eq!(app.root.events, 1);
    }

    #[test]
    fn lifecycle_messages_are_delivered_on_mount_and_unmount() {
        let app =
            TreeApp::new(LifecycleMessageNode::default()).on_message(|root, message, _ctx| {
                root.messages.push(message);
            });
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event], Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.messages, vec!["mounted", "unmounted"]);
    }

    #[test]
    fn pending_layout_runs_before_pending_focus_request() {
        let app = TreeApp::new(DynamicFocusNode::default());
        let event = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([event.clone(), event], Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.focused.as_deref(), Some("new"));
    }

    #[test]
    fn pending_last_focus_request_restores_focus_after_modal_layout() {
        let app = TreeApp::new(ModalRestoreNode::default());
        let events = [
            TuiEvent::Key(KeyEvent::from(Key::Tab)),
            TuiEvent::Key(KeyEvent::from(Key::Enter)),
            TuiEvent::Key(KeyEvent::from(Key::Char('x'))),
            TuiEvent::Key(KeyEvent::from(Key::Null)),
        ];

        let app = app.run_test_events(events, Rect::new(0, 0, 20, 5));

        assert_eq!(app.root.focused.last().map(String::as_str), Some("two"));
    }

    #[test]
    fn removing_focused_middle_child_repairs_focus_to_next_child() {
        let app = TreeApp::new(RemoveFocusedMiddleNode::new());
        let tab = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let enter = TuiEvent::Key(KeyEvent::from(Key::Enter));

        let app = app.run_test_events([tab, enter.clone(), enter], Rect::new(0, 0, 3, 1));

        assert!(app.root.removed);
        assert!(
            app.root
                .focus_log
                .borrow()
                .contains(&("three".to_owned(), true))
        );
    }

    #[test]
    fn new_uses_global_animation_settings_by_default() {
        let _guard = PresetGuard::replace(Preset::new().with_animation(AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        }));

        let app = TreeApp::<_, ()>::new(QuitNode::default());

        assert!(!app.animation_settings.enabled);
    }

    #[test]
    fn default_focus_keybindings_enqueue_next_and_previous_requests() {
        let next = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let previous = TuiEvent::Key(KeyEvent::from(Key::BackTab));
        let esc_unfocus = TuiEvent::Key(KeyEvent::from(Key::Esc));
        let ctrl_left_bracket_unfocus = TuiEvent::Key(KeyEvent {
            code: Key::Char('['),
            modifiers: KeyModifiers::CONTROL,
        });
        let bindings = FocusKeyBindings::default();

        assert_eq!(
            focus_request_from_event_with_bindings(
                &next,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Next)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &previous,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Previous)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &esc_unfocus,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Unfocus)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &ctrl_left_bracket_unfocus,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Unfocus)
        );
    }

    #[test]
    fn custom_focus_keybindings_enqueue_configured_requests() {
        let bindings = FocusKeyBindings::new()
            .with_next([KeySpec::plain('l')])
            .with_previous([KeySpec::plain('h')]);
        let next = TuiEvent::Key(KeyEvent {
            code: Key::Char('l'),
            modifiers: KeyModifiers::NONE,
        });
        let previous = TuiEvent::Key(KeyEvent {
            code: Key::Char('h'),
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(
            focus_request_from_event_with_bindings(
                &next,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Next)
        );
        assert_eq!(
            focus_request_from_event_with_bindings(
                &previous,
                &effects(Propagation::Continue),
                &bindings
            ),
            Some(FocusRequest::Previous)
        );
    }

    #[test]
    fn focus_keybindings_do_not_run_after_stopped_propagation() {
        let event = TuiEvent::Key(KeyEvent::from(Key::Tab));
        let bindings = FocusKeyBindings::default();

        assert_eq!(
            focus_request_from_event_with_bindings(
                &event,
                &effects(Propagation::Stopped),
                &bindings
            ),
            None
        );
    }

    #[test]
    fn custom_focus_keybinding_does_not_run_after_child_consumes_key() {
        let event = TuiEvent::Key(KeyEvent::from(Key::Char('x')));
        let bindings = FocusKeyBindings::new().with_next([KeySpec::plain('x')]);
        let mut effects = effects(Propagation::Continue);
        effects.outcome = EventOutcome::Handled;
        effects.propagation = Propagation::Stopped;

        assert_eq!(
            focus_request_from_event_with_bindings(&event, &effects, &bindings),
            None
        );
    }

    #[test]
    fn mouse_events_route_to_topmost_hit_region() {
        let event = TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 3,
            row: 1,
            modifiers: KeyModifiers::NONE,
        });
        let hit_regions = [
            HitRegion::new(
                TreePath::from_keys([ChildKey::from("back")]),
                Rect::new(0, 0, 5, 5),
            ),
            HitRegion::new(
                TreePath::from_keys([ChildKey::from("front")]),
                Rect::new(0, 0, 5, 5),
            ),
        ];

        let path = route_path_for_event(&event, &hit_regions, TreePath::new());

        assert_eq!(path.keys(), &[ChildKey::from("front")]);
    }

    #[test]
    fn mouse_events_dispatch_to_hit_flex_child() {
        let app = TreeApp::new(MouseRouteNode::new());
        let event = TuiEvent::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 7,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        let app = app.run_test_events([event], Rect::new(0, 0, 10, 1));

        assert_eq!(app.root.event_log.borrow().as_slice(), ["right"]);
    }

    struct PresetGuard(Preset);

    impl PresetGuard {
        fn replace(next: Preset) -> Self {
            let previous = preset();
            set_preset(next);
            Self(previous)
        }
    }

    impl Drop for PresetGuard {
        fn drop(&mut self) {
            set_preset(self.0.clone());
        }
    }
}
