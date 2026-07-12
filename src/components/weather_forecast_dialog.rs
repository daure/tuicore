use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;
use time::{Date, Month, OffsetDateTime, UtcOffset};

use super::weather_indicator::{
    HourlyWeather, WeatherReport, WeatherSummary, weather_condition_icon,
};
use crate::{
    Animated, AnimationSettings, Dialog, DialogCloseReason, EventCtx, EventOutcome, FocusCtx,
    FocusId, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, ScrollAxes, TickResult,
    TuiNode, line_width, theme,
};

const PERIOD_WIDTHS: [usize; 4] = [30, 30, 29, 29];
const FORECAST_DAY_TABLE_WIDTH: usize = 123;
const FORECAST_DAY_HEADER_LEFT: usize = 30;
const FORECAST_DAY_HEADER_RIGHT: usize = 29;
const WEATHER_ART_WIDTH: usize = 12;
const DIALOG_HEIGHT_PERCENT: u16 = 80;
const LUX_PER_SHORTWAVE_WATT: f64 = 120.0;

pub struct WeatherForecastDialog<M = ()> {
    dialog: Dialog<M>,
    content: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherForecastError {
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherForecastDay {
    label: String,
    condition: String,
    temperature_range: String,
    body: String,
    walk_times: String,
}

impl<M> WeatherForecastDialog<M> {
    pub fn new() -> Self {
        Self {
            dialog: Dialog::new().scrollable(ScrollAxes::Both),
            content: Vec::new(),
        }
    }

    pub fn report(mut self, report: WeatherReport) -> Self {
        self.set_report(report);
        self
    }

    pub fn set_report(&mut self, report: WeatherReport) {
        if let Some(location) = report.summary().location_name() {
            self.dialog.set_top_left(location);
        } else {
            self.dialog.clear_title(crate::DialogTitlePosition::TopLeft);
        }
        self.dialog
            .clear_title(crate::DialogTitlePosition::BottomLeft);
        self.content = forecast_lines(report.raw());
        self.dialog
            .set_content_lines(colorized_lines(self.content.clone()));
    }

    pub fn content(mut self, lines: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.set_content(lines);
        self
    }

    pub fn set_content(&mut self, lines: impl IntoIterator<Item = impl Into<String>>) {
        self.content = lines.into_iter().map(Into::into).collect();
        self.dialog
            .set_content_lines(colorized_lines(self.content.clone()));
    }

    pub fn on_close(mut self, handler: impl Fn(DialogCloseReason) -> M + 'static) -> Self {
        self.dialog = self.dialog.on_close(handler);
        self
    }

    pub fn dialog(&self) -> &Dialog<M> {
        &self.dialog
    }

    pub fn dialog_mut(&mut self) -> &mut Dialog<M> {
        &mut self.dialog
    }

    fn dialog_area(&self, area: Rect) -> Rect {
        let content = self.dialog.content_size();
        let width = (content.width as u16).saturating_add(3).min(area.width);
        let max_height = percent_of(area.height, DIALOG_HEIGHT_PERCENT).max(1);
        let height = (content.height as u16)
            .saturating_add(3)
            .min(max_height)
            .min(area.height);
        Rect::new(
            area.x + area.width.saturating_sub(width) / 2,
            area.y + area.height.saturating_sub(height) / 2,
            width,
            height,
        )
    }
}

fn percent_of(value: u16, percent: u16) -> u16 {
    ((u32::from(value) * u32::from(percent)) / 100).min(u32::from(u16::MAX)) as u16
}

impl std::fmt::Display for WeatherForecastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WeatherForecastError {}

impl WeatherForecastError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl WeatherReport {
    pub fn from_open_meteo_json(
        location: impl Into<String>,
        json: impl AsRef<str>,
    ) -> Result<Self, WeatherForecastError> {
        let location = location.into();
        let root: Value = serde_json::from_str(json.as_ref()).map_err(|error| {
            WeatherForecastError::new(format!("invalid Open-Meteo JSON: {error}"))
        })?;
        let daily = root
            .get("daily")
            .ok_or_else(|| WeatherForecastError::new("Open-Meteo response missing daily data"))?;
        let hourly = root
            .get("hourly")
            .ok_or_else(|| WeatherForecastError::new("Open-Meteo response missing hourly data"))?;

        let dates = string_array(daily, "time")?;
        let daily_codes = i64_array(daily, "weather_code")?;
        let highs = f64_array(daily, "temperature_2m_max")?;
        let lows = f64_array(daily, "temperature_2m_min")?;
        let hourly_times = string_array(hourly, "time")?;
        let hourly_temps = f64_array(hourly, "temperature_2m")?;
        let hourly_feels_like = optional_f64_array(hourly, "apparent_temperature");
        let hourly_codes = i64_array(hourly, "weather_code")?;
        let hourly_winds = f64_array(hourly, "wind_speed_10m")?;
        let hourly_precip = f64_array(hourly, "precipitation")?;
        let hourly_precip_probability = optional_f64_array(hourly, "precipitation_probability");
        let hourly_visibility = optional_f64_array(hourly, "visibility");
        let minutely = root.get("minutely_15");
        let radiation_times = minutely
            .and_then(|value| string_array(value, "time").ok())
            .unwrap_or_default();
        let shortwave_radiation = minutely
            .and_then(|value| f64_array(value, "shortwave_radiation").ok())
            .unwrap_or_default();

        let utc_offset_seconds = open_meteo_utc_offset_seconds(&root);
        let start_index = current_forecast_start_index(&dates, provider_today(utc_offset_seconds));
        let mut rendered_days = Vec::new();
        for (index, date) in dates.iter().enumerate().skip(start_index).take(7) {
            let date = date.as_str();
            let condition = condition_for_wmo(*daily_codes.get(index).unwrap_or(&0));
            let range = format!(
                "{}/{} °C",
                rounded(*lows.get(index).unwrap_or(&0.0)),
                rounded(*highs.get(index).unwrap_or(&0.0))
            );
            let periods = [
                open_meteo_period(
                    "Morning",
                    date,
                    "06:00",
                    &hourly_times,
                    &hourly_temps,
                    hourly_feels_like.as_deref(),
                    &hourly_codes,
                    &hourly_winds,
                    &hourly_precip,
                    hourly_precip_probability.as_deref(),
                    hourly_visibility.as_deref(),
                ),
                open_meteo_period(
                    "Noon",
                    date,
                    "12:00",
                    &hourly_times,
                    &hourly_temps,
                    hourly_feels_like.as_deref(),
                    &hourly_codes,
                    &hourly_winds,
                    &hourly_precip,
                    hourly_precip_probability.as_deref(),
                    hourly_visibility.as_deref(),
                ),
                open_meteo_period(
                    "Evening",
                    date,
                    "18:00",
                    &hourly_times,
                    &hourly_temps,
                    hourly_feels_like.as_deref(),
                    &hourly_codes,
                    &hourly_winds,
                    &hourly_precip,
                    hourly_precip_probability.as_deref(),
                    hourly_visibility.as_deref(),
                ),
                open_meteo_period(
                    "Night",
                    date,
                    "00:00",
                    &hourly_times,
                    &hourly_temps,
                    hourly_feels_like.as_deref(),
                    &hourly_codes,
                    &hourly_winds,
                    &hourly_precip,
                    hourly_precip_probability.as_deref(),
                    hourly_visibility.as_deref(),
                ),
            ];
            rendered_days.push(
                WeatherForecastDay::new(
                    date_label(date).unwrap_or_else(|| date.to_string()),
                    condition,
                    range,
                    forecast_day_body(&periods),
                )
                .with_walk_times(walk_footer(date, &radiation_times, &shortwave_radiation))
                .render_text(),
            );
        }

        let first_condition = daily_codes
            .get(start_index)
            .map(|code| condition_for_wmo(*code).to_string())
            .unwrap_or_else(|| "Weather".to_string());
        let first_temperature = highs
            .get(start_index)
            .map(|value| format!("{} °C", rounded(*value)))
            .unwrap_or_else(|| "--".to_string());
        let summary =
            WeatherSummary::new(first_temperature, first_condition).location(location.clone());
        let hourly_summary =
            HourlyWeather::new(hourly_times.iter().enumerate().map(|(index, time)| {
                let temperature = hourly_temps
                    .get(index)
                    .map(|value| {
                        let temperature = rounded(*value);
                        hourly_feels_like
                            .as_ref()
                            .and_then(|values| values.get(index))
                            .map(|feels_like| {
                                format!("{}({}) °C", temperature, rounded(*feels_like))
                            })
                            .unwrap_or_else(|| format!("{temperature} °C"))
                    })
                    .unwrap_or_else(|| "--".to_string());
                let condition = hourly_codes
                    .get(index)
                    .map(|code| condition_for_wmo(*code).to_string())
                    .unwrap_or_else(|| "Weather".to_string());
                (time.clone(), temperature, condition)
            }))
            .with_utc_offset(utc_offset_seconds);
        let raw = format!(
            "Weather report: {location}\n\n{}\nLocation: {location}",
            rendered_days.join("\n")
        );
        Ok(Self::from_parts(raw, summary).with_hourly(hourly_summary))
    }
}

fn current_forecast_start_index(dates: &[String], today: Date) -> usize {
    let today = today.to_string();
    dates.iter().position(|date| date >= &today).unwrap_or(0)
}

fn provider_today(utc_offset_seconds: Option<i32>) -> Date {
    utc_offset_seconds
        .and_then(|seconds| UtcOffset::from_whole_seconds(seconds).ok())
        .map(|offset| OffsetDateTime::now_utc().to_offset(offset).date())
        .unwrap_or_else(local_today)
}

fn local_today() -> Date {
    OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date()
}

fn open_meteo_utc_offset_seconds(root: &Value) -> Option<i32> {
    root.get("utc_offset_seconds")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
}

impl WeatherForecastDay {
    pub fn new(
        label: impl Into<String>,
        condition: impl Into<String>,
        temperature_range: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            condition: condition.into(),
            temperature_range: temperature_range.into(),
            body: body.into(),
            walk_times: "󰖨 Lux 30m --:-- · 20m --:--".to_string(),
        }
    }

    fn with_walk_times(mut self, walk_times: String) -> Self {
        self.walk_times = walk_times;
        self
    }

    pub fn render_text(&self) -> String {
        let icon = weather_condition_icon(&self.condition, false);
        let title = format!(" {} {} {} ", self.label, icon, self.temperature_range);
        [
            forecast_day_title_line(&title),
            forecast_day_period_line(),
            self.body.clone(),
            forecast_day_bottom_line(&self.walk_times),
        ]
        .join("\n")
    }
}

fn forecast_day_bottom_line(walk_times: &str) -> String {
    let title = format!("└─ {walk_times} ┴");
    let border = separator('└', '┴', '┘');
    format!(
        "{title}{}",
        border
            .chars()
            .skip(display_width(&title))
            .collect::<String>()
    )
}

fn walk_footer(date: &str, times: &[String], radiation: &[f64]) -> String {
    let walk_30 = earliest_walk_start(date, times, radiation, 30, 2_500.0);
    let walk_20 = earliest_walk_start(date, times, radiation, 20, 10_000.0);
    format!(
        "󰖨 Lux 30m {} · 20m {}",
        walk_30.as_deref().unwrap_or("--:--"),
        walk_20.as_deref().unwrap_or("--:--")
    )
}

fn earliest_walk_start(
    date: &str,
    times: &[String],
    radiation: &[f64],
    duration_minutes: usize,
    minimum_lux: f64,
) -> Option<String> {
    let intervals = duration_minutes.div_ceil(15);
    times.iter().enumerate().find_map(|(index, timestamp)| {
        let time = timestamp.strip_prefix(&format!("{date}T"))?;
        let first_interval_end = time_minutes(time)?;
        if first_interval_end < 15 || first_interval_end % 15 != 0 {
            return None;
        }
        let qualifies = (0..intervals).all(|offset| {
            let sample_index = index + offset;
            times
                .get(sample_index)
                .and_then(|sample| sample.strip_prefix(&format!("{date}T")))
                .and_then(time_minutes)
                .is_some_and(|minutes| {
                    minutes % 15 == 0 && minutes == first_interval_end + offset * 15
                })
                && radiation
                    .get(sample_index)
                    .is_some_and(|watts| watts * LUX_PER_SHORTWAVE_WATT >= minimum_lux)
        });
        qualifies.then(|| format_minutes(first_interval_end - 15))
    })
}

fn format_minutes(minutes: usize) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn time_minutes(time: &str) -> Option<usize> {
    let (hour, minute) = time.split_once(':')?;
    let hour = hour.parse::<usize>().ok()?;
    let minute = minute.parse::<usize>().ok()?;
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

impl<M> Default for WeatherForecastDialog<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TuiNode<M> for WeatherForecastDialog<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.dialog.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.dialog.layout(self.dialog_area(area), ctx)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        self.dialog.render_with_content_lines(
            frame,
            self.dialog_area(area),
            colorized_lines(self.content.clone()),
        );
    }

