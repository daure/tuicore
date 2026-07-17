---
name: tuicore-notes
description: Guide to tuicore layout, primitives, components, and integrations.
---

# tuicore Design Primitives & Architecture

## Default sizing rule

Prefer `FlexItem::content()` for controls, forms, toolbars, and panels with intrinsic-height children.

Use `fixed(n)` only when the exact rendered size is known, including chrome:
- `Panel` border/title usually adds 2 rows/cols.
- bordered `Dropdown` measures 3 rows.
- `fixed(n)` bypasses child `measure()` and can crush children.

Use `fill(1)` for main content regions that should consume remaining space.

## Layout components

- `Flex` — primary row/column layout. Use for most screens and control bars.
- `Split` — two-pane layout with ratio/gap. Use for main left/right or top/bottom regions.
- `Grid` — aligned rows/columns. Use when cells need predictable tracks.
- `Stack` — layered or active-child composition. Use for overlays/alternate bodies.
- `Overlay` — base + positioned layer. Use when a component must sit above another.
- `DialogLayer` + `DockSpec` — modal/overlay host. Use `docked(DockSpec::bottom(80))` for top/bottom/left/right docked overlays so placement and chrome borders stay in sync.
- `Panel` — chrome wrapper with title/border/help. Remember inner area is smaller.
- `Tabs` — tab header + selected body. Body should own its own layout.
- `Menu` — trigger-driven popup actions. Use for compact command lists.
- `StatusBar` — app chrome for menu/theme/AI/weather/time indicators.
- `AiDock` — modal chat/tool assistant surface with configurable tool approval policy.
- `WeatherIndicator` / `WeatherForecastDialog` / `DateTimeIndicator` — status-bar primitives for weather and time display.

## Overlay portals

Popup components (`Dropdown`, `Menu`, date/time dropdowns, status menus) register overlay geometry during normal `layout()` via `OverlayManager`, then enqueue portal draws during normal `render(frame, area, ctx)` via `RenderCtx`. Consumers do not host or forward overlays manually. Custom containers only call child `layout()` and `render(..., ctx)`; `ctx.flush(frame)` at root draws queued portals.

Focus-mode plain Enter emits a control submit request and enters/opens input mode. Enter while input mode is active remains control-local and emits no submit request.

`Dropdown` owns its border and label. In validated forms, wrap it with embedded `FormField` mode and synchronize `Dropdown::set_error` with field feedback instead of adding a second panel border.

## Core Trait (`TuiNode`)

All UI elements implement `TuiNode<M>` which drives lifecycle and layout:
- `measure(proposal)` -> `LayoutSizeHint`: sizing hints (min, preferred, stretch).
- `layout(area, ctx)` -> `LayoutResult`: computes child boundaries. Register focus targets here.
- `render(frame, area, ctx)`: Direct rendering using ratatui; popup owners enqueue portals on `RenderCtx` here.
- `event(event, ctx)` -> `EventOutcome`: Handles key/mouse/hotkey inputs.
- `tick(dt, settings)` -> `TickResult`: Advances animations. Updating state happens here, leaving render pure.
- `focus(target, focused, ctx)`: Reacts to focus changes.

### Event Handling & Propagation
- **Outcomes**: Return `EventOutcome::Handled` to claim an event, or `EventOutcome::Ignored` to let others handle it.
- **Redraws**: If state changes, call `ctx.request_redraw()` (and optionally `ctx.request_layout()` if layout size changes).
- **Communication**: Emit updates up the tree using `ctx.emit(msg)` (where `M` is the message type).
- **Notifications**: Use `ctx.notify(Notification::info/success/warning/error(title, body))` for user-facing toasts. `TreeApp` displays them automatically with its built-in `ToastRack`; apps only customize by passing `.notifications(ToastRack::new().max_visible(...))` or observing via `.on_notification(...)`.
- **Clipboard/Yank**: Components that support `TuiEvent::Yank` should call `ctx.copy_to_clipboard(value)`. This requests clipboard copy and automatically queues an info toast titled `Copied to clipboard` with the copied text in quotes.
- **Propagation**: If you handle a key/hotkey and want to block parent containers from receiving it, call `ctx.stop_propagation()`.

## Store, EventLog & StateInspect

Use `Store` when an app wants a small central place for app state and app events. Define your own state and event enum, then pass a reducer closure:

