mod date_picker;
mod date_picker_dropdown;
mod date_time_picker;
mod date_time_picker_dropdown;
mod time_picker;

pub use date_picker::DatePicker;
pub use date_picker_dropdown::DatePickerDropdown;
pub use date_time_picker::{DateTimePicker, DateTimePickerLayout};
pub use date_time_picker_dropdown::DateTimePickerDropdown;
pub use time_picker::{TimeField, TimePicker, TimePrecision};

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use time::{Date, Month, OffsetDateTime, PrimitiveDateTime, Time};

use crate::event::{Key, KeyEvent};
use crate::{AxisExpand, EventCtx, EventOutcome, HintSource, LayoutSize, LayoutSizeHint, theme};

const DATE_PICKER_FOCUS: &str = "date-picker";
const DATE_PICKER_DROPDOWN_FOCUS: &str = "date-picker-dropdown";
const DATE_TIME_PICKER_DROPDOWN_FOCUS: &str = "date-time-picker-dropdown";
const TIME_PICKER_FOCUS: &str = "time-picker";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickerOutcome {
    pub handled: bool,
    pub changed: bool,
    pub selected: bool,
    pub canceled: bool,
}

impl PickerOutcome {
    pub const IGNORED: Self = Self {
        handled: false,
        changed: false,
        selected: false,
        canceled: false,
    };

    const fn handled(changed: bool) -> Self {
        Self {
            handled: true,
            changed,
            selected: false,
            canceled: false,
        }
    }

    const fn selected(changed: bool) -> Self {
        Self {
            handled: true,
            changed,
            selected: true,
            canceled: false,
        }
    }

    const fn canceled(changed: bool) -> Self {
        Self {
            handled: true,
            changed,
            selected: false,
            canceled: true,
        }
    }
}

fn picker_size_hint(width: u16, height: u16) -> LayoutSizeHint {
    LayoutSizeHint {
        source: HintSource::Measured,
        min: LayoutSize::new(width, height),
        preferred: LayoutSize::new(width, height),
        expand: AxisExpand {
            width: false,
            height: false,
        },
    }
}

fn finish_event<M>(ctx: &mut EventCtx<M>, outcome: PickerOutcome) -> EventOutcome {
    if outcome.changed || outcome.selected || outcome.canceled {
        ctx.request_redraw();
    }
    if outcome.handled {
        ctx.stop_propagation();
        EventOutcome::Handled
    } else {
        EventOutcome::Ignored
    }
}

fn today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

fn first_of_month(date: Date) -> Date {
    date.replace_day(1).expect("first day is valid")
}

fn last_of_month(date: Date) -> Date {
    date.replace_day(date.month().length(date.year()))
        .expect("month length day is valid")
}

fn date_in_month(year: i32, month: Month, day: u8) -> Date {
    date_in_month_checked(year, month, day).unwrap_or_else(|| {
        if year < Date::MIN.year() {
            Date::MIN
        } else {
            Date::MAX
        }
    })
}

fn date_in_month_checked(year: i32, month: Month, day: u8) -> Option<Date> {
    Date::from_calendar_date(year, month, day.min(month.length(year))).ok()
}

fn year_page_start(year: i32) -> i32 {
    year.saturating_sub(year.rem_euclid(24))
}

fn centered_grid(area: Rect, rows: u16, width: u16) -> Rect {
    let height = rows.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height,
    }
}

fn choice_style(selected: bool, focused: bool) -> Style {
    if selected && focused {
        Style::default()
            .fg(theme().highlight_fg())
            .bg(theme().highlight_bg())
            .add_modifier(Modifier::BOLD)
    } else if selected {
        Style::default()
            .fg(theme().selected_fg())
            .bg(theme().selected_bg())
    } else {
        Style::default().fg(theme().text_fg())
    }
}

fn month_abbr(month: Month) -> &'static str {
    match month {
        Month::January => "JAN",
        Month::February => "FEB",
        Month::March => "MAR",
        Month::April => "APR",
        Month::May => "MAY",
        Month::June => "JUN",
        Month::July => "JUL",
        Month::August => "AUG",
        Month::September => "SEP",
        Month::October => "OCT",
        Month::November => "NOV",
        Month::December => "DEC",
    }
}

fn parse_editor_date(value: &str) -> Option<Date> {
    let value = value.trim().lines().next()?.trim();
    let mut parts = value.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

fn parse_editor_datetime(value: &str) -> Option<PrimitiveDateTime> {
    let value = value.trim().lines().next()?.trim();
    let mut parts = value.split_whitespace();
    let date = parse_editor_date(parts.next()?)?;
    let time = parse_editor_time(parts.next()?)?;
    if parts.next().is_some() {
        return None;
    }
    Some(date.with_time(time))
}

pub(super) fn today_time() -> Time {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .time()
}

pub(super) fn parse_editor_time(value: &str) -> Option<Time> {
    let value = value.trim().lines().next()?.trim();
    let mut parts = value.split(':');
    let hour = parts.next()?.parse::<u8>().ok()?;
    let minute = parts.next()?.parse::<u8>().ok()?;
    let second = parts
        .next()
        .map(|second| second.parse::<u8>().ok())
        .unwrap_or(Some(0))?;
    if parts.next().is_some() {
        return None;
    }
    Time::from_hms(hour, minute, second).ok()
}

pub(super) fn format_picker_time(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}

pub(super) fn format_iso_time(time: Time) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        time.hour(),
        time.minute(),
        time.second()
    )
}

pub(super) fn format_iso_datetime(value: PrimitiveDateTime) -> String {
    format!("{}T{}", value.date(), format_iso_time(value.time()))
}

fn date_limit(delta: i32) -> Date {
    if delta.is_negative() {
        Date::MIN
    } else {
        Date::MAX
    }
}

fn add_months(date: Date, months: i32) -> Date {
    let zero_based = i64::from(date.month() as u8) - 1 + i64::from(months);
    let year_delta = zero_based.div_euclid(12);
    let Some(year) = i64::from(date.year())
        .checked_add(year_delta)
        .and_then(|year| i32::try_from(year).ok())
    else {
        return date_limit(months);
    };
    let month_index = zero_based.rem_euclid(12) + 1;
    let Ok(month) = Month::try_from(month_index as u8) else {
        return date_limit(months);
    };
    date_in_month_checked(year, month, date.day()).unwrap_or_else(|| date_limit(months))
}

fn wrap_step(value: u8, delta: i8, step: u8, max: u8) -> u8 {
    let next = i16::from(value) + i16::from(delta) * i16::from(step);
    next.rem_euclid(i16::from(max)) as u8
}

fn plain_digit(key: KeyEvent) -> Option<u8> {
    if !key.modifiers.is_empty() {
        return None;
    }
    let Key::Char(ch) = key.code else {
        return None;
    };
    ch.to_digit(10).map(|digit| digit as u8)
}
