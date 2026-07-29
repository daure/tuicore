use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::Style,
    text::Line,
    widgets::Paragraph,
};
use time::{Date, Month, OffsetDateTime};

use crate::{
    Animated, AnimationSettings, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint,
    RenderCtx, TickResult, TuiNode, theme,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SeasonalGlyphs {
    #[default]
    NerdFont,
    Unicode,
}

#[derive(Debug, Clone)]
pub struct SeasonalEmptyState {
    message: String,
    date: Date,
    glyphs: SeasonalGlyphs,
}

impl SeasonalEmptyState {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            date: current_date(),
            glyphs: SeasonalGlyphs::default(),
        }
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    pub fn glyphs(mut self, glyphs: SeasonalGlyphs) -> Self {
        self.glyphs = glyphs;
        self
    }

    pub fn date(mut self, date: Date) -> Self {
        self.date = date;
        self
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    pub fn set_glyphs(&mut self, glyphs: SeasonalGlyphs) {
        self.glyphs = glyphs;
    }

    pub fn set_date(&mut self, date: Date) -> bool {
        if self.date == date {
            return false;
        }
        self.date = date;
        true
    }

    fn lines(&self) -> [Line<'_>; 3] {
        [
            Line::from(self.message.as_str()),
            Line::default(),
            Line::from(ornament(self.date, self.glyphs)),
        ]
    }

    fn sync_current_date(&mut self) -> bool {
        self.set_date(current_date())
    }

    pub(crate) fn render_state(&self, frame: &mut Frame, area: Rect) {
        if area.is_empty() {
            return;
        }
        let height = area.height.min(3);
        let centered = Rect::new(
            area.x,
            area.y + area.height.saturating_sub(height) / 2,
            area.width,
            height,
        );
        frame.render_widget(
            Paragraph::new(self.lines().to_vec())
                .style(Style::default().fg(theme().subtle_fg()))
                .alignment(Alignment::Center),
            centered,
        );
    }
}

impl Animated for SeasonalEmptyState {
    fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
        let changed = self.sync_current_date();
        let result = if changed {
            TickResult::CHANGED
        } else {
            TickResult::IDLE
        };
        result.merge(TickResult::scheduled_after(REFRESH_INTERVAL))
    }
}

impl<M> TuiNode<M> for SeasonalEmptyState {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = self
            .lines()
            .iter()
            .map(Line::width)
            .max()
            .unwrap_or_default()
            .min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(width, 3).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, _ctx: &mut RenderCtx<'a>) {
        self.render_state(frame, area);
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(self, dt, settings)
    }
}

fn current_date() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

fn ornament(date: Date, glyphs: SeasonalGlyphs) -> &'static str {
    match (date.month(), glyphs) {
        (Month::March | Month::April | Month::May, SeasonalGlyphs::NerdFont) => "╶┄ ❧ · 󰧱 · ☙ ┄╴",
        (Month::June | Month::July | Month::August, SeasonalGlyphs::NerdFont) => "╶━ ✦ ·  · ✦ ━╴",
        (Month::September | Month::October | Month::November, SeasonalGlyphs::NerdFont) => {
            "╶─ ☙ · 󰲓 · ❧ ─╴"
        }
        (Month::December | Month::January | Month::February, SeasonalGlyphs::NerdFont) => {
            "╶┄ ✧ ·  · ✧ ┄╴"
        }
        (Month::March | Month::April | Month::May, SeasonalGlyphs::Unicode) => "╶┄ ❧ · ❀ · ☙ ┄╴",
        (Month::June | Month::July | Month::August, SeasonalGlyphs::Unicode) => "╶━ ✦ · ☼ · ✦ ━╴",
        (Month::September | Month::October | Month::November, SeasonalGlyphs::Unicode) => {
            "╶─ ☙ · ❧ · ❧ ─╴"
        }
        (Month::December | Month::January | Month::February, SeasonalGlyphs::Unicode) => {
            "╶┄ ✧ · ❄ · ✧ ┄╴"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn date(year: i32, month: Month, day: u8) -> Date {
        Date::from_calendar_date(year, month, day).unwrap()
    }

    #[test]
    fn season_boundaries_select_nerd_font_ornaments() {
        for (date, expected) in [
            (date(2026, Month::March, 1), "╶┄ ❧ · 󰧱 · ☙ ┄╴"),
            (date(2026, Month::May, 31), "╶┄ ❧ · 󰧱 · ☙ ┄╴"),
            (date(2026, Month::June, 1), "╶━ ✦ ·  · ✦ ━╴"),
            (date(2026, Month::August, 31), "╶━ ✦ ·  · ✦ ━╴"),
            (date(2026, Month::September, 1), "╶─ ☙ · 󰲓 · ❧ ─╴"),
            (date(2026, Month::November, 30), "╶─ ☙ · 󰲓 · ❧ ─╴"),
            (date(2026, Month::December, 1), "╶┄ ✧ ·  · ✧ ┄╴"),
            (date(2027, Month::February, 28), "╶┄ ✧ ·  · ✧ ┄╴"),
        ] {
            assert_eq!(ornament(date, SeasonalGlyphs::NerdFont), expected);
        }
    }

    #[test]
    fn unicode_fallback_maps_each_season() {
        for (date, expected) in [
            (date(2026, Month::March, 1), "╶┄ ❧ · ❀ · ☙ ┄╴"),
            (date(2026, Month::June, 1), "╶━ ✦ · ☼ · ✦ ━╴"),
            (date(2026, Month::September, 1), "╶─ ☙ · ❧ · ❧ ─╴"),
            (date(2026, Month::December, 1), "╶┄ ✧ · ❄ · ✧ ┄╴"),
        ] {
            assert_eq!(ornament(date, SeasonalGlyphs::Unicode), expected);
        }
    }

    #[test]
    fn renders_any_message_centered_above_ornament() {
        let state = SeasonalEmptyState::new("Nothing matched")
            .date(date(2026, Month::December, 1))
            .glyphs(SeasonalGlyphs::NerdFont);
        let mut terminal = Terminal::new(TestBackend::new(40, 7)).unwrap();
        terminal
            .draw(|frame| {
                <SeasonalEmptyState as TuiNode<()>>::render(
                    &state,
                    frame,
                    frame.area(),
                    &mut RenderCtx::new(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let line = |y| {
            (0..40)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        };

        assert_eq!(line(2), "             Nothing matched            ");
        assert!(line(3).trim().is_empty());
        assert_eq!(line(4).trim(), "╶┄ ✧ ·  · ✧ ┄╴");
        assert_eq!(buffer.cell((13, 2)).unwrap().fg, theme().subtle_fg());
    }
}
