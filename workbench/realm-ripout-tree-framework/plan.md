# Plan: Replace Tuirealm with a Tuicore Tree Framework

## Purpose

Remove `tuirealm` while the crate is small, keep `ratatui` for rendering, and build a lightweight tree-first framework that supports nested components, event bubbling, lifecycle hooks, focus routing, and container/layout composition.

## Drivers

- Components should compose as a tree, not as flat mounted siblings only.
- Containers like panels, dialogs, tabs, splits, stacks, and table cells should own children and hide lifecycle plumbing.
- Render purity stays mandatory: event/update starts animations, `tick(dt)` advances them, render only reads state.
- Consumers should not manually wire ratatui/tuirealm details for common usage.
- Avoid wrapper explosion: no `PanelTextInput`, `DialogTextarea`, etc.
- Keep Rust APIs explicit, typed, small, and library-first.

## Target Architecture

```text
TuicoreApp
└── root TuiNode
    └── PanelHost
        └── Split
            ├── DataView
            └── PanelHost
                └── TextInput
```

`TuicoreApp` owns terminal/runtime. `TuiNode` owns tree behavior. Container nodes own lifecycle propagation to children. Leaf nodes implement focused behavior and rendering.

## Target Consumer API

The public surface should read almost declaratively while staying typed Rust:

```rust
fn main() -> tuicore::Result<()> {
    tuicore::run(
        Panel::new()
            .top_left("Requests")
            .host(
                Split::horizontal(
                    DataView::list(requests, request_id, request_label),
                    Panel::new()
                        .top_left("Filter")
                        .host(TextInput::new().placeholder("Search…")),
                )
                .ratio(30, 70),
            ),
    )
}
```

For app messages:

```rust
TextInput::new()
    .placeholder("Search…")
    .on_submit(|value| Msg::Search(value))
```

Consumer code should express structure, not lifecycle plumbing. No user should need to know that a panel border uses a tween.

### Custom consumer components

Consumers should build custom components by composition, not inheritance. A custom `JiraPanel` can wrap a tree of built-in nodes and delegate `TuiNode` behavior to it:

```rust
pub struct JiraPanel {
    inner: PanelHost<Flex<Msg>>,
}

impl JiraPanel {
    pub fn new(issues: Vec<Issue>) -> Self {
        Self {
            inner: Panel::new()
                .top_left("Jira")
                .host(
                    Flex::column()
                        .child("search", TextInput::new().placeholder("Filter…"), FlexItem::fixed(1))
                        .child("issues", IssueList::new(issues), FlexItem::fill(1)),
                ),
        }
    }
}
```

Add a delegation helper so consumers do not hand-write lifecycle forwarding:

```rust
impl_node_delegate!(JiraPanel, inner);
```

or trait-based equivalent:

```rust
pub trait DelegatesNode<M> {
    type Inner: TuiNode<M>;
    fn inner(&self) -> &Self::Inner;
    fn inner_mut(&mut self) -> &mut Self::Inner;
}
```

Prefer the macro first because it avoids coherence surprises. Add trait blanket forwarding only if it stays simple and object-safe.

This keeps custom components easy while preserving explicit Rust ownership.

## Core Concepts

### `TuiNode`

Primary component trait replacing tuirealm `Component` / `AppComponent`.

```rust
pub trait TuiNode<M = ()> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult;
    fn render(&self, frame: &mut Frame, area: Rect);
    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        EventOutcome::Ignored
    }
    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.event(event, ctx)
    }
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        TickResult::IDLE
    }
    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {}

    fn init(&mut self, _ctx: &mut LifecycleCtx<M>) {}
    fn mount(&mut self, _ctx: &mut LifecycleCtx<M>) {}
    fn unmount(&mut self, _ctx: &mut LifecycleCtx<M>) {}
    fn destroy(&mut self, _ctx: &mut LifecycleCtx<M>) {}
}
```

Defaults should make simple leaves small: no-op `event`, no-op `tick`, no-op `focus`, no-op lifecycle hooks. `layout` and `render` are required.
`render` takes `&self`; render must not mutate state. Any hit-test or layout cache needed for mouse targeting must be updated outside render by a layout/event phase, not by drawing.

