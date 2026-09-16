# tuicore

Reusable `ratatui` components and direct `crossterm` tree runtime helpers.

## Add to your app

Add the crates.io version to your app's `Cargo.toml`:

```toml
[dependencies]
tuicore = "0.40"
```

For local development against your working copy, keep the version dependency in the app and add a personal override to `~/.cargo/config.toml`:

```toml
[patch.crates-io]
tuicore = { path = "/absolute/path/to/tuicore" }
```

Cargo uses the local version when it satisfies the dependency requirement and is selected by the lockfile; run `cargo update -p tuicore` when needed. CI machines without this override use the published crate. Publish required Tuicore changes before updating consumer applications.

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
- Layout/components: `Panel`, `Panel::host`, `Split`, `Flex`, `FlexItem`, `Tabs`, `List`, `Spinner`, `Image`
- Shared state helpers: `ScrollState`, `FocusChain`, `FocusRouter`
- Runtime config: `init`, `theme`, `preset`, `keybindings`, `animation_settings`

Run the gallery from source:

```sh
cargo run --bin gallery
```

## Install the gallery

The [latest GitHub Release](https://github.com/daure/tuicore/releases/latest) provides a prebuilt gallery for **Ubuntu 24.04 or newer on x86_64**, plus the library's `.crate` package and SHA-256 checksums. Rust is not required to run the gallery.

```sh
installer="$(mktemp)"
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/daure/tuicore/releases/latest/download/tuicore-installer.sh \
  -o "$installer" && sh "$installer"
rm -f "$installer"
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
gallery --version
gallery
```

The installer places `gallery` in `$CARGO_HOME/bin` (default `~/.cargo/bin`). Close the gallery and rerun the installer commands to update it. A terminal is required for interactive use; `gallery --help` and `gallery --version` work without one. Applications use Tuicore through Cargo, independently of the gallery installation.

## Release

From a clean `main` checkout with Git push access, Python 3.11+, Rust, and an authenticated GitHub CLI (`gh auth login`):

```sh
cargo release          # patch release
cargo release minor
cargo release major
# Equivalent command without compiling the tiny Cargo helper:
./scripts/release.sh patch
```

The command checks the branch and version availability, then runs the release workflow's formatting, script tests, Clippy, and Rust tests before changing the version. It bumps Tuicore, updates the lockfile without personal Cargo overrides, creates an annotated tag without opening an editor or pager, commits, and atomically pushes `main` and its `vX.Y.Z` tag. It returns without waiting for GitHub Actions. Existing `cargo patch`, `cargo minor`, and `cargo major` aliases use the same release flow.

The [Release workflow](https://github.com/daure/tuicore/actions/workflows/release.yml) checks formatting, runs strict Clippy and tests, validates the crate package, then builds and smoke-tests the gallery with cargo-dist. After checks pass it publishes the library to crates.io using the encrypted `CARGO_REGISTRY_TOKEN` repository secret, then publishes the GitHub Release containing the library package, gallery archive, installer, and checksums. Configure that secret with a token authorized to publish `tuicore`.

Normal pushes to `main` run checks and warm debug and optimized dependency caches. Tagged releases restore those caches; only `main` saves them so later tags can access them. Release-version commits skip the redundant branch build. Gallery builds disable LTO and strip symbols to favor build speed. No temporary Actions artifact uploads are needed. First builds after cache eviction or toolchain/dependency changes can take longer. To warm the cache manually without publishing, run `gh workflow run release.yml --ref main`.

Inspect runs with `gh run list --workflow release.yml` and `gh run watch RUN_ID`. Retry transient failures with `gh run rerun RUN_ID --failed`; an already-published, non-yanked crate version is skipped. For a source fix, commit the fix and make a new patch release. Tags are immutable: do not move a published tag. If the local push fails, inspect the release commit/tag and use the retry command printed by the script.

## License

Licensed under either MIT or Apache-2.0, at your option.