    fn event(&mut self, event: &crate::event::TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.dialog.event(event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        Animated::tick(&mut self.dialog, dt, settings)
    }

    fn focus(&mut self, target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.dialog.focus(target, focused, ctx);
    }
}

fn forecast_day_title_line(title: &str) -> String {
    let title_width = display_width(title);
    let middle = FORECAST_DAY_TABLE_WIDTH
        .saturating_sub(FORECAST_DAY_HEADER_LEFT)
        .saturating_sub(FORECAST_DAY_HEADER_RIGHT)
        .saturating_sub(title_width)
        .saturating_sub(6);
    format!(
        "┌{}┬{}┤{}├{}┬{}┐",
        "─".repeat(FORECAST_DAY_HEADER_LEFT),
        "─".repeat(middle / 2),
        title,
        "─".repeat(middle - middle / 2),
        "─".repeat(FORECAST_DAY_HEADER_RIGHT)
    )
}

fn forecast_day_period_line() -> String {
    row([
        center("Morning", PERIOD_WIDTHS[0]),
        center("Noon", PERIOD_WIDTHS[1]),
        center("Evening", PERIOD_WIDTHS[2]),
        center("Night", PERIOD_WIDTHS[3]),
    ])
}

fn forecast_day_body(periods: &[OpenMeteoPeriod; 4]) -> String {
    [
        separator('├', '┼', '┤'),
        art_row(periods, 0, |period| &period.condition),
        art_row(periods, 1, |period| &period.temperature),
        art_row(periods, 2, |period| &period.wind),
        art_row(periods, 3, |period| &period.visibility),
        art_row(periods, 4, |period| &period.precipitation),
    ]
    .join("\n")
}

#[derive(Clone)]
struct OpenMeteoPeriod {
    width: usize,
    code: i64,
    condition: String,
    temperature: String,
    wind: String,
    visibility: String,
    precipitation: String,
}

fn open_meteo_period(
    label: &str,
    date: &str,
    hour: &str,
    times: &[String],
    temps: &[f64],
    feels_like: Option<&[f64]>,
    codes: &[i64],
    winds: &[f64],
    precipitation: &[f64],
    precipitation_probability: Option<&[f64]>,
    visibility: Option<&[f64]>,
) -> OpenMeteoPeriod {
    let index = times
        .iter()
        .position(|time| time == &format!("{date}T{hour}"))
        .or_else(|| times.iter().position(|time| time.starts_with(date)))
        .unwrap_or_default();
    let part_index = match label {
        "Morning" => 0,
        "Noon" => 1,
        "Evening" => 2,
        _ => 3,
    };
    let probability = precipitation_probability
        .and_then(|values| values.get(index))
        .map(|value| format!(" | {}%", rounded(*value)))
        .unwrap_or_default();
    let temperature = *temps.get(index).unwrap_or(&0.0);
    let feels = feels_like
        .and_then(|values| values.get(index))
        .copied()
        .unwrap_or(temperature);
    let code = *codes.get(index).unwrap_or(&0);
    OpenMeteoPeriod {
        width: PERIOD_WIDTHS[part_index],
        code,
        condition: condition_for_wmo(code).to_string(),
        temperature: format!("{}({}) °C", rounded(temperature), rounded(feels)),
        wind: format!("{} km/h", rounded(*winds.get(index).unwrap_or(&0.0))),
        visibility: visibility
            .and_then(|values| values.get(index))
            .map(|meters| format!("{} km", rounded(*meters / 1000.0)))
            .unwrap_or_else(|| "-- km".to_string()),
        precipitation: format!(
            "{:.1} mm{}",
            precipitation.get(index).copied().unwrap_or_default(),
            probability
        ),
    }
}

fn art_row(
    periods: &[OpenMeteoPeriod; 4],
    art_index: usize,
    value: fn(&OpenMeteoPeriod) -> &String,
) -> String {
    row(periods.each_ref().map(|period| {
        art_value_cell(
            weather_ascii_art(period.code)[art_index],
            value(period),
            period.width,
        )
    }))
}

fn art_value_cell(art: &str, value: &str, width: usize) -> String {
    let gap = "  ";
    let art = pad_right(art, WEATHER_ART_WIDTH);
    let available = width
        .saturating_sub(WEATHER_ART_WIDTH)
        .saturating_sub(gap.len());
    let cell = format!("{art}{gap}{}", truncate(value, available));
    pad_right(&truncate(&cell, width), width)
}

fn weather_ascii_art(code: i64) -> [&'static str; 5] {
    match code {
        0 => [
            "   \\   /   ",
            "    .-.    ",
            " ― (   ) ― ",
            "    `-’    ",
            "   /   \\   ",
        ],
        1 | 2 => [
            "  _`/\"\".-. ",
            "   ,\\_(   ).",
            "    /(___(__)",
            "            ",
            "            ",
        ],
        3 => [
            "            ",
            "     .--.   ",
            "  .-(    ). ",
            " (___.__)__)",
            "            ",
        ],
        45 | 48 => [
            "            ",
            " _ - _ - _ -",
            "  _ - _ - _ ",
            " _ - _ - _ -",
            "            ",
        ],
        51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 => [
            "     .-.    ",
            "    (   ).  ",
            "   (___(__) ",
            "    ‘ ‘ ‘ ‘ ",
            "   ‘ ‘ ‘ ‘  ",
        ],
        71 | 73 | 75 | 77 => [
            "     .-.    ",
            "    (   ).  ",
            "   (___(__) ",
            "    * * * * ",
            "   * * * *  ",
        ],
        80..=82 => [
            " _`/\"\".-. ",
            "  ,\\_(   ).",
            "   /(___(__)",
            "     ‘ ‘ ‘ ‘",
            "    ‘ ‘ ‘ ‘ ",
        ],
        85 | 86 => [
            " _`/\"\".-. ",
            "  ,\\_(   ).",
            "   /(___(__)",
            "     * * * *",
            "    * * * * ",
        ],
        95..=99 => [
            "     .-.    ",
            "    (   ).  ",
            "   (___(__) ",
            "    ⚡‘ ⚡‘  ",
            "    ‘ ‘ ‘ ‘ ",
        ],
        _ => [
            "    .-.     ",
            "     __)    ",
            "    (       ",
            "     `-’    ",
            "      •     ",
        ],
    }
}

fn separator(left: char, middle: char, right: char) -> String {
    let mut line = String::new();
    line.push(left);
    for (index, width) in PERIOD_WIDTHS.iter().enumerate() {
        if index > 0 {
            line.push(middle);
        }
        line.push_str(&"─".repeat(*width));
    }
    line.push(right);
    line
}

fn row(cells: [String; 4]) -> String {
    format!("│{}│{}│{}│{}│", cells[0], cells[1], cells[2], cells[3])
}

fn center(value: &str, width: usize) -> String {
    let truncated = truncate(value, width);
    let len = display_width(&truncated);
    let padding = width.saturating_sub(len);
    format!(
        "{}{}{}",
        " ".repeat(padding / 2),
        truncated,
        " ".repeat(padding - padding / 2)
    )
}

fn pad_right(value: &str, width: usize) -> String {
    let padding = width.saturating_sub(display_width(value));
    format!("{value}{}", " ".repeat(padding))
}

fn display_width(value: &str) -> usize {
    line_width(&Line::from(value))
}

fn truncate(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_string();
    }
    let ellipsis_width = display_width("…");
    let max_width = width.saturating_sub(ellipsis_width);
    let mut truncated = String::new();
    let mut current_width = 0;
    for ch in value.chars() {
        let char_width = display_width(&ch.to_string());
        if current_width + char_width > max_width {
            break;
        }
        truncated.push(ch);
        current_width += char_width;
    }
    truncated.push('…');
    truncated
}

