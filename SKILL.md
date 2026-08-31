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
Cursor-owning children can request an ancestor viewport reveal with `EventCtx::request_reveal` or
`request_reveal_centered`; only events initiate these requests.

Use `ChildSlot::new(key, child)` or `Children::new().child(...)` for custom composites. Their APIs
forward measure/layout/events/focus/tick/lifecycle and support context-aware insert/replace/remove.
Invalid keys yield `DuplicateChildKey` or `MissingChildKey`; identity is `TreePath` + `FocusId`.

Measurement vocabulary: `AxisProposal`, `LayoutProposal`, `LayoutSize`, `AxisExpand`,
`LayoutSizeHint`, `HintSource`, `LayoutAxis`, `OverflowPolicyName`, and
`LayoutOverflowDiagnostic`. Runtime registration values are `FocusTarget`, `FocusRepair`, and
`HitRegion`. Input values are `KeyEvent`, `Key`, `KeyModifiers`, `MouseEvent`, `MouseEventKind`,
`MouseButton`, `HotkeyEvent`, `ExternalEditorRequest`, `ExternalEditorResponse`, and
`UnsupportedEvent`. Mouse capture is enabled by `TreeApp`: interactive components register hit
regions during layout, handle state changes in `event`, and keep render pure. Built-in buttons,
text inputs, toggles, dropdowns, menus, calendars, tabs, DataViews, panels, and scroll containers
accept basic clicks or wheel scrolling.

Low-level custom-loop exports exist but normal apps should not need them: `TerminalGuard`,
`EventSource`, `LayoutEngine`, `FocusManager`, `TreeDispatcher`, `Scheduler`, `Renderer`,
`DispatchEffects`, and `FocusTransition`. Event handlers can call `EventCtx::request_tick` when
asynchronous work needs polling beyond the current animation window. Runtime errors use `Result`.

## Layout and layers

- `Flex::row()`/`column()`: keyed children with `FlexItem::{fixed, fill, percent, fit_content,
  content}`; configure gap, separator, padding, `MainAlign`, `CrossAlign`, `CrossSize`. Content modes
  default to shrink factor 1; use `.shrink(weight)` for relative yielding, `.shrink(0)` to protect an
  item, and measured minimum as shrink floor.
- `Grid::new()`: `GridTrack::{fixed, percent, fill, fit_content}` and `GridItem::new(row, column)`;
  configure spans, alignment, gaps, padding, `GridSeparators`, and `GridSeparatorAxes`.
- `Split::horizontal`/`vertical`: two panes with ratio/constraints, gap, separator.
- `Stack::new()`: overlap using `StackItem`, `StackAlign`, and `StackSize`; use `Tabs` for pages.
- `ScrollContainer::vertical(child)`: one viewport, `ScrollState`, and scrollbar around arbitrary
  measured `TuiNode` content. Use `FlexItem::fill(1)` for its viewport and `fit_content()` for
  stacked content: the child measures to natural height, while the container clips and translates
  it. Configure `scrollbars`, `scroll_behavior`, `padding`, and `focus_reveal`. Child input routes
  first; unhandled configured keys and wheel events scroll the container. Tab focus auto-reveals
  descendants. `horizontal` and `both` select other axes. Copy `examples/scroll_container.rs` for
  mixed content or stacked tree DataViews.

  ```rust
  let page = ScrollContainer::vertical(
      Flex::<()>::column()
          .gap(1)
          .child("sprint-5", sprint_5.parent_vertical_scroll(), FlexItem::fit_content())
          .child("backlog", backlog.hotkey("shift+b").parent_vertical_scroll(), FlexItem::fit_content()),
  )
  .scrollbars(ScrollbarConfig::default())
  .focus_reveal(true);

  let root = Flex::<()>::column().child("page", page, FlexItem::fill(1));
  ```

  Custom composite nodes forward `focus_reveal_area` and `focus_reveal_centered` through the same
  child path as `dispatch_focus`; their defaults preserve ordinary focus-area reveal.
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

- `Button<M>`: `new(label)`, hotkey/tab-stop/focus/`on_press`; configure disabled state with
  `disabled(bool)`/`set_disabled` and inspect it with `is_disabled`. Disabled buttons use muted
  styling, skip focus traversal and hotkeys, and ignore presses. Direct operations return
  `ButtonOutcome`. `HotkeyLabelMode` controls label rendering.