```rust
let store = Store::new(AppState::default(), |state, event| {
    match event {
        AppEvent::TodosLoaded(todos) => state.todos = todos,
        AppEvent::ToggleDone(id) => state.toggle_done(id),
    }

    DispatchOutcome::changed()
});
```

- Components still emit messages with `ctx.emit(msg)`; app/root code maps those messages to store events and calls `store.dispatch(event)`.
- Use `DispatchOutcome::changed()` after state changes, `DispatchOutcome::layout()` when layout size may change, `DispatchOutcome::redraw()` for visual-only redraws, and `DispatchOutcome::unchanged()` when nothing changed.
- Persistence and data loading live outside the store: load from Postgres/files/HTTP in app code or a service, then dispatch events like `TodosLoaded(todos)` or `TodoSaved(todo)`.
- Add `EventLog::bounded(n)` only when you want in-app debugging. Connect it with `store.with_observer(log.observer())` or `observer_with_label(...)`. Default labels do not include event payloads.
- Implement `StateInspect` on app state when you want a safe debug tree for a Store/State view. Expose only fields that are useful and safe to show.
- Store debug UI is opt-in: wire `StatusBar::on_store_view_open(...)` to an app/root message, then open a `StoreDebugView::dialog(state.inspect(), event_log.entries())` dialog owned by the app.

## Child Management & Containers

Containers with child elements use the `Children<M>` structure (wrapping list of `ChildSlot`) to properly route layout, events, and focus:
- **Layout**: In `layout()`, position each child by calling `slot.layout(child_area, ctx)` (this registers hierarchy and hit-testing regions).
- **Event Routing**: Containers must implement `dispatch_event` and `dispatch_focus` explicitly (the default implementations do not descend). Forward events using `slot.dispatch_event(&route, event, ctx)` and `slot.dispatch_focus(&target, focused, ctx)`.
- **Ticking & Render**: Loop and forward to `slot.tick(dt, settings)` and `slot.render(frame, area, ctx)`.

## Reactive forms

- Build typed state with `FormBuilder`, `FormControl`, `FormGroup`, and `FormArray`; implement `FormModel` for typed control structs.
- Keep cross-field errors on `FormGroup`. Present a group error beside the relevant field without copying it into that control.
- Route deliberate edits through `begin_edit()` and `input(value)`, then call `end_edit()` when input/open mode closes. Default `ErrorDisplay::OnInputExit` reveals errors after edit completion; `OnInput` reveals them on first input.
- Read current validity from `errors()`/`status()` and rendered feedback from `presented_errors()` or `visible_errors(submitted)`. Presented control errors latch through editing and refresh automatically at the configured `OnInputExit` or `OnInput` trigger, on `set_value`/`reset`, and on submission.
- Keep group-owned cross-field feedback latched with `FormGroup::refresh_presented_errors()` at its relevant control trigger; submission refreshes it automatically. Wrap controls in `FormField` and pass the first visible error through `set_error` for semantic error chrome and feedback text.
- Use `on_submit` as focus-mode callback when Enter requests editing. Use `on_edit_end` for active-to-inactive transitions, including configured finish keys, cancel, and focus loss. Active text/password Enter and textarea Ctrl+Enter finish editing without invoking `on_submit`; textarea Enter inserts newline. Focus and Tab without edit activation leave controls pristine.
- Gallery validated form uses Ctrl+Enter for whole-form submission except when routed to textarea; there Ctrl+Enter exits textarea input mode without submitting the form.

## Semantic Theming & Colors

No raw color literals in components. Retrieve all colors from the active theme:
- **Usage**: Access via `tuicore::theme()` (e.g., `theme().text_fg()`).
- **Core Roles**:
  - Backgrounds: `background_bg` (root/canvas), `surface_bg` (panels/cards), `backdrop_bg` (dialog overlays).
  - Text: `text_fg` (primary), `muted_fg` (secondary/help), `subtle_fg` (disabled/placeholders).
  - States: `selected_fg`/`selected_bg` (active elements), `highlight_fg`/`highlight_bg` (focused controls), `success_fg`, `warning_fg`, `error_fg`.
  - Accents/Chrome: `accent_fg` (links/keys), `border_fg` (borders), `key_fg` (hotkeys).
  - Weather: `weather_sun_fg`, `weather_cool_fg`, `weather_warm_fg`, `weather_hot_fg`, `weather_rain_fg`.