fn string_array(parent: &Value, key: &str) -> Result<Vec<String>, WeatherForecastError> {
    parent
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| WeatherForecastError::new(format!("Open-Meteo field missing: {key}")))?
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                WeatherForecastError::new(format!("Open-Meteo field is not string: {key}"))
            })
        })
        .collect()
}

fn f64_array(parent: &Value, key: &str) -> Result<Vec<f64>, WeatherForecastError> {
    parent
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| WeatherForecastError::new(format!("Open-Meteo field missing: {key}")))?
        .iter()
        .map(|value| {
            value.as_f64().ok_or_else(|| {
                WeatherForecastError::new(format!("Open-Meteo field is not number: {key}"))
            })
        })
        .collect()
}

fn optional_f64_array(parent: &Value, key: &str) -> Option<Vec<f64>> {
    parent
        .get(key)?
        .as_array()?
        .iter()
        .map(Value::as_f64)
        .collect()
}

fn i64_array(parent: &Value, key: &str) -> Result<Vec<i64>, WeatherForecastError> {
    parent
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| WeatherForecastError::new(format!("Open-Meteo field missing: {key}")))?
        .iter()
        .map(|value| {
            value.as_i64().ok_or_else(|| {
                WeatherForecastError::new(format!("Open-Meteo field is not integer: {key}"))
            })
        })
        .collect()
}