- `Toggle<M>`: `new(label)`, checked/value/style/hotkey/`on_change`; `ToggleStyle`, `ToggleOutcome`.
- `TextInput<M>` / `PasswordInput<M>`: `new`, value, placeholder, panel/style, hotkey, focus,
  max length, optional ASCII digit-only input via `numbers_only(true)`, submit/change/edit-end
  callbacks, key/paste handling; password adds mask char.
  `InputChrome`, `InputPanelChrome`, `InputOutcome`, `TextInputKeyBindings` configure behavior.
- `TextareaInput<M>`: multiline equivalent with rows, wrapping, optional live syntax highlighting
  via `language(Language)`, external editor, repeatable message-producing `action_hotkey` bindings,
  and `TextareaInputKeyBindings`; `min_rows` is measured minimum height before panel chrome.
- `TagInput<Id>`: `new(strings)` or `with_options`; configure selected/custom tags, placeholder,
  hotkey/style/panel; drain `take_events() -> Vec<TagInputEvent<Id>>`. Values use `SelectedTag`.

### Choice and commands

- `Dropdown<T, Id>`: `single`/`multi`; retained selection, search, commit, popup, label, hotkey, and
  callbacks. Configure disabled state with `disabled(bool)`/`set_disabled` and inspect it with
  `is_disabled`; disabled fields use muted dashed rounded chrome and remain focusable, hotkey-openable,
  searchable, and navigable while locking committed selection and `on_select` callbacks. Search results can require `min_search_chars`, be capped with `max_filtered_items`,
  and use `visible_without_search` for a default subset before querying all options. Replace options
  with `set_rows` while preserving open/query state, and set a query programmatically with
  `set_search_query`. Switch matching at runtime with `set_search_mode`.
  `DropdownSearchMode::External` keeps search input active but disables local filtering and match
  highlighting so a remote service can own results. Configure its loading state with
  `external_loading`/`set_external_loading` and a custom message with
  `external_loading_message`/`set_external_loading_message`; loading renders the shared `Spinner`.
  Inspect IDs/query/open state; open/close/cancel/commit return `DropdownOutcome`.
  Configure with `DropdownActionKeys`, `DropdownCommitMode`, `DropdownLabelPosition`,
   `DropdownPopupDirection`, `DropdownSearchMode`, and `DropdownVariant`. Bordered dropdowns also
   support a bottom-left border label via `bottom_left`, with optional `bottom_left_style`.
- `Menu<Id>`: transient commands from `MenuItem::new`; configure search/popup/action keys/hotkey,
  open contextually, drain activation. `MenuButton<Id, M>::new` owns trigger, focus, and popup.
  Supporting outputs/config: `MenuOutcome`, `MenuActionKeys`, `MenuPopupDirection`,
  `MenuSearchMode`.

### Lists, data, and trees

- `List`: simple strings via `new(items)`; selection/navigation/scroll configuration and
  `ListOutcome`.
- `DataView<T, Id>`: typed rows with stable IDs. `new(rows, id)` + `Column`s, or `list`.
  Configure rows, headers, action/filter/search, local/external transforms, sorting, pagination,
  tree expansion, activation/selection, per-row disabled checkboxes via `selection_disabled_by`
  and `selection_disabled_glyph`, copying, empty state, scrolling, fixed `row_height`, or
  per-row `row_height_by`. Both height paths clamp to at least one; `set_row_height` clears a
  dynamic policy and `configured_row_height` remains its fixed fallback. Mutate with set/push/
  append/update/remove APIs. Drain `DataViewTypedEvent<Id>` using `take_events`/`drain_events`;
  direct operations return `DataViewOutcome`.
