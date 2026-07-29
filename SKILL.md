---
name: tuicore-notes
description: Guide to tuicore layout, primitives, components, and integrations.
---

# tuicore lay of land

Tuicore is a library-first `ratatui` component runtime. Reusable components live in
`src/components/`; complete usage lives in `examples/`, especially `examples/gallery.rs`.

## Runtime

- `TuiNode<M>` is the component lifecycle: `measure`, `layout`, `render`, `event`, `focus`, and
  `tick`.
- `TreeApp` and tree routing connect nodes, focus, overlays, messages, and notifications.
- `TreeApp` dims the final composed frame on terminal focus loss by default; configure or disable it with `TerminalFocusEffect`.
- State changes happen in input/update or `tick`; rendering only reads state.
- Containers use `Children`/`ChildSlot` and explicitly forward layout, render, events, focus, and
  ticks. See existing containers in `src/components/` before writing one.

## Layout

- Prefer `Flex::row()`/`Flex::column()` for general composition and `Grid` for aligned tracks.
- Use `FlexItem::fit_content()`/`content()` for measured controls and `fill(1)` for remaining space.
- Use `Split` for two panes, `Stack` for alternate/layered children, and `Overlay` or
  `DialogLayer` for floating content.
- Avoid guessed fixed sizes. Component `measure()` includes required chrome.

## Shared primitives

- Theme supplies semantic colors through `theme()`; components never own raw palette colors.
- Preset supplies structural defaults through `preset()`: borders, tabs, scrolling, animation.
- Global `KeyBindings` defines built-in behavior keys; component builders define local overrides.
- `FocusChain`/`FocusRouter` help composite controls; apps own overall focus topology.
- `ScrollState` owns viewport offset, smooth movement, and scrollbar behavior.
- `Store` is optional synchronous app state. Components emit messages; apps dispatch store events.
  `EventLog` and `StateInspect` add opt-in debugging.

## Component navigation

- Inputs, forms, pickers, tables, dialogs, menus, tabs, status widgets, and layout containers live
  under `src/components/` and are re-exported from `src/components/mod.rs` and `src/lib.rs`.
- `DataView` handles read-oriented rows, transforms, selection, and scrolling.
- `ListControl` composes mutable rows over `DataView`; use its rustdocs and gallery demo for
  add/edit/remove/reorder configuration.
- Prefer `MenuButton` for normal menus: it owns the `Button` trigger, `Menu` popup, overlay anchor,
  synchronized hotkey, event/focus routing, ticks, lifecycle, and focus return. Use standalone
  `Menu` only when a custom or non-button trigger must own opening and routing.
- Popup owners register geometry during `layout()` and enqueue portal draws during `render()`;
  root `RenderCtx` flushes overlays.
- Reactive form types live in `src/form.rs`; controls expose validation through `FormField` chrome.
- AI integration centers on `AiDock`; Rig-facing types may remain public.

## App integration

- Register focus regions in `layout()` and route focus/events through child slots.
- Emit component messages with `ctx.emit`, redraw/layout requests with `EventCtx`, and user notices
  with `ctx.notify`.
- Advance animations through app ticks and honor global animation disable.
- Copy established patterns from gallery examples; keep reusable behavior in `src/`, not examples.
