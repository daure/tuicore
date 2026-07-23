use std::collections::VecDeque;
use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear};

use crate::{
    Animated, AnimationSettings, Easing, LayoutCtx, LayoutResult, TickResult, TuiNode, Tween,
    line_width, theme,
};

use super::typography::{Paragraph, ParagraphOverflow, wrapped_text_line_count};

const DEFAULT_HISTORY_LIMIT: usize = 100;
const DEFAULT_MAX_VISIBLE: usize = 3;
const TOAST_MAX_WIDTH: u16 = 54;
const TOAST_MIN_WIDTH: u16 = 20;
const TOAST_MIN_BODY_LINES: u16 = 1;
const TOAST_MAX_BODY_LINES: u16 = 4;
const TOAST_BORDER_HEIGHT: u16 = 2;
const TOAST_GAP: u16 = 0;
const SLIDE_TRAVEL: f64 = TOAST_MAX_WIDTH as f64;
const ENTER_DURATION: Duration = Duration::from_millis(180);
const EXIT_DURATION: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NotificationId(u64);

impl NotificationId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    kind: NotificationKind,
    title: String,
    body: String,
    ttl: Option<Duration>,
    sticky: bool,
}

impl Notification {
    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NotificationKind::Info, title, body)
    }

    pub fn success(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NotificationKind::Success, title, body)
    }

    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NotificationKind::Warning, title, body)
    }

    pub fn error(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(NotificationKind::Error, title, body)
    }

    pub fn new(kind: NotificationKind, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            body: body.into(),
            ttl: None,
            sticky: false,
        }
    }

    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self.sticky = false;
        self
    }

    pub fn sticky(mut self) -> Self {
        self.ttl = None;
        self.sticky = true;
        self
    }

    pub fn kind(&self) -> NotificationKind {
        self.kind
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    fn visible_ttl(&self) -> Option<Duration> {
        if self.sticky {
            return None;
        }
        Some(self.ttl.unwrap_or_else(|| default_ttl(self.kind)))
    }
}

#[derive(Debug, Clone)]
pub struct NotificationCenter {
    active: Vec<ActiveToast>,
    history: VecDeque<Notification>,
    history_limit: usize,
    next_id: u64,
}

impl Default for NotificationCenter {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationCenter {
    pub fn new() -> Self {
        Self::with_history_limit(DEFAULT_HISTORY_LIMIT)
    }

    pub fn with_history_limit(history_limit: usize) -> Self {
        Self {
            active: Vec::new(),
            history: VecDeque::new(),
            history_limit,
            next_id: 1,
        }
    }

    pub fn push(&mut self, notification: Notification) -> NotificationId {
        let id = NotificationId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.push_history(notification.clone());
        self.active.insert(0, ActiveToast::new(id, notification));
        id
    }

    pub fn dismiss(&mut self, id: NotificationId) {
        if let Some(toast) = self.active.iter_mut().find(|toast| toast.id == id) {
            toast.start_exit();
        }
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }

    pub fn active(&self) -> impl ExactSizeIterator<Item = &Notification> + '_ {
        self.active.iter().map(|toast| &toast.notification)
    }

    pub fn history(&self) -> impl ExactSizeIterator<Item = &Notification> + '_ {
        self.history.iter()
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.tick_inner(dt, settings)
    }

    fn push_history(&mut self, notification: Notification) {
        if self.history_limit == 0 {
            return;
        }
        self.history.push_back(notification);
        while self.history.len() > self.history_limit {
            self.history.pop_front();
        }
    }

    fn tick_inner(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let mut changed = false;
        let mut layout = false;
        let mut active = false;

        for toast in &mut self.active {
            let tick = toast.tick(dt, settings);
            changed |= tick.changed;
            layout |= tick.layout;
            active |= tick.active;
        }

        let before = self.active.len();
        self.active.retain(|toast| !toast.is_finished());
        changed |= self.active.len() != before;

        TickResult {
            changed,
            layout,
            active,
            next_tick: None,
        }
    }
}

impl Animated for NotificationCenter {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.tick_inner(dt, settings)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastIcons {
    NerdFont,
    Ascii,
    Plain,
}

#[derive(Debug, Clone)]
pub struct ToastRack {
    center: NotificationCenter,
    max_visible: usize,
    max_width: u16,
    icons: ToastIcons,
}

impl Default for ToastRack {
    fn default() -> Self {
        Self::new()
    }
}

impl ToastRack {
    pub fn new() -> Self {
        Self {
            center: NotificationCenter::new(),
            max_visible: DEFAULT_MAX_VISIBLE,
            max_width: TOAST_MAX_WIDTH,
            icons: ToastIcons::NerdFont,
        }
    }

    pub fn center(&self) -> &NotificationCenter {
        &self.center
    }

    pub fn center_mut(&mut self) -> &mut NotificationCenter {
        &mut self.center
    }

    pub fn push(&mut self, notification: Notification) -> NotificationId {
        self.center.push(notification)
    }

