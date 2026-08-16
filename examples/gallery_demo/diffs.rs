use std::time::Duration;

use ratatui::{Frame, layout::Rect};
use tuicore::{
    AnimationSettings, ChildKey, DiffStyle, DiffViewer, EventCtx, EventOutcome, EventRoute, Flex,
    FlexItem, FocusCtx, FocusTarget, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    LifecycleCtx, Panel, PanelHost, RenderCtx, TickResult, Toggle, TuiEvent, TuiNode,
};

pub(crate) struct DiffDemo<M = ()> {
    scaffold: Flex<M>,
    controls: Flex<M>,
    headers: Toggle<M>,
    wrap: Toggle<M>,
    panel: PanelHost<DiffViewer, M>,
}

impl<M: 'static> DiffDemo<M> {
    fn new(title: &'static str, old: &'static str, new: &'static str, style: DiffStyle) -> Self {
        Self {
            scaffold: Flex::column()
                .gap(1)
                .child(controls_region_key(), DiffRegion, FlexItem::fixed(1))
                .child(panel_region_key(), DiffRegion, FlexItem::fixed(22)),
            controls: Flex::row()
                .gap(3)
                .child(headers_region_key(), DiffRegion, FlexItem::fixed(12))
                .child(wrap_region_key(), DiffRegion, FlexItem::fixed(16)),
            headers: Toggle::new("Headers").checked(true),
            wrap: Toggle::new("Wrap lines").checked(true),
            panel: Panel::new()
                .top_left(title)
                .bottom_left("20 rows · scroll to inspect")
                .host(
                    DiffViewer::new(old, new)
                        .labels("before", "after")
                        .style(style)
                        .context_lines(2)
                        .min_rows(20)
                        .max_rows(20),
                ),
        }
    }

    #[cfg(test)]
    pub(crate) fn viewer(&self) -> &DiffViewer {
        self.panel.child()
    }

    #[cfg(test)]
    pub(crate) fn headers_enabled(&self) -> bool {
        self.headers.is_checked()
    }

    #[cfg(test)]
    pub(crate) fn wrapping_enabled(&self) -> bool {
        self.wrap.is_checked()
    }

    fn sync_viewer(&mut self) {
        self.panel
            .child_mut()
            .set_show_headers(self.headers.is_checked());
        self.panel.child_mut().set_wrap(self.wrap.is_checked());
    }

    fn region(&self, key: ChildKey) -> Rect {
        self.scaffold
            .child_rect(&key)
            .expect("diff demo region should be laid out")
    }

    fn control_region(&self, key: ChildKey) -> Rect {
        self.controls
            .child_rect(&key)
            .expect("diff control region should be laid out")
    }
}

impl<M: 'static> TuiNode<M> for DiffDemo<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.scaffold.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.sync_viewer();
        self.scaffold.layout(area, ctx);
        let controls = self.region(controls_region_key());
        let panel = self.region(panel_region_key());
        self.controls.layout(controls, ctx);
        let headers = self.control_region(headers_region_key());
        let wrap = self.control_region(wrap_region_key());
        ctx.push_slot(headers_child_key(), headers, |ctx| {
            self.headers.layout(headers, ctx);
        });
        ctx.push_slot(wrap_child_key(), wrap, |ctx| {
            self.wrap.layout(wrap, ctx);
        });
        ctx.push_slot(panel_child_key(), panel, |ctx| {
            <PanelHost<DiffViewer, M> as TuiNode<M>>::layout(&mut self.panel, panel, ctx);
        });
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        self.headers
            .render(frame, self.control_region(headers_region_key()));
        self.wrap
            .render(frame, self.control_region(wrap_region_key()));
        <PanelHost<DiffViewer, M> as TuiNode<M>>::render(
            &self.panel,
            frame,
            self.region(panel_region_key()),
            ctx,
        );
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let outcome = if let Some(route) = child_route(route, headers_child_key()) {
            self.headers.dispatch_event(&route, event, ctx)
        } else if let Some(route) = child_route(route, wrap_child_key()) {
            self.wrap.dispatch_event(&route, event, ctx)
        } else if let Some(route) = child_route(route, panel_child_key()) {
            self.panel.dispatch_event(&route, event, ctx)
        } else {
            EventOutcome::Ignored
        };
        self.sync_viewer();
        outcome
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        if let Some(target) = target.for_child(&headers_child_key()) {
            self.headers.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&wrap_child_key()) {
            self.wrap.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&panel_child_key()) {
            self.panel.dispatch_focus(&target, focused, ctx);
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.headers
            .tick(dt, settings)
            .merge(self.wrap.tick(dt, settings))
            .merge(<PanelHost<DiffViewer, M> as TuiNode<M>>::tick(
                &mut self.panel,
                dt,
                settings,
            ))
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.headers.init(ctx);
        self.wrap.init(ctx);
        self.panel.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.headers.mount(ctx);
        self.wrap.mount(ctx);
        self.panel.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.panel.unmount(ctx);
        self.wrap.unmount(ctx);
        self.headers.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.panel.destroy(ctx);
        self.wrap.destroy(ctx);
        self.headers.destroy(ctx);
    }
}

