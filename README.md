# Tuicore

Composable Rust components and a runtime for terminal applications.

[![Crates.io](https://img.shields.io/crates/v/tuicore.svg)](https://crates.io/crates/tuicore)
[![Docs](https://docs.rs/tuicore/badge.svg)](https://docs.rs/tuicore)
[![Build](https://github.com/daure/tuicore/actions/workflows/release.yml/badge.svg)](https://github.com/daure/tuicore/actions/workflows/release.yml)

[Quick start](#quick-start) · [Examples](#examples) · [API docs](https://docs.rs/tuicore) · [Reference](https://github.com/daure/tuicore/blob/main/docs/reference.md)

Built on Ratatui and Crossterm. Compose UI trees with Rust builders; the runtime handles terminal setup, events, focus, and animation ticks.

## Features

- Panels, flex layouts, grids, tabs, and scroll containers.
- Inputs, searchable dropdowns, tables, trees, forms, and dialogs.
- Shared themes, configurable keybindings, and animations.

## Quick start

Add to `Cargo.toml`:

```toml
[dependencies]
tuicore = "0.40"
```

Create an app:

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

Use a [Nerd Font](https://www.nerdfonts.com/) for icons.

## Examples

Run the component gallery:

```sh
git clone https://github.com/daure/tuicore.git
cd tuicore
cargo run --bin gallery
```

| Example | Focus |
| --- | --- |
| [App shell](https://github.com/daure/tuicore/blob/main/examples/app_shell.rs) | Tabs, data, details, status bar |
| [Layouts](https://github.com/daure/tuicore/blob/main/examples/layouts.rs) | Layout composition |
| [Data views](https://github.com/daure/tuicore/blob/main/examples/data_view.rs) | Tables and trees |
| [Forms](https://github.com/daure/tuicore/blob/main/examples/forms.rs) | Controls and validation |
| [Dialogs](https://github.com/daure/tuicore/blob/main/examples/dialogs.rs) | Modals |

[Prebuilt gallery](https://github.com/daure/tuicore/blob/main/docs/reference.md#install-the-gallery): Ubuntu 24.04+, x86_64; no Rust required.

## Documentation and contributing

- [API reference](https://docs.rs/tuicore) · [Layouts and local dependencies](https://github.com/daure/tuicore/blob/main/docs/reference.md#add-to-your-app)
- [Design and integration](https://github.com/daure/tuicore/blob/main/SKILL.md)
- [Verification and releases](https://github.com/daure/tuicore/blob/main/docs/reference.md#release)
- [Issues](https://github.com/daure/tuicore/issues)

## License

[MIT](https://github.com/daure/tuicore/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/daure/tuicore/blob/main/LICENSE-APACHE), at your option.