### Layout phase

Ratatui `Layout` only calculates rectangles. Tuicore adds a layout phase so events and focus can use geometry without mutating render.

```rust
pub struct LayoutCtx {
    focus_paths: Vec<FocusTarget>,
    hit_regions: Vec<HitRegion>,
}

pub struct LayoutResult {
    area: Rect,
}
```

Runtime order per frame/event cycle:

1. `root.layout(root_area, &mut layout_ctx)` updates child rects, focus targets, and hit regions.
2. Runtime dispatches input events using cached focus paths / hit regions.
3. Runtime calls `root.tick(dt, settings)` when due.
4. Runtime reruns layout before render after any handled event, emitted message update, focus change, tick with `changed`/`active`, or explicit layout request.
5. Runtime calls `root.render(frame, root_area)` only when redraw is needed.

Components that need geometry for keys, scroll page size, cursor bounds, or mouse use cached layout state written by `layout`, not render.
Start simple: rerun layout before every render. Add dirty-layout optimization only after correctness is proven.

Focus and hit registration should flow through layout helpers so render order, focus order, and hit regions stay aligned:

```rust
ctx.push_slot(ChildKey::new("body"), child_area, |ctx| child.layout(child_area, ctx));
ctx.register_focusable(FocusId::new("input"), enabled);
ctx.register_hit_region(HitRegion::new(ctx.current_path(), child_area));
```

### `TuiEvent`

Tuicore-owned event model. Wrap crossterm internally, but do not leak backend-specific types through public component APIs.

```rust
pub enum TuiEvent<U = ()> {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
    User(U),
}
```

Do not expose ticks as normal events. The runtime calls `root.tick(dt, settings)` once per frame. This keeps animation advancement separate from input handling.
V1 should keep `TuiEvent` non-generic unless user events are needed by a real component. If `User(U)` stays, then `TuiNode`, runtime, and contexts must become generic over `U` consistently.

### Typed messages

Tuicore replaces tuirealm `Msg` with typed node messages:

```rust
pub struct EventCtx<M> {
    messages: Vec<M>,
    redraw: bool,
    layout: bool,
    quit: bool,
    focus_request: Option<FocusRequest>,
    propagation: Propagation,
    animation: AnimationSettings,
}

impl<M> EventCtx<M> {
    pub fn emit(&mut self, msg: M);
    pub fn request_redraw(&mut self);
    pub fn request_layout(&mut self);
    pub fn request_quit(&mut self);
    pub fn focus(&mut self, target: FocusRequest);
    pub fn focus_next(&mut self);
    pub fn focus_previous(&mut self);
    pub fn stop_propagation(&mut self);
}
```

Components expose typed callback builders where useful:

```rust
TextInput::new().on_submit(|value| Msg::Search(value))
```

The app runtime drains emitted messages and passes them to an app update handler when one is configured.

Runtime APIs:

```rust
tuicore::run(root)?;

TuicoreApp::new(root)
    .on_message(|root, msg, ctx| match msg {
        Msg::Search(value) => ctx.request_redraw(),
    })
    .run()?;

TuicoreApp::with_model(root, model)
    .on_message(|root, model, msg, ctx| {
        model.last_msg = Some(msg);
        ctx.request_layout();
        ctx.request_redraw();
    })
    .run()?;
```

Update contract:

- Runtime drains messages after event dispatch.
- Update handler receives `&mut Root`, message, and `&mut AppCtx`.
- Model variant also receives `&mut Model` for application state outside the component tree.
- Updates may mutate root/model, request focus/layout/redraw/quit, or enqueue follow-up messages.
- After updates, runtime reruns layout before rendering when `AppCtx::request_layout()` is called. V1 may conservatively rerun layout after every message update.

### Event propagation

Start with focused-path key routing and simple bubbling:

- root receives event
- focused child path gets first chance
- child can handle, bubble, stop propagation, request focus, request redraw, emit messages, or request quit
- mouse targeting can come after layout nodes can report hit regions

