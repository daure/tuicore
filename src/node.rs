use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use crate::animation::{AnimationSettings, TickResult};
use crate::event::{ExternalEditorRequest, KeyEvent, TuiEvent};

pub trait TuiNode<M = ()> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::legacy_fill().normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult;

    fn render(&self, frame: &mut Frame, area: Rect);

    fn render_overlay(&self, _frame: &mut Frame, _area: Rect) {}

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
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.as_ref().measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.as_mut().layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.as_ref().render(frame, area);
    }

    fn render_overlay(&self, frame: &mut Frame, area: Rect) {
        self.as_ref().render_overlay(frame, area);
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
    clear: bool,
    external_editor: Option<ExternalEditorRequest>,
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
    overflow_diagnostics: Vec<LayoutOverflowDiagnostic>,
    path: TreePath,
    focus_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutResult {
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisProposal {
    Unbounded,
    AtMost(u16),
    Exact(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutProposal {
    pub width: AxisProposal,
    pub height: AxisProposal,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LayoutSize {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AxisExpand {
    pub width: bool,
    pub height: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintSource {
    Measured,
    LegacyUnmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutSizeHint {
    pub source: HintSource,
    pub min: LayoutSize,
    pub preferred: LayoutSize,
    pub expand: AxisExpand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutAxis {
    Width,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicyName {
    Clip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutOverflowDiagnostic {
    pub path: TreePath,
    pub child_index: Option<usize>,
    pub axis: LayoutAxis,
    pub needed: u16,
    pub available: u16,
    pub policy: OverflowPolicyName,
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
    pub tab_stop: bool,
    pub hotkey: Option<KeyEvent>,
    pub hotkeys: Vec<KeyEvent>,
    pub hotkey_sequences: Vec<String>,
    pub suppress_global_hotkeys: bool,
    pub focused_events_before_global_hotkeys: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRequest {
    Next,
    Previous,
    Unfocus,
    FirstChild,
    FirstChildOf { path: TreePath, id: FocusId },
    Last,
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
            clear: false,
            external_editor: None,
        }
    }

    pub fn emit(&mut self, msg: M) {
        self.messages.push(msg);
    }

    pub fn request_redraw(&mut self) {
        self.redraw = true;
    }

    pub fn request_clear(&mut self) {
        self.clear = true;
        self.redraw = true;
    }

    pub fn request_external_editor(&mut self, value: impl Into<String>, line: usize, col: usize) {
        self.external_editor = Some(ExternalEditorRequest {
            value: value.into(),
            line,
            col,
        });
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

    pub fn unfocus(&mut self) {
        self.focus(FocusRequest::Unfocus);
    }

    pub fn repair_focus(&mut self, repair: FocusRepair) {
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

    pub fn clear_requested(&self) -> bool {
        self.clear
    }

    pub fn external_editor_request(&self) -> Option<&ExternalEditorRequest> {
        self.external_editor.as_ref()
    }

    pub fn forward_non_message_effects_from<N>(&mut self, child: &mut EventCtx<N>) {
        if child.redraw_requested() {
            self.request_redraw();
        }
        if child.layout_requested() {
            self.request_layout();
        }
        if child.quit_requested() {
            self.request_quit();
        }
        if child.clear_requested() {
            self.request_clear();
        }
        if let Some(request) = child.focus_request().cloned() {
            self.focus(request);
        }
        if let Some(repair) = child.focus_repair() {
            self.repair_focus(repair);
        }
        if child.propagation() == Propagation::Stopped {
            self.stop_propagation();
        }
        if let Some(request) = child.take_external_editor_request() {
            self.external_editor = Some(request);
            self.request_redraw();
        }
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

    pub(crate) fn take_external_editor_request(&mut self) -> Option<ExternalEditorRequest> {
        self.external_editor.take()
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

    pub fn with_focus_fallback<R>(
        &mut self,
        id: FocusId,
        area: Rect,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_focus_fallback_status(id, area, layout).0
    }

    pub fn with_focus_fallback_status<R>(
        &mut self,
        id: FocusId,
        area: Rect,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> (R, bool) {
        let focus_count = self.focus_paths.len();
        let result = layout(self);
        let inserted = self.focus_paths.len() == focus_count;
        if inserted {
            self.register_focusable(id, area, true);
        }
        (result, inserted)
    }

    pub fn with_focus_fallback_hotkey<R>(
        &mut self,
        id: FocusId,
        area: Rect,
        hotkey: KeyEvent,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_focus_fallback_hotkey_status(id, area, hotkey, layout)
            .0
    }

    pub fn with_focus_fallback_hotkey_status<R>(
        &mut self,
        id: FocusId,
        area: Rect,
        hotkey: KeyEvent,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> (R, bool) {
        let focus_count = self.focus_paths.len();
        let result = layout(self);
        let inserted = self.focus_paths.len() == focus_count;
        if inserted {
            self.register_focusable_with_hotkey(id, area, true, hotkey);
        } else if let Some(target) = self.focus_paths.get_mut(focus_count) {
            if target.hotkey.is_none() {
                target.hotkey = Some(hotkey);
            } else if target.hotkey != Some(hotkey) && !target.hotkeys.contains(&hotkey) {
                target.hotkeys.push(hotkey);
            }
            if let Some(sequence) = hotkey_sequence_from_event(hotkey)
                && !target.hotkey_sequences.contains(&sequence)
            {
                target.hotkey_sequences.push(sequence);
            }
        }
        (result, inserted)
    }

    pub fn with_focus_fallback_hotkey_sequence_status<R>(
        &mut self,
        id: FocusId,
        area: Rect,
        hotkey: impl Into<String>,
        layout: impl FnOnce(&mut Self) -> R,
    ) -> (R, bool) {
        let focus_count = self.focus_paths.len();
        let result = layout(self);
        let inserted = self.focus_paths.len() == focus_count;
        if inserted {
            self.register_focusable_with_hotkey_sequences(id, area, true, vec![hotkey.into()]);
        } else if let Some(target) = self.focus_paths.get_mut(focus_count) {
            add_focus_hotkey_sequence(target, hotkey.into());
        }
        (result, inserted)
    }

    pub fn focus_disabled(&self) -> bool {
        self.focus_disabled
    }

    pub fn set_focus_disabled(&mut self, disabled: bool) {
        self.focus_disabled = disabled;
    }

    pub fn register_focusable(&mut self, id: FocusId, area: Rect, enabled: bool) {
        if self.focus_disabled {
            return;
        }
        self.focus_paths.push(FocusTarget {
            id,
            path: self.current_path(),
            area,
            enabled,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        });
    }

    pub fn register_text_entry_focusable(
        &mut self,
        id: FocusId,
        area: Rect,
        enabled: bool,
        active: bool,
    ) {
        self.register_focusable(id.clone(), area, enabled);
        self.set_focus_text_entry_active(id, active);
    }

    pub fn register_focusable_with_hotkey(
        &mut self,
        id: FocusId,
        area: Rect,
        enabled: bool,
        hotkey: KeyEvent,
    ) {
        if self.focus_disabled {
            return;
        }
        self.focus_paths.push(FocusTarget {
            id,
            path: self.current_path(),
            area,
            enabled,
            tab_stop: true,
            hotkey: Some(hotkey),
            hotkeys: Vec::new(),
            hotkey_sequences: hotkey_sequence_from_event(hotkey).into_iter().collect(),
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        });
    }

    pub fn register_focusable_with_hotkeys(
        &mut self,
        id: FocusId,
        area: Rect,
        enabled: bool,
        hotkeys: Vec<KeyEvent>,
    ) {
        if self.focus_disabled {
            return;
        }
        self.focus_paths.push(FocusTarget {
            id,
            path: self.current_path(),
            area,
            enabled,
            tab_stop: true,
            hotkey: hotkeys.first().copied(),
            hotkey_sequences: hotkeys
                .iter()
                .filter_map(|hotkey| hotkey_sequence_from_event(*hotkey))
                .collect(),
            hotkeys,
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        });
    }

    pub fn register_focusable_with_hotkey_sequences(
        &mut self,
        id: FocusId,
        area: Rect,
        enabled: bool,
        hotkey_sequences: Vec<String>,
    ) {
        if self.focus_disabled {
            return;
        }
        let hotkey_sequences = normalized_hotkey_sequences(hotkey_sequences);
        let hotkeys = hotkey_sequences
            .iter()
            .filter_map(|hotkey| crate::hotkey::hotkey_sequence_to_event(hotkey))
            .collect::<Vec<_>>();
        self.focus_paths.push(FocusTarget {
            id,
            path: self.current_path(),
            area,
            enabled,
            tab_stop: true,
            hotkey: hotkeys.first().copied(),
            hotkeys,
            hotkey_sequences,
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        });
    }

    pub fn register_text_entry_focusable_with_hotkey_sequences(
        &mut self,
        id: FocusId,
        area: Rect,
        enabled: bool,
        hotkey_sequences: Vec<String>,
        active: bool,
    ) {
        self.register_focusable_with_hotkey_sequences(id.clone(), area, enabled, hotkey_sequences);
        self.set_focus_text_entry_active(id, active);
    }

    pub fn set_focus_text_entry_active(&mut self, id: FocusId, active: bool) -> bool {
        let path = self.current_path();
        let Some(target) = self
            .focus_paths
            .iter_mut()
            .rev()
            .find(|target| target.id == id && target.path == path)
        else {
            return false;
        };
        target.suppress_global_hotkeys = active;
        target.focused_events_before_global_hotkeys = active;
        true
    }

    pub fn set_focus_suppresses_global_hotkeys(&mut self, id: FocusId, suppress: bool) -> bool {
        let path = self.current_path();
        let Some(target) = self
            .focus_paths
            .iter_mut()
            .rev()
            .find(|target| target.id == id && target.path == path)
        else {
            return false;
        };
        target.suppress_global_hotkeys = suppress;
        true
    }

    pub fn set_focus_receives_events_before_global_hotkeys(
        &mut self,
        id: FocusId,
        receives_first: bool,
    ) -> bool {
        let path = self.current_path();
        let Some(target) = self
            .focus_paths
            .iter_mut()
            .rev()
            .find(|target| target.id == id && target.path == path)
        else {
            return false;
        };
        target.focused_events_before_global_hotkeys = receives_first;
        true
    }

    pub fn set_focus_tab_stop(&mut self, id: FocusId, tab_stop: bool) -> bool {
        let Some(target) = self
            .focus_paths
            .iter_mut()
            .rev()
            .find(|target| target.path == self.path && target.id == id)
        else {
            return false;
        };
        target.tab_stop = tab_stop;
        true
    }

    pub fn set_focus_hotkey(&mut self, id: FocusId, hotkey: KeyEvent) -> bool {
        let path = self.current_path();
        let Some(target) = self
            .focus_paths
            .iter_mut()
            .rev()
            .find(|target| target.id == id && target.path == path)
        else {
            return false;
        };
        let old_sequence = target.hotkey.and_then(hotkey_sequence_from_event);
        target.hotkey = Some(hotkey);
        if let Some(first) = target.hotkeys.first_mut() {
            *first = hotkey;
        } else {
            target.hotkeys.push(hotkey);
        }
        let old_index = old_sequence.and_then(|sequence| {
            target
                .hotkey_sequences
                .iter()
                .position(|hotkey| hotkey == &sequence)
        });
        if let Some(index) = old_index {
            target.hotkey_sequences.remove(index);
        }
        if let Some(sequence) = hotkey_sequence_from_event(hotkey) {
            let index = old_index.unwrap_or(0).min(target.hotkey_sequences.len());
            if !target.hotkey_sequences.contains(&sequence) {
                target.hotkey_sequences.insert(index, sequence);
            }
        }
        true
    }

    pub fn register_hit_region(&mut self, region: HitRegion) {
        self.hit_regions.push(region);
    }

    pub fn record_overflow(
        &mut self,
        axis: LayoutAxis,
        needed: u16,
        available: u16,
        policy: OverflowPolicyName,
    ) {
        self.overflow_diagnostics.push(LayoutOverflowDiagnostic {
            path: self.current_path(),
            child_index: None,
            axis,
            needed,
            available,
            policy,
        });
    }

    pub fn record_child_overflow(
        &mut self,
        child_index: usize,
        axis: LayoutAxis,
        needed: u16,
        available: u16,
        policy: OverflowPolicyName,
    ) {
        self.overflow_diagnostics.push(LayoutOverflowDiagnostic {
            path: self.current_path(),
            child_index: Some(child_index),
            axis,
            needed,
            available,
            policy,
        });
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

    pub fn overflow_diagnostics(&self) -> &[LayoutOverflowDiagnostic] {
        &self.overflow_diagnostics
    }
}

impl LayoutResult {
    pub fn new(area: Rect) -> Self {
        Self { area }
    }
}

impl LayoutProposal {
    pub fn unbounded() -> Self {
        Self {
            width: AxisProposal::Unbounded,
            height: AxisProposal::Unbounded,
        }
    }

    pub fn at_most(width: u16, height: u16) -> Self {
        Self {
            width: AxisProposal::AtMost(width),
            height: AxisProposal::AtMost(height),
        }
    }

    pub fn at_most_area(area: Rect) -> Self {
        Self::at_most(area.width, area.height)
    }

    pub fn exact(width: u16, height: u16) -> Self {
        Self {
            width: AxisProposal::Exact(width),
            height: AxisProposal::Exact(height),
        }
    }

    pub fn exact_area(area: Rect) -> Self {
        Self::exact(area.width, area.height)
    }
}

impl LayoutSize {
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl AxisExpand {
    pub fn both() -> Self {
        Self {
            width: true,
            height: true,
        }
    }
}

impl LayoutSizeHint {
    pub fn unmeasured() -> Self {
        Self {
            source: HintSource::LegacyUnmeasured,
            min: LayoutSize::default(),
            preferred: LayoutSize::default(),
            expand: AxisExpand::default(),
        }
    }

    pub fn legacy_fill() -> Self {
        Self {
            source: HintSource::LegacyUnmeasured,
            min: LayoutSize::default(),
            preferred: LayoutSize::default(),
            expand: AxisExpand::both(),
        }
    }

    pub fn fixed(width: u16, height: u16) -> Self {
        let size = LayoutSize::new(width, height);
        Self {
            source: HintSource::Measured,
            min: size,
            preferred: size,
            expand: AxisExpand::default(),
        }
    }

    pub fn content(width: u16, height: u16) -> Self {
        Self {
            source: HintSource::Measured,
            min: LayoutSize::default(),
            preferred: LayoutSize::new(width, height),
            expand: AxisExpand::default(),
        }
    }

    pub fn normalized(mut self, proposal: LayoutProposal) -> Self {
        self.preferred.width = normalize_axis(self.min.width, self.preferred.width, proposal.width);
        self.preferred.height =
            normalize_axis(self.min.height, self.preferred.height, proposal.height);
        self
    }
}

fn normalize_axis(min: u16, preferred: u16, proposal: AxisProposal) -> u16 {
    let preferred = preferred.max(min);
    match proposal {
        AxisProposal::Unbounded => preferred,
        AxisProposal::AtMost(max) => preferred.min(max).max(min),
        AxisProposal::Exact(exact) => exact.max(min),
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
            tab_stop: self.tab_stop,
            hotkey: self.hotkey.clone(),
            hotkeys: self.hotkeys.clone(),
            hotkey_sequences: self.hotkey_sequences.clone(),
            suppress_global_hotkeys: self.suppress_global_hotkeys,
            focused_events_before_global_hotkeys: self.focused_events_before_global_hotkeys,
        })
    }
}

fn add_focus_hotkey_sequence(target: &mut FocusTarget, hotkey: String) {
    let hotkey = crate::hotkey::normalize_hotkey(&hotkey);
    if hotkey.is_empty() || target.hotkey_sequences.contains(&hotkey) {
        return;
    }
    if let Some(event) = crate::hotkey::hotkey_sequence_to_event(&hotkey) {
        if target.hotkey.is_none() {
            target.hotkey = Some(event);
        } else if target.hotkey != Some(event) && !target.hotkeys.contains(&event) {
            target.hotkeys.push(event);
        }
    }
    target.hotkey_sequences.push(hotkey);
}

fn normalized_hotkey_sequences(hotkey_sequences: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();
    for hotkey in hotkey_sequences {
        let hotkey = crate::hotkey::normalize_hotkey(&hotkey);
        if !hotkey.is_empty() && !normalized.contains(&hotkey) {
            normalized.push(hotkey);
        }
    }
    normalized
}

fn hotkey_sequence_from_event(hotkey: KeyEvent) -> Option<String> {
    let crate::Key::Char(ch) = hotkey.code else {
        return None;
    };
    if hotkey.modifiers != crate::KeyModifiers::NONE {
        return None;
    }
    Some(ch.to_ascii_lowercase().to_string())
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

    #[test]
    fn measurement_constructors_normalize_hints() {
        let fixed = LayoutSizeHint::fixed(5, 3).normalized(LayoutProposal::exact(7, 2));

        assert_eq!(fixed.source, HintSource::Measured);
        assert_eq!(fixed.min, LayoutSize::new(5, 3));
        assert_eq!(fixed.preferred, LayoutSize::new(7, 3));

        let content = LayoutSizeHint::content(10, 4).normalized(LayoutProposal::at_most(6, 9));

        assert_eq!(content.min, LayoutSize::default());
        assert_eq!(content.preferred, LayoutSize::new(6, 4));
    }

    #[test]
    fn default_measure_returns_legacy_fill_hint() {
        struct Probe;

        impl TuiNode<()> for Probe {
            fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
                LayoutResult::new(area)
            }

            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let hint = Probe.measure(LayoutProposal::at_most(10, 5));

        assert_eq!(hint.source, HintSource::LegacyUnmeasured);
        assert_eq!(hint.preferred, LayoutSize::default());
        assert!(hint.expand.width);
        assert!(hint.expand.height);
    }

    #[test]
    fn non_focusable_wrapper_prevents_focus_registration() {
        struct FocusableProbe;
        impl TuiNode<()> for FocusableProbe {
            fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
                ctx.register_focusable(FocusId::new("probe"), area, true);
                LayoutResult::new(area)
            }
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        // Test normal focusable probe registers focus target
        let mut probe = FocusableProbe;
        let mut ctx = LayoutCtx::new();
        probe.layout(Rect::new(0, 0, 10, 10), &mut ctx);
        assert_eq!(ctx.focus_paths.len(), 1);

        // Test NonFocusable probe does not register focus target
        let mut non_focusable = NonFocusable::new(FocusableProbe);
        let mut ctx = LayoutCtx::new();
        non_focusable.layout(Rect::new(0, 0, 10, 10), &mut ctx);
        assert_eq!(ctx.focus_paths.len(), 0);
    }

    #[test]
    fn focus_fallback_registers_only_when_child_has_no_focus_target() {
        let mut ctx = LayoutCtx::new();

        ctx.with_focus_fallback(FocusId::new("fallback"), Rect::new(0, 0, 10, 1), |_ctx| {});

        assert_eq!(ctx.focus_targets().len(), 1);
        assert_eq!(ctx.focus_targets()[0].id.as_str(), "fallback");

        let mut ctx = LayoutCtx::new();
        ctx.with_focus_fallback(FocusId::new("fallback"), Rect::new(0, 0, 10, 1), |ctx| {
            ctx.register_focusable(FocusId::new("child"), Rect::new(0, 0, 10, 1), true);
        });

        assert_eq!(ctx.focus_targets().len(), 1);
        assert_eq!(ctx.focus_targets()[0].id.as_str(), "child");
    }

    #[test]
    fn set_focus_hotkey_updates_target_on_current_path() {
        let mut ctx = LayoutCtx::new();

        ctx.push_slot(ChildKey::new("slot"), Rect::new(0, 0, 10, 1), |ctx| {
            ctx.register_focusable(FocusId::new("child"), Rect::new(0, 0, 10, 1), true);
            assert!(
                ctx.set_focus_hotkey(FocusId::new("child"), KeyEvent::from(crate::Key::Char('c')),)
            );
        });

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(crate::Key::Char('c')))
        );
    }

    #[test]
    fn set_focus_hotkey_replaces_old_primary_sequence() {
        let mut ctx = LayoutCtx::new();

        ctx.register_focusable_with_hotkey(
            FocusId::new("child"),
            Rect::new(0, 0, 10, 1),
            true,
            KeyEvent::from(crate::Key::Char('a')),
        );
        assert!(
            ctx.set_focus_hotkey(FocusId::new("child"), KeyEvent::from(crate::Key::Char('b')),)
        );

        assert_eq!(
            ctx.focus_targets()[0].hotkey,
            Some(KeyEvent::from(crate::Key::Char('b')))
        );
        assert_eq!(ctx.focus_targets()[0].hotkey_sequences, vec!["b"]);
    }

    #[test]
    fn on_blur_decorator_emits_message() {
        struct Probe;
        impl TuiNode<&'static str> for Probe {
            fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
                LayoutResult::new(area)
            }
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let mut decorator = OnBlur::new(Probe, || "blurred");
        let mut ctx = FocusCtx::new(AnimationSettings::default());

        decorator.focus(None, false, &mut ctx);

        assert_eq!(ctx.drain_messages().collect::<Vec<_>>(), vec!["blurred"]);
    }

    #[test]
    fn on_blur_decorator_emits_message_through_dispatcher_focus_blur() {
        struct Probe;
        impl TuiNode<&'static str> for Probe {
            fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
                LayoutResult::new(area)
            }
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let target = FocusTarget {
            id: FocusId::new("probe"),
            path: TreePath::new(),
            area: Rect::default(),
            enabled: true,
            tab_stop: true,
            hotkey: None,
            hotkeys: Vec::new(),
            hotkey_sequences: Vec::new(),
            suppress_global_hotkeys: false,
            focused_events_before_global_hotkeys: false,
        };
        let mut decorator = OnBlur::new(Probe, || "blurred");
        let mut dispatcher = crate::TreeDispatcher::new();

        let effects = dispatcher.dispatch_focus(
            &mut decorator,
            crate::FocusTransition {
                previous: Some(target),
                current: None,
            },
            AnimationSettings::default(),
        );

        assert_eq!(effects.messages, vec!["blurred"]);
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

pub struct NonFocusable<N> {
    inner: N,
}

impl<N> NonFocusable<N> {
    pub fn new(inner: N) -> Self {
        Self { inner }
    }
}

impl<N, M> TuiNode<M> for NonFocusable<N>
where
    N: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.inner.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let was_disabled = ctx.focus_disabled();
        ctx.set_focus_disabled(true);
        let result = self.inner.layout(area, ctx);
        ctx.set_focus_disabled(was_disabled);
        result
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.inner.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.inner.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.inner.dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.inner.tick(dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.inner.focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.inner.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.destroy(ctx);
    }
}

pub struct OnBlur<N, M> {
    inner: N,
    on_blur: Option<Box<dyn Fn() -> M>>,
}

impl<N, M> OnBlur<N, M> {
    pub fn new(inner: N, on_blur: impl Fn() -> M + 'static) -> Self {
        Self {
            inner,
            on_blur: Some(Box::new(on_blur)),
        }
    }

    fn emit_blur(&self, ctx: &mut FocusCtx<M>) {
        if let Some(on_blur) = &self.on_blur {
            ctx.emit(on_blur());
        }
    }
}

impl<N, M> TuiNode<M> for OnBlur<N, M>
where
    N: TuiNode<M>,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.inner.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.inner.layout(area, ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        self.inner.render(frame, area);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.inner.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.inner.dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: std::time::Duration, settings: AnimationSettings) -> TickResult {
        self.inner.tick(dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        if !focused {
            self.emit_blur(ctx);
        }
        self.inner.focus(target, focused, ctx);
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if !focused {
            self.emit_blur(ctx);
        }
        self.inner.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.inner.destroy(ctx);
    }
}