fn condition_for_wmo(code: i64) -> &'static str {
    match code {
        0 => "Sunny",
        1 | 2 => "Partly cloudy",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 | 56 | 57 => "Drizzle",
        61 | 63 | 65 | 66 | 67 => "Rain",
        71 | 73 | 75 | 77 => "Snow",
        80..=82 => "Rain showers",
        85 | 86 => "Snow showers",
        95..=99 => "Thunderstorm",
        _ => "Weather",
    }
}

fn rounded(value: f64) -> i64 {
    value.round() as i64
}

fn date_label(date: &str) -> Option<String> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    let month = Month::try_from(month).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    Some(format!(
        "{} {:02} {}",
        weekday_label(date.weekday()),
        day,
        month_label(month)
    ))
}

fn weekday_label(weekday: time::Weekday) -> &'static str {
    match weekday {
        time::Weekday::Monday => "Mon",
        time::Weekday::Tuesday => "Tue",
        time::Weekday::Wednesday => "Wed",
        time::Weekday::Thursday => "Thu",
        time::Weekday::Friday => "Fri",
        time::Weekday::Saturday => "Sat",
        time::Weekday::Sunday => "Sun",
    }
}

fn month_label(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

fn forecast_lines(raw: &str) -> Vec<String> {
    let mut in_forecast = false;
    raw.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("Weather report:") || trimmed.starts_with("Location:") {
                return None;
            }
            if trimmed.starts_with('┌') {
                in_forecast = true;
            }
            in_forecast.then(|| line.to_owned())
        })
        .collect()
}

