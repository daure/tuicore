use std::time::Duration;

use ratatui::{Frame, Terminal, backend::TestBackend, layout::Rect};

use crate::node::RevealAlignment;
use crate::{
    Animated, AnimationSettings, AxisExpand, AxisProposal, ChildKey, ChildSlot, EventCtx,
    EventOutcome, EventRoute, FocusCtx, FocusTarget, HintSource, LayoutProposal, LayoutResult,
    LayoutSizeHint, LifecycleCtx, Padding, RenderCtx, ScrollAxes, ScrollBehavior, ScrollDelta,
    ScrollOffset, ScrollSize, ScrollState, ScrollbarConfig, TickResult, TreePath, TuiEvent,
    TuiNode, animation_settings, preset,
};

/// A viewport that scrolls one arbitrary child node.
///
/// The child is measured on its scrolling axis without a viewport constraint, then rendered into
/// an isolated buffer before the visible portion is copied into the outer frame. This preserves
/// ordinary `TuiNode` composition while keeping child drawing clipped to the container viewport.
pub struct ScrollContainer<C, M = ()> {
    child: ChildSlot<C, M>,
    axes: ScrollAxes,
    scroll: ScrollState,
    padding: Padding,
    focus_reveal: bool,
    focused: bool,
    geometry: crate::ScrollGeometry,
    content_area: Rect,
    focus_areas: Vec<FocusTarget>,
    pending_reveal_path: Option<TreePath>,
}