#[derive(Clone, Copy)]
struct DiffRegion;

impl<M> TuiNode<M> for DiffRegion {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, _frame: &mut Frame, _area: Rect, _ctx: &mut RenderCtx<'a>) {}
}

pub(crate) fn side_by_side_diff_demo<M: 'static>() -> DiffDemo<M> {
    demo(
        "Side-by-side / Split",
        SIDE_OLD,
        SIDE_NEW,
        DiffStyle::SideBySide,
    )
}

pub(crate) fn inline_diff_demo<M: 'static>() -> DiffDemo<M> {
    demo(
        "Inline / Unified",
        INLINE_OLD,
        INLINE_NEW,
        DiffStyle::Inline,
    )
}

pub(crate) fn word_diff_demo<M: 'static>() -> DiffDemo<M> {
    demo("Word / Intra-line", WORD_OLD, WORD_NEW, DiffStyle::Word)
}

pub(crate) fn raw_patch_diff_demo<M: 'static>() -> DiffDemo<M> {
    demo(
        "Raw patch / Patch view",
        PATCH_OLD,
        PATCH_NEW,
        DiffStyle::RawPatch,
    )
}

fn demo<M: 'static>(
    title: &'static str,
    old: &'static str,
    new: &'static str,
    style: DiffStyle,
) -> DiffDemo<M> {
    DiffDemo::new(title, old, new, style)
}

pub(crate) fn headers_child_key() -> ChildKey {
    ChildKey::new("diff-headers")
}

pub(crate) fn wrap_child_key() -> ChildKey {
    ChildKey::new("diff-wrap")
}

fn panel_child_key() -> ChildKey {
    ChildKey::new("diff-panel")
}

fn controls_region_key() -> ChildKey {
    ChildKey::new("diff-controls-region")
}

fn panel_region_key() -> ChildKey {
    ChildKey::new("diff-panel-region")
}

fn headers_region_key() -> ChildKey {
    ChildKey::new("diff-headers-region")
}

fn wrap_region_key() -> ChildKey {
    ChildKey::new("diff-wrap-region")
}

fn child_route(route: &EventRoute, key: ChildKey) -> Option<EventRoute> {
    route.path.without_first_if(&key).map(EventRoute::new)
}

const SIDE_OLD: &str = r#"use crate::{Item, Summary};

pub fn summarize(items: &[Item]) -> Summary {
    let mut accepted = 0;
    let mut rejected = 0;
    for item in items {
        if item.score > 70 {
            accepted += 1;
        } else {
            rejected += 1;
        }
    }
    tracing::info!(accepted, rejected, "finished processing every candidate from the nightly import queue in the primary region");
    Summary { accepted, rejected }
}

fn retry_limit() -> usize {
    3
}

fn retry_delay(attempt: usize) -> u64 {
    100 * attempt as u64
}

pub fn should_archive(summary: &Summary) -> bool {
    summary.rejected > 10
}

pub fn report_name(day: &str) -> String {
    format!("nightly-{day}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_low_scores() {
        assert_eq!(summarize(&fixtures()).rejected, 2);
    }
}
"#;

const SIDE_NEW: &str = r#"use crate::{Item, Summary};

pub fn summarize(items: &[Item]) -> Summary {
    let mut accepted = 0;
    let mut deferred = 0;
    for item in items {
        if item.score >= 80 {
            accepted += 1;
        } else if item.score >= 50 {
            deferred += 1;
        }
    }
    tracing::info!(accepted, deferred, "finished processing every candidate from the nightly import and validation queue in all configured regions");
    Summary { accepted, deferred }
}

fn retry_limit() -> usize {
    5
}

fn retry_delay(attempt: usize) -> u64 {
    250 * 2_u64.pow(attempt as u32)
}

pub fn should_archive(summary: &Summary) -> bool {
    summary.deferred > 25
}

pub fn report_name(day: &str, region: &str) -> String {
    format!("nightly-{region}-{day}")
}

pub fn validation_enabled() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defers_reviewable_scores() {
        assert_eq!(summarize(&fixtures()).deferred, 2);
    }
}
"#;

const INLINE_OLD: &str = r#"[server]
host = "127.0.0.1"
port = 8080
workers = 2
graceful_shutdown_seconds = 10

[cache]
enabled = false
ttl_seconds = 30
namespace = "gallery-preview-development-cache-with-an-intentionally-long-name-for-horizontal-scroll-validation"
eviction = "lru"

[logging]
format = "text"
level = "info"
include_targets = false
include_thread_ids = false

[limits]
request_bytes = 1048576
burst_requests = 16
requests_per_second = 50

[database]
pool_min = 1
pool_max = 8
connect_timeout_seconds = 5

[telemetry]
enabled = false
sample_ratio = 0.0
endpoint = "http://localhost:4317"

[features]
audit_log = false
regional_failover = false
streaming_exports = false
"#;

const INLINE_NEW: &str = r#"[server]
host = "0.0.0.0"
port = 8080
workers = 8
graceful_shutdown_seconds = 30