- Columns use `Column::text`/`rich` or `Column::multiline`, then sortable/reorderable/search/filter/
  sizing/visibility builders. Multiline output never auto-grows rows: declared fixed or per-row
  height is authoritative and clips extra lines. Trees use `TreeAdapter`; supporting types:
  `CellContext`, `ColumnSizing`,
  `ActivationMode`, `SelectionMode`, `SelectionTrigger`, `SelectionPropagation`, `CheckState`,
  `SelectionGlyphs`, `TreeGlyphs`, `SortDirection`, `DataViewEvent`, `DataViewSort`,
   `DataViewPagination`, `DataViewTransformMode`, `DataViewTransformState`, `DataViewFilter`.
   Use `parent_vertical_scroll()` or `vertical_scroll(DataViewVerticalScroll::ParentDelegated)`
   inside `ScrollContainer` to keep a tree's native navigation/check glyphs while one outer page
   owns vertical reveal and scrolling. Delegated mode hides its local vertical scrollbar and sends
   immediate centered row-reveal requests upward for configured line navigation (`j`/`k` by
   default); page/top/bottom navigation keeps normal outer scrolling animation. Vertical navigation
   at a tree boundary bubbles to the outer page. Tabbing back to a delegated DataView centers its
   active row in the outer viewport.
   Call `.scrollbars(...)` after delegation only when intentionally restoring local chrome. Default
   `Local` behavior remains unchanged.
- `ListControl<T, Id, M>`: mutable `DataView`; construct `new`, `new_fields`, or `list`. Define
  `ListControlField::text`/`dropdown`/`dropdown_options`, validation and conditional visibility;
  configure columns,
  edit/remove confirmation/reorder/tree/checking, title/panel/hotkey/row limits, and
  `ListControlKeyBindings`. Use `allow_horizontal_moving(false)` to prevent indent/outdent during
  active tree reorders while retaining vertical movement. Always drain
  `take_events() -> Vec<ListControlEvent<Id>>`; failures use
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
  `compact_summary_title`, renderers, ordering, activation, hotkey, and `CalendarKeyBindings`; drain events. Build
  `CalendarSpan::timed`/`all_day`/`all_day_range`; types include `CalendarEntryRole`,
  `CalendarOutcome`, `CalendarTypedEvent`, and `CalendarView`.

### Panels, dialogs, tabs, display

- `Panel<M>`: non-modal chrome; configure titles, border/tone, text, focus, scroll. Use
  `one_row(true)` for a top-border-only panel without side or bottom borders. Repeat
  `action_hotkey(sequence, || message)` to add custom actions beside the optional focus hotkey in
  the bottom-right badge. `host(child)` gives `PanelHost<C, M>`. Types: `PanelTitlePosition`,
  `PanelTone`.
- `SpeedReader`: `new` for plain text or `markdown` for Markdown-aware blocks; configure title, WPM,
  natural pauses, fixed extra `markdown_block_pause(Duration)`, and keybindings; `dialog` creates a
  closable host.
- `Dialog<M>`: chrome/content/actions/close/scroll; `host(child)` gives `DialogHost<C, M>`. Build
  `DialogAction::new`; configure `DialogKeyBindings`, `DialogTitlePosition`; outputs use
  `DialogCloseReason`. Place modals in `DialogLayer`.
- `ConfirmationDialog<M>`: `new(title, description)`, labels/hotkeys/callback, drain outcomes;
  `ConfirmationDialogKeyBindings`, `ConfirmationDialogOutcome`.
- `Tabs<M>` + `Tab<M>`: `Tab::new(title, body)`/`text`, then `Tabs::new`; configure selection,
  looping, variant, borders, modal mode, hotkeys/focus/close. Repeat
  `action_hotkey(sequence, |selected_index| message)` for custom bottom-right actions alongside the
  optional focus hotkey. `TabsVariant` provides `Minimal`, `Underline`, `Boxed`, and `OneRow`;
  `OneRow` (preset value `one-row`) matches minimal styling with only the top tab-row border and no
  left, right, or bottom border. Types: `TabsSelectionMemory`, `ModalCloseReason`.
- `FormField<C, M>`: `new(label, child)`, embedded mode, error and child access.
- `Chip`: `new(label)`, icons and `ChipColorRole`. `Header`: `new(text)`, optional icon.
- `Paragraph`: `new(text)`, wrap, `ParagraphOverflow`, max lines/style. `Spinner`: `new`, style,
  must tick. `SeasonalEmptyState`: `new(message)`, `SeasonalGlyphs`, must tick when live.
- `SpeedReader`: `new(plain_text)` or `markdown(source)`; configure title, WPM, natural pauses, and
  `SpeedReaderKeyBindings`; inspect `SpeedReaderState`/progress and tick while playing. `dialog`
  wraps it in a `DialogHost`.

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
components. Configure axes/behavior/scrollbars, including a single-axis scrollbar with
`vertical_scrollbar`; use offsets, scroll/clamp/layout/render/tick APIs.
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
