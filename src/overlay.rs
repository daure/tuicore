use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use ratatui::{Frame, layout::Rect};

use crate::node::TreePath;
use crate::runtime::renderer::{
    DirectKittyIntent, DirectKittyResident, DirectKittySourceRect, GraphicsFrame, GraphicsLevel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OverlayId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum OverlayLayer {
    #[default]
    Popup,
    Popover,
    Modal,
    Tooltip,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutsideMousePolicy {
    #[default]
    PassThrough,
    Dismiss,
    Capture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OverlayPolicy {
    pub outside_mouse: OutsideMousePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlaySpec {
    pub id: OverlayId,
    pub owner_path: Option<TreePath>,
    pub route_path: Option<TreePath>,
    pub anchor: Rect,
    pub area: Rect,
    pub bounds: Option<Rect>,
    pub layer: OverlayLayer,
    pub z_index: i32,
    pub policy: OverlayPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayoutEntry {
    pub id: OverlayId,
    pub owner_path: TreePath,
    pub route_path: TreePath,
    pub anchor: Rect,
    pub area: Rect,
    pub bounds: Rect,
    pub layer: OverlayLayer,
    pub z_index: i32,
    pub order: u64,
    pub policy: OverlayPolicy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OverlayManager {
    entries: Vec<OverlayLayoutEntry>,
    next_order: u64,
}

pub struct RenderCtx<'a> {
    portals: Vec<PortalTask<'a>>,
    next_order: u64,
    overlays_disabled: bool,
    portal_offset: (i32, i32),
    graphics_residents: Vec<DirectKittyResident>,
    graphics_intents: Vec<DirectKittyIntent>,
    graphics_level: GraphicsLevel,
    graphics_offset: (i32, i32),
    graphics_clip: Option<SignedRect>,
    direct_kitty_suppression_depth: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SignedRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

struct PortalTask<'a> {
    layer: OverlayLayer,
    z_index: i32,
    order: u64,
    area: Rect,
    direct_kitty_suppressed: bool,
    render: PortalRender<'a>,
}

type PortalRenderFn<'a> = dyn FnOnce(&mut Frame<'_>, Rect) + 'a;
type PortalRenderWithCtxFn<'a> = dyn FnOnce(&mut Frame<'_>, Rect, &mut RenderCtx<'a>) + 'a;

enum PortalRender<'a> {
    Simple(Box<PortalRenderFn<'a>>),
    WithCtx(Box<PortalRenderWithCtxFn<'a>>),
}

impl OverlayId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn for_path(namespace: u64, path: &TreePath) -> Self {
        let mut hasher = DefaultHasher::new();
        namespace.hash(&mut hasher);
        path.hash(&mut hasher);
        Self(hasher.finish())
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for OverlayId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl OverlaySpec {
    pub fn new(id: impl Into<OverlayId>, anchor: Rect, area: Rect) -> Self {
        Self {
            id: id.into(),
            owner_path: None,
            route_path: None,
            anchor,
            area,
            bounds: None,
            layer: OverlayLayer::default(),
            z_index: 0,
            policy: OverlayPolicy::default(),
        }
    }
}

impl OverlayManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        spec: OverlaySpec,
        current_path: TreePath,
        default_bounds: Rect,
    ) -> OverlayLayoutEntry {
        let entry = OverlayLayoutEntry {
            id: spec.id,
            owner_path: spec.owner_path.unwrap_or_else(|| current_path.clone()),
            route_path: spec.route_path.unwrap_or(current_path),
            anchor: spec.anchor,
            area: spec.area,
            bounds: spec.bounds.unwrap_or(default_bounds),
            layer: spec.layer,
            z_index: spec.z_index,
            order: self.next_order,
            policy: spec.policy,
        };
        self.next_order += 1;
        self.entries.push(entry.clone());
        entry
    }

    pub fn entries(&self) -> &[OverlayLayoutEntry] {
        &self.entries
    }

    pub(crate) fn translate_entries_from(&mut self, start: usize, x_offset: i32, y_offset: i32) {
        for entry in &mut self.entries[start..] {
            entry.anchor = translate_rect(entry.anchor, x_offset, y_offset);
            entry.area = translate_rect(entry.area, x_offset, y_offset);
        }
    }

    pub fn sorted_entries(&self) -> Vec<OverlayLayoutEntry> {
        let mut entries = self.entries.clone();
        sort_overlay_entries(&mut entries);
        entries
    }

    pub fn drain_sorted(&mut self) -> Vec<OverlayLayoutEntry> {
        let mut entries = std::mem::take(&mut self.entries);
        sort_overlay_entries(&mut entries);
        entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for RenderCtx<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> RenderCtx<'a> {
    pub fn new() -> Self {
        Self {
            portals: Vec::new(),
            next_order: 0,
            overlays_disabled: false,
            portal_offset: (0, 0),
            graphics_residents: Vec::new(),
            graphics_intents: Vec::new(),
            graphics_level: GraphicsLevel::base(),
            graphics_offset: (0, 0),
            graphics_clip: None,
            direct_kitty_suppression_depth: 0,
        }
    }

    pub fn push_portal(
        &mut self,
        layer: OverlayLayer,
        z_index: i32,
        area: Rect,
        render: impl FnOnce(&mut Frame<'_>, Rect) + 'a,
    ) {
        self.push_portal_task(layer, z_index, area, PortalRender::Simple(Box::new(render)));
    }

    pub fn push_portal_with_ctx(
        &mut self,
        layer: OverlayLayer,
        z_index: i32,
        area: Rect,
        render: impl FnOnce(&mut Frame<'_>, Rect, &mut RenderCtx<'a>) + 'a,
    ) {
        self.push_portal_task(
            layer,
            z_index,
            area,
            PortalRender::WithCtx(Box::new(render)),
        );
    }

    fn push_portal_task(
        &mut self,
        layer: OverlayLayer,
        z_index: i32,
        area: Rect,
        render: PortalRender<'a>,
    ) -> GraphicsLevel {
        if self.overlays_disabled {
            return self.graphics_level;
        }
        let order = self.next_order;
        let level = GraphicsLevel::new(layer, z_index, order);
        let task = PortalTask {
            layer,
            z_index,
            order,
            area: translate_rect(area, self.portal_offset.0, self.portal_offset.1),
            direct_kitty_suppressed: self.direct_kitty_suppression_depth > 0,
            render,
        };
        self.next_order += 1;
        self.portals.push(task);
        level
    }

    pub fn with_overlays_disabled<R>(&mut self, render: impl FnOnce(&mut Self) -> R) -> R {
        let was_disabled = self.overlays_disabled;
        self.overlays_disabled = true;
        let result = render(self);
        self.overlays_disabled = was_disabled;
        result
    }

    pub fn with_direct_kitty_suppressed<R>(&mut self, render: impl FnOnce(&mut Self) -> R) -> R {
        let previous = self.direct_kitty_suppression_depth;
        self.direct_kitty_suppression_depth = previous.saturating_add(1);
        let result = render(self);
        self.direct_kitty_suppression_depth = previous;
        result
    }

    pub(crate) fn with_portal_offset<R>(
        &mut self,
        x_offset: i32,
        y_offset: i32,
        render: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous = self.portal_offset;
        self.portal_offset = (
            previous.0.saturating_add(x_offset),
            previous.1.saturating_add(y_offset),
        );
        let result = render(self);
        self.portal_offset = previous;
        result
    }

    pub(crate) fn translate_portal_rect(&self, area: Rect) -> Rect {
        translate_rect(area, self.portal_offset.0, self.portal_offset.1)
    }

    pub(crate) fn with_direct_kitty_viewport<R>(
        &mut self,
        x_offset: i32,
        y_offset: i32,
        viewport: Rect,
        render: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous_offset = self.graphics_offset;
        let previous_clip = self.graphics_clip;
        let viewport_clip = SignedRect::from(viewport).translate(previous_offset);
        self.graphics_offset = (
            previous_offset.0.saturating_add(x_offset),
            previous_offset.1.saturating_add(y_offset),
        );
        self.graphics_clip = Some(match previous_clip {
            Some(clip) => clip.intersection(viewport_clip).unwrap_or_default(),
            None => viewport_clip,
        });
        let result = render(self);
        self.graphics_offset = previous_offset;
        self.graphics_clip = previous_clip;
        result
    }

    pub fn flush(&mut self, frame: &mut Frame<'_>) {
        while !self.portals.is_empty() {
            self.portals
                .sort_by_key(|portal| (portal.layer, portal.z_index, portal.order));
            let portal = self.portals.remove(0);
            let previous_level = self.graphics_level;
            let previous_suppression_depth = self.direct_kitty_suppression_depth;
            self.graphics_level = GraphicsLevel::new(portal.layer, portal.z_index, portal.order);
            if portal.direct_kitty_suppressed {
                self.direct_kitty_suppression_depth =
                    self.direct_kitty_suppression_depth.saturating_add(1);
            }
            match portal.render {
                PortalRender::Simple(render) => render(frame, portal.area),
                PortalRender::WithCtx(render) => render(frame, portal.area, self),
            }
            self.direct_kitty_suppression_depth = previous_suppression_depth;
            self.graphics_level = previous_level;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.portals.is_empty()
    }

    pub(crate) fn register_direct_kitty(&mut self, mut intent: DirectKittyIntent) {
        if self.direct_kitty_suppression_depth > 0 {
            return;
        }
        self.graphics_residents.push(DirectKittyResident {
            image_id: intent.id.image_id,
            payload: Arc::clone(&intent.payload),
        });
        let Some(transformed) =
            transform_direct_kitty_intent(intent, self.graphics_offset, self.graphics_clip)
        else {
            return;
        };
        intent = transformed;
        intent.level = self.graphics_level;
        intent.z_index = self.graphics_level.kitty_image_z_index();
        self.graphics_intents.push(intent);
    }

    pub(crate) fn take_graphics_frame(&mut self) -> GraphicsFrame {
        GraphicsFrame {
            residents: std::mem::take(&mut self.graphics_residents),
            intents: std::mem::take(&mut self.graphics_intents),
        }
    }
}

impl From<Rect> for SignedRect {
    fn from(area: Rect) -> Self {
        Self {
            left: i32::from(area.x),
            top: i32::from(area.y),
            right: i32::from(area.x) + i32::from(area.width),
            bottom: i32::from(area.y) + i32::from(area.height),
        }
    }
}

impl SignedRect {
    fn translate(self, offset: (i32, i32)) -> Self {
        Self {
            left: self.left.saturating_add(offset.0),
            top: self.top.saturating_add(offset.1),
            right: self.right.saturating_add(offset.0),
            bottom: self.bottom.saturating_add(offset.1),
        }
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let intersection = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (intersection.left < intersection.right && intersection.top < intersection.bottom)
            .then_some(intersection)
    }

    fn to_rect(self) -> Option<Rect> {
        Some(Rect::new(
            u16::try_from(self.left).ok()?,
            u16::try_from(self.top).ok()?,
            u16::try_from(self.right - self.left).ok()?,
            u16::try_from(self.bottom - self.top).ok()?,
        ))
    }
}

fn transform_direct_kitty_intent(
    mut intent: DirectKittyIntent,
    offset: (i32, i32),
    clip: Option<SignedRect>,
) -> Option<DirectKittyIntent> {
    let translated = SignedRect::from(intent.area).translate(offset);
    if translated.left >= translated.right || translated.top >= translated.bottom {
        return None;
    }
    let visible = match clip {
        Some(clip) => translated.intersection(clip)?,
        None => translated,
    };
    intent.source_rect = crop_source_rect(intent.source_rect, translated, visible);
    intent.area = visible.to_rect()?;
    Some(intent)
}

fn crop_source_rect(
    source: DirectKittySourceRect,
    destination: SignedRect,
    visible: SignedRect,
) -> DirectKittySourceRect {
    let destination_width = (destination.right - destination.left) as u32;
    let destination_height = (destination.bottom - destination.top) as u32;
    let visible_left = (visible.left - destination.left) as u32;
    let visible_top = (visible.top - destination.top) as u32;
    let visible_right = (visible.right - destination.left) as u32;
    let visible_bottom = (visible.bottom - destination.top) as u32;
    let left = proportional_floor(visible_left, source.width, destination_width);
    let top = proportional_floor(visible_top, source.height, destination_height);
    let right = proportional_ceil(visible_right, source.width, destination_width);
    let bottom = proportional_ceil(visible_bottom, source.height, destination_height);
    DirectKittySourceRect {
        x: source.x.saturating_add(left),
        y: source.y.saturating_add(top),
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

fn proportional_floor(value: u32, source: u32, destination: u32) -> u32 {
    ((u64::from(value) * u64::from(source)) / u64::from(destination)) as u32
}

fn proportional_ceil(value: u32, source: u32, destination: u32) -> u32 {
    let product = u64::from(value) * u64::from(source);
    product.div_ceil(u64::from(destination)) as u32
}

pub(crate) fn translate_rect(area: Rect, x_offset: i32, y_offset: i32) -> Rect {
    let x = (i32::from(area.x) + x_offset).clamp(0, i32::from(u16::MAX)) as u16;
    let y = (i32::from(area.y) + y_offset).clamp(0, i32::from(u16::MAX)) as u16;
    Rect::new(x, y, area.width, area.height)
}

fn sort_overlay_entries(entries: &mut [OverlayLayoutEntry]) {
    entries.sort_by_key(|entry| (entry.layer, entry.z_index, entry.order));
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::runtime::renderer::DirectKittyPlacementId;

    #[test]
    fn overlay_manager_sorts_by_layer_z_index_then_order() {
        let mut manager = OverlayManager::new();
        let bounds = Rect::new(0, 0, 80, 24);

        manager.register(spec(1, OverlayLayer::Modal, 0), TreePath::new(), bounds);
        manager.register(spec(2, OverlayLayer::Popup, 10), TreePath::new(), bounds);
        manager.register(spec(3, OverlayLayer::Popup, 5), TreePath::new(), bounds);
        manager.register(spec(4, OverlayLayer::Popup, 5), TreePath::new(), bounds);
        manager.register(spec(5, OverlayLayer::System, -1), TreePath::new(), bounds);

        let ids = manager
            .sorted_entries()
            .into_iter()
            .map(|entry| entry.id.get())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec![3, 4, 2, 1, 5]);
    }

    #[test]
    fn render_ctx_flushes_portals_by_layer_z_index_then_order() {
        let order = Rc::new(RefCell::new(Vec::new()));
        let mut ctx = RenderCtx::new();

        push_probe(&mut ctx, &order, 1, OverlayLayer::Tooltip, 0);
        push_probe(&mut ctx, &order, 2, OverlayLayer::Popup, 1);
        push_probe(&mut ctx, &order, 3, OverlayLayer::Popup, 0);
        push_probe(&mut ctx, &order, 4, OverlayLayer::Popup, 0);

        let mut terminal = Terminal::new(TestBackend::new(10, 5)).expect("terminal should build");
        terminal
            .draw(|frame| ctx.flush(frame))
            .expect("draw should flush portals");

        assert_eq!(*order.borrow(), vec![3, 4, 2, 1]);
        assert!(ctx.is_empty());
    }

    #[test]
    fn fully_clipped_direct_kitty_image_remains_resident() {
        let mut ctx = RenderCtx::new();
        let intent = DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id: 7,
                placement_id: 1,
            },
            area: Rect::new(0, 0, 4, 4),
            source_rect: DirectKittySourceRect {
                x: 0,
                y: 0,
                width: 40,
                height: 80,
            },
            generation: 0,
            payload: Arc::from("payload"),
            level: GraphicsLevel::base(),
            z_index: 0,
        };

        ctx.with_direct_kitty_viewport(0, -5, Rect::new(0, 0, 4, 4), |ctx| {
            ctx.register_direct_kitty(intent);
        });
        let frame = ctx.take_graphics_frame();

        assert!(frame.intents.is_empty());
        assert_eq!(frame.residents.len(), 1);
        assert_eq!(frame.residents[0].image_id, 7);
        assert_eq!(&*frame.residents[0].payload, "payload");
    }

    #[test]
    fn direct_kitty_suppression_nests_and_propagates_to_portals() {
        let mut ctx = RenderCtx::new();
        let intent = DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id: 7,
                placement_id: 1,
            },
            area: Rect::new(0, 0, 4, 4),
            source_rect: DirectKittySourceRect {
                x: 0,
                y: 0,
                width: 40,
                height: 80,
            },
            generation: 0,
            payload: Arc::from("payload"),
            level: GraphicsLevel::base(),
            z_index: 0,
        };
        let visible = intent.clone();

        ctx.with_direct_kitty_suppressed(|ctx| {
            ctx.with_direct_kitty_suppressed(|ctx| {
                ctx.push_portal_with_ctx(
                    OverlayLayer::Popup,
                    0,
                    Rect::default(),
                    move |_frame, _area, ctx| ctx.register_direct_kitty(intent),
                );
            });
        });
        ctx.register_direct_kitty(visible);
        let mut terminal = Terminal::new(TestBackend::new(10, 5)).expect("terminal should build");
        terminal
            .draw(|frame| ctx.flush(frame))
            .expect("draw should flush portals");
        let frame = ctx.take_graphics_frame();

        assert_eq!(frame.residents.len(), 1);
        assert_eq!(frame.intents.len(), 1);
        assert_eq!(frame.intents[0].id.image_id, 7);
    }

    fn spec(id: u64, layer: OverlayLayer, z_index: i32) -> OverlaySpec {
        let mut spec = OverlaySpec::new(id, Rect::default(), Rect::default());
        spec.layer = layer;
        spec.z_index = z_index;
        spec
    }

    fn push_probe<'a>(
        ctx: &mut RenderCtx<'a>,
        order: &Rc<RefCell<Vec<u64>>>,
        id: u64,
        layer: OverlayLayer,
        z_index: i32,
    ) {
        let order = Rc::clone(order);
        ctx.push_portal(layer, z_index, Rect::default(), move |_frame, _area| {
            order.borrow_mut().push(id);
        });
    }
}
