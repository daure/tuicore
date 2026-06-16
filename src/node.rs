use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use crate::animation::{AnimationSettings, TickResult};
use crate::event::TuiEvent;

pub trait TuiNode<M = ()> {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult;

    fn render(&self, frame: &mut Frame, area: Rect);

    fn event(&mut self, _event: &TuiEvent, _ctx: &mut EventCtx<M>) -> EventOutcome {
        EventOutcome::Ignored
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        if route.path.is_empty() {
            self.event(event, ctx)
        } else {
            EventOutcome::Ignored
        }
    }

    fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
        TickResult::IDLE
    }

    fn focus(&mut self, _target: Option<&FocusId>, _focused: bool, _ctx: &mut FocusCtx<M>) {}

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if target.path.is_empty() {
            self.focus(Some(&target.id), focused, ctx);
        }
    }

    fn init(&mut self, _ctx: &mut LifecycleCtx<M>) {}

    fn mount(&mut self, _ctx: &mut LifecycleCtx<M>) {}

    fn unmount(&mut self, _ctx: &mut LifecycleCtx<M>) {}

    fn destroy(&mut self, _ctx: &mut LifecycleCtx<M>) {}
}

impl<M, N> TuiNode<M> for Box<N>
where
    N: TuiNode<M> + ?Sized,
{
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.as_mut().layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.as_ref().render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.as_mut().event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.as_mut().dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.as_mut().tick(dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.as_mut().focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.as_mut().dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.as_mut().init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.as_mut().mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.as_mut().unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.as_mut().destroy(ctx);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCtx<M> {
    messages: Vec<M>,
    redraw: bool,
    layout: bool,
    quit: bool,
    focus_request: Option<FocusRequest>,
    focus_repair: Option<FocusRepair>,
    propagation: Propagation,
    animation: AnimationSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventOutcome {
    Ignored,
    Handled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Propagation {
    #[default]
    Continue,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LayoutCtx {
    focus_paths: Vec<FocusTarget>,
    hit_regions: Vec<HitRegion>,
    path: TreePath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutResult {
    pub area: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusCtx<M> {
    messages: Vec<M>,
    focus_request: Option<FocusRequest>,
    redraw: bool,
    animation: AnimationSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleCtx<M> {
    messages: Vec<M>,
    redraw: bool,
    layout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChildKey(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct TreePath(Vec<ChildKey>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusTarget {
    pub id: FocusId,
    pub path: TreePath,
    pub area: Rect,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRequest {
    Next,
    Previous,
    Target(FocusId),
    Path(TreePath),
    TargetAt { path: TreePath, id: FocusId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusRepair {
    RemovedChild { index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRoute {
    pub path: TreePath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitRegion {
    pub path: TreePath,
    pub area: Rect,
}

impl<M> Default for EventCtx<M> {
    fn default() -> Self {
        Self::new(AnimationSettings::default())
    }
}

impl<M> EventCtx<M> {
    pub fn new(animation: AnimationSettings) -> Self {
        Self {
            messages: Vec::new(),
            redraw: false,
            layout: false,
            quit: false,
            focus_request: None,
            focus_repair: None,
            propagation: Propagation::Continue,
            animation,
        }
    }

    pub fn emit(&mut self, msg: M) {
        self.messages.push(msg);
    }

    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn request_layout(&mut self) {
        self.layout = true;
    }

    pub fn request_quit(&mut self) {
        self.quit = true;
    }

    pub fn focus(&mut self, target: FocusRequest) {
        self.focus_request = Some(target);
    }

    pub fn focus_next(&mut self) {
        self.focus(FocusRequest::Next);
    }

    pub fn focus_previous(&mut self) {
        self.focus(FocusRequest::Previous);
    }

    pub(crate) fn repair_focus(&mut self, repair: FocusRepair) {
        self.focus_repair = Some(repair);
    }

    pub fn stop_propagation(&mut self) {
        self.propagation = Propagation::Stopped;
    }

    pub fn messages(&self) -> &[M] {
        &self.messages
    }

    pub fn drain_messages(&mut self) -> impl Iterator<Item = M> + '_ {
        self.messages.drain(..)
    }

    pub fn redraw_requested(&self) -> bool {
        self.redraw
    }

    pub fn layout_requested(&self) -> bool {
        self.layout
    }

    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    pub fn focus_request(&self) -> Option<&FocusRequest> {
        self.focus_request.as_ref()
    }

    pub(crate) fn focus_repair(&self) -> Option<FocusRepair> {
        self.focus_repair
    }

    pub fn propagation(&self) -> Propagation {
        self.propagation
    }

    pub fn animation(&self) -> AnimationSettings {
        self.animation
    }
}

impl EventOutcome {
    pub fn handled(self) -> bool {
        matches!(self, Self::Handled)
    }

    pub fn merge(self, other: Self) -> Self {
        if self.handled() || other.handled() {
            Self::Handled
        } else {
            Self::Ignored
        }
    }

    pub fn bubble<M>(
        self,
        ctx: &mut EventCtx<M>,
        parent: impl FnOnce(&mut EventCtx<M>) -> Self,
    ) -> Self {
        if ctx.propagation() == Propagation::Stopped {
            self
        } else {
            self.merge(parent(ctx))
        }
    }
}

impl LayoutCtx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_slot<R>(
        &mut self,
        key: ChildKey,
        area: Rect,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let path = self.path.child(key.clone());
        self.hit_regions.push(HitRegion::new(path, area));
        self.path.0.push(key);
        let result = layout(self);
        self.path.0.pop();
        result
    }

    pub fn register_focusable(&mut self, id: FocusId, area: Rect, enabled: bool) {
        self.focus_paths.push(FocusTarget {
            id,
            path: self.current_path(),
            area,
            enabled,
        });
    }

    pub fn register_hit_region(&mut self, region: HitRegion) {
        self.hit_regions.push(region);
    }

    pub fn current_path(&self) -> TreePath {
        self.path.clone()
    }

    pub fn focus_targets(&self) -> &[FocusTarget] {
        &self.focus_paths
    }

    pub fn hit_regions(&self) -> &[HitRegion] {
        &self.hit_regions
    }
}

impl LayoutResult {
    pub fn new(area: Rect) -> Self {
        Self { area }
    }
}

impl<M> FocusCtx<M> {
    pub fn new(animation: AnimationSettings) -> Self {
        Self {
            messages: Vec::new(),
            focus_request: None,
            redraw: false,
            animation,
        }
    }

    pub fn emit(&mut self, msg: M) {
        self.messages.push(msg);
    }

    pub fn focus(&mut self, target: FocusRequest) {
        self.focus_request = Some(target);
    }

    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn messages(&self) -> &[M] {
        &self.messages
    }

    pub fn drain_messages(&mut self) -> impl Iterator<Item = M> + '_ {
        self.messages.drain(..)
    }

    pub fn focus_request(&self) -> Option<&FocusRequest> {
        self.focus_request.as_ref()
    }

    pub fn redraw_requested(&self) -> bool {
        self.redraw
    }

    pub fn animation(&self) -> AnimationSettings {
        self.animation
    }
}

impl<M> Default for FocusCtx<M> {
    fn default() -> Self {
        Self::new(AnimationSettings::default())
    }
}

impl<M> LifecycleCtx<M> {
    pub fn emit(&mut self, msg: M) {
        self.messages.push(msg);
    }

    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn request_layout(&mut self) {
        self.layout = true;
    }

    pub fn messages(&self) -> &[M] {
        &self.messages
    }

    pub fn drain_messages(&mut self) -> impl Iterator<Item = M> + '_ {
        self.messages.drain(..)
    }

    pub fn redraw_requested(&self) -> bool {
        self.redraw
    }

    pub fn layout_requested(&self) -> bool {
        self.layout
    }
}

impl<M> Default for LifecycleCtx<M> {
    fn default() -> Self {
        Self {
            messages: Vec::new(),
            redraw: false,
            layout: false,
        }
    }
}

impl FocusId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FocusTarget {
    pub fn for_child(&self, key: &ChildKey) -> Option<Self> {
        self.path.without_first_if(key).map(|path| Self {
            id: self.id.clone(),
            path,
            area: self.area,
            enabled: self.enabled,
        })
    }
}

impl ChildKey {
    pub const BODY: &'static str = "body";
    pub const FIRST: &'static str = "first";
    pub const SECOND: &'static str = "second";

    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn body() -> Self {
        Self::new(Self::BODY)
    }

    pub fn first() -> Self {
        Self::new(Self::FIRST)
    }

    pub fn second() -> Self {
        Self::new(Self::SECOND)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_child_key_uses_reserved_name() {
        assert_eq!(ChildKey::body().as_str(), ChildKey::BODY);
    }
}

impl From<&str> for ChildKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for ChildKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&ChildKey> for ChildKey {
    fn from(value: &ChildKey) -> Self {
        value.clone()
    }
}

impl TreePath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_keys(keys: impl IntoIterator<Item = ChildKey>) -> Self {
        Self(keys.into_iter().collect())
    }

    pub fn keys(&self) -> &[ChildKey] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn first(&self) -> Option<&ChildKey> {
        self.0.first()
    }

    pub fn without_first(&self) -> Self {
        Self(self.0.iter().skip(1).cloned().collect())
    }

    pub fn without_first_if(&self, key: &ChildKey) -> Option<Self> {
        match self.first() {
            Some(first) if first == key => Some(self.without_first()),
            _ => None,
        }
    }

    pub fn child(&self, key: ChildKey) -> Self {
        let mut keys = self.0.clone();
        keys.push(key);
        Self(keys)
    }
}

impl EventRoute {
    pub fn new(path: TreePath) -> Self {
        Self { path }
    }
}

impl HitRegion {
    pub fn new(path: TreePath, area: Rect) -> Self {
        Self { path, area }
    }

    pub fn contains(&self, column: u16, row: u16) -> bool {
        column >= self.area.x
            && column < self.area.x.saturating_add(self.area.width)
            && row >= self.area.y
            && row < self.area.y.saturating_add(self.area.height)
    }
}
