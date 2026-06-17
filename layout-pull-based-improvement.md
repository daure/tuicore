# Pull-based layout improvement plan

## Purpose

Make tuicore layout smarter without making it dogmatic.

Components should be able to describe their normal-flow sizing needs, containers should be able to ask before assigning rectangles, and downstream apps should be able to override policy where product needs differ. Existing push-based usage must keep working.

This is not a rewrite of tuicore layout. It is an additive measurement contract plus smarter container behavior, introduced in controlled phases. The contract is shared; layout components stay small and composable instead of becoming one god container.

## Problem

Tuicore layout is currently mostly **push-based**: a parent computes a `Rect`, then calls `TuiNode::layout(area, ctx)` and `TuiNode::render(frame, area)`. The child must fit inside whatever rectangle it receives.

That creates bad ergonomics:

- Parents need component-specific magic numbers.
- Components cannot say “I need this much space.”
- Small parent mistakes clip children silently.
- Examples and apps duplicate layout knowledge that belongs in reusable components.
- Library users must remember style-specific internals instead of composing components declaratively.

Recent dropdown work exposed this clearly. A bordered dropdown trigger needs **3 rows**: top border, content row, bottom border. A filled dropdown trigger needs **1 row**. Panel gallery title controls gave the dropdown only 2 rows, so selected trigger text disappeared. The dropdown component knew its style, but the parent owned height.

## Design north star

Adopt a lightweight, Rust-friendly version of the proven layout model used by Flutter, SwiftUI, CSS intrinsic sizing, iced, egui, and ratatui constraints:

1. **Constraints/proposals go down.** A parent describes available width/height.
2. **Size hints go up.** A child reports minimum and preferred normal-flow size under that proposal.
3. **Parent allocates final rectangles.** Containers apply explicit, documented policy.
4. **Measurement/render stay pure.** `measure` and `render` read state only. `layout` may register layout metadata in `LayoutCtx`, but must not start animations or perform render-time effects. Input/update starts animations; `tick(dt)` advances them.

The system should be:

- **Customizable:** callers can choose fixed, fit-content, fill, percent, shrink, max, overflow, and alignment policies.
- **Predictable:** allocation order and overflow behavior are documented and testable.
- **Incremental:** unmeasured components do not break existing apps.
- **Component-owned:** borders, presets, trigger variants, configured visible rows, headers, and overlays belong to component measurement instead of parent magic numbers.
- **Non-magical:** smart defaults, explicit escape hatches.

## Shared layout spacing and alignment plan

Spacing belongs to layout components, not `TuiNode`. `TuiNode` describes component capability; containers own relationships between children.

Add shared spacing primitives:

```rust
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

pub struct Gap {
    pub row: u16,
    pub column: u16,
}
```

Implementation notes:

- Move `Padding` out of `flex` into a shared spacing module if practical; keep `tuicore::Padding` re-export unchanged.
- Add `Gap` as shared public type and re-export as `tuicore::Gap`.
- Keep old APIs for compatibility:
  - `Flex::gap(u16)` remains uniform main-axis gap.
  - `Grid::gap(column, row)` remains legacy order.
- Add explicit APIs:
  - `Grid::gaps(Gap)` maps `column` to column gaps and `row` to row gaps.
  - `Grid::padding(Padding)` resolves tracks inside padded inner area; measurement includes padding; overflow diagnostics compare against inner size.
- Do **not** add `Flex::gaps(Gap)` in this pass unless there is a concrete cross-axis use. Flex has one item sequence; `gap(u16)` is clearer and avoids ignored cross-axis fields.

Flex alignment improvements:

- Add `MainAlign::SpaceAround` and `MainAlign::SpaceEvenly` as accepted pre-1.0 source-break risk for downstream exhaustive matches.
- Keep deterministic integer behavior:
  - base `gap` is minimum inter-item gap.
  - Implement justification as a space-vector: leading space, inter-item spaces, trailing space.
  - `SpaceBetween`: single child uses `[0, 0]`; otherwise spare distributes across `count - 1` inter-item spaces. Remainder goes left-to-right so last child still reaches trailing edge when possible.
  - `SpaceAround`: spare distributes across `count * 2` half-spaces. Leading/trailing spaces get one half-space; inter-item spaces get two half-spaces. Remainder goes left-to-right across half-spaces before folding into full spaces.
  - `SpaceEvenly`: spare distributes across `count + 1` spaces. Remainder goes left-to-right from leading edge to trailing edge.
  - Existing `Center` and `End` keep spare as offset only.