```rust
pub enum EventOutcome {
    Ignored,
    Handled,
}

pub enum Propagation {
    Continue,
    Stop,
}
```

Propagation contract:

1. Runtime builds `EventRoute` from current focus path for keys or hit region for mouse.
2. `TreeDispatcher` calls `root.dispatch_event(&route, event, ctx)`.
3. Containers peel one route segment, dispatch to matching child slot, then call their own `event` if propagation continues.
4. Leaves use default `dispatch_event`, which calls `event`.
5. If propagation is not stopped, ancestors receive the same event from nearest parent to root.
6. `EventOutcome::Handled` records that a node consumed the event intent, but does not stop bubbling by itself.
7. `EventCtx::stop_propagation()` prevents further ancestors from receiving the event. Input-specific handled keys should stop propagation so parent shortcuts do not leak.
8. The runtime redraws when any node requests redraw, emits message requiring redraw, or `tick` returns changed/active.
9. Capture phase is deferred until a concrete use case requires it.

```rust
pub enum EventRoute {
    Path(TreePath),
    Focus(FocusTarget),
}
```

Tree identity uses one path model for focus, events, and hit-testing:

```rust
pub struct ChildKey(...);
pub struct TreePath(Vec<ChildKey>);
```

`LayoutCtx::push_slot` pushes a stable `ChildKey` while laying out a child. `LayoutCtx::register_focusable(local_id, enabled)` records `FocusTarget { path: current_tree_path, id: local_id, enabled }`. Focus routes use the full `FocusTarget`; mouse/general routes can use a `TreePath`.

```rust
pub enum FocusRequest {
    Target(FocusTarget),
    Local(FocusId),
    Next,
    Previous,
}
```

Node focus hooks receive the local focus target so one component can expose multiple focusable regions:

```rust
fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>)
```

`EventCtx` exposes current `AnimationSettings` so event handlers can start animations while honoring the global animation kill switch. `FocusCtx` does the same for focus transitions:

```rust
pub struct FocusCtx<M> {
    animation: AnimationSettings,
    redraw: bool,
    messages: Vec<M>,
}
```

### Lifecycle hooks

Add optional hooks with default no-op methods:

- `init(&mut self, ctx)` once before first render/event
- `mount(&mut self, ctx)` when inserted into tree or app starts
- `unmount(&mut self, ctx)` when removed from tree
- `destroy(&mut self, ctx)` final cleanup before drop/app shutdown

Use hooks sparingly. Core behavior should still live in event/tick/render.

Lifecycle ordering:

1. `init` runs once before first event/tick/render.
2. `mount` runs when a node is attached to a live tree.
3. Parent `init` runs before child `init`.
4. Parent `mount` runs before child `mount`; child `unmount` runs before parent `unmount`.
5. Child `destroy` runs before parent `destroy`.
6. `destroy` runs once after `unmount` and before drop/app shutdown.
7. Replacement means old child `unmount` + `destroy`, then new child `init` + `mount`.
8. Hooks are infallible at first. Fallible resource acquisition should emit messages/errors through explicit component APIs rather than panic inside lifecycle.

### Containers and slots

Containers own children and propagate lifecycle. Generic dispatch does not introspect struct fields; each container routes to its children using shared helper traits.

```rust
pub trait HostsChild<M> {
    type Child: TuiNode<M>;
    fn child(&self) -> &Self::Child;
    fn child_mut(&mut self) -> &mut Self::Child;
}
```

Shared helpers keep containers small:

- `dispatch_child_event(child, event, ctx)`
- `tick_child(child, dt, settings)`
- `layout_child(child, area, ctx)`
- `mount_child(child, ctx)` / `unmount_child(child, ctx)` / `destroy_child(child, ctx)`
- `focus_child(child, target, focused)`

Responsibility split:

- `PanelHost`, `Split`, `Stack`, `Tabs`, etc. own child dispatch order because they know structure and slots.
- `TreeDispatcher` calls the root, applies `EventCtx` requests, coordinates focus manager, and never tries to discover arbitrary fields.
- `FocusManager` owns current focus path and traversal decisions.