## Structural Presets

Components query global default configurations from the active preset unless overridden by component-specific builder APIs:
- **Usage**: Access via `tuicore::preset()` (e.g., `preset().border()`, `preset().scroll()`).
- **Core Presets**:
  - `border()`: The default border style (`BorderKind::Rounded`, `Double`, `Thick`, `Plain`).
  - `tabs()`: Default variant (`Minimal`, `Underline`, `Boxed`) and bordering layout for tab bars.
  - `scroll()`: Defaults for scrollbar gutters, styling, and visibility policies.
  - `animation()`: Default easing curve, durations, and target FPS.
- **Dynamic Swapping**: Apps can dynamically switch presets at runtime by calling `tuicore::set_preset(new_preset)` (e.g., to toggle thick/plain borders or disable animations). Custom components query these settings during layout or rendering to automatically adapt.

## Focus Management

Components participate in focus navigation by registering focus targets and reacting to focus state changes:
- **Registration**: During `layout(area, ctx)`, components register focusable regions by calling `ctx.register_focusable(FocusId, area, is_active)` or `ctx.register_focusable_with_hotkey_sequences(...)`. Text-entry wrappers should use `ctx.register_text_entry_focusable(...)` while typing so global hotkeys do not steal input characters.
- **Focus Lifecycle**: The runtime alerts components to focus changes by calling `focus(target_id, focused, ctx)`:
  - `focused: bool`: indicates if the component gained (`true`) or lost (`false`) focus.
  - `target_id: Option<&FocusId>`: indicates which sub-element within the component gained focus.
- **Requesting Focus**: Event handlers can request focus changes on the active `EventCtx` (e.g., `ctx.focus(FocusRequest::Next)`, `ctx.focus_next()`, `ctx.focus_previous()`, or `ctx.unfocus()`).
- **Focus Routing**: Complex components and containers manage focus internally or route focus using `FocusChain` / `FocusRouter` (helper utilities that advance focus along an array of IDs based on key inputs). Custom containers must forward focus down the tree in `dispatch_focus(target, focused, ctx)` via `slot.dispatch_focus()`.

## Keyboard Shortcuts & Mnemonics

Tuicore matches sequence-based hotkeys and triggers events globally or locally:
- **Matching**: Components use `HotkeySequenceMatcher::new([keys])` inside key event handlers.
- **Registration**: Register hotkeys in `layout` using `ctx.register_focusable_with_hotkey_sequences`.
- **Events**:
  - `HotkeyEvent::Pending(prefix)`: A partial match sequence is active. Underline/highlight prefix.
  - `HotkeyEvent::Canceled`: Sequence canceled or timed out.
  - `HotkeyEvent::Commit(seq)`: Sequence fully typed. Perform action.
- **Rendering**: Format mnemonic text via `hotkey_label_spans` to underline active letters.
- **App Configuration**: Built-in library actions map to a static schema in `KeyBindings` (`keybindings.toml`). Consuming apps configure their own custom hotkeys by parsing custom config settings and assigning hotkeys/sequences to components programmatically at runtime.

## Scrolling & Animations

- **Scrolling**: Wrap viewports in `ScrollState` to handle smooth scroll transitions, scrollbar rendering, and mouse wheel propagation.
- **Animations**: Components implement `Animated` to tick animation frames. Use `Tween` (for numeric transitions) and `ColorTween` (for color interpolations) inside component tick loops to handle easing (e.g., cubic/quad/back) and step integration. Disable animations globally via `animation_settings().enabled`.

## Layout Best Practices

Follow these general structural and sizing rules to build robust screens:
1. **Screen Shells**: Prefer `Flex::column()` to structure the main page layout.
2. **Fixed Chrome vs Content**: Use `FlexItem::content()` for headers, status bars, control panels, forms, and toolbars containing intrinsic-height children.
3. **Flexible Main Panes**: Use `FlexItem::fill(1)` for main panels, lists, grid views, and text logs to consume the remaining active space.
4. **Avoid Magic Offsets**: Do not hardcode guessed `fixed(n)` heights for panels wrapping dynamic content (like inputs or dropdowns), as this bypasses child measurement and risks crushing element layout. Let the layout engine measure children organically.
