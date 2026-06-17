use std::{error::Error, time::Duration};

use ratatui::{Frame, layout::Rect};
use tuicore::{
    AnimationSettings, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget, Key, KeyEvent,
    KeyModifiers, LayoutCtx, LayoutResult, LifecycleCtx, Panel, PanelHost, TextInput, TickResult,
    TuiEvent, TuiNode,
};

fn main() -> Result<(), Box<dyn Error>> {
    tuicore::init();
    tuicore::run(Quickstart::new())?;
    Ok(())
}

struct Quickstart {
    inner: PanelHost<TextInput>,
}

impl Quickstart {
    fn new() -> Self {
        Self {
            inner: Panel::new()
                .top_left("Filter")
                .host(TextInput::new().placeholder("Search…")),
        }
    }

    fn quit_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };

        matches!(*code, Key::Char(value) if value.eq_ignore_ascii_case(&'q'))
            && modifiers.contains(KeyModifiers::CONTROL)
    }
}

impl TuiNode for Quickstart {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.inner.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.inner.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            self.inner.event(event, ctx)
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<()>,
    ) -> EventOutcome {
        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            self.inner.dispatch_event(route, event, ctx)
        }
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<()>) {
        self.inner.dispatch_focus(target, focused, ctx);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.inner.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.inner.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.inner.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.inner.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.inner.destroy(ctx);
    }
}
