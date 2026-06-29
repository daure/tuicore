use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

use crate::animation::{
    Animated, AnimationSettings, AnimationSpec, ResolvedAnimationSpec, ScrollAnimator, TickResult,
};
use crate::event::{Key, KeyEvent, KeyModifiers};
use crate::{KeyBindings, theme, ui::keybindings};

const HORIZONTAL_JUMP: isize = 8;

#[derive(Debug, Clone)]
pub struct ScrollState {
    x: AxisScroll,
    y: AxisScroll,
    axes: ScrollAxes,
    behavior: ScrollBehavior,
    scrollbars: ScrollbarConfig,
    pending_top_prefix: bool,
}

#[derive(Debug, Clone)]
struct AxisScroll {
    target: usize,
    animator: ScrollAnimator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollOffset {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollSize {
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollDelta {
    pub x: isize,
    pub y: isize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxes {
    Vertical,
    Horizontal,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollBehavior {
    pub line_step: usize,
    pub page_overlap: usize,
    pub animation: AnimationSpec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollOutcome {
    pub handled: bool,
    pub changed: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollLayout {
    pub outer: Rect,
    pub viewport: Rect,
    pub vertical_bar: Option<Rect>,
    pub horizontal_bar: Option<Rect>,
    pub corner: Option<Rect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollGeometry {
    pub layout: ScrollLayout,
    pub viewport: ScrollSize,
    pub content: ScrollSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarConfig {
    pub vertical: ScrollbarVisibility,
    pub horizontal: ScrollbarVisibility,
    pub gutter: ScrollbarGutter,
    pub style: ScrollbarStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarVisibility {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarGutter {
    Overlay,
    Reserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarStyle {
    ThinTrack,
    ThickTrack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollPreset {
    pub line_step: usize,
    pub page_overlap: usize,
    pub vertical_scrollbar: ScrollbarVisibility,
    pub horizontal_scrollbar: ScrollbarVisibility,
    pub gutter: ScrollbarGutter,
    pub style: ScrollbarStyle,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new(ScrollAxes::Vertical)
    }
}

impl ScrollState {
    pub fn new(axes: ScrollAxes) -> Self {
        Self {
            x: AxisScroll::default(),
            y: AxisScroll::default(),
            axes,
            behavior: ScrollBehavior::default(),
            scrollbars: ScrollbarConfig::default(),
            pending_top_prefix: false,
        }
    }

    pub fn behavior(mut self, behavior: ScrollBehavior) -> Self {
        self.behavior = behavior;
        self
    }

    pub fn scrollbars(mut self, config: ScrollbarConfig) -> Self {
        self.scrollbars = config;
        self
    }

    pub fn with_axes(mut self, axes: ScrollAxes) -> Self {
        self.axes = axes;
        self
    }

    pub fn from_preset(axes: ScrollAxes, preset: ScrollPreset) -> Self {
        Self::new(axes)
            .behavior(ScrollBehavior {
                line_step: preset.line_step,
                page_overlap: preset.page_overlap,
                animation: AnimationSpec::default(),
            })
            .scrollbars(ScrollbarConfig {
                vertical: preset.vertical_scrollbar,
                horizontal: preset.horizontal_scrollbar,
                gutter: preset.gutter,
                style: preset.style,
            })
    }

    pub fn offset(&self) -> ScrollOffset {
        ScrollOffset::new(self.x.offset(), self.y.offset())
    }

    pub fn target_offset(&self) -> ScrollOffset {
        ScrollOffset::new(self.x.target, self.y.target)
    }

    pub fn is_active(&self) -> bool {
        self.x.is_active() || self.y.is_active()
    }

    pub fn scroll_by(
        &mut self,
        delta: ScrollDelta,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        self.scroll_by_with_snap(delta, viewport, content, settings, false, false)
    }

    fn scroll_by_with_snap(
        &mut self,
        delta: ScrollDelta,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
        snap_x: bool,
        snap_y: bool,
    ) -> ScrollOutcome {
        let target = ScrollOffset::new(
            apply_delta(self.x.target, delta.x),
            apply_delta(self.y.target, delta.y),
        );
        self.scroll_to_with_snap(target, viewport, content, settings, snap_x, snap_y)
    }

    pub fn scroll_to(
        &mut self,
        offset: ScrollOffset,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        self.scroll_to_with_snap(offset, viewport, content, settings, false, false)
    }

    fn scroll_to_with_snap(
        &mut self,
        offset: ScrollOffset,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
        snap_x: bool,
        snap_y: bool,
    ) -> ScrollOutcome {
        let max = max_offset(viewport, content);
        let spec = settings.resolve(self.behavior.animation);
        let before = self.offset();
        let mut handled = false;
        let mut changed = false;

        if self.axes.horizontal() {
            handled = true;
            let target = offset.x.min(max.x);
            changed |= if snap_x {
                self.x.snap_to(target)
            } else {
                self.x.start_to(target, spec)
            };
        } else {
            changed |= self.x.snap_to(0);
        }

        if self.axes.vertical() {
            handled = true;
            let target = offset.y.min(max.y);
            changed |= if snap_y {
                self.y.snap_to(target)
            } else {
                self.y.start_to(target, spec)
            };
        } else {
            changed |= self.y.snap_to(0);
        }

        changed |= before != self.offset();
        ScrollOutcome {
            handled,
            changed,
            active: self.is_active(),
        }
    }

    pub fn on_key(
        &mut self,
        key: impl Into<KeyEvent>,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let key = key.into();
        let keybindings = keybindings();
        if self.axes.vertical() && keybindings.top_prefix_matches(key) {
            if self.pending_top_prefix {
                self.pending_top_prefix = false;
                return self.scroll_to(
                    ScrollOffset::new(self.x.target, 0),
                    viewport,
                    content,
                    settings,
                );
            }
            self.pending_top_prefix = true;
            return ScrollOutcome {
                handled: true,
                changed: false,
                active: self.is_active(),
            };
        }

        self.pending_top_prefix = false;
        if self.axes.vertical() && keybindings.bottom_matches(key) {
            return self.scroll_to(
                ScrollOffset::new(self.x.target, max_offset(viewport, content).y),
                viewport,
                content,
                settings,
            );
        }

        if self.axes.vertical() && keybindings.page_up_matches(key) {
            return self.scroll_by(
                ScrollDelta::new(0, -(self.behavior.page_step(viewport.height) as isize)),
                viewport,
                content,
                settings,
            );
        }
        if self.axes.vertical() && keybindings.page_down_matches(key) {
            return self.scroll_by(
                ScrollDelta::new(0, self.behavior.page_step(viewport.height) as isize),
                viewport,
                content,
                settings,
            );
        }

        if self.axes.vertical() && keybindings.line_up_matches(key) {
            self.scroll_by_with_snap(
                ScrollDelta::new(0, -(self.behavior.line_step() as isize)),
                viewport,
                content,
                settings,
                false,
                true,
            )
        } else if self.axes.vertical() && keybindings.line_down_matches(key) {
            self.scroll_by_with_snap(
                ScrollDelta::new(0, self.behavior.line_step() as isize),
                viewport,
                content,
                settings,
                false,
                true,
            )
        } else if self.axes.vertical() && keybindings.home_matches(key) {
            self.scroll_to(
                ScrollOffset::new(self.x.target, 0),
                viewport,
                content,
                settings,
            )
        } else if self.axes.vertical() && keybindings.end_matches(key) {
            self.scroll_to(
                ScrollOffset::new(self.x.target, max_offset(viewport, content).y),
                viewport,
                content,
                settings,
            )
        } else if let Some(delta) =
            horizontal_jump(&keybindings, key).filter(|_| self.axes.horizontal())
        {
            self.scroll_by_with_snap(
                ScrollDelta::new(delta, 0),
                viewport,
                content,
                settings,
                true,
                false,
            )
        } else if self.axes.horizontal() && keybindings.line_left_matches(key) {
            self.scroll_by_with_snap(
                ScrollDelta::new(-(self.behavior.line_step() as isize), 0),
                viewport,
                content,
                settings,
                true,
                false,
            )
        } else if self.axes.horizontal() && keybindings.line_right_matches(key) {
            self.scroll_by_with_snap(
                ScrollDelta::new(self.behavior.line_step() as isize, 0),
                viewport,
                content,
                settings,
                true,
                false,
            )
        } else {
            ScrollOutcome::idle()
        }
    }

    pub fn clamp_to(
        &mut self,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let max = max_offset(viewport, content);
        let spec = settings.resolve(self.behavior.animation);
        let before = self.offset();
        let target = ScrollOffset::new(
            if self.axes.horizontal() {
                self.x.target.min(max.x)
            } else {
                0
            },
            if self.axes.vertical() {
                self.y.target.min(max.y)
            } else {
                0
            },
        );
        let current = self.offset();
        let current_out = current.x > max.x
            || current.y > max.y
            || (!self.axes.horizontal() && current.x != 0)
            || (!self.axes.vertical() && current.y != 0);

        let mut changed = false;
        if current_out || !spec.enabled {
            changed |= self.x.snap_to(target.x);
            changed |= self.y.snap_to(target.y);
        } else {
            changed |= self.x.start_to(target.x, spec);
            changed |= self.y.start_to(target.y, spec);
        }

        changed |= before != self.offset();
        ScrollOutcome {
            handled: changed,
            changed,
            active: self.is_active(),
        }
    }

    pub fn snap_horizontal_to_start(&mut self) -> ScrollOutcome {
        let changed = self.x.snap_to(0);
        ScrollOutcome {
            handled: changed,
            changed,
            active: self.is_active(),
        }
    }

    pub fn layout(&self, area: Rect, content: ScrollSize) -> ScrollLayout {
        if area.is_empty() {
            return ScrollLayout {
                outer: area,
                viewport: area,
                vertical_bar: None,
                horizontal_bar: None,
                corner: None,
            };
        }

        let (show_v, show_h, viewport) = self.resolve_bars(area, content);
        let reserve = self.scrollbars.gutter == ScrollbarGutter::Reserve;
        let vertical_height = if reserve && show_h {
            area.height.saturating_sub(1)
        } else {
            area.height
        };
        let horizontal_width = if reserve && show_v {
            area.width.saturating_sub(1)
        } else {
            area.width
        };

        ScrollLayout {
            outer: area,
            viewport,
            vertical_bar: show_v.then_some(Rect::new(
                area.x + area.width.saturating_sub(1),
                area.y,
                1,
                vertical_height,
            )),
            horizontal_bar: show_h.then_some(Rect::new(
                area.x,
                area.y + area.height.saturating_sub(1),
                horizontal_width,
                1,
            )),
            corner: (reserve && show_v && show_h).then_some(Rect::new(
                area.x + area.width.saturating_sub(1),
                area.y + area.height.saturating_sub(1),
                1,
                1,
            )),
        }
    }

    pub fn geometry(&self, area: Rect, content: ScrollSize) -> ScrollGeometry {
        let layout = self.layout(area, content);
        ScrollGeometry {
            layout,
            viewport: ScrollSize::from_area(layout.viewport),
            content,
        }
    }

    pub fn render_scrollbars(
        &self,
        frame: &mut Frame,
        layout: ScrollLayout,
        content: ScrollSize,
        focused: bool,
    ) {
        let theme = theme();
        let track_style = Style::default().fg(theme.border_fg());
        let thumb_style = Style::default().fg(if focused {
            theme.accent_fg()
        } else {
            theme.muted_fg()
        });
        let offset = self.offset();
        let chars = self.scrollbars.style.chars();

        if let Some(area) = layout.vertical_bar {
            let viewport_height = layout.viewport.height as usize;
            let content_length = content
                .height
                .saturating_sub(viewport_height)
                .saturating_add(1);
            let mut state = ScrollbarState::new(content_length)
                .position(offset.y)
                .viewport_content_length(viewport_height);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_symbol(Some(chars.vertical_track))
                    .thumb_symbol(chars.vertical_thumb)
                    .track_style(track_style)
                    .thumb_style(thumb_style),
                area,
                &mut state,
            );
        }

        if let Some(area) = layout.horizontal_bar {
            let viewport_width = layout.viewport.width as usize;
            let content_length = content
                .width
                .saturating_sub(viewport_width)
                .saturating_add(1);
            let mut state = ScrollbarState::new(content_length)
                .position(offset.x)
                .viewport_content_length(viewport_width);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .track_symbol(Some(chars.horizontal_track))
                    .thumb_symbol(chars.horizontal_thumb)
                    .track_style(track_style)
                    .thumb_style(thumb_style),
                area,
                &mut state,
            );
        }

        if let Some(area) = layout.corner {
            frame.render_widget(Paragraph::new(" ").style(track_style), area);
        }
    }

    fn resolve_bars(&self, area: Rect, content: ScrollSize) -> (bool, bool, Rect) {
        let reserve = self.scrollbars.gutter == ScrollbarGutter::Reserve;
        let mut show_v = false;
        let mut show_h = false;
        let mut viewport = area;

        for _ in 0..3 {
            let width = if reserve && show_v {
                area.width.saturating_sub(1)
            } else {
                area.width
            };
            let height = if reserve && show_h {
                area.height.saturating_sub(1)
            } else {
                area.height
            };
            viewport = Rect::new(area.x, area.y, width, height);

            let next_v = self.bar_visible(
                self.scrollbars.vertical,
                self.axes.vertical(),
                content.height > viewport.height as usize,
            );
            let next_h = self.bar_visible(
                self.scrollbars.horizontal,
                self.axes.horizontal(),
                content.width > viewport.width as usize,
            );
            if next_v == show_v && next_h == show_h {
                break;
            }
            show_v = next_v;
            show_h = next_h;
        }

        if !reserve {
            viewport = area;
        } else {
            viewport.width = if show_v {
                area.width.saturating_sub(1)
            } else {
                area.width
            };
            viewport.height = if show_h {
                area.height.saturating_sub(1)
            } else {
                area.height
            };
        }

        (show_v, show_h, viewport)
    }

    fn bar_visible(
        &self,
        visibility: ScrollbarVisibility,
        axis_enabled: bool,
        overflow: bool,
    ) -> bool {
        axis_enabled
            && match visibility {
                ScrollbarVisibility::Auto => overflow,
                ScrollbarVisibility::Always => true,
                ScrollbarVisibility::Never => false,
            }
    }
}

impl Animated for ScrollState {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let spec = settings.resolve(self.behavior.animation);
        self.x
            .tick(dt, settings, spec)
            .merge(self.y.tick(dt, settings, spec))
    }
}

fn horizontal_jump(keybindings: &KeyBindings, key: KeyEvent) -> Option<isize> {
    let plain_control = key.modifiers.contains(KeyModifiers::CONTROL)
        && !key
            .modifiers
            .intersects(KeyModifiers::SHIFT | KeyModifiers::ALT)
        && matches!(key.code, Key::Char(_));
    if !plain_control {
        return None;
    }

    let base_key = uncontrol_key(key);
    if keybindings.line_left_matches(base_key) {
        Some(-HORIZONTAL_JUMP)
    } else if keybindings.line_right_matches(base_key) {
        Some(HORIZONTAL_JUMP)
    } else {
        None
    }
}

fn uncontrol_key(mut key: KeyEvent) -> KeyEvent {
    key.modifiers.remove(KeyModifiers::CONTROL);
    if let Key::Char(c) = key.code {
        key.code = Key::Char(c.to_ascii_lowercase());
    }
    key
}

impl Default for AxisScroll {
    fn default() -> Self {
        Self {
            target: 0,
            animator: ScrollAnimator::new(0.0),
        }
    }
}

impl AxisScroll {
    fn offset(&self) -> usize {
        self.animator.current().max(0.0).round() as usize
    }

    fn is_active(&self) -> bool {
        self.animator.is_active() || (self.animator.current() - self.target as f64).abs() >= 0.5
    }

    fn start_to(&mut self, target: usize, spec: ResolvedAnimationSpec) -> bool {
        let changed = self.target != target;
        if !changed {
            return false;
        }

        self.target = target;

        if !spec.enabled || spec.duration.is_zero() {
            self.animator.snap_to(target as f64);
            return true;
        }

        self.animator
            .animate_to(target as f64, spec.duration, spec.easing);
        true
    }

    fn snap_to(&mut self, target: usize) -> bool {
        let changed = self.target != target || self.offset() != target || self.is_active();
        self.target = target;
        self.animator.snap_to(target as f64);
        changed
    }

    fn tick(
        &mut self,
        dt: Duration,
        settings: AnimationSettings,
        spec: ResolvedAnimationSpec,
    ) -> TickResult {
        let before_offset = self.offset();

        if !settings.enabled || !spec.enabled {
            let changed = self.snap_to(self.target);
            return TickResult {
                changed,
                active: false,
                next_tick: None,
            };
        }

        let _tick = self.animator.tick(dt, settings);
        TickResult {
            changed: before_offset != self.offset(),
            active: self.is_active(),
            next_tick: None,
        }
    }
}

impl ScrollOffset {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl ScrollSize {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub fn from_area(area: Rect) -> Self {
        Self {
            width: area.width as usize,
            height: area.height as usize,
        }
    }
}

impl ScrollDelta {
    pub fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }
}

impl ScrollAxes {
    pub fn vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }

    pub fn horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
}

impl Default for ScrollAxes {
    fn default() -> Self {
        Self::Vertical
    }
}

impl Default for ScrollBehavior {
    fn default() -> Self {
        Self {
            line_step: 1,
            page_overlap: 1,
            animation: AnimationSpec::default(),
        }
    }
}

impl ScrollBehavior {
    fn line_step(self) -> usize {
        self.line_step.max(1)
    }

    fn page_step(self, viewport: usize) -> usize {
        viewport.saturating_sub(self.page_overlap).max(1)
    }
}

impl ScrollOutcome {
    pub fn needs_redraw(self) -> bool {
        self.changed || self.active
    }

    pub fn idle() -> Self {
        Self {
            handled: false,
            changed: false,
            active: false,
        }
    }
}

impl Default for ScrollbarConfig {
    fn default() -> Self {
        Self {
            vertical: ScrollbarVisibility::Auto,
            horizontal: ScrollbarVisibility::Auto,
            gutter: ScrollbarGutter::Reserve,
            style: ScrollbarStyle::ThinTrack,
        }
    }
}

impl ScrollbarStyle {
    fn chars(self) -> ScrollbarChars {
        match self {
            Self::ThinTrack => ScrollbarChars {
                vertical_track: "│",
                vertical_thumb: "┃",
                horizontal_track: "─",
                horizontal_thumb: "━",
            },
            Self::ThickTrack => ScrollbarChars {
                vertical_track: "┃",
                vertical_thumb: "┃",
                horizontal_track: "━",
                horizontal_thumb: "━",
            },
        }
    }
}

struct ScrollbarChars {
    vertical_track: &'static str,
    vertical_thumb: &'static str,
    horizontal_track: &'static str,
    horizontal_thumb: &'static str,
}

impl Default for ScrollPreset {
    fn default() -> Self {
        Self {
            line_step: 1,
            page_overlap: 1,
            vertical_scrollbar: ScrollbarVisibility::Auto,
            horizontal_scrollbar: ScrollbarVisibility::Auto,
            gutter: ScrollbarGutter::Reserve,
            style: ScrollbarStyle::ThinTrack,
        }
    }
}

pub fn text_size(text: &Text<'_>) -> ScrollSize {
    ScrollSize::new(text.width(), text.lines.len())
}

pub fn line_width(line: &Line<'_>) -> usize {
    line.width()
}

pub fn paragraph_scroll(offset: ScrollOffset) -> (u16, u16) {
    (saturating_u16(offset.y), saturating_u16(offset.x))
}

fn max_offset(viewport: ScrollSize, content: ScrollSize) -> ScrollOffset {
    ScrollOffset::new(
        content.width.saturating_sub(viewport.width),
        content.height.saturating_sub(viewport.height),
    )
}

fn apply_delta(value: usize, delta: isize) -> usize {
    if delta.is_negative() {
        value.saturating_sub(delta.unsigned_abs())
    } else {
        value.saturating_add(delta as usize)
    }
}

fn saturating_u16(value: usize) -> u16 {
    value.min(u16::MAX as usize) as u16
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::Key;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    use super::*;

    #[test]
    fn scroll_to_clamps_and_disabled_animation_snaps() {
        let mut scroll = ScrollState::new(ScrollAxes::Both).behavior(ScrollBehavior {
            line_step: 1,
            page_overlap: 1,
            animation: AnimationSpec {
                enabled: None,
                duration: Some(Duration::from_millis(100)),
                easing: None,
            },
        });
        let mut settings = AnimationSettings::default();
        settings.enabled = false;

        let outcome = scroll.scroll_to(
            ScrollOffset::new(99, 99),
            ScrollSize::new(5, 4),
            ScrollSize::new(8, 10),
            settings,
        );

        assert!(outcome.handled);
        assert!(outcome.changed);
        assert!(!outcome.active);
        assert_eq!(scroll.offset(), ScrollOffset::new(3, 6));
        assert_eq!(scroll.target_offset(), ScrollOffset::new(3, 6));
    }

    #[test]
    fn reserve_layout_keeps_corner_when_both_axes_overflow() {
        let scroll = ScrollState::new(ScrollAxes::Both).scrollbars(ScrollbarConfig {
            vertical: ScrollbarVisibility::Auto,
            horizontal: ScrollbarVisibility::Auto,
            gutter: ScrollbarGutter::Reserve,
            style: ScrollbarStyle::ThinTrack,
        });

        let layout = scroll.layout(Rect::new(2, 3, 10, 5), ScrollSize::new(20, 10));

        assert_eq!(layout.viewport, Rect::new(2, 3, 9, 4));
        assert_eq!(layout.vertical_bar, Some(Rect::new(11, 3, 1, 4)));
        assert_eq!(layout.horizontal_bar, Some(Rect::new(2, 7, 9, 1)));
        assert_eq!(layout.corner, Some(Rect::new(11, 7, 1, 1)));
    }

    #[test]
    fn horizontal_arrow_scroll_updates_offset_immediately() {
        let mut scroll = ScrollState::new(ScrollAxes::Horizontal).behavior(ScrollBehavior {
            line_step: 1,
            page_overlap: 1,
            animation: AnimationSpec {
                enabled: None,
                duration: Some(Duration::from_millis(150)),
                easing: None,
            },
        });

        let outcome = scroll.on_key(
            KeyEvent::from(Key::Right),
            ScrollSize::new(5, 1),
            ScrollSize::new(20, 1),
            AnimationSettings::default(),
        );

        assert!(outcome.handled);
        assert!(outcome.changed);
        assert_eq!(scroll.offset().x, 1);
        assert_eq!(scroll.target_offset().x, 1);
        assert!(!scroll.is_active());
    }

    #[test]
    fn controlled_horizontal_vim_keys_jump_eight_columns() {
        let mut scroll = ScrollState::new(ScrollAxes::Horizontal);
        let mut settings = AnimationSettings::default();
        settings.enabled = false;

        let right = scroll.on_key(
            KeyEvent {
                code: Key::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            },
            ScrollSize::new(5, 1),
            ScrollSize::new(20, 1),
            settings,
        );

        assert!(right.handled);
        assert!(right.changed);
        assert_eq!(scroll.offset().x, 8);

        let left = scroll.on_key(
            KeyEvent {
                code: Key::Char('h'),
                modifiers: KeyModifiers::CONTROL,
            },
            ScrollSize::new(5, 1),
            ScrollSize::new(20, 1),
            settings,
        );

        assert!(left.handled);
        assert!(left.changed);
        assert_eq!(scroll.offset().x, 0);
    }

    #[test]
    fn vim_top_and_bottom_keys_match_home_and_end_scrolling() {
        let mut scroll = ScrollState::new(ScrollAxes::Vertical);
        let mut settings = AnimationSettings::default();
        settings.enabled = false;

        let bottom = scroll.on_key(
            KeyEvent::from(Key::Char('G')),
            ScrollSize::new(1, 5),
            ScrollSize::new(1, 20),
            settings,
        );

        assert!(bottom.handled);
        assert!(bottom.changed);
        assert_eq!(scroll.offset().y, 15);

        let prefix = scroll.on_key(
            KeyEvent::from(Key::Char('g')),
            ScrollSize::new(1, 5),
            ScrollSize::new(1, 20),
            settings,
        );
        let top = scroll.on_key(
            KeyEvent::from(Key::Char('g')),
            ScrollSize::new(1, 5),
            ScrollSize::new(1, 20),
            settings,
        );

        assert!(prefix.handled);
        assert!(!prefix.changed);
        assert!(top.handled);
        assert!(top.changed);
        assert_eq!(scroll.offset().y, 0);
    }

    #[test]
    fn scroll_animation_uses_configured_duration_and_easing() {
        let mut scroll = ScrollState::new(ScrollAxes::Vertical).behavior(ScrollBehavior {
            line_step: 1,
            page_overlap: 1,
            animation: AnimationSpec {
                enabled: None,
                duration: Some(Duration::from_millis(100)),
                easing: Some(crate::Easing::Linear),
            },
        });

        let outcome = scroll.scroll_to(
            ScrollOffset::new(0, 10),
            ScrollSize::new(1, 5),
            ScrollSize::new(1, 20),
            AnimationSettings::default(),
        );
        let tick = scroll.tick(Duration::from_millis(50), AnimationSettings::default());

        assert!(outcome.active);
        assert!(tick.active);
        assert_eq!(scroll.offset().y, 5);

        scroll.tick(Duration::from_millis(50), AnimationSettings::default());

        assert_eq!(scroll.offset().y, 10);
        assert!(!scroll.is_active());
    }

    #[test]
    fn horizontal_snap_to_start_clears_target_and_animation() {
        let mut scroll = ScrollState::new(ScrollAxes::Both).behavior(ScrollBehavior {
            line_step: 1,
            page_overlap: 1,
            animation: AnimationSpec {
                enabled: None,
                duration: Some(Duration::from_millis(100)),
                easing: Some(crate::Easing::Linear),
            },
        });

        scroll.scroll_to(
            ScrollOffset::new(10, 4),
            ScrollSize::new(5, 5),
            ScrollSize::new(20, 20),
            AnimationSettings::default(),
        );
        scroll.tick(Duration::from_millis(50), AnimationSettings::default());

        let outcome = scroll.snap_horizontal_to_start();

        assert!(outcome.handled);
        assert!(outcome.changed);
        assert_eq!(scroll.offset(), ScrollOffset::new(0, 2));
        assert_eq!(scroll.target_offset(), ScrollOffset::new(0, 4));
    }

    #[test]
    fn thin_track_scrollbar_uses_tira_style_glyphs() {
        let scroll = ScrollState::new(ScrollAxes::Both).scrollbars(ScrollbarConfig {
            vertical: ScrollbarVisibility::Always,
            horizontal: ScrollbarVisibility::Always,
            gutter: ScrollbarGutter::Reserve,
            style: ScrollbarStyle::ThinTrack,
        });
        let layout = scroll.layout(Rect::new(0, 0, 6, 4), ScrollSize::new(20, 10));
        let mut terminal = Terminal::new(TestBackend::new(6, 4)).expect("terminal should build");

        terminal
            .draw(|frame| scroll.render_scrollbars(frame, layout, ScrollSize::new(20, 10), true))
            .expect("scrollbar should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((5, 0)).unwrap().symbol(), "┃");
        assert_eq!(buffer.cell((4, 3)).unwrap().symbol(), "─");
    }
}
