use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::{Animated, AnimationSettings, LayoutCtx, LayoutResult, TickResult, TuiNode, theme};

/// Braille dot frames for the loading spinner, cycling clockwise.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Wall-clock time each spinner frame is shown.
const SPINNER_FRAME_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug, Clone)]
pub struct Spinner {
    frame: usize,
    elapsed: Duration,
    style: Option<Style>,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frame: 0,
            elapsed: Duration::ZERO,
            style: None,
        }
    }

    /// Set the Style of the spinner.
    pub fn style(mut self, style: Style) -> Self {
        self.style = Some(style);
        self
    }

    /// Get the current frame string.
    pub fn glyph(&self) -> &'static str {
        SPINNER_FRAMES[self.frame % SPINNER_FRAMES.len()]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }

        let glyph = self.glyph();
        let style = self
            .style
            .unwrap_or_else(|| Style::default().fg(theme().accent_fg()));
        let span = Span::styled(glyph, style);
        let paragraph = Paragraph::new(span).alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }
}

impl Animated for Spinner {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        if !settings.enabled {
            return TickResult::IDLE;
        }

        self.elapsed += dt;
        let mut changed = false;
        while self.elapsed >= SPINNER_FRAME_INTERVAL {
            self.elapsed -= SPINNER_FRAME_INTERVAL;
            self.frame = self.frame.wrapping_add(1);
            changed = true;
        }

        TickResult {
            changed,
            active: true,
        }
    }
}

impl<M> TuiNode<M> for Spinner {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_ticks_and_advances_frames() {
        let mut spinner = Spinner::new();
        let settings = AnimationSettings::default();

        assert_eq!(spinner.glyph(), "⠋");

        // Tick less than interval -> no change
        let result = Animated::tick(&mut spinner, Duration::from_millis(50), settings);
        assert!(!result.changed);
        assert!(result.active);
        assert_eq!(spinner.glyph(), "⠋");

        // Tick more -> advances frame
        let result = Animated::tick(&mut spinner, Duration::from_millis(60), settings);
        assert!(result.changed);
        assert!(result.active);
        assert_eq!(spinner.glyph(), "⠙");
    }

    #[test]
    fn spinner_stays_idle_when_animations_disabled() {
        let mut spinner = Spinner::new();
        let mut settings = AnimationSettings::default();
        settings.enabled = false;

        assert_eq!(spinner.glyph(), "⠋");

        let result = Animated::tick(&mut spinner, Duration::from_millis(150), settings);
        assert!(!result.changed);
        assert!(!result.active);
        assert_eq!(spinner.glyph(), "⠋");
    }
}
