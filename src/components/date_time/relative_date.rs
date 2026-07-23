use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use time::{Date, OffsetDateTime, PrimitiveDateTime, UtcOffset, Weekday};

use crate::{
    AnimationSettings, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx,
    TickResult, TuiNode, line_width, theme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RelativeDateMode {
    #[default]
    Distance,
    CalendarWeek,
}

/// One-line date-time distance display.
///
/// Distance mode uses elapsed whole days through six days, then floor-rounded
/// whole weeks (seven through thirteen days are one week).
pub struct RelativeDate {
    target: OffsetDateTime,
    reference: Option<OffsetDateTime>,
    mode: RelativeDateMode,
    text: String,
}

impl RelativeDate {
    pub fn new(target: OffsetDateTime) -> Self {
        let reference = local_datetime();
        Self {
            target,
            reference: None,
            mode: RelativeDateMode::Distance,
            text: format_relative(reference, target, RelativeDateMode::Distance),
        }
    }

    pub fn reference(mut self, reference: OffsetDateTime) -> Self {
        self.set_reference(reference);
        self
    }

    pub fn mode(mut self, mode: RelativeDateMode) -> Self {
        self.set_mode(mode);
        self
    }

    pub fn set_target(&mut self, target: OffsetDateTime) {
        self.target = target;
        self.refresh(self.current_reference());
    }

    pub fn set_reference(&mut self, reference: OffsetDateTime) {
        self.reference = Some(reference);
        self.refresh(reference);
    }

    pub fn use_live_reference(&mut self) {
        self.reference = None;
        self.refresh(local_datetime());
    }

    pub fn set_mode(&mut self, mode: RelativeDateMode) {
        self.mode = mode;
        self.refresh(self.current_reference());
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    fn current_reference(&self) -> OffsetDateTime {
        self.reference.unwrap_or_else(local_datetime)
    }

    fn refresh(&mut self, reference: OffsetDateTime) -> bool {
        let next = format_relative(reference, self.target, self.mode);
        if next == self.text {
            false
        } else {
            self.text = next;
            true
        }
    }
}

impl<M> TuiNode<M> for RelativeDate {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let width = line_width(&self.text.clone().into()).min(u16::MAX as usize) as u16;
        LayoutSizeHint::content(width, 1).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        frame.render_widget(
            Paragraph::new(self.text.as_str()).style(Style::default().fg(theme().text_fg())),
            area,
        );
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        if self.reference.is_none() {
            ctx.request_tick();
        }
    }

    fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
        if self.reference.is_some() {
            return TickResult::IDLE;
        }

        let reference = local_datetime();
        let changed = self.refresh(reference);
        TickResult {
            changed,
            layout: changed,
            ..TickResult::scheduled_after(next_update_wait(reference, self.target))
        }
    }
}

fn local_datetime() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}

fn format_relative(
    reference: OffsetDateTime,
    target: OffsetDateTime,
    mode: RelativeDateMode,
) -> String {
    if reference == target {
        return "Now".to_string();
    }

    let target = target.to_offset(reference.offset());
    let day_delta = (target.date() - reference.date()).whole_days();
    if day_delta == 0 {
        return format_same_date(reference, target);
    }
    if day_delta == -1 {
        return "Yesterday".to_string();
    }
    if day_delta == 1 {
        return "Tomorrow".to_string();
    }

    match mode {
        RelativeDateMode::Distance => format_distance_days(day_delta),
        RelativeDateMode::CalendarWeek => format_calendar_week(reference.date(), target.date()),
    }
}

fn format_same_date(reference: OffsetDateTime, target: OffsetDateTime) -> String {
    let seconds = (target - reference).whole_seconds();
    let future = target > reference;
    let elapsed = seconds.unsigned_abs();
    if elapsed < 60 {
        return if future {
            "In a moment"
        } else {
            "A moment ago"
        }
        .to_string();
    }
    if elapsed < 3_600 {
        return quantity(elapsed / 60, "minute", future);
    }
    quantity(elapsed / 3_600, "hour", future)
}

fn format_distance_days(day_delta: i64) -> String {
    let future = day_delta > 0;
    let days = day_delta.unsigned_abs();
    if days < 7 {
        quantity(days, "day", future)
    } else {
        quantity(days / 7, "week", future)
    }
}