    pub fn max_visible(mut self, max_visible: usize) -> Self {
        self.max_visible = max_visible;
        self
    }

    pub fn max_width(mut self, max_width: u16) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn icons(mut self, icons: ToastIcons) -> Self {
        self.icons = icons;
        self
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.center.tick(dt, settings)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.width < TOAST_MIN_WIDTH
            || area.height < TOAST_MIN_BODY_LINES + TOAST_BORDER_HEIGHT + 1
            || self.max_visible == 0
        {
            return;
        }

        let width = area.width.min(self.max_width).min(TOAST_MAX_WIDTH);
        if width < TOAST_MIN_WIDTH {
            return;
        }

        let spare_width = area.width.saturating_sub(width);
        let right_margin = u16::from(spare_width > 0);
        let base_x = area
            .x
            .saturating_add(spare_width.saturating_sub(right_margin));
        let mut y = area.y.saturating_add(1);
        for toast in self.center.active.iter().take(self.max_visible) {
            let height = toast_height(toast.notification.body(), width.saturating_sub(2));
            if y.saturating_add(height) > area.bottom() {
                break;
            }
            let offset = toast.offset().round().clamp(0.0, f64::from(width)) as u16;
            let toast_area = Rect::new(base_x.saturating_add(offset), y, width, height);
            self.render_toast(frame, toast, toast_area);
            y = y.saturating_add(height + TOAST_GAP);
        }
    }

    fn render_toast(&self, frame: &mut Frame, toast: &ActiveToast, area: Rect) {
        let palette = theme();
        let accent = match toast.notification.kind() {
            NotificationKind::Info => palette.accent_fg(),
            NotificationKind::Success => palette.success_fg(),
            NotificationKind::Warning => palette.warning_fg(),
            NotificationKind::Error => palette.error_fg(),
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(accent))
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    self.icon(toast.notification.kind()),
                    Style::default().fg(accent),
                ),
                Span::raw(" "),
                Span::styled(
                    trim_line(toast.notification.title(), area.width.saturating_sub(7)),
                    Style::default()
                        .fg(palette.text_fg())
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        let inner = block.inner(area);

        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        Paragraph::new(toast.notification.body())
            .style(Style::default().fg(palette.muted_fg()))
            .overflow(ParagraphOverflow::Ellipsis)
            .max_lines(inner.height as usize)
            .render(frame, inner);
    }

    fn icon(&self, kind: NotificationKind) -> &'static str {
        match (self.icons, kind) {
            (ToastIcons::Plain, _) => "",
            (ToastIcons::Ascii, NotificationKind::Info) => "i",
            (ToastIcons::Ascii, NotificationKind::Success) => "+",
            (ToastIcons::Ascii, NotificationKind::Warning) => "!",
            (ToastIcons::Ascii, NotificationKind::Error) => "x",
            (ToastIcons::NerdFont, NotificationKind::Info) => "󰋼",
            (ToastIcons::NerdFont, NotificationKind::Success) => "󰄬",
            (ToastIcons::NerdFont, NotificationKind::Warning) => "󰀪",
            (ToastIcons::NerdFont, NotificationKind::Error) => "󰅙",
        }
    }
}

impl Animated for ToastRack {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Self::tick(self, dt, settings)
    }
}

impl<M> TuiNode<M> for ToastRack {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        Self::render(self, frame, area);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

#[derive(Debug, Clone)]
struct ActiveToast {
    id: NotificationId,
    notification: Notification,
    phase: ToastPhase,
    elapsed: Duration,
    offset: Tween,
}

impl ActiveToast {
    fn new(id: NotificationId, notification: Notification) -> Self {
        let mut offset = Tween::idle(SLIDE_TRAVEL);
        offset.start(SLIDE_TRAVEL, 0.0, ENTER_DURATION, Easing::EaseOutCubic);
        Self {
            id,
            notification,
            phase: ToastPhase::Entering,
            elapsed: Duration::ZERO,
            offset,
        }
    }

    fn start_exit(&mut self) {
        if self.phase == ToastPhase::Exiting {
            return;
        }
        self.phase = ToastPhase::Exiting;
        self.offset.start(
            self.offset.value(),
            SLIDE_TRAVEL,
            EXIT_DURATION,
            Easing::EaseInOut,
        );
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        if !settings.enabled && self.offset.is_active() {
            self.offset.snap_to_end();
        }

        let mut tick = self.offset.tick(dt, settings);
        if self.phase == ToastPhase::Entering && !self.offset.is_active() {
            self.phase = ToastPhase::Visible;
            tick.changed = true;
        }

        if self.phase != ToastPhase::Exiting {
            if let Some(ttl) = self.notification.visible_ttl() {
                self.elapsed = self.elapsed.saturating_add(dt);
                if self.elapsed >= ttl {
                    self.start_exit();
                    tick.changed = true;
                    if !settings.enabled {
                        self.offset.snap_to_end();
                    }
                } else {
                    tick.active = true;
                }
            }
        }

        if self.phase == ToastPhase::Exiting && self.offset.is_active() {
            tick.active = true;
        }

        tick
    }

