# tuicore

Reusable `ratatui` components and direct `crossterm` tree runtime helpers.

## Use from another TUI app on disk

Add `tuicore` as a path dependency from your app's `Cargo.toml`:

```toml
[dependencies]
tuicore = { path = "../tuicore" } # replace with your local path
```

Minimal app:

```rust
use std::{error::Error, time::Duration};

use ratatui::{Frame, layout::Rect};
use tuicore::{
    AnimationSettings, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget,
    Key, KeyEvent, KeyModifiers, LayoutCtx, LayoutResult, LifecycleCtx, Panel,
    PanelHost, TextInput, TickResult, TuiEvent, TuiNode,
};

fn main() -> Result<(), Box<dyn Error>> {
    tuicore::init();
    tuicore::run(App::new())?;
    Ok(())
}

struct App {
    body: PanelHost<TextInput>,
}

impl App {
    fn new() -> Self {
        Self {
            body: Panel::new()
                .top_left("Filter")
                .host(TextInput::new().placeholder("Search…")),
        }
    }

    fn quit_key(event: &TuiEvent) -> bool {
        let TuiEvent::Key(KeyEvent { code, modifiers }) = event else {
            return false;
        };

        *code == Key::Esc
            || (matches!(*code, Key::Char(value) if value.eq_ignore_ascii_case(&'c'))
                && modifiers.contains(KeyModifiers::CONTROL))
    }
}

impl TuiNode for App {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.body.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.body.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
        if Self::quit_key(event) {
            ctx.request_quit();
            ctx.stop_propagation();
            EventOutcome::Handled
        } else {
            self.body.event(event, ctx)
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
            self.body.dispatch_event(route, event, ctx)
        }
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<()>) {
        self.body.dispatch_focus(target, focused, ctx);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.body.tick(dt, settings)
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.body.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.body.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.body.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<()>) {
        self.body.destroy(ctx);
    }
}
```

Layout composition:

```rust
use tuicore::{Flex, FlexItem, Panel, Split, TextInput};

let sidebar = Panel::new().top_left("Nav").content(["Home", "Logs"]);
let search = Panel::new()
    .top_left("Search")
    .host(TextInput::new().placeholder("Filter…"));
let details = Panel::new().top_left("Details").content(["Ready"]);

let main = Flex::column()
    .gap(1)
    .child("search", search, FlexItem::fixed(3))
    .child("details", details, FlexItem::fill(1));

let root = Split::horizontal(sidebar, main).ratio(1, 3);
```

Useful public exports:

- App wiring: `tuicore::run`, `TreeApp`
- Tree contracts: `TuiNode`, `EventCtx`, `LayoutCtx`, `FocusCtx`, `LifecycleCtx`
- Events and keys: `TuiEvent`, `KeyEvent`, `Key`, `KeyModifiers`
- Layout/components: `Panel`, `Panel::host`, `Split`, `Flex`, `FlexItem`, `Tabs`, `List`, `Spinner`
- Shared state helpers: `ScrollState`, `FocusChain`, `FocusRouter`
- Runtime config: `init`, `theme`, `preset`, `keybindings`, `animation_settings`

Run examples:

```sh
cargo run --example quickstart
cargo run --example gallery_new_runtime
```