- Add `FlexItem::align_self(CrossAlign)` with `align: Option<CrossAlign>`.
- Effective cross alignment = item override or container alignment.
- Cross-size rules:
  - `CrossSize::Auto + Stretch` fills cross axis.
  - `CrossSize::Auto + Start/Center/End` uses child preferred cross size when measured; legacy fallback fills for compatibility.
  - `CrossSize::Fixed + any alignment` uses `size.min(cross_available)` and positions by effective alignment; `Stretch` behaves like `Start` for fixed-size children. Containment wins in v1; overflow diagnostics can be added later if fixed cross-size is clipped.

Do not add CSS margin, wrap, order, or baseline alignment yet. Those are separate complexity gates.

## Current layout-related pieces

### `TuiNode`

File: `src/node.rs`

Current contract:

```rust
pub trait TuiNode<M = ()> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult;
    fn render(&self, frame: &mut Frame, area: Rect);
    // event/focus/lifecycle/tick...
}
```

`LayoutResult` currently only contains the assigned area:

```rust
pub struct LayoutResult {
    pub area: Rect,
}
```

There is no measurement pass, no size hint, and no intrinsic size concept.

### `LayoutCtx`

File: `src/node.rs`

Current responsibilities:

- tracks current tree path
- registers focus targets
- registers hit regions
- supports `push_slot(...)` for child paths

It does not collect size requests, overflow diagnostics, or overlay placement data.

### `Flex`

File: `src/components/flex.rs`

Current capabilities:

- row/column layout
- `FlexItem::fixed(size)`
- `FlexItem::fill(weight)`
- `FlexItem::percent(percent)`
- cross-axis `Auto` or `Fixed`
- gap/padding/justify/align

Current limitation: `FlexItem::fixed(3)` requires the parent/app to know the child needs 3 rows. `FlexItem::fixed(1)` requires knowing a filled dropdown only needs 1 row. There is no way to say “ask this child for its content size.”

### `Split`

File: `src/components/split.rs`

Current capabilities:

- two children
- Ratatui constraints for first/second area
- ratio and explicit constraints

Current limitation: still parent-driven. Split cannot ask one child for fixed/min size and give the rest to the other child unless caller encodes that manually as constraints.

### Missing layout primitives

The smarter contract should be used by more than Flex/Split immediately:

- `Stack`: layers children in the same area with alignment and optional insets. Useful for badges, centered empty states, scrims, and decorative overlays.
- `Overlay`: anchors an overlay child to a base child without making overlay consume normal-flow space. Useful for dropdowns, popovers, command palettes, and contextual help.
- `Grid`: lays children into rows/columns using fixed, percent, fill, and fit-content tracks. Useful for forms, dashboards, galleries, and settings pages.

These should be first-class components, not app-local examples.

### `Dropdown`

File: `src/components/dropdown.rs`

Example component that needs intrinsic sizing:

- `DropdownVariant::Bordered` trigger wants height `3`.
- `DropdownVariant::Filled` trigger wants height `1`.
- Popup is overlay and should not consume normal layout height.
- Trigger height is style-dependent and belongs in the dropdown component, not every caller.

## Core API decisions

### Measurement is immutable and side-effect free

Add a default `measure` method to `TuiNode`:

```rust
pub trait TuiNode<M = ()> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::legacy_fill()
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult;
    fn render(&self, frame: &mut Frame, area: Rect);
}
```

Rules:

- `measure` is pure, idempotent, and side-effect free.
- It may be called multiple times per frame.
- It must not register focus, hit regions, overlays, or lifecycle effects.
- It may read immutable component state, config, theme/preset choices already stored in the component, and cached content metrics if those caches are maintained outside render.
- If future components need expensive measurement, add explicit cache support later; do not make v1 measurement mutable.

### Proposals are axis-aware

