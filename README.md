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
use std::error::Error;

use tuicore::{Panel, TextInput};

fn main() -> Result<(), Box<dyn Error>> {
    tuicore::init();
    tuicore::run(
        Panel::new()
            .top_left("Filter")
            .host(TextInput::new().placeholder("Search…")),
    )?;
    Ok(())
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
cargo run --example gallery
```