Where lifecycle correctness matters, containers store child slots instead of raw fields:

```rust
pub struct ChildSlot<C, M> {
    child: C,
    initialized: bool,
    mounted: bool,
    _msg: PhantomData<M>,
}
```

`ChildSlot` owns `init/mount/unmount/destroy` ordering for replacement/removal. Static containers can keep direct fields only when lifecycle has already been handled by construction and no replacement API exists.

Dynamic child mutation must go through `Children`, not raw `Vec`, so lifecycle, focus, layout, and redraw stay coherent:

```rust
pub enum ChildMutation {
    Added,
    Removed,
    Replaced,
    Moved,
    Unchanged,
}

// Builder/pre-mount construction:
Flex::column().child("issue-123", IssueRow::new(issue), FlexItem::fixed(1));

// Live mutation:
children.insert("issue-123", IssueRow::new(issue), FlexItem::fixed(1), ctx)?;
children.remove("issue-123", ctx)?;
children.replace("issue-123", IssueRow::new(updated), ctx)?;
```

Live mutation APIs take `&mut MutationCtx<M>`, which carries lifecycle state, focus invalidation, and layout/redraw requests.

Mutation rules:

- Added live child runs `init` + `mount`.
- Removed live child runs `unmount` + `destroy`.
- Replaced child runs old `unmount` + `destroy`, then new `init` + `mount`.
- Removed focused child asks `FocusManager` to move focus to nearest valid target.
- Mutations request layout and redraw through context.
- Dynamic lists should use stable domain keys like `issue-123` or `tab-settings`; index keys are only safe for static lists.

Built-in containers use reserved child keys instead of raw string literals:

- `PanelHost`: `ChildKey::BODY`
- `Split`: `ChildKey::FIRST` and `ChildKey::SECOND`, independent of horizontal/vertical direction
- `Overlay`: layer keys supplied by overlay APIs when implemented

This keeps focus/event paths stable across orientation changes and internal refactors.

First concrete hosts:

- `PanelHost<C>` from `Panel::host(child)`
- future `DialogHost<C>`
- future table cell editor host

Single-slot hosts are strongly typed by default. For genuinely dynamic or heterogeneous containers, use a boxed escape hatch:

```rust
pub type NodeBox<M = ()> = Box<dyn TuiNode<M>>;

pub struct ChildEntry<M> {
    key: ChildKey,
    slot: ChildSlot<NodeBox<M>, M>,
    last_rect: Rect,
}

pub struct Children<M> {
    entries: Vec<ChildEntry<M>>,
}
```

Boxed nodes delegate `TuiNode` methods:

```rust
impl<M, N: TuiNode<M> + ?Sized> TuiNode<M> for Box<N> { /* forward all methods */ }
```

Use `Children` for `Flex`, `Tabs`, and overlays where dynamic child sets matter. All child add/remove/replace APIs go through `Children` so stable keys, lifecycle ordering, and last layout rects stay coherent. Prefer typed hosts/splits for fixed structure.

### Layout nodes

Wrap ratatui `Layout` into tree-aware containers.

First layout node:

```rust
pub struct Split<L, R> {
    direction: Direction,
    constraints: [Constraint; 2],
    left: L,
    right: R,
}
```

Common split ratios get ergonomic sugar:

```rust
Split::horizontal(left, right).ratio(30, 70)
```

`.constraints(...)` stays as ratatui escape hatch for advanced layouts.

Later:

- `Flex` for many children
- `Grid` only if needed
- `Overlay` for dialogs/popovers

## Layout Component Set

Tuicore layout components are tree nodes. They own children, calculate rects with ratatui primitives internally, and propagate layout/event/tick/focus/lifecycle. They are not extensions on ratatui `Layout`; they are consumer-facing components that use ratatui `Layout` as implementation detail.

### Layout design goals