impl<C, M> ScrollContainer<C, M>
where
    C: TuiNode<M>,
{
    pub fn vertical(child: C) -> Self {
        Self::new(child, ScrollAxes::Vertical)
    }

    pub fn horizontal(child: C) -> Self {
        Self::new(child, ScrollAxes::Horizontal)
    }

    pub fn both(child: C) -> Self {
        Self::new(child, ScrollAxes::Both)
    }

    fn new(child: C, axes: ScrollAxes) -> Self {
        Self {
            child: ChildSlot::new(ChildKey::body(), child),
            axes,
            scroll: ScrollState::from_preset(axes, preset().scroll()),
            padding: Padding::default(),
            focus_reveal: true,
            focused: false,
            geometry: ScrollState::new(axes).geometry(Rect::default(), ScrollSize::default()),
            content_area: Rect::default(),
            focus_areas: Vec::new(),
            pending_reveal_path: None,
        }
    }

    pub fn scrollbars(mut self, config: ScrollbarConfig) -> Self {
        self.scroll = self.scroll.scrollbars(config);
        self
    }

    pub fn scroll_behavior(mut self, behavior: ScrollBehavior) -> Self {
        self.scroll = self.scroll.behavior(behavior);
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn focus_reveal(mut self, enabled: bool) -> Self {
        self.focus_reveal = enabled;
        self
    }

    pub fn child(&self) -> &C {
        self.child.child()
    }

    pub fn child_mut(&mut self) -> &mut C {
        self.child.child_mut()
    }

    pub fn into_child(self) -> C {
        self.child.into_child()
    }

    pub fn offset(&self) -> ScrollOffset {
        self.scroll.offset()
    }

    pub fn target_offset(&self) -> ScrollOffset {
        self.scroll.target_offset()
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn scroll_by(&mut self, delta: ScrollDelta, settings: AnimationSettings) -> bool {
        let outcome = self.scroll.scroll_by(
            delta,
            self.geometry.viewport,
            self.geometry.content,
            settings,
        );
        self.apply_scroll(outcome)
    }

    pub fn scroll_to(&mut self, offset: ScrollOffset, settings: AnimationSettings) -> bool {
        let outcome = self.scroll.scroll_to(
            offset,
            self.geometry.viewport,
            self.geometry.content,
            settings,
        );
        self.apply_scroll(outcome)
    }

    pub fn scroll_geometry(&self) -> crate::ScrollGeometry {
        self.geometry
    }

    fn inner_area(&self, area: Rect) -> Rect {
        Rect::new(
            area.x.saturating_add(self.padding.left),
            area.y.saturating_add(self.padding.top),
            area.width
                .saturating_sub(self.padding.left.saturating_add(self.padding.right)),
            area.height
                .saturating_sub(self.padding.top.saturating_add(self.padding.bottom)),
        )
    }

    fn content_size(&self, viewport: ScrollSize) -> ScrollSize {
        let proposal = LayoutProposal {
            width: if self.scroll_axes().horizontal() {
                AxisProposal::Unbounded
            } else {
                AxisProposal::AtMost(viewport.width.min(u16::MAX as usize) as u16)
            },
            height: if self.scroll_axes().vertical() {
                AxisProposal::Unbounded
            } else {
                AxisProposal::AtMost(viewport.height.min(u16::MAX as usize) as u16)
            },
        };
        let hint = self.child.measure(proposal);
        ScrollSize::new(
            if self.scroll_axes().horizontal() {
                usize::from(hint.preferred.width).max(viewport.width)
            } else {
                viewport.width
            },
            if self.scroll_axes().vertical() {
                usize::from(hint.preferred.height).max(viewport.height)
            } else {
                viewport.height
            },
        )
    }

    fn resolve_geometry(&self, area: Rect) -> crate::ScrollGeometry {
        let mut content = self.content_size(ScrollSize::from_area(area));
        let mut geometry = self.scroll.geometry(area, content);
        for _ in 0..3 {
            let next = self.content_size(geometry.viewport);
            if next == content {
                break;
            }
            content = next;
            geometry = self.scroll.geometry(area, content);
        }
        geometry
    }

    fn scroll_axes(&self) -> ScrollAxes {
        self.axes
    }

    fn apply_scroll(&mut self, outcome: crate::ScrollOutcome) -> bool {
        outcome.changed || outcome.active
    }

    fn reveal(
        &mut self,
        target: Rect,
        settings: AnimationSettings,
        alignment: RevealAlignment,
    ) -> bool {
        let viewport = self.geometry.viewport;
        let content = self.geometry.content;
        let current = self.scroll.target_offset();
        let target_x = reveal_axis(
            current.x,
            viewport.width,
            usize::from(target.x),
            usize::from(target.width),
            alignment,
        );
        let target_y = reveal_axis(
            current.y,
            viewport.height,
            usize::from(target.y),
            usize::from(target.height),
            alignment,
        );
        let outcome = self.scroll.scroll_to(
            ScrollOffset::new(target_x, target_y),
            viewport,
            content,
            settings,
        );
        self.apply_scroll(outcome)
    }

    fn handle_scroll_event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        let outcome = match event {
            TuiEvent::Key(key) => self.scroll.on_key(
                *key,
                self.geometry.viewport,
                self.geometry.content,
                ctx.animation(),
            ),
            TuiEvent::Mouse(mouse) => match mouse.kind {
                crate::MouseEventKind::ScrollUp => self.scroll.scroll_by_immediately(
                    ScrollDelta::new(0, -1),
                    self.geometry.viewport,
                    self.geometry.content,
                ),
                crate::MouseEventKind::ScrollDown => self.scroll.scroll_by_immediately(
                    ScrollDelta::new(0, 1),
                    self.geometry.viewport,
                    self.geometry.content,
                ),
                crate::MouseEventKind::ScrollLeft => self.scroll.scroll_by_immediately(
                    ScrollDelta::new(-1, 0),
                    self.geometry.viewport,
                    self.geometry.content,
                ),
                crate::MouseEventKind::ScrollRight => self.scroll.scroll_by_immediately(
                    ScrollDelta::new(1, 0),
                    self.geometry.viewport,
                    self.geometry.content,
                ),
                _ => crate::ScrollOutcome::idle(),
            },
            _ => crate::ScrollOutcome::idle(),
        };
        if !outcome.changed && !outcome.active {
            return EventOutcome::Ignored;
        }
        ctx.request_redraw();
        if outcome.active {
            ctx.request_tick();
        }
        ctx.request_layout();
        ctx.stop_propagation();
        EventOutcome::Handled
    }

    fn rebased_event(&self, event: &TuiEvent) -> TuiEvent {
        let TuiEvent::Mouse(mouse) = event else {
            return event.clone();
        };
        let offset = self.scroll.offset();
        TuiEvent::Mouse(crate::MouseEvent {
            column: mouse
                .column
                .saturating_sub(self.geometry.layout.viewport.x)
                .saturating_add(offset.x.min(u16::MAX as usize) as u16),
            row: mouse
                .row
                .saturating_sub(self.geometry.layout.viewport.y)
                .saturating_add(offset.y.min(u16::MAX as usize) as u16),
            ..*mouse
        })
    }
}

impl<C, M> TuiNode<M> for ScrollContainer<C, M>
where
    C: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let child = self.child.measure(LayoutProposal {
            width: inset_axis_proposal(proposal.width, self.padding.left, self.padding.right),
            height: inset_axis_proposal(proposal.height, self.padding.top, self.padding.bottom),
        });
        let horizontal_padding = self.padding.left.saturating_add(self.padding.right);
        let vertical_padding = self.padding.top.saturating_add(self.padding.bottom);
        LayoutSizeHint {
            source: if child.source == HintSource::LegacyUnmeasured {
                HintSource::LegacyUnmeasured
            } else {
                HintSource::Measured
            },
            min: crate::LayoutSize::new(
                child.min.width.saturating_add(horizontal_padding),
                child.min.height.saturating_add(vertical_padding),
            ),
            preferred: crate::LayoutSize::new(
                child.preferred.width.saturating_add(horizontal_padding),
                child.preferred.height.saturating_add(vertical_padding),
            ),
            expand: AxisExpand {
                width: true,
                height: true,
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut crate::LayoutCtx) -> LayoutResult {
        let inner = self.inner_area(area);
        self.geometry = self.resolve_geometry(inner);
        ctx.register_hit_region(crate::HitRegion::new(
            ctx.current_path(),
            self.geometry.layout.outer,
        ));
        let _ = self.scroll.clamp_to(
            self.geometry.viewport,
            self.geometry.content,
            animation_settings(),
        );
        self.content_area = Rect::new(
            0,
            0,
            self.geometry.content.width.min(u16::MAX as usize) as u16,
            self.geometry.content.height.min(u16::MAX as usize) as u16,
        );

        let focus_start = ctx.focus_target_count();
        let hit_start = ctx.hit_region_count();
        let overlay_start = ctx.overlay_count();
        self.child.layout(self.content_area, ctx);
        self.focus_areas = ctx.focus_targets()[focus_start..].to_vec();
        if let Some(path) = self.pending_reveal_path.take()
            && let Some(area) = self
                .focus_areas
                .iter()
                .find(|target| target.path == path)
                .map(|target| target.area)
        {
            self.reveal(area, animation_settings(), RevealAlignment::Nearest);
        }
        let offset = self.scroll.offset();
        let x_offset = i32::from(self.geometry.layout.viewport.x) - offset.x as i32;
        let y_offset = i32::from(self.geometry.layout.viewport.y) - offset.y as i32;
        ctx.translate_focus_targets_from(
            focus_start,
            x_offset,
            y_offset,
            self.geometry.layout.viewport,
        );
        ctx.translate_hit_regions_from(
            hit_start,
            x_offset,
            y_offset,
            self.geometry.layout.viewport,
        );
        ctx.translate_overlays_from(overlay_start, x_offset, y_offset);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        let viewport = self.geometry.layout.viewport;
        if !viewport.is_empty() && !self.content_area.is_empty() {
            let mut terminal = Terminal::new(TestBackend::new(
                self.content_area.width,
                self.content_area.height,
            ))
            .expect("scroll container offscreen terminal should initialize");
            let offset = self.scroll.offset();
            let x_offset = i32::from(viewport.x) - offset.x as i32;
            let y_offset = i32::from(viewport.y) - offset.y as i32;
            terminal
                .draw(|child_frame| {
                    ctx.with_portal_offset(x_offset, y_offset, |ctx| {
                        self.child.render(child_frame, self.content_area, ctx)
                    })
                })
                .expect("scroll container offscreen render should succeed");
            if terminal.backend().cursor_visible() {
                let position = terminal.backend().cursor_position();
                let visible_x = usize::from(position.x)
                    .checked_sub(offset.x)
                    .filter(|x| *x < usize::from(viewport.width));
                let visible_y = usize::from(position.y)
                    .checked_sub(offset.y)
                    .filter(|y| *y < usize::from(viewport.height));
                if let (Some(x), Some(y)) = (visible_x, visible_y) {
                    frame.set_cursor_position((viewport.x + x as u16, viewport.y + y as u16));
                }
            }
            let source = terminal.backend().buffer();
            let destination = frame.buffer_mut();
            for row in 0..viewport.height {
                let source_y = offset.y.saturating_add(usize::from(row));
                if source_y >= usize::from(self.content_area.height) {
                    break;
                }
                for column in 0..viewport.width {
                    let source_x = offset.x.saturating_add(usize::from(column));
                    if source_x >= usize::from(self.content_area.width) {
                        break;
                    }
                    if let Some(cell) = source.cell((source_x as u16, source_y as u16)) {
                        destination[(viewport.x + column, viewport.y + row)] = cell.clone();
                    }
                }
            }
        }
        self.scroll.render_scrollbars(
            frame,
            self.geometry.layout,
            self.geometry.content,
            self.focused,
        );
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let child_event = self.rebased_event(event);
        let child = self.child.dispatch_event(route, &child_event, ctx);
        if let Some((area, alignment)) = ctx.take_reveal_request() {
            let mut settings = ctx.animation();
            if alignment == RevealAlignment::Center {
                settings.enabled = false;
            }
            if self.reveal(area, settings, alignment) {
                ctx.request_redraw();
                if alignment != RevealAlignment::Center {
                    ctx.request_tick();
                    ctx.request_layout();
                }
            }
        }
        if ctx.layout_requested() {
            self.pending_reveal_path = Some(route.path.clone());
        }
        child.bubble(ctx, |ctx| self.handle_scroll_event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let scroll = self.scroll.tick(dt, settings);
        let mut result = self.child.tick(dt, settings).merge(scroll);
        if scroll.changed {
            result.layout = true;
        }
        result
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.focused = focused;
        self.child.dispatch_focus(target, focused, ctx);
        if focused && self.focus_reveal {
            let child_reveal_area = self.child.focus_reveal_area(target);
            let alignment =
                if child_reveal_area.is_some() && self.child.focus_reveal_centered(target) {
                    RevealAlignment::Center
                } else {
                    RevealAlignment::Nearest
                };
            if let Some(area) = child_reveal_area.or_else(|| {
                self.focus_areas
                    .iter()
                    .find(|area| {
                        area.id == target.id && area.path.strip_suffix(&target.path).is_some()
                    })
                    .map(|area| area.area)
            }) && self.reveal(area, ctx.animation(), alignment)
            {
                ctx.request_layout();
            }
        }
        ctx.request_redraw();
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.child.destroy(ctx);
    }
}

fn inset_axis_proposal(proposal: AxisProposal, start: u16, end: u16) -> AxisProposal {
    let inset = start.saturating_add(end);
    match proposal {
        AxisProposal::Unbounded => AxisProposal::Unbounded,
        AxisProposal::AtMost(size) => AxisProposal::AtMost(size.saturating_sub(inset)),
        AxisProposal::Exact(size) => AxisProposal::Exact(size.saturating_sub(inset)),
    }
}

fn reveal_axis(
    offset: usize,
    viewport: usize,
    start: usize,
    size: usize,
    alignment: RevealAlignment,
) -> usize {
    if alignment == RevealAlignment::Center && size < viewport {
        return start.saturating_add(size / 2).saturating_sub(viewport / 2);
    }
    let end = start.saturating_add(size);
    let visible_end = offset.saturating_add(viewport);
    if start < offset {
        start
    } else if end > visible_end {
        end.saturating_sub(viewport)
    } else {
        offset
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, text::Line, widgets::Paragraph};

    use super::*;
    use crate::{
        ChildKey, ChildSlot, DataView, EventCtx, EventRoute, FocusCtx, FocusId, Key, KeyEvent,
        LayoutCtx, RenderCtx, ScrollbarGutter, ScrollbarStyle, ScrollbarVisibility, TreePath,
    };

    struct Lines(Vec<&'static str>);

    impl TuiNode<()> for Lines {
        fn measure(&self, _proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint::content(1, self.0.len().min(u16::MAX as usize) as u16)
        }

        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render<'a>(&'a self, frame: &mut Frame, area: Rect, _ctx: &mut RenderCtx<'a>) {
            frame.render_widget(
                Paragraph::new(self.0.iter().copied().map(Line::from).collect::<Vec<_>>()),
                area,
            );
        }
    }

    struct FocusableLines(Lines);

    impl TuiNode<()> for FocusableLines {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            self.0.measure(proposal)
        }

        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(
                FocusId::new("last-row"),
                Rect::new(
                    area.x,
                    area.y.saturating_add(area.height.saturating_sub(1)),
                    area.width,
                    1,
                ),
                true,
            );
            LayoutResult::new(area)
        }

        fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
            self.0.render(frame, area, ctx);
        }
    }

    #[test]
    fn renders_only_visible_child_rows_after_scrolling() {
        let mut node = ScrollContainer::vertical(Lines(vec!["one", "two", "three", "four"]));
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 2), &mut layout);
        let settings = AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        };
        assert!(node.scroll_by(ScrollDelta::new(0, 2), settings));

        let mut terminal = Terminal::new(TestBackend::new(8, 2)).unwrap();
        terminal
            .draw(|frame| {
                let mut render = RenderCtx::new();
                node.render(frame, Rect::new(0, 0, 8, 2), &mut render);
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), "t");
        assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), "f");
    }

    #[test]
    fn measure_includes_padding_without_expanding_the_child_proposal() {
        let node = ScrollContainer::vertical(Lines(vec!["one", "two"]))
            .padding(Padding::horizontal_vertical(2, 1));

        let hint = node.measure(LayoutProposal::at_most(10, 10));

        assert_eq!(hint.min, crate::LayoutSize::new(4, 2));
        assert_eq!(hint.preferred, crate::LayoutSize::new(5, 4));
    }

    #[test]
    fn repeated_wheel_input_snaps_the_container_without_animation() {
        let mut node = ScrollContainer::vertical(Lines(vec![
            "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        ]));
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 2), &mut layout);
        let settings = AnimationSettings::default();
        let route = EventRoute::new(TreePath::new().child(ChildKey::body()));

        for wheel in 0..3 {
            let mut ctx = EventCtx::new(settings);
            let outcome = node.dispatch_event(
                &route,
                &TuiEvent::Mouse(crate::MouseEvent {
                    kind: crate::MouseEventKind::ScrollDown,
                    column: 0,
                    row: 0,
                    modifiers: crate::KeyModifiers::NONE,
                }),
                &mut ctx,
            );

            assert_eq!(outcome, EventOutcome::Handled, "wheel {wheel}");
            assert!(ctx.layout_requested());
            assert!(!ctx.tick_requested());
        }
        assert_eq!(node.offset().y, 3);
        assert_eq!(node.target_offset().y, 3);
        assert!(!node.scroll.is_active());
    }

    #[test]
    fn wheel_scrolls_from_the_vertical_scrollbar_gutter() {
        let mut node = ScrollContainer::vertical(Lines(vec![
            "one", "two", "three", "four", "five", "six", "seven", "eight",
        ]))
        .scrollbars(ScrollbarConfig {
            vertical: ScrollbarVisibility::Always,
            horizontal: ScrollbarVisibility::Never,
            gutter: ScrollbarGutter::Reserve,
            style: ScrollbarStyle::ThinTrack,
        });
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 3), &mut layout);
        let geometry = node.scroll_geometry();
        let gutter = geometry.layout.vertical_bar.unwrap();
        let hit_region = &layout.hit_regions()[0];

        assert_eq!(hit_region.area, geometry.layout.outer);
        assert!(hit_region.contains(gutter.x, gutter.y));

        let outcome = node.dispatch_event(
            &EventRoute::new(hit_region.path.clone()),
            &TuiEvent::Mouse(crate::MouseEvent {
                kind: crate::MouseEventKind::ScrollDown,
                column: gutter.x,
                row: gutter.y,
                modifiers: crate::KeyModifiers::NONE,
            }),
            &mut EventCtx::default(),
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(node.offset().y, 1);
    }

    #[test]
    fn focused_descendant_is_revealed() {
        let mut node =
            ScrollContainer::vertical(FocusableLines(Lines(vec!["one", "two", "three", "four"])));
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 2), &mut layout);
        let target = layout.focus_targets()[0].clone();
        let settings = AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        };
        let mut focus = FocusCtx::new(settings);

        node.dispatch_focus(&target, true, &mut focus);

        assert_eq!(node.offset().y, 2);
        assert!(focus.layout_requested());
    }

    #[test]
    fn focus_reveal_uses_the_container_relative_path_when_nested() {
        let mut outer = ChildSlot::new(
            "outer",
            ScrollContainer::vertical(FocusableLines(Lines(vec!["one", "two", "three", "four"]))),
        );
        let mut layout = LayoutCtx::new();
        outer.layout(Rect::new(0, 0, 8, 2), &mut layout);
        let target = layout.focus_targets()[0].clone();
        let settings = AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        };
        let mut focus = FocusCtx::new(settings);

        outer.dispatch_focus(&target, true, &mut focus);

        assert_eq!(outer.child().offset().y, 2);
        assert!(focus.layout_requested());
    }

    #[test]
    fn delegated_data_view_boundary_key_scrolls_the_outer_container() {
        let mut node = ScrollContainer::<_, ()>::vertical(
            DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll(),
        );
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 3), &mut layout);
        let settings = AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        };
        let target = layout.focus_targets()[0].clone();
        node.dispatch_focus(&target, true, &mut FocusCtx::new(settings));
        node.scroll_to(ScrollOffset::new(0, 17), settings);
        let mut event = EventCtx::new(settings);
        let route = EventRoute::new(TreePath::new().child(ChildKey::body()));

        let outcome =
            node.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Up)), &mut event);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(node.offset().y, 16);
    }

    #[test]
    fn tab_focus_reveals_the_delegated_data_view_highlight() {
        let mut node = ScrollContainer::<_, ()>::vertical(
            DataView::list(0..20, |id| *id, |id| id.to_string()).parent_vertical_scroll(),
        );
        let area = Rect::new(0, 0, 8, 3);
        let settings = AnimationSettings {
            enabled: false,
            ..AnimationSettings::default()
        };
        let mut layout = LayoutCtx::new();
        node.layout(area, &mut layout);
        let target = layout.focus_targets()[0].clone();
        node.child_mut().highlight_id(&10);
        let mut focus = FocusCtx::new(settings);

        node.dispatch_focus(&target, true, &mut focus);

        assert_eq!(node.offset().y, 9);
        assert!(focus.layout_requested());
    }

    #[test]
    fn delegated_navigation_immediately_centers_the_highlight() {
        let mut node = ScrollContainer::<_, ()>::vertical(
            DataView::list(0..20, |id| *id, |id| id.to_string())
                .focused(true)
                .parent_vertical_scroll(),
        );
        node.child_mut().highlight_id(&10);
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 3), &mut layout);
        let mut event = EventCtx::new(AnimationSettings::default());
        let route = EventRoute::new(TreePath::new().child(ChildKey::body()));

        let outcome = node.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::Char('j'))),
            &mut event,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(node.offset().y, 10);
        assert!(!event.layout_requested());
    }

    #[test]
    fn delegated_page_navigation_keeps_outer_scroll_animation() {
        let mut node = ScrollContainer::<_, ()>::vertical(
            DataView::list(0..20, |id| *id, |id| id.to_string())
                .focused(true)
                .parent_vertical_scroll(),
        );
        node.child_mut().highlight_id(&10);
        let mut layout = LayoutCtx::new();
        node.layout(Rect::new(0, 0, 8, 3), &mut layout);
        let mut event = EventCtx::new(AnimationSettings::default());
        let route = EventRoute::new(TreePath::new().child(ChildKey::body()));

        let outcome = node.dispatch_event(
            &route,
            &TuiEvent::Key(KeyEvent::from(Key::PageDown)),
            &mut event,
        );

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(node.offset().y, 0);
        assert_eq!(node.target_offset().y, 17);
    }
}