    fn is_finished(&self) -> bool {
        self.phase == ToastPhase::Exiting && !self.offset.is_active()
    }

    fn offset(&self) -> f64 {
        self.offset.value()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastPhase {
    Entering,
    Visible,
    Exiting,
}

fn default_ttl(kind: NotificationKind) -> Duration {
    match kind {
        NotificationKind::Info | NotificationKind::Success => Duration::from_secs(3),
        NotificationKind::Warning => Duration::from_secs(5),
        NotificationKind::Error => Duration::from_secs(8),
    }
}

fn toast_height(body: &str, width: u16) -> u16 {
    let body_lines = wrapped_text_line_count(body, width, TOAST_MAX_BODY_LINES as usize) as u16;
    body_lines.clamp(TOAST_MIN_BODY_LINES, TOAST_MAX_BODY_LINES) + TOAST_BORDER_HEIGHT
}

fn trim_line(value: &str, max_width: u16) -> String {
    let mut value = value.to_string();
    while line_width(&Line::from(value.as_str())) > max_width as usize {
        if value.pop().is_none() {
            break;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn notification_constructors_set_kind_and_text() {
        let notification = Notification::success("Saved", "Profile updated");

        assert_eq!(notification.kind(), NotificationKind::Success);
        assert_eq!(notification.title(), "Saved");
        assert_eq!(notification.body(), "Profile updated");
    }

    #[test]
    fn center_push_tracks_active_and_bounded_history() {
        let mut center = NotificationCenter::with_history_limit(2);

        let first = center.push(Notification::info("One", "Body"));
        let second = center.push(Notification::warning("Two", "Body"));
        let third = center.push(Notification::error("Three", "Body"));

        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);
        assert_eq!(third.get(), 3);
        assert_eq!(center.active().len(), 3);
        assert_eq!(
            center
                .history()
                .map(Notification::title)
                .collect::<Vec<_>>(),
            ["Two", "Three"]
        );
    }

    #[test]
    fn finite_notification_expires_without_animation() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut center = NotificationCenter::new();
        center.push(Notification::info("Short", "Body").ttl(Duration::from_millis(10)));

        let tick = center.tick(Duration::from_millis(10), settings);

        assert!(tick.changed);
        assert!(!tick.active);
        assert_eq!(center.active().len(), 0);
    }

    #[test]
    fn finite_notification_keeps_scheduler_active_without_animation_until_ttl() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut center = NotificationCenter::new();
        center.push(Notification::info("Short", "Body").ttl(Duration::from_millis(10)));

        let tick = center.tick(Duration::from_millis(5), settings);

        assert!(tick.changed);
        assert!(tick.active);
        assert_eq!(center.active().len(), 1);
    }

    #[test]
    fn sticky_notification_does_not_keep_scheduler_active_after_entry() {
        let mut center = NotificationCenter::new();
        center.push(Notification::info("Pinned", "Body").sticky());
        let mut settings = AnimationSettings::default();
        settings.max_dt = Duration::from_secs(1);

        let tick = center.tick(ENTER_DURATION, settings);

        assert!(tick.changed);
        assert!(!tick.active);
        assert_eq!(center.active().len(), 1);
    }

    #[test]
    fn dismiss_starts_exit_and_removes_after_tick() {
        let mut center = NotificationCenter::new();
        let id = center.push(Notification::info("Dismiss", "Body").sticky());
        let mut settings = AnimationSettings::default();
        settings.max_dt = Duration::from_secs(1);
        center.tick(ENTER_DURATION, settings);

        center.dismiss(id);
        let tick = center.tick(EXIT_DURATION, settings);

        assert!(tick.changed);
        assert!(!tick.active);
        assert_eq!(center.active().len(), 0);
    }

    #[test]
    fn toast_render_stays_inside_nonzero_full_width_area() {
        let mut rack = ToastRack::new().max_width(20);
        rack.push(Notification::info("Inside", "Body").sticky());
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        rack.tick(Duration::ZERO, settings);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).expect("terminal should build");

        terminal
            .draw(|frame| rack.render(frame, Rect::new(10, 0, 20, 5)))
            .expect("toast should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((9, 1)).unwrap().symbol(), " ");
        assert_ne!(buffer.cell((10, 1)).unwrap().symbol(), " ");
    }

    #[test]
    fn toast_render_requires_top_margin_space() {
        let mut rack = ToastRack::new().max_width(20);
        rack.push(Notification::info("Hidden", "Body").sticky());
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        rack.tick(Duration::ZERO, settings);
        let mut terminal = Terminal::new(TestBackend::new(24, 3)).expect("terminal should build");

        terminal
            .draw(|frame| rack.render(frame, frame.area()))
            .expect("toast should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((0, 0)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 1)).unwrap().symbol(), " ");
        assert_eq!(buffer.cell((0, 2)).unwrap().symbol(), " ");
    }
}
