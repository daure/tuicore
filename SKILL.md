---
name: tuicore-layout
description: Use when working with tuicore.
---

# tuicore Layout Notes

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
- `Panel` — chrome wrapper with title/border/help. Remember inner area is smaller.
- `Tabs` — tab header + selected body. Body should own its own layout.

## Dropdown overlays

Dropdown popup rendering uses the overlay render pass. Containers should forward `render_overlay` to children. If a custom container owns child nodes, implement overlay forwarding or dropdowns nested inside it may not appear.

## PoC guidance

For quick PoCs:
1. Use `Flex::column()` for screen shell.
2. Use `FlexItem::content()` for headers/forms/actions.
3. Use `FlexItem::fill(1)` for main panes/lists/output.
4. Avoid guessed `fixed(3)` heights for panels with inputs/dropdowns.