fn format_calendar_week(reference: Date, target: Date) -> String {
    let week_delta = (monday_of(target) - monday_of(reference)).whole_days() / 7;
    match week_delta {
        0 => format!("This {}", weekday_name(target.weekday())),
        1 => format!("Next {}", weekday_name(target.weekday())),
        -1 => format!("Last {}", weekday_name(target.weekday())),
        _ => quantity(week_delta.unsigned_abs(), "week", week_delta > 0),
    }
}

fn monday_of(date: Date) -> Date {
    date - time::Duration::days(i64::from(date.weekday().number_days_from_monday()))
}

fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

fn quantity(value: u64, unit: &str, future: bool) -> String {
    let suffix = if value == 1 { "" } else { "s" };
    if future {
        format!("In {value} {unit}{suffix}")
    } else {
        format!("{value} {unit}{suffix} ago")
    }
}

fn next_update_wait(reference: OffsetDateTime, target: OffsetDateTime) -> Duration {
    next_update_wait_with_offset(reference, target, |candidate| {
        UtcOffset::local_offset_at(candidate).ok()
    })
}

fn next_update_wait_with_offset(
    reference: OffsetDateTime,
    target: OffsetDateTime,
    offset_at: impl FnMut(OffsetDateTime) -> Option<UtcOffset>,
) -> Duration {
    let target = target.to_offset(reference.offset());
    let until_midnight = resolve_next_midnight(reference, offset_at)
        .map(|midnight| midnight - reference)
        .and_then(|duration| duration.try_into().ok())
        .unwrap_or(Duration::from_secs(60));
    if reference.date() != target.date() {
        return until_midnight.max(Duration::from_millis(1));
    }

    let delta = target - reference;
    let elapsed = delta.unsigned_abs();
    let whole_seconds = delta.whole_seconds().unsigned_abs();
    let boundary = if target == reference {
        Duration::from_millis(1)
    } else if target > reference {
        if whole_seconds < 60 {
            elapsed
        } else {
            let unit = if whole_seconds < 3_600 { 60 } else { 3_600 };
            elapsed.saturating_sub(Duration::from_secs(whole_seconds / unit * unit))
                + Duration::from_millis(1)
        }
    } else {
        let unit = if whole_seconds < 60 {
            60
        } else if whole_seconds < 3_600 {
            60
        } else {
            3_600
        };
        Duration::from_secs((whole_seconds / unit + 1) * unit).saturating_sub(elapsed)
    };
    boundary
        .max(Duration::from_millis(1))
        .min(until_midnight)
        .max(Duration::from_millis(1))
}

