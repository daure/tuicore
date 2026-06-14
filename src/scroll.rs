use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use tuirealm::event::{Key, KeyEvent};

use crate::animation::{
    Animated, AnimationSettings, AnimationSpec, Easing, ResolvedAnimationSpec, TickResult, Tween,
};
use crate::{theme, ui::keybindings};

#[derive(Debug, Clone)]
pub struct ScrollState {
    x: AxisScroll,
    y: AxisScroll,
    axes: ScrollAxes,
    behavior: ScrollBehavior,
    scrollbars: ScrollbarConfig,
}

#[derive(Debug, Clone)]
struct AxisScroll {
    target: usize,
    tween: Tween,
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
pub struct ScrollPreset {
    pub line_step: usize,
    pub page_overlap: usize,
    pub vertical_scrollbar: ScrollbarVisibility,
    pub horizontal_scrollbar: ScrollbarVisibility,
    pub gutter: ScrollbarGutter,
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
        let target = ScrollOffset::new(
            apply_delta(self.x.target, delta.x),
            apply_delta(self.y.target, delta.y),
        );
        self.scroll_to(target, viewport, content, settings)
    }

    pub fn scroll_to(
        &mut self,
        offset: ScrollOffset,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let max = max_offset(viewport, content);
        let spec = settings.resolve(self.behavior.animation);
        let before = self.offset();
        let mut handled = false;
        let mut changed = false;

        if self.axes.horizontal() {
            handled = true;
            changed |= self.x.start_to(offset.x.min(max.x), spec);
        } else {
            changed |= self.x.snap_to(0);
        }

        if self.axes.vertical() {
            handled = true;
            changed |= self.y.start_to(offset.y.min(max.y), spec);
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
        key: KeyEvent,
        viewport: ScrollSize,
        content: ScrollSize,
        settings: AnimationSettings,
    ) -> ScrollOutcome {
        let keybindings = keybindings();
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

        match key.code {
            Key::Up if self.axes.vertical() => self.scroll_by(
                ScrollDelta::new(0, -(self.behavior.line_step() as isize)),
                viewport,
                content,
                settings,
            ),
            Key::Down if self.axes.vertical() => self.scroll_by(
                ScrollDelta::new(0, self.behavior.line_step() as isize),
                viewport,
                content,
                settings,
            ),
            Key::Home if self.axes.vertical() => self.scroll_to(
                ScrollOffset::new(self.x.target, 0),
                viewport,
                content,
                settings,
            ),
            Key::End if self.axes.vertical() => self.scroll_to(
                ScrollOffset::new(self.x.target, max_offset(viewport, content).y),
                viewport,
                content,
                settings,
            ),
            Key::Left if self.axes.horizontal() => self.scroll_by(
                ScrollDelta::new(-(self.behavior.line_step() as isize), 0),
                viewport,
                content,
                settings,
            ),
            Key::Right if self.axes.horizontal() => self.scroll_by(
                ScrollDelta::new(self.behavior.line_step() as isize, 0),
                viewport,
                content,
                settings,
            ),
            _ => ScrollOutcome::idle(),
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
        let style = Style::default().fg(if focused {
            theme.accent_fg()
        } else {
            theme.border_fg()
        });
        let offset = self.offset();

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
                Scrollbar::new(ScrollbarOrientation::VerticalRight).style(style),
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
                Scrollbar::new(ScrollbarOrientation::HorizontalBottom).style(style),
                area,
                &mut state,
            );
        }

        if let Some(area) = layout.corner {
            frame.render_widget(Paragraph::new(" ").style(style), area);
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

impl Default for AxisScroll {
    fn default() -> Self {
        Self {
            target: 0,
            tween: Tween::idle(0.0),
        }
    }
}

impl AxisScroll {
    fn offset(&self) -> usize {
        self.tween.value().max(0.0).round() as usize
    }

    fn is_active(&self) -> bool {
        self.tween.is_active()
    }

    fn start_to(&mut self, target: usize, spec: ResolvedAnimationSpec) -> bool {
        let changed = self.target != target;
        if !changed {
            return false;
        }

        self.target = target;
        let from = self.tween.value().max(0.0);

        if !spec.enabled || spec.duration.is_zero() {
            self.tween
                .start(target as f64, target as f64, Duration::ZERO, spec.easing);
            return true;
        }

        self.tween
            .start(from, target as f64, spec.duration, spec.easing);
        true
    }

    fn snap_to(&mut self, target: usize) -> bool {
        let changed = self.target != target || self.offset() != target || self.is_active();
        self.target = target;
        self.tween
            .start(target as f64, target as f64, Duration::ZERO, Easing::Linear);
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
            };
        }

        let _tick = self.tween.tick(dt, settings);
        TickResult {
            changed: before_offset != self.offset(),
            active: self.tween.is_active(),
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
        }
    }
}

impl Default for ScrollPreset {
    fn default() -> Self {
        Self {
            line_step: 1,
            page_overlap: 1,
            vertical_scrollbar: ScrollbarVisibility::Auto,
            horizontal_scrollbar: ScrollbarVisibility::Auto,
            gutter: ScrollbarGutter::Reserve,
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
        });

        let layout = scroll.layout(Rect::new(2, 3, 10, 5), ScrollSize::new(20, 10));

        assert_eq!(layout.viewport, Rect::new(2, 3, 9, 4));
        assert_eq!(layout.vertical_bar, Some(Rect::new(11, 3, 1, 4)));
        assert_eq!(layout.horizontal_bar, Some(Rect::new(2, 7, 9, 1)));
        assert_eq!(layout.corner, Some(Rect::new(11, 7, 1, 1)));
    }
}