`max_width`/`max_height` are not enough. Text height depends on width, percent needs definite parents, and some containers need exact cross-axis sizing.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisProposal {
    /// Parent has no useful bound on this axis.
    Unbounded,
    /// Child may use up to this amount.
    AtMost(u16),
    /// Parent intends this exact size on this axis.
    Exact(u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutProposal {
    pub width: AxisProposal,
    pub height: AxisProposal,
}
```

Helpers should keep common cases ergonomic:

```rust
impl LayoutProposal {
    pub fn at_most_area(area: Rect) -> Self;
    pub fn exact(area: Rect) -> Self;
    pub fn at_most(width: u16, height: u16) -> Self;
}
```

### Size hints separate minimum, preferred, and fill behavior

Use familiar concepts from CSS intrinsic sizing and iced/ratatui lengths, but keep the public model small.

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LayoutSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LayoutSizeHint {
    /// Whether this is real intrinsic data or legacy fallback data.
    pub source: HintSource,
    /// Smallest normal-flow size where component still has meaningful output.
    pub min: LayoutSize,
    /// Desired normal-flow size under the proposal, before parent allocation.
    pub preferred: LayoutSize,
    /// Whether the component is happy to consume extra width/height.
    pub expand: AxisExpand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HintSource {
    /// Component implemented measurement intentionally.
    Measured,
    /// Default compatibility hint from a component that has not opted in yet.
    LegacyUnmeasured,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AxisExpand {
    pub width: bool,
    pub height: bool,
}
```

Invariants:

- `min.width <= preferred.width` and `min.height <= preferred.height`.
- `preferred` should already respect hard `Exact` proposals when practical.
- `preferred` should be clamped to `AtMost` unless overflow is intrinsic and unavoidable.
- `min` may exceed `AtMost` only when the component cannot produce meaningful output inside the proposal.
- Hints describe **normal-flow** size only; overlays are separate.

Default behavior must preserve legacy push layout:

```rust
impl LayoutSizeHint {
    pub fn unmeasured() -> Self;
    pub fn legacy_fill() -> Self;
    pub fn fixed(width: u16, height: u16) -> Self;
    pub fn content(width: u16, height: u16) -> Self;
    pub fn normalized(self, proposal: LayoutProposal) -> Self;
}
```

`legacy_fill()` / `unmeasured()` should avoid the “content child gets zero space” trap. For unmeasured components, containers should treat `source == LegacyUnmeasured` as “no intrinsic data” and use documented fallback instead of allocating zero. A measured component that genuinely wants zero size must return `source == Measured`, so Flex can distinguish that from legacy compatibility.

Normalization rules:

- If `min > preferred`, raise `preferred` to `min` and record a debug diagnostic.
- If `preferred > AtMost`, clamp `preferred` to `AtMost` unless that would go below `min`.
- If `min > Exact`, keep `min`, set preferred to `min`, and let the parent record overflow.
- Saturate all arithmetic; layout math must not underflow when gaps/padding exceed available space.
- `normalized(...)` itself should not require `LayoutCtx` and should not perform global side effects. In v1 it may use `debug_assert!` / test-only tracing for invalid hints; container `layout` records path-aware diagnostics after it knows child path/index.
- Percent math must widen before division: compute as `u32`/`usize`, divide, then clamp to `u16`.

`AxisExpand` is advisory. It tells containers that a component can use extra space if the container/item policy grows it. It does **not** make `FitContent` behave like `Fill`; `Fill(weight)` remains the only Phase 1 Flex mode that consumes extra main-axis space automatically.

### Public length model should be explicit

Long-term, a generic length-like model can grow over time:

```rust
pub enum LayoutLength {
    Fixed(u16),
    Percent(u16),
    Ratio(u32, u32),
    FitContent,
    Shrink,
    Fill(u16),
}
```

Phase 1 should not introduce a parallel generic length API. Extend the existing `FlexMain` surface only:

```rust
pub enum FlexMain {
    Fixed(u16),
    Percent(u16),
    FitContent,
    Fill(u16),
}
```

Naming decision: prefer `FitContent` over `Content` or `Auto`. It says “use intrinsic content, clamped by available space” and maps cleanly to CSS/iced mental models. Keep `FlexItem::content()` as a friendly alias if desired, but document it as fit-content.

### Overflow is explicit, not silent

Add minimal overflow diagnostics in the same phase as `FitContent`, but do not change the public `LayoutResult` shape in this plan. Adding a field to `LayoutResult` would break downstream struct literals that currently use `LayoutResult { area }`.

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OverflowState {
    pub x: AxisOverflow,
    pub y: AxisOverflow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AxisOverflow {
    #[default]
    None,
    Clipped { needed: u16, available: u16 },
}

ctx.record_overflow(path, axis, needed, available);
```

Concrete diagnostic shape:

```rust
pub struct LayoutOverflowDiagnostic {
    pub path: TreePath,
    pub child_index: Option<usize>,
    pub axis: LayoutAxis,
    pub needed: u16,
    pub available: u16,
    pub policy: OverflowPolicyName,
}
```

Diagnostics should be recorded during `layout`, after `push_slot` establishes the child path. Do not solve every overflow UI in v1. Do make clipping visible to tests and debugging without a breaking `LayoutResult` change.

## Flex allocation policy

`FitContent` cannot ship without deterministic behavior. Define this before implementation.

For both row and column, resolve the main axis in this order:

1. Subtract padding and gaps from available main-axis space using saturating math.
2. Resolve `Fixed` items to their requested basis. If available content space is zero, assigned rectangles are zero-sized but overflow diagnostics retain the requested fixed basis.
3. Resolve `Percent` items against definite parent main-axis size. If parent is indefinite, v1 treats percent as `FitContent` and records an indefinite-percent diagnostic.
4. Measure `FitContent` items with a proposal that reflects known cross-axis bounds and remaining main-axis bounds.
5. Use each `FitContent` preferred main size as its basis and min main size as its shrink floor.
6. Assign remaining positive space to `Fill(weight)` items by weight.
7. If fixed + percent + fit-content bases exceed available space, shrink shrinkable fit-content items toward their min size.
8. If still too large, mark overflow on the axis and clip according to existing render behavior.

Precise integer rules:

- Padding/gap subtraction is saturating. If gaps alone exceed available space, children receive zero main-axis space and overflow diagnostics are recorded.
- Percent sizes use widened integer floor division: `(available as u32 * percent as u32) / 100`, then clamp to `u16`.
- Percent totals above 100 are allowed but likely overflow; they are not normalized down silently.
- Remainders from weighted fill distribution are assigned in stable child order, one cell at a time.
- `Fill(0)` behaves as `Fill(1)` for compatibility unless a later API explicitly reserves `0` as “disabled.”
- Shrink debt is distributed across shrinkable fit-content children in stable rounds until each reaches min; then remaining debt becomes overflow.
- `LegacyUnmeasured` under `FitContent` uses fallback basis `1` on the main axis, clamped to available space, and `0` on the cross axis unless the cross-axis mode assigns size. This preserves visibility without pretending to know intrinsic size. Containers may later expose `FitFallback` to override this.

Cross-axis policy:

- `Auto` should use child preferred cross size when available, clamped to parent cross-axis bounds.
- `Fixed` cross-axis stays fixed.
- Alignment still controls placement inside cross-axis slack.

Reflow policy:

- Phase 1 Flex is one-pass plus optional targeted remeasure: measure with known cross-axis proposal, assign main sizes, then remeasure only children whose height/width depends on the now-exact opposite axis if the component marks that dependency later.
- Until that marker exists, v1 documents “no iterative reflow.” Components with wrapping text should prefer conservative min/preferred hints.
- Phase 2 can add explicit `MeasureDependency` if Panel/Text wrapping needs stronger guarantees.

Customization knobs to add over time:

```rust
pub enum FlexOverflowPolicy {
    Clip,
    Truncate,
    Scroll,
}

pub enum FitFallback {
    LegacyFill,
    Fixed(u16),
    Min(u16),
}
```

Phase 1 can default to clip + diagnostics, but API shape should not block scroll/truncate later.

## Overlay sizing policy

Normal-flow measurement excludes overlays.

For dropdown:

- Trigger contributes to normal flow.
- Popup panel is overlay and does not increase parent row height.
- Popup placement may use viewport/assigned trigger area during layout/render, but does not change `measure` normal-flow hint.

Future extension:

```rust
pub struct OverlaySizeHint {
    pub min: LayoutSize,
    pub preferred: LayoutSize,
    pub anchor: OverlayAnchor,
}
```

Phase 1 should document overlay as a v1 limitation: normal-flow sizing gets fixed first; smarter viewport-clamped overlay sizing and z-layer policy can follow.

## Component measurement checklist

Every component that implements `measure` must answer:

- What is normal-flow content?
- What is overlay content?
- How do borders/padding/presets affect min and preferred size?
- Does width affect height through wrapping/truncation?
- Does configured visible row/line count affect height?
- Does scroll behavior mean “min can be smaller than preferred”?
- What happens under tiny proposals (`0`, `1`, border-only spaces)?
- Which tests prove measure and render agree?

Suggested initial hints:

- `Dropdown`: trigger height by variant; popup excluded.
- `TextInput`: height `1`, preferred width from placeholder/value if useful, expandable width.
- `TextareaInput`: min `1`, preferred configured visible lines, expandable width/height if configured.
- `Panel`: border/title contribution plus child hint when child is measurable.
- `Tabs`: header height plus active body hint when useful.
- `Spinner`: min/preferred visual size.
- `DataView`: header + one row minimum, preferred visible rows, usually fill.

## Phased plan

### Phase 1 — Contract, deterministic Flex, dropdown proof

Goal: make one real bug disappear without locking tuicore into a weak public API.

Subphase 1A — measurement contract:

- Add `AxisProposal`, `LayoutProposal`, `LayoutSize`, `AxisExpand`, and `LayoutSizeHint`.
- Add `HintSource::{Measured, LegacyUnmeasured}` or equivalent so containers can distinguish real zero-size hints from compatibility fallback.
- Add default pure `TuiNode::measure(&self, proposal) -> LayoutSizeHint`.
- Default measurement preserves legacy behavior via `unmeasured()` / `legacy_fill()` and documented `FitContent` fallback.
- Add hint normalization and debug diagnostics for invalid hints.

Subphase 1B — Flex fit-content:

- Add `FlexItem::fit_content()` and optionally `FlexItem::content()` alias.
- Implement deterministic Flex main-axis policy: fixed/percent/fit-content/fill order, shrink-to-min, overflow diagnostics.
- Add minimal overflow reporting through non-breaking `LayoutCtx` diagnostics.

Subphase 1C — dropdown proof:

- Implement `measure` for `Dropdown`:
  - bordered trigger: min/preferred height `3`
  - filled trigger: min/preferred height `1`
  - width hint: trigger chrome + selected label or placeholder display width, using Unicode display width where the project already does so or a clearly documented helper otherwise
  - min width: smallest meaningful trigger chrome/content width
  - `expand.width = true`, `expand.height = false`
  - popup excluded from normal flow
- Update gallery panel title controls to use fit-content dropdown fields instead of fixed row magic.
- Add docs showing “parent asks for content size; component owns variant height.”

Tests:

- Vertical `FlexItem::fit_content()` gives bordered dropdown 3 rows.
- Vertical `FlexItem::fit_content()` gives filled dropdown 1 row.
- Horizontal fit-content uses child preferred width when provided.
- Popup does not affect normal-flow height.
- Fit-content items shrink toward min when terminal space is too small.
- Overflow diagnostic is emitted when min sizes cannot fit.
- Unmeasured child with fit-content uses fallback and does not disappear silently.
- Existing fixed/fill/percent behavior remains compatible.
- Percent rounding, remainder distribution, gap saturation, and shrink-debt distribution follow documented rules.

Exit criteria:

- Parent code no longer needs dropdown variant height knowledge.
- Existing examples compile and render with no intentional regressions.
- New layout behavior is documented and covered by tests.

### Phase 2 — Container hardening and customization

Goal: make the system useful beyond dropdowns and safe for downstream apps.

Deliverables:

- Harden Flex for edge cases:
  - zero-sized areas
  - large gaps/padding
  - percent over-allocation
  - multiple fit-content children
  - mixed fit-content/fill/percent/fixed children
- Add configurable per-item policy where needed:
  - min override
  - max override
  - fit fallback
  - shrink allowed/disallowed
  - overflow policy placeholder (`Clip` first; scroll/truncate later)
- Add `Split` support for intrinsic sizing:
  - first/second can be fixed, ratio, fill, or fit-content
  - one side can request content size and the other receives remainder
  - overflow diagnostics when both sides cannot fit
- Add `Stack`:
  - children share the same parent area by default
  - per-child alignment and inset
  - optional per-child fixed size or fit-content size
  - render order matches child order; event/focus routing stays child-key based
- Add `Overlay`:
  - base child receives full normal-flow area
  - overlay child is anchored relative to base/viewport area
  - overlay does not affect normal-flow measurement
  - placement supports top/bottom/left/right/center anchors and clamps to available area
- Add `Grid`:
  - configurable rows and columns with fixed/percent/fill/fit-content tracks
  - per-child row/column/span/alignment
  - deterministic integer distribution and overflow diagnostics
  - fit-content tracks use child measurement when available and fallback otherwise
- Add a reusable helper for clamping hints:
  - normalize min/preferred invariants
  - clamp to proposals
  - record diagnostics on invalid hints in debug/test builds
- Document migration guidance:
  - when to use fixed vs fit-content vs fill
  - measured component list
  - warning not to replace every fixed size blindly
  - how fallback works for custom/unmeasured nodes

Tests:

- Allocation priority table tests for Flex.
- Split content + fill tests.
- Stack alignment/inset and layered event routing tests.
- Overlay normal-flow exclusion and clamped anchor tests.
- Grid fixed/percent/fill/fit-content track tests.
- Percent in definite and indefinite proposals.
- Custom min/max override tests.
- Debug diagnostics for invalid child hints.

Exit criteria:

- Flex and Split support content-aware layouts with predictable rules.
- Users have escape hatches for product-specific behavior.
- Documentation explains the mental model without requiring internals knowledge.

### Phase 3 — Broader component hints and example cleanup

Goal: remove parent-owned component internals from common tuicore usage.

Deliverables:

- Implement measurement for high-value components using the checklist:
  - `TextInput`
  - `TextareaInput`
  - `Panel`
  - `Tabs`
  - `Spinner`
  - `DataView` where practical
- Update examples/gallery layouts to prefer `fit_content()` for intrinsic controls and `fill()` for content regions.
- Add gallery pages for every layout type:
  - Flex page: fixed, percent, fit-content, fill, gap, padding, alignment
  - Split page: horizontal/vertical ratio and content+fill examples
  - Stack page: layered panel, centered empty state, badge overlay
  - Overlay page: base content with anchored popover/dropdown-style overlay
  - Grid page: dashboard/form layout with mixed track sizing
- Add one composition gallery page or section showing these layouts nested together.
- Audit examples for magic numbers tied to borders, headers, tabs, visible rows, or preset internals.
- Add component docs with sizing behavior.
- Add a “custom component measurement” guide for downstream authors.
- Consider an overlay measurement follow-up if dropdown/popup placement still needs smarter viewport behavior.

Tests:

- Component-specific measure/render agreement tests.
- Gallery regression tests for known clipped controls.
- Tiny-terminal behavior snapshots where feasible.
- Custom node example proving downstream components can opt into measurement.

Exit criteria:

- Common examples use declarative sizing instead of component-internal magic numbers.
- Measured components document their normal-flow and overlay behavior.
- Downstream authors can implement `measure` without reading Flex internals.

## Design decisions answered

- **Should measurement be immutable or mutable?** Immutable `&self` for v1. Pure, repeatable, side-effect free. Add cache support later if profiling proves need.
- **Should hints be axis-aware?** Yes. Proposals are per-axis (`Unbounded`, `AtMost`, `Exact`) because width-dependent height and definite/indefinite percent sizing matter.
- **Should overlays expose overlay size separately?** Not in normal-flow v1. Document overlay exclusion now; add `OverlaySizeHint` later if placement needs it.
- **Should `FlexItem::Auto` replace or complement `Content`?** Prefer explicit `FitContent`; `Auto` is too ambiguous. Alias `content()` only if docs say it means fit-content.
- **What happens when content-sized children cannot fit?** Shrink fit-content items toward min, then record overflow diagnostics and clip under existing rendering until richer overflow policies land.
- **How do unmeasured children behave?** Existing layout keeps working. `fit_content()` on unmeasured children uses documented fallback and must not allocate zero silently.

## Success criteria

- Parent code can ask for fit-content size without knowing dropdown variant internals.
- Bordered dropdown trigger receives 3 rows automatically.
- Filled dropdown trigger receives 1 row automatically.
- Popup remains overlay and does not affect normal-flow height.
- Flex behavior is deterministic for fixed/percent/fit-content/fill mixes.
- Overflow is visible in tests/debugging instead of purely silent clipping.
- Existing push-based code keeps working through default `measure()`.
- APIs remain small, Rust-idiomatic, and customizable.
