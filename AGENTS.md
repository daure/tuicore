# tuicore constitution

- Library-first Rust TUI crate. Examples live in `examples/`; reusable code lives in `src/`.
- Core stack: `ratatui` + `tuirealm`. Components should remain usable by downstream apps.
- Keep APIs small, composable, and Rust-idiomatic. Prefer explicit state ownership over magic.
- Render purity: input/update starts animations, `tick(dt)` advances, `view/render` only reads state.
- Cross-cutting concerns:
  - Theme = semantic colors only; components use roles from `theme()`.
  - Preset = structural defaults: borders, tabs, scroll, animation.
  - Keybindings = configurable behavior keys; do not hardcode when a binding exists.
  - Animation = global defaults + optional component override; global disabled is kill switch.
  - Borders = shared `BorderKind` and helpers.
  - Scrolling = reusable `ScrollState`; smooth offset animation only, no stagger/per-row delay.
- Components so far:
  - `Tabs`: reusable tab container with variants, borders, focus styling, animated transitions.
  - `Panel`: bordered container with left/right title slots and optional scrollable text content.
  - `ScrollState`: reusable vertical/horizontal scroll state, layout, key handling, scrollbars.
- Avoid half APIs: public config should be constructible in Rust, not only via TOML.
- Keep gallery demos honest: use real components/patterns consumers should copy.