- Read like web layout: declare direction, sizing, gap, alignment, and children.
- Hide ratatui area-splitting plumbing for common cases.
- Keep escape hatches for ratatui `Constraint` when needed.
- Avoid god layout class: small specialized layout nodes over one giant engine.
- Support one or many children without wrapper explosion.
- Preserve typed children where possible, with boxed escape hatch only for dynamic lists.

### Core layout primitives

#### `Split<L, R>`

Two-child layout for common master/detail, sidebar/content, label/input pairs.

```rust
Split::horizontal(left, right).ratio(30, 70)
Split::vertical(top, bottom).fixed_top(3)
Split::horizontal(nav, body).left(Constraint::Length(28)).right(Constraint::Fill(1))
```

Use first because it is simple, strongly typed, and covers the current 30/70 need.

#### `Flex<M>`

One-dimensional multi-child layout inspired by flexbox, backed by ratatui `Layout`.

```rust
Flex::row()
    .gap(1)
    .child("sidebar", sidebar, FlexItem::percent(30))
    .child("content", content, FlexItem::fill(1))

Flex::column()
    .child("header", header, FlexItem::fixed(3))
    .child("body", body, FlexItem::fill(1))
    .child("status", status, FlexItem::fixed(1))
```

`Flex` is the many-child workhorse. It uses boxed `Children<M>` internally because Rust cannot easily store arbitrary heterogeneous typed child lists without tuples or macros. For fixed two-child layouts, prefer `Split<L, R>` so concrete child types are preserved.

Builder shape:

```rust
Flex::row()
    .gap(1)
    .padding(Padding::horizontal(1))
    .justify(MainAlign::Start)
    .align(CrossAlign::Stretch)
    .child("sidebar", sidebar, FlexItem::percent(30))
    .child("content", content, FlexItem::fill(1))
```

`child` accepts any node and boxes it:

```rust
fn child<C: TuiNode<M> + 'static>(self, key: impl Into<ChildKey>, child: C, item: FlexItem) -> Self
```

`Flex` v1 requires explicit, unique, stable `ChildKey`s. Order-derived automatic keys are not part of v1 because they break stable focus/event paths during insert/reorder.

Empty `Flex` is allowed as an inert placeholder: it renders nothing, registers no focus targets, and handles no events. Builders may add `.require_non_empty()` later if consumers need validation.

`Children` invariants:

- Keys are unique.
- Same key preserves its `ChildSlot` across reorder.
- Removed key runs child `unmount` + `destroy` when live.
- New key runs child `init` + `mount` when container is live.
- Replacing a child under the same key runs old `unmount` + `destroy`, then new `init` + `mount`.
- `.child(...) -> Self` validates duplicate keys in all builds and panics with a clear duplicate-key message because duplicate literal keys are programmer errors in declarative builders.
- `.try_child(...) -> Result<Self, DuplicateChildKey>` is available for dynamic construction.
- All dynamic `Children` mutators return `Result`.

V1 axis API is intentionally small:

- `gap(u16)`
- `padding(Padding)`
- `justify(MainAlign::{Start, Center, End, SpaceBetween})`
- `align(CrossAlign::{Start, Center, End, Stretch})`

No wrap, grow/shrink algorithm, or CSS cascade in v1.

V1 has no intrinsic measurement. Cross-axis alignment needs explicit cross size unless stretching:

```rust
pub enum CrossSize {
    Fill,
    Fixed(u16),
    Percent(u16),
}
```

`FlexItem` defaults to `CrossSize::Fill`. `CrossAlign::Start/Center/End` positions explicit cross-size rects; `CrossAlign::Stretch` uses the full cross-axis area. Builders like `FlexItem::fill(1).cross_size(CrossSize::Fixed(3))` keep common cases terse.

Sizing API maps to ratatui constraints:

```rust
pub struct FlexItem {
    main: MainSize,
    cross: CrossSize,
}

pub enum MainSize {
    Fixed(u16),
    Min(u16),
    Max(u16),
    Percent(u16),
    Ratio(u32, u32),
    Fill(u16),
}
```

#### `Grid<M>`

Two-dimensional layout for dashboards/forms where rows and columns are explicit.

