---
name: tuicore-notes
description: Self-contained guide to tuicore runtime, components, and integrations.
---

# tuicore

Library-first `ratatui` component runtime. Components own UI state; apps own domain state. Call
`init()`/`try_init()` before building trees because components may snapshot globals. Sole full-app
reference: **`examples/app_shell.rs`** (`TreeApp`, `Flex`, `Tabs`, `DataView`, detail, `StatusBar`).

## Runtime contract

`TuiNode<M>` defines `measure(LayoutProposal) -> LayoutSizeHint`, `layout(Rect, LayoutCtx) ->
LayoutResult`, pure `render(Frame, Rect, RenderCtx)`, local `event(TuiEvent, EventCtx) ->
EventOutcome`, routed `dispatch_event(EventRoute, ...)`, `focus`/`dispatch_focus`, `tick(Duration,
AnimationSettings) -> TickResult`, and `init`/`mount`/`unmount`/`destroy` with `LifecycleCtx`.

State changes and animation starts belong in events, focus, lifecycle, message handlers, or `tick`,
never `render`. Merge child ticks: `changed` redraws, `layout` relayouts, `active` keeps ticking, and
`next_tick` schedules work. Custom composites forward every contract method, route child first, and
use stable `ChildKey`s. `Handled` does not stop bubbling; call `EventCtx::stop_propagation`.

Prefer `TreeApp::new(root).run()`; configure `animation_settings`, `terminal_focus_effect`,
`runtime_keybindings`, `initial_focus`, `on_message`, `on_notification`, and `notifications`.
`run(root)` is shorthand for `TuiNode<()>`. `TerminalFocusEffect` is `Disabled` or
`Dim(FocusDimSettings)`. `EventCtx` emits messages and requests focus, redraw, layout, clear, quit,
notifications, clipboard, or external-editor work. `FocusCtx` and `LifecycleCtx` expose relevant
subsets. `EventOutcome::{Ignored, Handled}` and `Propagation::{Continue, Stopped}` describe routing.

Use `ChildSlot::new(key, child)` or `Children::new().child(...)` for custom composites. Their APIs
forward measure/layout/events/focus/tick/lifecycle and support context-aware insert/replace/remove.
Invalid keys yield `DuplicateChildKey` or `MissingChildKey`; identity is `TreePath` + `FocusId`.

Measurement vocabulary: `AxisProposal`, `LayoutProposal`, `LayoutSize`, `AxisExpand`,
`LayoutSizeHint`, `HintSource`, `LayoutAxis`, `OverflowPolicyName`, and
`LayoutOverflowDiagnostic`. Runtime registration values are `FocusTarget`, `FocusRepair`, and
`HitRegion`. Input values are `KeyEvent`, `Key`, `KeyModifiers`, `MouseEvent`, `MouseEventKind`,
`MouseButton`, `HotkeyEvent`, `ExternalEditorRequest`, `ExternalEditorResponse`, and
`UnsupportedEvent`.

Low-level custom-loop exports exist but normal apps should not need them: `TerminalGuard`,
`EventSource`, `LayoutEngine`, `FocusManager`, `TreeDispatcher`, `Scheduler`, `Renderer`,
`DispatchEffects`, and `FocusTransition`. Runtime errors use `Result`.

## Layout and layers

- `Flex::row()`/`column()`: keyed children with `FlexItem::{fixed, fill, percent, fit_content,
  content}`; configure gap, separator, padding, `MainAlign`, `CrossAlign`, `CrossSize`.
- `Grid::new()`: `GridTrack::{fixed, percent, fill, fit_content}` and `GridItem::new(row, column)`;
  configure spans, alignment, gaps, padding, `GridSeparators`, and `GridSeparatorAxes`.
- `Split::horizontal`/`vertical`: two panes with ratio/constraints, gap, separator.
- `Stack::new()`: overlap using `StackItem`, `StackAlign`, and `StackSize`; use `Tabs` for pages.
- `Overlay::new(base, layer)`: anchored `OverlayAnchor` + `OverlaySize`.
- `DialogLayer::new(base, layer)`: modal/docked layer using `DialogBackdrop`,
  `DialogLayerPlacement`, `DockSpec`, `DockSide`, and `DockChrome`; replace alternatives with
  `replace_layer`, nest layers for nested modals.
