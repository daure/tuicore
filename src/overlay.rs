use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ratatui::{Frame, layout::Rect};

use crate::node::TreePath;

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
}

struct PortalTask<'a> {
    layer: OverlayLayer,
    z_index: i32,
    order: u64,
    area: Rect,
    render: PortalRender<'a>,
}

enum PortalRender<'a> {
    Simple(Box<dyn FnOnce(&mut Frame<'_>, Rect) + 'a>),
    WithCtx(Box<dyn FnOnce(&mut Frame<'_>, Rect, &mut RenderCtx<'a>) + 'a>),
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
    ) {
        if self.overlays_disabled {
            return;
        }
        let task = PortalTask {
            layer,
            z_index,
            order: self.next_order,
            area,
            render,
        };
        self.next_order += 1;
        self.portals.push(task);
    }

    pub fn with_overlays_disabled<R>(&mut self, render: impl FnOnce(&mut Self) -> R) -> R {
        let was_disabled = self.overlays_disabled;
        self.overlays_disabled = true;
        let result = render(self);
        self.overlays_disabled = was_disabled;
        result
    }

    pub fn flush(&mut self, frame: &mut Frame<'_>) {
        while !self.portals.is_empty() {
            self.portals
                .sort_by_key(|portal| (portal.layer, portal.z_index, portal.order));
            let portal = self.portals.remove(0);
            match portal.render {
                PortalRender::Simple(render) => render(frame, portal.area),
                PortalRender::WithCtx(render) => render(frame, portal.area, self),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.portals.is_empty()
    }
}

fn sort_overlay_entries(entries: &mut [OverlayLayoutEntry]) {
    entries.sort_by_key(|entry| (entry.layer, entry.z_index, entry.order));
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

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