```rust
Grid::new()
    .columns([Track::fixed(20), Track::fill(1), Track::fill(2)])
    .rows([Track::fixed(3), Track::fill(1)])
    .cell(0, 0, label)
    .cell(1, 0, input)
    .cell_span(1, 1, 2, 1, table)
```

Do not implement first unless gallery/forms need it. `Flex` + nested `Split` should carry most early use cases.

No intrinsic measurement in v1. Children receive allocated rects. `Fixed`, `Min`, `Max`, `Percent`, `Ratio`, and `Fill` are explicit constraints only. Measurement can be revisited after grid/forms prove a concrete need.

#### `Overlay<M>`

Layered layout for dialogs, popovers, command palettes, and toasts.

```rust
Overlay::new(base)
    .modal(dialog)
    .popover(anchor, suggestions)
```

Overlay owns z-order, focus trap, event capture, and backdrop behavior. Defer until dialog/popover work starts.

#### `PanelHost<C>` / single-child hosts

Single child host for chrome/container components.

```rust
Panel::new().top_left("Search").host(TextInput::new())
Dialog::new().title("Edit").body(TextareaInput::new())
```

Hosts are not layout managers by themselves. They reserve inner area and delegate to one child.

### Layout tree examples

#### App shell

```rust
Panel::new()
    .top_left("tuicore")
    .host(
        Flex::column()
            .child("toolbar", toolbar, FlexItem::fixed(1))
            .child(
                "main",
                Split::horizontal(nav, content).ratio(25, 75),
                FlexItem::fill(1),
            )
            .child("status", status, FlexItem::fixed(1)),
    )
```

#### Form row

```rust
Split::horizontal(Label::new("Name"), TextInput::new())
    .left(Constraint::Length(16))
    .right(Constraint::Fill(1))
```

#### Dashboard later

```rust
Grid::new()
    .columns([Track::fill(1), Track::fill(1), Track::fill(1)])
    .rows([Track::fixed(7), Track::fill(1)])
    .cell(0, 0, metric_a)
    .cell(1, 0, metric_b)
    .cell(2, 0, metric_c)
    .cell_span(0, 1, 3, 1, table)
```

### Layout implementation rules

- Layout nodes store child rects from the `layout` phase.
- Render uses stored rects and never recomputes mutable state.
- Event routing uses stored rects/focus paths.
- Layout nodes register slots with stable `ChildKey`s so focus paths are stable.
- `gap`, `padding`, and alignment are structural defaults from preset where possible.
- Preset structural defaults apply at construction; builder calls override per node.
- Raw ratatui `Constraint` stays available as escape hatch, but common APIs use semantic helpers like `.ratio(30, 70)`, `FlexItem::fill(1)`, and `Track::fixed(3)`.
- `Flex` and `Grid` should not own visual chrome; use `PanelHost`, `DialogHost`, etc. for chrome.
- Layout components own no visual styling or hardcoded colors.

### Focus

Preserve current `FocusChain` / `FocusRouter` ideas, but attach them to tree paths.

- Every focusable node can expose a `FocusId`.
- Layout/container nodes decide traversal order.
- `Tab` / `Shift+Tab` move through focusable descendants.
- Focus changes call `focus(target, false)` on old path and `focus(target, true)` on new path.

Focus types:

```rust
pub struct FocusId(...);
pub struct FocusTarget { path: TreePath, id: FocusId, enabled: bool }

pub trait FocusScope {
    fn focusables(&self, out: &mut Vec<FocusTarget>);
}
```

Focus registration normally happens during layout via `LayoutCtx::register_focusable`, so focus order follows structural/render order. `FocusScope` remains only as an optional traversal override. Leaf `FocusId`s are local. Containers append stable `ChildKey`s while collecting descendants so the runtime sees full `TreePath`s. Containers provide default traversal order matching render order and expose builder overrides like `.focus_order(...)`. Apps can still own top-level focus topology by choosing root focus scopes and handling `FocusRequest`s.
Before dispatch, `FocusManager` validates the current `FocusTarget` against latest layout focus targets. If missing or disabled, it repairs focus to nearest enabled target by traversal order and emits focus change calls.