- Shared spacing/chrome: `Padding`, `Gap`, `Separator`, `SeparatorColorRole`.

Prefer fit-content plus fill over guessed sizes. Built-in popup controls own portals. Custom popup
owners register `OverlaySpec` during layout and enqueue drawing through `RenderCtx`; related exports
are `OverlayId`, `OverlayLayer`, `OutsideMousePolicy`, `OverlayPolicy`, `OverlayLayoutEntry`, and
`OverlayManager`.

## Component selection

### Actions and input

- `Button<M>`: `new(label)`, hotkey/tab-stop/focus/`on_press`; direct operations return
  `ButtonOutcome`. `HotkeyLabelMode` controls label rendering.
- `Toggle<M>`: `new(label)`, checked/value/style/hotkey/`on_change`; `ToggleStyle`, `ToggleOutcome`.
- `TextInput<M>` / `PasswordInput<M>`: `new`, value, placeholder, panel/style, hotkey, focus,
  max length, submit/change/edit-end callbacks, key/paste handling; password adds mask char.
  `InputChrome`, `InputPanelChrome`, `InputOutcome`, `TextInputKeyBindings` configure behavior.
- `TextareaInput<M>`: multiline equivalent with rows, wrapping, external editor, and
  `TextareaInputKeyBindings`.
- `TagInput<Id>`: `new(strings)` or `with_options`; configure selected/custom tags, placeholder,
  hotkey/style/panel; drain `take_events() -> Vec<TagInputEvent<Id>>`. Values use `SelectedTag`.

### Choice and commands

- `Dropdown<T, Id>`: `single`/`multi`; retained selection, search, commit, popup, label, hotkey, and
  callbacks. Inspect IDs/query/open state; open/close/cancel/commit return `DropdownOutcome`.
  Configure with `DropdownActionKeys`, `DropdownCommitMode`, `DropdownLabelPosition`,
  `DropdownPopupDirection`, `DropdownSearchMode`, and `DropdownVariant`.
- `Menu<Id>`: transient commands from `MenuItem::new`; configure search/popup/action keys/hotkey,
  open contextually, drain activation. `MenuButton<Id, M>::new` owns trigger, focus, and popup.
  Supporting outputs/config: `MenuOutcome`, `MenuActionKeys`, `MenuPopupDirection`,
  `MenuSearchMode`.

### Lists, data, and trees

- `List`: simple strings via `new(items)`; selection/navigation/scroll configuration and
  `ListOutcome`.
- `DataView<T, Id>`: typed rows with stable IDs. `new(rows, id)` + `Column`s, or `list`.
  Configure rows, headers, action/filter/search, local/external transforms, sorting, pagination,
  tree expansion, activation/selection, copying, empty state, and scrolling. Mutate with set/push/
  append/update/remove APIs. Drain `DataViewTypedEvent<Id>` using `take_events`/`drain_events`;
  direct operations return `DataViewOutcome`.
- Columns use `Column::text`/`rich`, then sortable/reorderable/search/filter/sizing/visibility
  builders. Trees use `TreeAdapter`; supporting types: `CellContext`, `ColumnSizing`,
  `ActivationMode`, `SelectionMode`, `SelectionTrigger`, `SelectionPropagation`, `CheckState`,
  `SelectionGlyphs`, `TreeGlyphs`, `SortDirection`, `DataViewEvent`, `DataViewSort`,
  `DataViewPagination`, `DataViewTransformMode`, `DataViewTransformState`, `DataViewFilter`.
- `ListControl<T, Id, M>`: mutable `DataView`; construct `new`, `new_fields`, or `list`. Define
  `ListControlField::text`/`dropdown`, validation and conditional visibility; configure columns,
  edit/remove confirmation/reorder/tree/checking, title/panel/hotkey/row limits, and
  `ListControlKeyBindings`. Always drain `take_events() -> Vec<ListControlEvent<Id>>`; failures use
  `ListControlReorderUnavailable`.
- `Checklist<T, Id, M>`: `new(items, id, label)` or `from_list_control`; configure tree, expansion,
  checks, cascading, panel/title/hotkey; inspect checked IDs and drain `ListControlEvent`s.

