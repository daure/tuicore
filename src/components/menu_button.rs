use std::hash::Hash;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::button::BUTTON_FOCUS;
use super::{Button, Menu, MenuItem};
use crate::{
    Animated, AnimationSettings, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusTarget, HotkeyLabelMode, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, RenderCtx, TickResult, TuiEvent, TuiNode,
};

const TRIGGER_SLOT: &str = "trigger";
const MENU_SLOT: &str = "menu";
const MENU_ARROW_DOWN: &str = "";
const MENU_ARROW_UP: &str = "";

pub struct MenuButton<Id, M = ()> {
    button: Button<M>,
    menu: Menu<Id>,
    trigger_label: String,
}

impl<Id, M> MenuButton<Id, M>
where
    Id: Clone + Eq + Hash + 'static,
{
    pub fn new(
        trigger_label: impl Into<String>,
        items: impl IntoIterator<Item = MenuItem<Id>>,
    ) -> Self {
        let trigger_label = trigger_label.into();
        let mut button = Button::new(trigger_label.clone());
        button.set_trailing_label(MENU_ARROW_DOWN);
        Self {
            button,
            menu: Menu::new(items),
            trigger_label,
        }
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        let hotkey = hotkey.into();
        self.button.set_hotkey(hotkey.clone());
        self.menu.set_trigger_hotkey(hotkey);
        self
    }

    pub fn hotkey_label_mode(mut self, mode: HotkeyLabelMode) -> Self {
        self.button.set_hotkey_label_mode(mode);
        self
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.trigger_label = label.into();
        self.button.set_label(self.trigger_label.clone());
        self.sync_trigger_label();
    }

    pub fn visible_items(mut self, count: u16) -> Self {
        self.menu = self.menu.visible_items(count);
        self
    }

    pub fn min_popup_width(mut self, width: u16) -> Self {
        self.menu = self.menu.min_popup_width(width);
        self
    }

    pub fn take_activated(&mut self) -> Vec<Id> {
        self.menu.take_activated()
    }

    pub fn is_open(&self) -> bool {
        self.menu.is_open()
    }

    fn trigger_key() -> ChildKey {
        ChildKey::new(TRIGGER_SLOT)
    }

    fn menu_key() -> ChildKey {
        ChildKey::new(MENU_SLOT)
    }

    fn sync_trigger_label(&mut self) {
        let arrow = if self.menu.is_open() {
            MENU_ARROW_UP
        } else {
            MENU_ARROW_DOWN
        };
        self.button.set_trailing_label(arrow);
    }

    fn dispatch_trigger(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome
    where
        M: 'static,
    {
        let (outcome, pressed) = self.button.dispatch_event_with_press(event, ctx);
        if pressed {
            self.menu.toggle_with_context(ctx);
            self.sync_trigger_label();
        }
        outcome
    }
}

impl<Id, M> TuiNode<M> for MenuButton<Id, M>
where
    Id: Clone + Eq + Hash + 'static,
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.button.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let trigger_path = ctx.current_path().child(Self::trigger_key());
        self.menu
            .set_return_focus_to(trigger_path, FocusId::new(BUTTON_FOCUS));
        ctx.push_slot(Self::trigger_key(), area, |ctx| {
            self.button.layout(area, ctx);
        });
        ctx.push_slot(Self::menu_key(), area, |ctx| {
            <Menu<Id> as TuiNode<M>>::layout(&mut self.menu, area, ctx);
        });
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        TuiNode::<M>::render(&self.button, frame, area, ctx);
        TuiNode::<M>::render(&self.menu, frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        if self.menu.is_open() {
            let outcome = self.menu.event(event, ctx);
            self.sync_trigger_label();
            outcome
        } else {
            self.dispatch_trigger(event, ctx)
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
        if let Some(path) = route.path.without_first_if(&Self::trigger_key()) {
            if !path.is_empty() {
                return EventOutcome::Ignored;
            }
            let (outcome, pressed) = self.button.dispatch_event_with_press(event, ctx);
            if pressed {
                self.menu.toggle_with_context(ctx);
                self.sync_trigger_label();
            }
            return outcome;
        }
        let Some(path) = route.path.without_first_if(&Self::menu_key()) else {
            return EventOutcome::Ignored;
        };
        let outcome = self.menu.dispatch_event(&EventRoute::new(path), event, ctx);
        self.sync_trigger_label();
        outcome
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.button, dt, settings).merge(Animated::tick(
            &mut self.menu,
            dt,
            settings,
        ))
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if let Some(target) = target.for_child(&Self::trigger_key()) {
            self.button.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&Self::menu_key()) {
            self.menu.dispatch_focus(&target, focused, ctx);
            self.sync_trigger_label();
        }
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.button.init(ctx);
        self.menu.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.button.mount(ctx);
        self.menu.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.menu.unmount(ctx);
        self.button.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.menu.destroy(ctx);
        self.button.destroy(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FocusRequest, HotkeyEvent, Key, KeyEvent, Propagation, TreePath};

    fn menu_button(hotkey: &str) -> MenuButton<&'static str> {
        MenuButton::new(
            "Open menu",
            [
                MenuItem::new("new", "New file"),
                MenuItem::new("open", "Open recent"),
            ],
        )
        .hotkey(hotkey)
    }

    fn layout(menu_button: &mut MenuButton<&'static str>) -> LayoutCtx {
        let mut ctx = LayoutCtx::new();
        ctx.with_overlay_bounds(Rect::new(0, 0, 40, 12), |ctx| {
            menu_button.layout(Rect::new(2, 1, 15, 1), ctx);
        });
        ctx
    }

    fn route(slot: &str) -> EventRoute {
        EventRoute::new(TreePath::from_keys([ChildKey::new(slot)]))
    }

    #[test]
    fn enter_and_space_on_trigger_open_and_request_runtime_updates() {
        for key in [Key::Enter, Key::Char(' ')] {
            let mut menu_button = menu_button("m");
            layout(&mut menu_button);
            let mut ctx = EventCtx::default();

            let outcome = menu_button.dispatch_event(
                &route(TRIGGER_SLOT),
                &TuiEvent::Key(KeyEvent::from(key)),
                &mut ctx,
            );

            assert_eq!(outcome, EventOutcome::Handled);
            assert!(menu_button.is_open());
            assert!(ctx.layout_requested());
            assert!(ctx.redraw_requested());
            assert_eq!(
                ctx.focus_request(),
                Some(&FocusRequest::TargetAt {
                    path: TreePath::from_keys([ChildKey::new(MENU_SLOT)]),
                    id: FocusId::new("search"),
                })
            );
            assert_eq!(ctx.propagation(), Propagation::Stopped);
        }
    }

    #[test]
    fn hotkey_opens_closed_menu_and_closes_from_popup() {
        let mut menu_button = menu_button("m");
        layout(&mut menu_button);
        let mut open_ctx = EventCtx::default();
        menu_button.dispatch_event(
            &route(TRIGGER_SLOT),
            &TuiEvent::Hotkey(HotkeyEvent::Commit("m".into())),
            &mut open_ctx,
        );
        assert!(menu_button.is_open());

        layout(&mut menu_button);
        let mut close_ctx = EventCtx::default();
        menu_button.dispatch_event(
            &route(MENU_SLOT),
            &TuiEvent::Key(KeyEvent::from(Key::Char('m'))),
            &mut close_ctx,
        );

        assert!(!menu_button.is_open());
        assert_eq!(
            close_ctx.focus_request(),
            Some(&FocusRequest::TargetAt {
                path: TreePath::from_keys([ChildKey::new(TRIGGER_SLOT)]),
                id: FocusId::new(BUTTON_FOCUS),
            })
        );
    }

    #[test]
    fn pending_multikey_hotkey_waits_for_commit() {
        let mut menu_button = menu_button("op");
        layout(&mut menu_button);
        let mut ctx = EventCtx::default();

        let pending = menu_button.dispatch_event(
            &route(TRIGGER_SLOT),
            &TuiEvent::Hotkey(HotkeyEvent::Pending("o".into())),
            &mut ctx,
        );

        assert_eq!(pending, EventOutcome::Ignored);
        assert!(!menu_button.is_open());

        let completed = menu_button.dispatch_event(
            &route(TRIGGER_SLOT),
            &TuiEvent::Hotkey(HotkeyEvent::Commit("op".into())),
            &mut ctx,
        );
        assert_eq!(completed, EventOutcome::Handled);
        assert!(menu_button.is_open());
    }

    #[test]
    fn activated_menu_item_is_observable() {
        let mut menu_button = menu_button("m");
        layout(&mut menu_button);
        let mut open_ctx = EventCtx::default();
        menu_button.dispatch_event(
            &route(TRIGGER_SLOT),
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut open_ctx,
        );
        layout(&mut menu_button);
        let mut activate_ctx = EventCtx::default();

        menu_button.dispatch_event(
            &route(MENU_SLOT),
            &TuiEvent::Key(KeyEvent::from(Key::Enter)),
            &mut activate_ctx,
        );

        assert_eq!(menu_button.take_activated(), vec!["new"]);
        assert!(!menu_button.is_open());
    }

    #[test]
    fn layout_uses_internal_focus_and_overlay_paths() {
        let mut menu_button = menu_button("m");
        let closed = layout(&mut menu_button);
        assert_eq!(
            closed.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::new(TRIGGER_SLOT)])
        );

        menu_button.menu.open();
        let mut open = layout(&mut menu_button);
        let overlays = open.drain_overlays();
        assert_eq!(overlays.len(), 1);
        assert_eq!(
            overlays[0].route_path,
            TreePath::from_keys([ChildKey::new(MENU_SLOT)])
        );
        assert_eq!(overlays[0].anchor, Rect::new(2, 1, 15, 1));
        assert_eq!(overlays[0].bounds, Rect::new(0, 0, 40, 12));
    }
}
