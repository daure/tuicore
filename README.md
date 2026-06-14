# tuicore

Reusable `ratatui` + `tuirealm` components and app helpers.

## Use from another TUI app on disk

Add `tuicore` as a path dependency from your app's `Cargo.toml`:

```toml
[dependencies]
tuicore = { path = "../tuicore" } # replace with your local path
tuirealm = "4"
```

Then initialize tuicore once before mounting components. Minimal mount-only example:

```rust
use tuicore::{Animated, Panel, TuicoreApp};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, NoUserEvent};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::ratatui::{layout::Rect, Frame};
use tuirealm::state::State;

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
enum Id {
    Main,
}

#[derive(Debug, PartialEq)]
enum Msg {
    Redraw,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tuicore::init();

    let mut app = TuicoreApp::<Id, Msg, NoUserEvent>::new();
    app.mount(
        Id::Main,
        MainPanel(Panel::new().top_left("Main").content(["Hello from tuicore"])),
    )?;
    app.active(&Id::Main)?;

    Ok(())
}

struct MainPanel(Panel);

impl Component for MainPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.0.view(frame, area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        self.0.query(attr)
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.0.attr(attr, value);
    }

    fn state(&self) -> State {
        self.0.state()
    }

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        self.0.perform(cmd)
    }
}

impl AppComponent<Msg, NoUserEvent> for MainPanel {
    fn on(&mut self, event: &Event<NoUserEvent>) -> Option<Msg> {
        match event {
            Event::Tick => {
                let settings = tuicore::animation_settings();
                self.0
                    .tick(settings.frame_duration(), settings)
                    .changed
                    .then_some(Msg::Redraw)
            }
            _ => Some(Msg::Redraw),
        }
    }
}
```

Useful public exports:

- App wiring: `TuicoreApp`
- Components: `Panel`, `Tabs`, `Tab`, `List`, `Spinner`
- Component variants: `PanelVariant`, `TabsVariant`, `BorderKind`
- Shared state helpers: `ScrollState`, `FocusChain`
- Runtime config: `init`, `theme`, `preset`, `keybindings`, `animation_settings`

Run the quickstart example to verify app wiring compiles, or the gallery to inspect full terminal loop patterns:

```sh
cargo run --example quickstart
cargo run --example gallery
```