### Date and calendar

- `DatePicker<M>`, `TimePicker<M>`, `DateTimePicker<M>`: `new`; configure value/range/weekday,
  precision/step/layout, hotkey, and `on_select`. Types: `PickerOutcome`, `TimeField`,
  `TimePrecision`, `DateTimePickerLayout`.
- `DatePickerDropdown<M>` / `DateTimePickerDropdown<M>`: compact field plus owned popup; configure
  value, placeholder, date/time options, hotkey/chrome/callbacks, and open state.
- `RelativeDate`: `new(target)`, live/fixed reference and `RelativeDateMode`; live mode must tick.
- `Calendar<T, Id, M>`: `new(entries, id_fn, span_fn, title_fn)`; configure view/date bounds,
  renderers, ordering, activation, hotkey, and `CalendarKeyBindings`; drain events. Build
  `CalendarSpan::timed`/`all_day`/`all_day_range`; types include `CalendarEntryRole`,
  `CalendarOutcome`, `CalendarTypedEvent`, and `CalendarView`.

### Panels, dialogs, tabs, display

- `Panel`: non-modal chrome; configure titles, border/tone, text, focus, scroll. `host(child)` gives
  `PanelHost<C>`. Types: `PanelTitlePosition`, `PanelTone`.
- `Dialog<M>`: chrome/content/actions/close/scroll; `host(child)` gives `DialogHost<C, M>`. Build
  `DialogAction::new`; configure `DialogKeyBindings`, `DialogTitlePosition`; outputs use
  `DialogCloseReason`. Place modals in `DialogLayer`.
- `ConfirmationDialog<M>`: `new(title, description)`, labels/hotkeys/callback, drain outcomes;
  `ConfirmationDialogKeyBindings`, `ConfirmationDialogOutcome`.
- `Tabs<M>` + `Tab<M>`: `Tab::new(title, body)`/`text`, then `Tabs::new`; configure selection,
  looping, variant, borders, modal mode, hotkeys/focus/close. Types: `TabsSelectionMemory`,
  `ModalCloseReason`.
- `FormField<C, M>`: `new(label, child)`, embedded mode, error and child access.
- `Chip`: `new(label)`, icons and `ChipColorRole`. `Header`: `new(text)`, optional icon.
- `Paragraph`: `new(text)`, wrap, `ParagraphOverflow`, max lines/style. `Spinner`: `new`, style,
  must tick. `SeasonalEmptyState`: `new(message)`, `SeasonalGlyphs`, must tick when live.

### Notifications, status, weather, AI

- `Notification`: generic or info/success/warning/error payload with TTL/sticky behavior.
  `NotificationCenter`: push/dismiss/history/tick. `ToastRack`: renders/owns center and must tick.
  Types: `NotificationId`, `NotificationKind`, `ToastIcons`.
- `StoreDebugView<M>`: `new`/`dialog`/`empty`, snapshot + log display; put modal form in layer.
- `DateTimeIndicator<M>`: clock action; configure `DateTimeIndicatorFormat`, icon/text/hotkey/
  callback; must tick.
- `WeatherIndicator<M>`: report/loading/hotkey/open callback and refresh state.
  `WeatherForecastDialog<M>`: report/content and close callback. Data/config: `WeatherSummary`,
  `WeatherReport`, `WeatherForecastDay`, `WeatherFetchError`, `WeatherForecastError`,
  `WeatherProviderConfig`. `weather_condition_icon` resolves glyphs.
- `StatusBar<M>`: menu/theme/weather/time/AI footer; `new`, menu items, weather/provider,
  callbacks, toggle menu, refresh query. Types: `StatusBarKeyBindings`, `StatusBarMenuItem`.
- `AiDock<M>`: `new(nonblocking_runner)`, close callback, borders, tools, `AiDockKeyBindings`, and
  `ToolPolicy`. Submit with `submit_prompt`; runner streams matching-request `LlmEvent`/
  `LlmEventKind`. Never block UI; stream polling requires ticks. `fade_buffer` supports dialog fade.

## Focus, hotkeys, and keybindings