fn colorized_lines(lines: impl IntoIterator<Item = impl Into<String>>) -> Vec<Line<'static>> {
    let lines = lines.into_iter().map(Into::into).collect::<Vec<String>>();
    let chars = lines
        .iter()
        .map(|line| line.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut styles = chars
        .iter()
        .map(|line| vec![Style::default(); line.len()])
        .collect::<Vec<_>>();

    for (chars, styles) in chars.iter().zip(styles.iter_mut()) {
        colorize_weather_values(chars, styles);
    }

    let mut art_palettes = vec![None; chars.len()];
    for (index, line) in chars.iter().enumerate() {
        if line.iter().any(|ch| *ch == '°') {
            let palettes = period_art_palettes(&chars, index);
            for target in index.saturating_sub(1)..=(index + 3).min(chars.len().saturating_sub(1)) {
                art_palettes[target] = Some(palettes.clone());
            }
        }
    }

    chars
        .into_iter()
        .zip(styles)
        .zip(art_palettes)
        .map(|((chars, mut styles), palettes)| {
            colorize_weather_art(&chars, &mut styles, palettes.as_deref());
            styled_line(chars, styles)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct ArtPalette {
    sun: Color,
    sunny: bool,
}

fn period_art_palettes(lines: &[Vec<char>], temperature_line: usize) -> Vec<ArtPalette> {
    let temperatures = period_value_colors(&lines[temperature_line], weather_value_color);
    let condition_line = temperature_line.saturating_sub(1);
    period_cells(&lines[condition_line])
        .into_iter()
        .enumerate()
        .map(|(index, (start, end))| ArtPalette {
            sun: temperatures
                .get(index)
                .copied()
                .unwrap_or_else(|| theme().weather_sun_fg()),
            sunny: lines[condition_line][start..end]
                .iter()
                .collect::<String>()
                .contains("Sunny"),
        })
        .collect()
}

fn period_value_colors(chars: &[char], value_color: fn(f64) -> Color) -> Vec<Color> {
    period_cells(chars)
        .into_iter()
        .filter_map(|(start, end)| {
            first_number(&chars[start + WEATHER_ART_WIDTH.min(end - start)..end])
        })
        .map(value_color)
        .collect()
}

fn period_cells(chars: &[char]) -> Vec<(usize, usize)> {
    let cell_edges = chars
        .iter()
        .enumerate()
        .filter_map(|(index, ch)| (*ch == '│').then_some(index))
        .collect::<Vec<_>>();
    cell_edges
        .windows(2)
        .map(|edges| (edges[0] + 1, edges[1]))
        .collect()
}

fn colorize_weather_art(chars: &[char], styles: &mut [Style], palettes: Option<&[ArtPalette]>) {
    for (cell_index, (start, cell_end)) in period_cells(chars).into_iter().enumerate() {
        let end = cell_end.min(start + WEATHER_ART_WIDTH);
        let droplet_count = chars[start..end.min(chars.len())]
            .iter()
            .filter(|ch| is_rain_drop(**ch))
            .count();
        for index in start..end.min(styles.len()) {
            if chars[index].is_whitespace() {
                continue;
            }
            if droplet_count > 2 && is_rain_drop(chars[index]) {
                styles[index] = Style::default().fg(theme().weather_rain_fg());
            } else if chars[index] == '*' {
                styles[index] = Style::default().fg(theme().accent_fg());
            } else if chars[index] == '⚡' {
                styles[index] = Style::default().fg(theme().error_fg());
            } else if palettes
                .and_then(|palettes| palettes.get(cell_index))
                .is_some_and(|palette| palette.sunny)
                && is_weather_art_char(chars[index])
            {
                let color = palettes
                    .and_then(|palettes| palettes.get(cell_index))
                    .map(|palette| palette.sun)
                    .unwrap_or_else(|| theme().weather_sun_fg());
                styles[index] = Style::default().fg(color);
            }
        }
    }
}

fn colorize_weather_values(chars: &[char], styles: &mut [Style]) {
    let line = chars.iter().collect::<String>();
    if !line.contains('│') {
        colorize_title_temperature(chars, styles);
        return;
    }
    let value_kind = if line.contains("°C") {
        Some(weather_value_color as fn(f64) -> Color)
    } else if line.contains("km/h") {
        Some(wind_value_color as fn(f64) -> Color)
    } else if line.contains("mm") {
        Some(precipitation_value_color as fn(f64) -> Color)
    } else {
        None
    };
    let Some(value_color) = value_kind else {
        return;
    };

    let mut index = 0;
    while index < chars.len() {
        let signed_number = matches!(chars[index], '+' | '-')
            && chars.get(index + 1).is_some_and(|ch| ch.is_ascii_digit());
        if !signed_number && !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        if signed_number {
            index += 1;
        }
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        let value = chars[start..index]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .ok();
        if let Some(value) = value {
            let style = Style::default().fg(value_color(value));
            for slot in styles.iter_mut().take(index).skip(start) {
                *slot = style;
            }
        }
    }
}

fn colorize_title_temperature(chars: &[char], styles: &mut [Style]) {
    let Some(degree_index) = chars.iter().position(|ch| *ch == '°') else {
        return;
    };
    let mut token_end = degree_index;
    while token_end > 0 && chars[token_end - 1].is_whitespace() {
        token_end -= 1;
    }
    let mut token_start = token_end;
    while token_start > 0 && !chars[token_start - 1].is_whitespace() {
        token_start -= 1;
    }

    let mut index = token_start;
    while index < token_end {
        let signed_number = matches!(chars[index], '+' | '-')
            && chars.get(index + 1).is_some_and(|ch| ch.is_ascii_digit());
        if !signed_number && !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        if signed_number {
            index += 1;
        }
        while index < token_end && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        if let Ok(value) = chars[start..index]
            .iter()
            .collect::<String>()
            .parse::<f64>()
        {
            let style = Style::default().fg(weather_value_color(value));
            for slot in styles.iter_mut().take(index).skip(start) {
                *slot = style;
            }
        }
    }
}

fn first_number(chars: &[char]) -> Option<f64> {
    let mut index = 0;
    while index < chars.len() {
        let signed_number = matches!(chars[index], '+' | '-')
            && chars.get(index + 1).is_some_and(|ch| ch.is_ascii_digit());
        if !signed_number && !chars[index].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index;
        if signed_number {
            index += 1;
        }
        while index < chars.len() && (chars[index].is_ascii_digit() || chars[index] == '.') {
            index += 1;
        }
        return chars[start..index].iter().collect::<String>().parse().ok();
    }
    None
}

fn weather_value_color(value: f64) -> Color {
    let theme = theme();
    if value <= 10.0 {
        theme.weather_rain_fg()
    } else if value <= 20.0 {
        theme.weather_cool_fg()
    } else if value <= 27.0 {
        theme.weather_sun_fg()
    } else if value <= 31.0 {
        theme.weather_warm_fg()
    } else {
        theme.weather_hot_fg()
    }
}

fn wind_value_color(value: f64) -> Color {
    let theme = theme();
    if value <= 8.0 {
        theme.weather_rain_fg()
    } else if value <= 14.0 {
        theme.weather_cool_fg()
    } else if value <= 20.0 {
        theme.weather_sun_fg()
    } else if value <= 28.0 {
        theme.weather_warm_fg()
    } else {
        theme.weather_hot_fg()
    }
}

fn precipitation_value_color(value: f64) -> Color {
    if value > 0.0 {
        theme().weather_rain_fg()
    } else {
        theme().subtle_fg()
    }
}

fn is_rain_drop(ch: char) -> bool {
    matches!(ch, '‘' | '’' | '`')
}

fn is_weather_art_char(ch: char) -> bool {
    matches!(
        ch,
        '\\' | '/' | '.' | '-' | '―' | '(' | ')' | '_' | ',' | '"'
    )
}

fn styled_line(chars: Vec<char>, styles: Vec<Style>) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut current_style = styles.first().copied().unwrap_or_default();

    for (ch, style) in chars.into_iter().zip(styles) {
        if style == current_style {
            current.push(ch);
        } else {
            spans.push(Span::styled(std::mem::take(&mut current), current_style));
            current.push(ch);
            current_style = style;
        }
    }
    if !current.is_empty() {
        spans.push(Span::styled(current, current_style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_meteo_summary_uses_current_hourly_weather() {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let current_hour = format!("{:02}:00", now.time().hour());
        let current_time = format!("{}T{current_hour}", now.date());
        let json = serde_json::json!({
            "hourly": {
                "time": [current_time],
                "temperature_2m": [12.0],
                "apparent_temperature": [14.0],
                "weather_code": [61],
                "wind_speed_10m": [8.0],
                "precipitation": [0.4]
            },
            "daily": {
                "time": [now.date().to_string()],
                "weather_code": [0],
                "temperature_2m_max": [30.0],
                "temperature_2m_min": [18.0]
            }
        });

        let report = WeatherReport::from_open_meteo_json("Here", json.to_string())
            .expect("valid Open-Meteo report");

        assert_eq!(report.summary().temperature(), "12(14) °C");
        assert_eq!(report.summary().condition(), "Rain");
    }

    #[test]
    fn open_meteo_forecast_starts_at_today_when_response_includes_past_days() {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let yesterday = now
            .saturating_sub(time::Duration::days(1))
            .date()
            .to_string();
        let today = now.date().to_string();
        let tomorrow = now
            .saturating_add(time::Duration::days(1))
            .date()
            .to_string();
        let json = serde_json::json!({
            "hourly": {
                "time": [format!("{yesterday}T12:00"), format!("{today}T12:00"), format!("{tomorrow}T12:00")],
                "temperature_2m": [10.0, 22.0, 24.0],
                "apparent_temperature": [11.0, 23.0, 25.0],
                "weather_code": [0, 61, 3],
                "wind_speed_10m": [7.0, 8.0, 9.0],
                "precipitation": [0.0, 0.4, 0.0]
            },
            "daily": {
                "time": [yesterday, today, tomorrow],
                "weather_code": [0, 61, 3],
                "temperature_2m_max": [30.0, 22.0, 24.0],
                "temperature_2m_min": [18.0, 14.0, 16.0]
            }
        });

        let report = WeatherReport::from_open_meteo_json("Here", json.to_string())
            .expect("valid Open-Meteo report");

        let today_label = date_label(&now.date().to_string()).expect("today should format");
        let yesterday_label = date_label(
            &now.saturating_sub(time::Duration::days(1))
                .date()
                .to_string(),
        )
        .expect("yesterday should format");
        assert!(report.raw().contains(&today_label));
        assert!(!report.raw().contains(&yesterday_label));
    }

    #[test]
    fn open_meteo_forecast_uses_response_timezone_for_today() {
        let provider_now = OffsetDateTime::now_utc()
            .to_offset(UtcOffset::from_whole_seconds(14 * 60 * 60).expect("valid offset"));
        let yesterday_utc = OffsetDateTime::now_utc()
            .saturating_sub(time::Duration::days(1))
            .date()
            .to_string();
        let provider_today = provider_now.date().to_string();
        let json = serde_json::json!({
            "utc_offset_seconds": 14 * 60 * 60,
            "hourly": {
                "time": [format!("{yesterday_utc}T12:00"), format!("{provider_today}T12:00")],
                "temperature_2m": [10.0, 22.0],
                "apparent_temperature": [11.0, 23.0],
                "weather_code": [0, 61],
                "wind_speed_10m": [7.0, 8.0],
                "precipitation": [0.0, 0.4]
            },
            "daily": {
                "time": [yesterday_utc, provider_today.clone()],
                "weather_code": [0, 61],
                "temperature_2m_max": [30.0, 22.0],
                "temperature_2m_min": [18.0, 14.0]
            }
        });

        let report = WeatherReport::from_open_meteo_json("Here", json.to_string())
            .expect("valid Open-Meteo report");

        let today_label = date_label(&provider_today).expect("provider today should format");
        assert!(report.raw().contains(&today_label));
    }

    #[test]
    fn dialog_height_is_capped_at_eighty_percent_of_app_height() {
        let dialog = WeatherForecastDialog::<()>::new()
            .content((0..100).map(|index| format!("forecast line {index}")));

        assert_eq!(dialog.dialog_area(Rect::new(0, 0, 120, 50)).height, 40);
    }

    #[test]
    fn prints_one_full_weather_day_for_visual_tuning() {
        let day = r#"┌──────────────────────────────┬─────────────────┤ Thu 25 Jun  16/30 °C ├──────────────────┬─────────────────────────────┐
│           Morning            │             Noon             │           Evening           │            Night            │
├──────────────────────────────┼──────────────────────────────┼─────────────────────────────┼─────────────────────────────┤
│   \   /     Sunny            │   \   /     Sunny            │   \   /     Sunny           │   \   /     Sunny           │
│    .-.      23(25) °C        │    .-.      28(29) °C        │    .-.      30(31) °C       │    .-.      25(27) °C       │
│ ― (   ) ―  13 km/h           │ ― (   ) ―  14 km/h           │ ― (   ) ―  18 km/h          │ ― (   ) ―  19 km/h          │
│    `-’      10 km            │    `-’      10 km            │    `-’      10 km           │    `-’      10 km           │
│   /   \     0.0 mm | 6%      │   /   \     0.0 mm | 3%      │   /   \     0.0 mm | 2%     │   /   \     0.0 mm | 5%     │
└─ 󰖨 Lux 30m --:-- · 20m --:-- ┴──────────────────────────────┴─────────────────────────────┴─────────────────────────────┘"#;
        let lines = day.lines().collect::<Vec<_>>();
        let body = lines[2..lines.len() - 1].join("\n");

        assert!(
            lines
                .iter()
                .take(lines.len() - 1)
                .all(|line| display_width(line) == display_width(lines[0]))
        );
        assert_eq!(
            WeatherForecastDay::new("Thu 25 Jun", "Sunny", "16/30 °C", body).render_text(),
            day
        );
        println!("{day}");
    }

    #[test]
    fn walk_forecast_uses_earliest_aligned_interval_meeting_each_threshold() {
        let date = "2026-07-11";
        let times =
            ["06:45", "07:00", "07:15", "07:30", "07:45"].map(|time| format!("{date}T{time}"));
        let radiation = [10.0, 21.0, 84.0, 84.0, 84.0];

        assert_eq!(
            walk_footer(date, &times, &radiation),
            "󰖨 Lux 30m 06:45 · 20m 07:00"
        );
    }

    #[test]
    fn walk_forecast_requires_every_interval_covering_duration() {
        let date = "2026-07-11";
        let times = ["07:00", "07:15", "07:30"].map(|time| format!("{date}T{time}"));
        let radiation = [100.0, 10.0, 100.0];

        assert_eq!(
            walk_footer(date, &times, &radiation),
            "󰖨 Lux 30m --:-- · 20m --:--"
        );
    }

    #[test]
    fn walk_forecast_rejects_non_quarter_hour_samples() {
        let date = "2026-07-11";
        let times = ["07:01", "07:16"].map(|time| format!("{date}T{time}"));
        let radiation = [100.0, 100.0];

        assert_eq!(
            walk_footer(date, &times, &radiation),
            "󰖨 Lux 30m --:-- · 20m --:--"
        );
    }
}