### App runtime

Replace `TuicoreApp` with direct ratatui + crossterm runtime.

Responsibilities:

- terminal setup/teardown
- event polling
- tick scheduling
- root event dispatch
- root tick dispatch
- redraw loop
- panic-safe terminal cleanup if feasible

Do not make `TuicoreApp` a god object. Split runtime into small modules:

- `TerminalGuard`: raw mode, alt screen, cleanup
- `EventSource`: crossterm polling and conversion to `TuiEvent`
- `Scheduler`: frame/tick timing
- `TreeDispatcher`: event target path, bubbling, lifecycle dispatch
- `LayoutEngine`: calls root layout, stores focus targets and hit regions
- `FocusManager`: focus paths, traversal, requests
- `Renderer`: draw root node into terminal frame
- `TuicoreApp`: thin facade/builder tying these together

`TreeDispatcher` does not own child traversal. It delegates to root/container nodes and applies requests from `EventCtx`.

## Constitution invariants

Every phase must preserve:

- Theme roles only; no raw component colors.
- Presets own structural defaults.
- Shared `BorderKind` and border helpers for chrome.
- Keybindings remain configurable; do not hardcode where a binding exists.
- Global `animation.enabled == false` is a kill switch for all animations.
- Scroll behavior uses shared `ScrollState` and smooth offset animation only.
- Render stays pure; layout/event/tick update state, render reads.

## Migration Phases

### Phase 0 — Constitution update

1. Update `AGENTS.md` to replace `ratatui` + `tuirealm` core stack wording with `ratatui` + direct `crossterm` + tuicore tree runtime.
2. Remove guidance about hiding tuirealm plumbing after the new runtime is accepted.
3. Keep all existing render purity, theme, preset, keybinding, animation, focus, and scroll rules.

### Phase 1 — Foundation traits and event types

1. Add `src/event.rs` with tuicore-owned `TuiEvent`, `Key`, `KeyEvent`, `KeyModifiers`, mouse event types.
2. Add direct `crossterm` dependency and use `ratatui::backend::CrosstermBackend`; stop relying on `tuirealm::ratatui` reexports.
3. Add conversions from crossterm events internally.
4. Add `src/node.rs` with `TuiNode`, lifecycle hooks, `EventCtx`, `EventOutcome`, `LayoutCtx`, and `LayoutResult`.
5. Port `keybindings`, `focus`, and `scroll` to tuicore event/key types.
6. Re-export new public framework types from `src/lib.rs`.
7. Keep existing tuirealm code compiling during this phase with temporary adapters only if needed.
8. Quarantine tuirealm: no new public API may mention `tuirealm`; adapters stay private and must be deleted by Phase 4.

Validation:

- `cargo check`
- unit tests for key conversion and modifier matching

### Phase 2 — Runtime shell

1. Replace or parallel `TuicoreApp` with a tree runtime that owns a root `TuiNode`.
2. Implement crossterm terminal adapter directly using ratatui backend.
3. Implement tick scheduling from animation settings.
4. Implement redraw requests from `EventCtx` and `TickResult`.
5. Implement minimal `LayoutEngine`, `TreeDispatcher`, `FocusManager`, routed dispatch, bubbling, `ChildSlot`, and `Children` before broad component migration.
6. Validate runtime with a no-op/test node before porting real components.
7. Keep old app wrapper temporarily only if needed for incremental port.

Validation:

- minimal no-op/test node runs on new runtime
- terminal restores on normal quit

### Phase 3 — Port leaf components

1. Port `Panel`, `TextInput`, and `PanelHost<C>` as first vertical slice.
2. Prove key routing, focus, submit message, tick propagation, and redraw through `Panel::host(TextInput)`.
3. Convert `examples/quickstart.rs` to `Panel::host(TextInput)` on the new runtime.
4. Port `TextareaInput`.
5. Port `Spinner`.
6. Port `List` and `DataView`.
7. Remove `Cmd`, `CmdResult`, `State`, `Attribute`, and `AttrValue` usage from component public APIs for migrated leaves.