[cache]
enabled = true
ttl_seconds = 120
namespace = "gallery-preview-production-cache-with-an-intentionally-long-name-and-region-suffix-for-horizontal-scroll-validation"
eviction = "tiny-lfu"
compression = "zstd"

[logging]
format = "json"
level = "debug"
include_targets = true
include_thread_ids = true

[limits]
request_bytes = 4194304
burst_requests = 64
requests_per_second = 200

[database]
pool_min = 4
pool_max = 32
connect_timeout_seconds = 3
idle_timeout_seconds = 60

[telemetry]
enabled = true
sample_ratio = 0.25
endpoint = "https://collector.internal.example.net:4317/v1/traces/production/eu-west-1"

[features]
audit_log = true
regional_failover = true
streaming_exports = true
background_compaction = true
"#;

const WORD_OLD: &str = r#"Release notes

The renderer updates each record immediately after parsing and writes a concise completion message.
Operators can inspect concise status messages in the activity panel.
Failed requests retry three times before the queue pauses.
Metrics use request identifiers supplied by the edge proxy.

Migration

Existing profiles retain the legacy compact spacing preset.
New profiles use balanced spacing and subtle separators.
The compatibility window closes after the September release.
Administrators should export a backup before enabling migration.

Diagnostics

Verbose traces include request identifiers, elapsed milliseconds, and the complete upstream service endpoint.
Exported reports remain available for seven days.
Health checks run every sixty seconds from one region.
Warnings include the service name but omit deployment metadata.

Operations

Workers drain active jobs before accepting a rolling restart.
The scheduler assigns ten records to each processing batch.
Archived jobs remain searchable from the history screen.
On-call engineers receive one alert after five failures.

Security

API tokens expire after thirty days.
Audit events record actor and action fields.
Encryption keys rotate once per quarter.
Sessions close after sixty minutes of inactivity.
"#;

const WORD_NEW: &str = r#"Release notes

The renderer batches each record efficiently after validation and writes a detailed completion summary.
Operators can inspect detailed status messages in the searchable timeline panel.
Failed requests retry five times with exponential backoff before the queue pauses.
Metrics use correlation identifiers generated by the regional edge proxy.

Migration guide

Existing profiles adopt the modern comfortable spacing preset.
New profiles use generous spacing and emphasized separators.
The compatibility window closes after the November maintenance release.
Administrators must verify an encrypted backup before enabling automatic migration.

Diagnostics and telemetry

Verbose traces include correlation identifiers, elapsed microseconds, and the complete upstream service endpoint with region metadata.
Exported reports remain available for thirty days.
Health checks run every fifteen seconds from three regions.
Warnings include the service name, deployment revision, and owning team metadata.

Operations

Workers checkpoint active jobs before beginning a zero-downtime rolling restart.
The scheduler assigns twenty-five records to each adaptive processing batch.
Archived jobs remain searchable and restorable from the history screen.
On-call engineers receive grouped alerts after three consecutive failures.

Security and compliance

API tokens expire after fourteen days and support scoped renewal.
Audit events record actor, action, source, and outcome fields.
Encryption keys rotate automatically every month.
Sessions close after thirty minutes of inactivity and require reauthentication.
"#;

const PATCH_OLD: &str = r#"use crate::transport::Client;

pub struct SyncJob {
    client: Client,
    batch_size: usize,
}

impl SyncJob {
    pub fn new(client: Client) -> Self {
        Self { client, batch_size: 50 }
    }

    pub async fn run(&self, records: Vec<Record>) -> Result<Report> {
        let chunks = records.chunks(self.batch_size);
        for chunk in chunks {
            self.client.send(chunk).await?;
        }
        Ok(Report::complete())
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

fn validate(records: &[Record]) -> Result<()> {
    for record in records {
        record.validate()?;
    }
    Ok(())
}

fn destination() -> &'static str {
    "primary"
}

const USER_AGENT: &str = "tuicore-gallery-sync-client/1.0 (single-region; synchronous-validation; verbose-telemetry; legacy-retry-disabled)";
"#;

const PATCH_NEW: &str = r#"use crate::transport::{Client, RetryPolicy};

pub struct SyncJob {
    client: Client,
    batch_size: usize,
    retry: RetryPolicy,
}

impl SyncJob {
    pub fn new(client: Client, retry: RetryPolicy) -> Self {
        Self { client, batch_size: 100, retry }
    }

    pub async fn run(&self, records: Vec<Record>) -> Result<Report> {
        validate(&records)?;
        let mut report = Report::default();
        for chunk in records.chunks(self.batch_size) {
            report.merge(self.client.send_with_retry(chunk, &self.retry).await?);
        }
        report.mark_complete();
        Ok(report)
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

fn validate(records: &[Record]) -> Result<()> {
    records.iter().try_for_each(Record::validate_strict)
}

fn destinations() -> &'static [&'static str] {
    &["primary", "eu-west", "ap-south"]
}

fn telemetry_enabled() -> bool {
    true
}

const USER_AGENT: &str = "tuicore-gallery-sync-client/2.0 (multi-region; asynchronous-validation; structured-telemetry; bounded-exponential-retry-enabled)";
"#;
