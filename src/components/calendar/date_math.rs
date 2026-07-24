use time::{Date, Duration, Month, OffsetDateTime, Time, Weekday};

pub(super) fn today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

pub(crate) fn week_range(date: Date, first_day_of_week: Weekday) -> (Date, Date) {
    let date_offset = i16::from(date.weekday().number_days_from_monday());
    let first_offset = i16::from(first_day_of_week.number_days_from_monday());
    let offset = i64::from((date_offset - first_offset).rem_euclid(7));
    let start = date - Duration::days(offset);
    (start, start + Duration::days(6))
}

pub(crate) fn weekday_labels(first_day_of_week: Weekday) -> [(&'static str, Weekday); 7] {
    let labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let start = first_day_of_week.number_days_from_monday() as usize;
    std::array::from_fn(|index| {
        let offset = (start + index) % labels.len();
        (labels[offset], weekday_from_monday_offset(offset))
    })
}

fn weekday_from_monday_offset(offset: usize) -> Weekday {
    [
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ][offset]
}

pub(super) fn weekday_short(date: Date) -> &'static str {
    match date.weekday().number_days_from_monday() {
        0 => "Mon",
        1 => "Tue",
        2 => "Wed",
        3 => "Thu",
        4 => "Fri",
        5 => "Sat",
        _ => "Sun",
    }
}

pub(super) fn first_of_month(date: Date) -> Date {
    date.replace_day(1).expect("first day is valid")
}

pub(super) fn last_of_month(date: Date) -> Date {
    date.replace_day(date.month().length(date.year()))
        .expect("month length day is valid")
}

pub(super) fn add_months(date: Date, months: i32) -> Date {
    let zero_based = i64::from(date.month() as u8) - 1 + i64::from(months);
    let year_delta = zero_based.div_euclid(12);
    let Some(year) = i64::from(date.year())
        .checked_add(year_delta)
        .and_then(|year| i32::try_from(year).ok())
    else {
        return if months.is_negative() {
            Date::MIN
        } else {
            Date::MAX
        };
    };
    let month_index = zero_based.rem_euclid(12) + 1;
    let Ok(month) = Month::try_from(month_index as u8) else {
        return if months.is_negative() {
            Date::MIN
        } else {
            Date::MAX
        };
    };
    Date::from_calendar_date(year, month, date.day().min(month.length(year))).unwrap_or_else(|_| {
        if months.is_negative() {
            Date::MIN
        } else {
            Date::MAX
        }
    })
}

pub(super) fn format_time(time: Time) -> String {
    format!("{:02}:{:02}", time.hour(), time.minute())
}