`FocusChain` is a local non-wrapping cursor. `FocusRouter::try_new(order)` validates local focus
order and handles configurable traversal; types are `FocusDirection`, `FocusOutcome`, `FocusWrap`,
and `FocusRouterError`. `NonFocusable` strips descendant registration; `OnBlur` emits on blur.
Runtime requests use `FocusRequest`.

`KeySpec` describes configurable keys. `KeyBindings` loads defaults/TOML; `KeyBindingsError`
reports strict failures. Shared groups: `RuntimeKeyBindings`, `FocusKeyBindings`,
`ClipboardKeyBindings`, `ButtonKeyBindings`, `TabsKeyBindings`, `ToggleKeyBindings`,
`DataViewKeyBindings`, `DropdownKeyBindings`, `DateTimePickerKeyBindings`. Empty arrays disable an
action. Keep shown labels synchronized with bindings.

`HotkeySequenceMatcher` handles timeout/prefix/cancel/commit and yields `HotkeyMatch`. Label helpers
are `hotkey_label_spans`, `hotkey_badge_spans`, `hotkey_edge_spans`, `hotkey_badge_width`,
`hotkey_underline_style`, and `hotkey_sequence_to_event`.

## Theme, preset, animation, scrolling

Initialization: `init`, `try_init`, `init_from_dir`, `try_init_from_dir`. Read globals with
`theme`, `keybindings`, `preset`, `animation_settings`; override through `set_theme`,
`set_theme_and_persist`, `set_keybindings`, `set_preset`. `UiInitError` reports strict failure.

`Theme`/`ThemeName` provide semantic colors; components must not invent raw colors. `Preset`
provides structural defaults. Types: `BorderKind`, `BorderChars`, `TabsPreset`, `TabsVariant`,
`DataViewPreset`, `DropdownPreset`; helpers `border_chars`, `border_set`.

`AnimationSettings` is global kill switch and timing source; resolve local `AnimationSpec` into
`ResolvedAnimationSpec`. `Animated`, `Easing`, `Tween`, `ColorTween`, `ScrollAnimator`, and
`lerp_color` are reusable primitives.

`ScrollState::new(axes)` uses primitive defaults; prefer `from_preset(axes, preset().scroll())` in
components. Configure axes/behavior/scrollbars; use offsets, scroll/clamp/layout/render/tick APIs.
Render reduced viewport and current offset; smooth offsets only. Types: `ScrollAxes`,
`ScrollBehavior`, `ScrollDelta`, `ScrollGeometry`, `ScrollLayout`, `ScrollOffset`, `ScrollOutcome`,
`ScrollPreset`, `ScrollSize`, `ScrollbarConfig`, `ScrollbarGutter`, `ScrollbarStyle`,
`ScrollbarVisibility`; helpers `text_size`, `line_width`, `paragraph_scroll`.

## Search, store, forms

`search_match(query, candidate, SearchMode)` and `search_ranked` return `SearchMatch`/
`RankedSearchMatch`; `MatchSpan` identifies highlights.

`Store::new(state, reducer)` is synchronous; reducers return `DispatchOutcome`. Use `StoreLike`,
`StoreObserver`, `EventLog`, `StoreLogEntry`, `StoreLogPhase`, `StateInspect`, `InspectValue`, and
`InspectField` for optional debugging. Store excludes persistence, HTTP, async runtimes, services.

Reactive forms: `FormControl::new`, `FormGroup::new`, `FormArray::new`, and `FormBuilder`.
`FormModel` supplies value/status/validate/present/reset; `FormStatus` is valid/invalid and
`ErrorDisplay` controls presentation. On input exit, surface errors in `FormField`; on submit call
`submit_attempt`.

## Integration checklist

1. Initialize globals, build tree, create `TreeApp`.
2. Prefer `Flex`/`Grid` and existing components over raw layout or custom runtime plumbing.
3. Give rows stable IDs and children stable keys.
4. Drain typed event queues or use callbacks; emit app messages with `EventCtx::emit`.
5. Mutate domain/component state in message handlers and request redraw/layout explicitly.
6. Let controls own focus, scrolling, animation, and popups.
7. Keep render pure; forward lifecycle and merge all child ticks in custom composites.
8. Use semantic theme roles, presets, configurable keys, Nerd Font icons with ASCII fallback.