Validation:

- component tests compile against tuicore event/key types
- gallery renders each component

### Phase 4 — Containers and layout tree

1. Implement `Split<L, R>` for two-child fixed/flex layouts.
2. Implement minimal `Flex` row/column with fixed/fill/percent/gap/padding/align/justify.
3. Make container nodes propagate tick, lifecycle hooks, focus, and event routing to children.
4. Port `Tabs` after `Split`, minimal `Flex`, and `Children<M>` are proven; use `Children<M>` for tab bodies.
5. Ensure examples use real tree composition consumers should copy.

Validation:

- panel + input focus highlight works without manual tick plumbing
- split 30/70 demo works
- nested panel/split/input tree ticks exactly once per frame

### Phase 5 — Mouse hit-testing, capture, and overlay routing

Focused-path routing, bubbling semantics, `FocusTarget`, `ChildKey`, `ChildSlot`, and `Children` must already be accepted before `Split`/`Flex` implementation. This phase deepens interaction behavior after the first components prove the shape.

1. Add mouse hit-testing after layout nodes can track child rects.
2. Add capture phase only if real use cases require it.
3. Add overlay/dialog-specific focus trap and backdrop behavior when those components start.
4. Expand `EventCtx` requests beyond redraw/layout/quit/focus only when real components need them.

Validation:

- `Esc` can bubble from input to dialog/panel/app policy
- `Tab` changes focus across nested tree
- input-specific keys call `stop_propagation()` and do not leak upward

### Phase 6 — Remove tuirealm

1. Remove tuirealm dependency once `cargo check --examples` passes and all `src/`, examples, and tests imports are off `tuirealm`; visual gallery parity may lag, but compile cannot.
2. Remove tuirealm imports from `src/` and `examples/`.
3. Remove tuirealm dependency from `Cargo.toml` if not already removed.
4. Replace tests using tuirealm test backend with ratatui test backend and tuicore events.
5. Update quickstart and gallery to show new tree framework.
6. Update docs/comments naming away from realm concepts.

Validation:

- `cargo fmt`
- `cargo check --examples`
- `cargo test`
- `rg tuirealm src examples tests Cargo.toml` returns no matches
- run gallery manually

## Validation checklist

- Render methods take `&self`; mutation happens in layout/event/tick/lifecycle.
- Lifecycle replacement test: old child unmounts/destroys before new child init/mounts.
- Focused child can handle event and prevent parent policy.
- Ignored child event bubbles to parent.
- Disabled animation snaps or stays inactive during event/focus/tick.
- Layout reruns after state changes before render.
- Panel-hosted input focus highlight works without manual tick plumbing.
- `rg tuirealm src examples tests Cargo.toml` returns no matches before final removal is accepted.

## DOM ideas to steal carefully

- Tree structure with parent/child ownership.
- Event bubbling and optional capture.
- Lifecycle hooks.
- Focus traversal through focusable descendants.
- Slot/body concepts for containers.
- Event context that can stop propagation and request side effects.

## DOM ideas to avoid

- Runtime diff/reconciliation.
- Global mutable DOM registry.
- Stringly typed attributes as primary API.
- Hidden mutation during render.
- Complex CSS cascade.

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Event model overbuilt too early | Delays component work | Start with focused-path routing + bubbling only |
| Heterogeneous multi-child ownership gets complex | Hard APIs | Start with typed `Split<L, R>` and single-slot hosts |
| Removing tuirealm breaks examples broadly | Migration churn | Port quickstart first, then components one by one |
| Focus tree becomes magic | User confusion | Keep focus IDs and traversal explicit |
| Lifecycle hooks become dumping ground | Side effects spread | Keep render purity and prefer event/tick for behavior |

## Plan handoff

1. Foundation: event types + `TuiNode` + runtime shell.
2. First vertical slice: quickstart with `Panel::host(TextInput)`.
3. Layout slice: `Split` with panel/list + panel/input.
4. Component migration: spinner, panel, inputs, list/data view, tabs.
5. Remove tuirealm and clean docs/tests.