fn resolve_next_midnight(
    reference: OffsetDateTime,
    mut offset_at: impl FnMut(OffsetDateTime) -> Option<UtcOffset>,
) -> Option<OffsetDateTime> {
    let local_midnight = PrimitiveDateTime::new(reference.date().next_day()?, time::Time::MIDNIGHT);
    let mut midnight = local_midnight.assume_offset(reference.offset());

    for _ in 0..2 {
        let Some(offset) = offset_at(midnight) else {
            break;
        };
        let resolved = local_midnight.assume_offset(offset);
        if resolved == midnight {
            break;
        }
        midnight = resolved;
    }

    (midnight > reference).then_some(midnight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::{Date, Month, Time, UtcOffset};

    fn at(year: i32, month: Month, day: u8, hour: u8, minute: u8, second: u8) -> OffsetDateTime {
        Date::from_calendar_date(year, month, day)
            .unwrap()
            .with_time(Time::from_hms(hour, minute, second).unwrap())
            .assume_offset(UtcOffset::UTC)
    }

    fn text(reference: OffsetDateTime, target: OffsetDateTime, mode: RelativeDateMode) -> String {
        RelativeDate::new(target)
            .reference(reference)
            .mode(mode)
            .text()
            .to_string()
    }

    #[test]
    fn exact_and_same_date_thresholds_use_truncated_units() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        let cases = [
            (0, "Now"),
            (-59, "A moment ago"),
            (59, "In a moment"),
            (-60, "1 minute ago"),
            (119, "In 1 minute"),
            (-3_599, "59 minutes ago"),
            (3_600, "In 1 hour"),
            (-7_200, "2 hours ago"),
        ];
        for (seconds, expected) in cases {
            assert_eq!(
                text(
                    reference,
                    reference + time::Duration::seconds(seconds),
                    RelativeDateMode::Distance
                ),
                expected
            );
        }
    }

    #[test]
    fn subsecond_targets_keep_direction_and_schedule_crossing() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        let half_second = time::Duration::milliseconds(500);

        assert_eq!(
            text(
                reference,
                reference + half_second,
                RelativeDateMode::Distance
            ),
            "In a moment"
        );
        assert_eq!(
            text(
                reference,
                reference - half_second,
                RelativeDateMode::Distance
            ),
            "A moment ago"
        );
        assert_eq!(
            next_update_wait(reference, reference + half_second),
            Duration::from_millis(500)
        );
        assert_eq!(
            next_update_wait(reference, reference - half_second),
            Duration::from_millis(59_500)
        );
        assert_eq!(
            next_update_wait(reference, reference),
            Duration::from_millis(1)
        );
        assert_eq!(
            text(
                reference + half_second,
                reference,
                RelativeDateMode::Distance
            ),
            "A moment ago"
        );
    }

    #[test]
    fn calendar_dates_take_precedence_over_elapsed_hours() {
        let reference = at(2026, Month::July, 20, 0, 10, 0);
        assert_eq!(
            text(
                reference,
                at(2026, Month::July, 19, 23, 50, 0),
                RelativeDateMode::Distance
            ),
            "Yesterday"
        );
        assert_eq!(
            text(
                reference,
                at(2026, Month::July, 21, 0, 5, 0),
                RelativeDateMode::Distance
            ),
            "Tomorrow"
        );
    }

    #[test]
    fn distance_mode_uses_days_then_floor_rounded_weeks() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        for (days, expected) in [
            (-2, "2 days ago"),
            (2, "In 2 days"),
            (6, "In 6 days"),
            (7, "In 1 week"),
            (8, "In 1 week"),
            (13, "In 1 week"),
            (14, "In 2 weeks"),
            (-21, "3 weeks ago"),
        ] {
            assert_eq!(
                text(
                    reference,
                    reference + time::Duration::days(days),
                    RelativeDateMode::Distance
                ),
                expected
            );
        }
    }

    #[test]
    fn calendar_week_labels_current_and_adjacent_weeks_from_monday() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        for (days, expected) in [
            (2, "This Wednesday"),
            (7, "Next Monday"),
            (13, "Next Sunday"),
            (14, "In 2 weeks"),
            (-4, "Last Thursday"),
            (-7, "Last Monday"),
            (-8, "2 weeks ago"),
        ] {
            assert_eq!(
                text(
                    reference,
                    reference + time::Duration::days(days),
                    RelativeDateMode::CalendarWeek
                ),
                expected
            );
        }
    }

    #[test]
    fn calendar_week_boundaries_work_from_midweek() {
        let reference = at(2026, Month::July, 22, 12, 0, 0);
        for (days, expected) in [
            (2, "This Friday"),
            (9, "Next Friday"),
            (12, "In 2 weeks"),
            (-2, "This Monday"),
            (-9, "Last Monday"),
            (-10, "2 weeks ago"),
        ] {
            assert_eq!(
                text(
                    reference,
                    reference + time::Duration::days(days),
                    RelativeDateMode::CalendarWeek
                ),
                expected
            );
        }
    }

    #[test]
    fn calendar_week_uses_this_for_same_week_dates_from_thursday_and_sunday() {
        let thursday = at(2026, Month::July, 23, 12, 0, 0);
        assert_eq!(
            text(
                thursday,
                at(2026, Month::July, 25, 12, 0, 0),
                RelativeDateMode::CalendarWeek
            ),
            "This Saturday"
        );
        assert_eq!(
            text(
                thursday,
                at(2026, Month::July, 20, 12, 0, 0),
                RelativeDateMode::CalendarWeek
            ),
            "This Monday"
        );

        let sunday = at(2026, Month::July, 26, 12, 0, 0);
        for (days, expected) in [
            (-6, "This Monday"),
            (2, "Next Tuesday"),
            (-7, "Last Sunday"),
        ] {
            assert_eq!(
                text(
                    sunday,
                    sunday + time::Duration::days(days),
                    RelativeDateMode::CalendarWeek
                ),
                expected
            );
        }
    }

    #[test]
    fn yesterday_and_tomorrow_outrank_calendar_week_labels() {
        let reference = at(2026, Month::July, 26, 12, 0, 0);
        assert_eq!(
            text(
                reference,
                reference - time::Duration::days(1),
                RelativeDateMode::CalendarWeek
            ),
            "Yesterday"
        );
        assert_eq!(
            text(
                reference,
                reference + time::Duration::days(1),
                RelativeDateMode::CalendarWeek
            ),
            "Tomorrow"
        );
    }

    #[test]
    fn calendar_week_offsets_cross_year_boundary() {
        let reference = at(2026, Month::December, 28, 12, 0, 0);
        assert_eq!(
            text(
                reference,
                at(2027, Month::January, 10, 12, 0, 0),
                RelativeDateMode::CalendarWeek
            ),
            "Next Sunday"
        );
        assert_eq!(
            text(
                reference,
                at(2027, Month::January, 11, 12, 0, 0),
                RelativeDateMode::CalendarWeek
            ),
            "In 2 weeks"
        );
    }

    #[test]
    fn fixed_reference_setters_refresh_without_wall_clock() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        let mut relative = RelativeDate::new(reference).reference(reference);
        relative.set_target(reference + time::Duration::days(8));
        assert_eq!(relative.text(), "In 1 week");
        relative.set_mode(RelativeDateMode::CalendarWeek);
        assert_eq!(relative.text(), "Next Tuesday");
    }

    #[test]
    fn scheduler_waits_for_moment_minute_hour_and_midnight_boundaries() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        for (seconds, expected) in [
            (30, Duration::from_secs(30)),
            (-30, Duration::from_secs(30)),
            (119, Duration::from_millis(59_001)),
            (-119, Duration::from_secs(1)),
            (7_199, Duration::from_millis(3_599_001)),
            (-7_199, Duration::from_secs(1)),
        ] {
            assert_eq!(
                next_update_wait_with_offset(
                    reference,
                    reference + time::Duration::seconds(seconds),
                    |_| None
                ),
                expected
            );
        }

        assert_eq!(
            next_update_wait_with_offset(reference, reference + time::Duration::days(1), |_| None),
            Duration::from_secs(12 * 60 * 60)
        );
    }

    #[test]
    fn next_midnight_uses_offset_at_that_local_time() {
        let before_spring_forward = Date::from_calendar_date(2026, Month::March, 8)
            .unwrap()
            .with_time(Time::from_hms(1, 0, 0).unwrap())
            .assume_offset(UtcOffset::from_hms(-5, 0, 0).unwrap());
        let transition = at(2026, Month::March, 8, 7, 0, 0);
        let midnight = resolve_next_midnight(before_spring_forward, |candidate| {
            Some(if candidate >= transition {
                UtcOffset::from_hms(-4, 0, 0).unwrap()
            } else {
                UtcOffset::from_hms(-5, 0, 0).unwrap()
            })
        })
        .unwrap();

        assert_eq!(midnight.offset(), UtcOffset::from_hms(-4, 0, 0).unwrap());
        assert_eq!(
            (midnight - before_spring_forward).unsigned_abs(),
            Duration::from_secs(22 * 60 * 60)
        );
    }

    #[test]
    fn next_midnight_falls_back_to_reference_offset() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        let midnight = resolve_next_midnight(reference, |_| None).unwrap();

        assert_eq!(midnight.offset(), reference.offset());
        assert_eq!(
            (midnight - reference).unsigned_abs(),
            Duration::from_secs(12 * 60 * 60)
        );
    }

    #[test]
    fn fixed_reference_tick_is_idle() {
        let reference = at(2026, Month::July, 20, 12, 0, 0);
        let mut relative = RelativeDate::new(reference).reference(reference);

        assert_eq!(
            <RelativeDate as TuiNode<()>>::tick(
                &mut relative,
                Duration::from_secs(1),
                AnimationSettings::default()
            ),
            TickResult::IDLE
        );
    }
}
