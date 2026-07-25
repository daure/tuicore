# tuicore

Reusable `ratatui` components and direct `crossterm` tree runtime helpers.

## Add to your app

After release, add the crates.io version to your app's `Cargo.toml`:

```toml
[dependencies]
tuicore = "0.1"
```

For local development, use `tuicore = { path = "../tuicore" }` instead.

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

## Release

Prerequisites: Bash, Git, Cargo, Python 3, a clean working tree, and crates.io
credentials configured with `cargo login`.

```sh
cargo patch
cargo minor
cargo major
```

These repository-local Cargo aliases work directly inside the Tuicore checkout.
Direct fallback: `./scripts/release.sh [major|minor|patch]`. With no argument, it
bumps the minor version (for example, `0.1.0` to `0.2.0`).
It updates the lockfile, tests, asks before committing, validates the clean crates.io
package and dry run, then tags and asks before publishing. It never pushes; successful
publishing prints the exact push commands.

## License

Licensed under either MIT or Apache-2.0, at your option.
