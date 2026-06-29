use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use time::{OffsetDateTime, UtcOffset};

use super::status_action::{StatusAction, measured_line, register_status_focus};
use crate::{
    Animated, AnimationSettings, EventCtx, EventOutcome, FocusCtx, FocusId, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, TickResult, TuiNode,
};

const WEATHER_FOCUS: &str = "weather-indicator";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherSummary {
    temperature: String,
    condition: String,
    location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherReport {
    raw: String,
    summary: WeatherSummary,
    hourly: Option<HourlyWeather>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HourlyWeather {
    hours: Vec<WeatherHour>,
    utc_offset_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WeatherHour {
    time: String,
    temperature: String,
    condition: String,
}

pub struct WeatherIndicator<M = ()> {
    report: Option<WeatherReport>,
    placeholder: String,
    loading: bool,
    refresh_needed: bool,
    use_ascii_icon: bool,
    tab_stop: bool,
    action: StatusAction<M>,
}

impl WeatherSummary {
    pub fn new(temperature: impl Into<String>, condition: impl Into<String>) -> Self {
        Self {
            temperature: temperature.into(),
            condition: condition.into(),
            location: None,
        }
    }

    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn temperature(&self) -> &str {
        &self.temperature
    }

    pub fn condition(&self) -> &str {
        &self.condition
    }

    pub fn location_name(&self) -> Option<&str> {
        self.location.as_deref()
    }
}

impl WeatherReport {
    pub fn custom(temperature: impl Into<String>, condition: impl Into<String>) -> Self {
        let summary = WeatherSummary::new(temperature, condition);
        let raw = format!("{} {}", summary.temperature, summary.condition);
        Self {
            raw,
            summary,
            hourly: None,
        }
    }

    pub fn from_wttr_text(text: impl Into<String>) -> Self {
        let raw = strip_ansi(&text.into());
        let summary = WeatherSummary {
            temperature: parse_temperature(&raw).unwrap_or_else(|| "--".to_string()),
            condition: parse_condition(&raw).unwrap_or_else(|| "Weather".to_string()),
            location: parse_location(&raw),
        };
        Self {
            raw,
            summary,
            hourly: None,
        }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn summary(&self) -> &WeatherSummary {
        &self.summary
    }

    pub(crate) fn refresh_summary(&mut self) -> bool {
        let Some(hourly) = &self.hourly else {
            return false;
        };
        let Some(summary) = hourly.current_summary(self.summary.location.clone()) else {
            return false;
        };
        if summary == self.summary {
            return false;
        }
        self.summary = summary;
        true
    }

    pub(crate) fn hourly_data_expired(&self) -> bool {
        self.hourly
            .as_ref()
            .is_some_and(HourlyWeather::expired_for_current_hour)
    }

    pub fn with_raw(mut self, raw: impl Into<String>) -> Self {
        self.raw = strip_ansi(&raw.into());
        self
    }

    pub(crate) fn from_parts(raw: impl Into<String>, summary: WeatherSummary) -> Self {
        Self {
            raw: raw.into(),
            summary,
            hourly: None,
        }
    }

    pub(crate) fn with_hourly(mut self, hourly: HourlyWeather) -> Self {
        self.hourly = Some(hourly);
        self.refresh_summary();
        self
    }
}

impl HourlyWeather {
    pub(crate) fn new(entries: impl IntoIterator<Item = (String, String, String)>) -> Self {
        Self {
            hours: entries
                .into_iter()
                .map(|(time, temperature, condition)| WeatherHour {
                    time,
                    temperature,
                    condition,
                })
                .collect(),
            utc_offset_seconds: None,
        }
    }

    pub(crate) fn with_utc_offset(mut self, utc_offset_seconds: Option<i32>) -> Self {
        self.utc_offset_seconds = utc_offset_seconds;
        self
    }

    fn current_summary(&self, location: Option<String>) -> Option<WeatherSummary> {
        let index = self.current_hour_index()?;
        let hour = self.hours.get(index)?;
        let mut summary = WeatherSummary::new(hour.temperature.clone(), hour.condition.clone());
        summary.location = location;
        Some(summary)
    }

    fn expired_for_current_hour(&self) -> bool {
        self.current_hour_index().is_none()
    }

    fn current_hour_index(&self) -> Option<usize> {
        let now = self.now();
        let today = format!("{}T", now.date());
        let current_hour = now.time().hour();
        let same_day = self
            .hours
            .iter()
            .enumerate()
            .filter_map(|(index, hour)| {
                hour.time
                    .strip_prefix(&today)
                    .and_then(|time| hourly_hour(time).map(|hour| (index, hour)))
            })
            .collect::<Vec<_>>();

        same_day
            .iter()
            .rev()
            .find(|(_, hour)| *hour <= current_hour)
            .or_else(|| same_day.first())
            .map(|(index, _)| *index)
    }

    fn now(&self) -> OffsetDateTime {
        self.utc_offset_seconds
            .and_then(|seconds| UtcOffset::from_whole_seconds(seconds).ok())
            .map(|offset| OffsetDateTime::now_utc().to_offset(offset))
            .unwrap_or_else(|| {
                OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
            })
    }
}

impl<M> WeatherIndicator<M> {
    pub fn new() -> Self {
        Self {
            report: None,
            placeholder: "Weather".to_string(),
            loading: false,
            refresh_needed: false,
            use_ascii_icon: false,
            tab_stop: true,
            action: StatusAction::new(),
        }
    }

    pub fn report(mut self, report: WeatherReport) -> Self {
        self.report = Some(report);
        self.loading = false;
        self.refresh_needed = false;
        self
    }

    pub fn set_report(&mut self, report: WeatherReport) {
        self.report = Some(report);
        self.loading = false;
        self.refresh_needed = false;
    }

    pub fn clear_report(&mut self) {
        self.report = None;
        self.refresh_needed = false;
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<String>) {
        self.placeholder = placeholder.into();
    }

    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn refresh_needed(&self) -> bool {
        self.refresh_needed
    }

    pub fn use_ascii_icon(mut self, use_ascii_icon: bool) -> Self {
        self.use_ascii_icon = use_ascii_icon;
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.action.set_hotkey(hotkey);
        self
    }

    pub fn tab_stop(mut self, tab_stop: bool) -> Self {
        self.tab_stop = tab_stop;
        self
    }

    pub fn on_open(mut self, handler: impl Fn() -> M + 'static) -> Self {
        self.action.set_on_press(handler);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.action.set_focused_immediate(focused);
        self
    }

    pub fn is_focused(&self) -> bool {
        self.action.focused()
    }

    pub fn set_focused(&mut self, focused: bool, settings: AnimationSettings) {
        self.action.set_focused(focused, settings);
    }

    pub(crate) fn label(&self) -> String {
        if self.loading {
            return format!(
                "{} Loading…",
                weather_condition_icon("loading", self.use_ascii_icon)
            );
        }
        let Some(report) = &self.report else {
            return format!(
                "{} {}",
                weather_condition_icon("weather", self.use_ascii_icon),
                self.placeholder
            );
        };
        let summary = report.summary();
        format!(
            "{} {} {}",
            weather_condition_icon(summary.condition(), self.use_ascii_icon),
            summary.temperature(),
            summary.condition()
        )
    }

    pub(crate) fn label_spans(
        &self,
        base_style: Style,
        hotkey_style: Style,
    ) -> Vec<ratatui::text::Span<'static>> {
        self.action
            .label_spans(self.label(), base_style, hotkey_style)
    }

    fn line(&self) -> Line<'static> {
        self.action.line(self.label())
    }
}

impl<M> Default for WeatherIndicator<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> TuiNode<M> for WeatherIndicator<M>
where
    M: 'static,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        measured_line(self.line(), proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        if self.action.hotkey().is_some() || self.action.has_press_handler() {
            register_status_focus(ctx, WEATHER_FOCUS, area, self.action.hotkey());
            ctx.set_focus_tab_stop(FocusId::new(WEATHER_FOCUS), self.tab_stop);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut crate::RenderCtx<'_>) {
        frame.render_widget(Paragraph::new(self.line()), area);
    }

    fn event(&mut self, event: &crate::event::TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.action.event(event, ctx)
    }

    fn focus(&mut self, _target: Option<&FocusId>, focused: bool, ctx: &mut FocusCtx<M>) {
        self.set_focused(focused, ctx.animation());
        ctx.request_redraw();
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let action_tick = self.action.tick(dt, settings);
        let Some(report) = &mut self.report else {
            return action_tick;
        };
        let changed = report.refresh_summary();
        let expired = report.hourly_data_expired();
        let refresh_changed = self.refresh_needed != expired;
        self.refresh_needed = expired;
        if changed || refresh_changed {
            TickResult::CHANGED.merge(action_tick)
        } else {
            action_tick
        }
    }
}

fn hourly_hour(time: &str) -> Option<u8> {
    time.get(..2)?.parse().ok()
}

pub fn weather_condition_icon(condition: &str, ascii: bool) -> &'static str {
    let condition = condition.to_ascii_lowercase();
    if condition.contains("storm") || condition.contains("thunder") {
        return if ascii { "!" } else { "" };
    }
    if condition.contains("snow") || condition.contains("sleet") {
        return if ascii { "S" } else { "" };
    }
    if condition.contains("rain") || condition.contains("shower") || condition.contains("drizzle") {
        return if ascii { "R" } else { "" };
    }
    if condition.contains("fog") || condition.contains("mist") || condition.contains("haze") {
        return if ascii { "~" } else { "" };
    }
    if condition.contains("cloud") || condition.contains("overcast") {
        return if ascii { "C" } else { "" };
    }
    if ascii { "*" } else { "" }
}

fn parse_location(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("Weather report:"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_temperature(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let chars = line.char_indices().collect::<Vec<_>>();
        let degree = chars.iter().position(|(_, c)| *c == '°')?;
        let degree_byte = chars[degree].0;
        let mut start = degree;
        while start > 0 && chars[start - 1].1.is_whitespace() {
            start -= 1;
        }
        while start > 0 && !chars[start - 1].1.is_whitespace() {
            start -= 1;
        }
        let start_byte = chars
            .get(start)
            .map(|(index, _)| *index)
            .unwrap_or(degree_byte);
        let unit_end = line[degree_byte..]
            .char_indices()
            .take_while(|(_, c)| !c.is_whitespace())
            .last()
            .map(|(index, c)| degree_byte + index + c.len_utf8())
            .unwrap_or(degree_byte + '°'.len_utf8());
        Some(line[start_byte..unit_end].trim().to_string())
    })
}

fn parse_condition(text: &str) -> Option<String> {
    let conditions = [
        "Sunny",
        "Clear",
        "Partly cloudy",
        "Cloudy",
        "Overcast",
        "Light rain",
        "Rain",
        "Snow",
        "Thunderstorm",
        "Fog",
        "Mist",
    ];
    for line in text.lines().take(8) {
        let line = line.to_ascii_lowercase();
        for condition in conditions {
            if line.contains(&condition.to_ascii_lowercase()) {
                return Some(condition.to_string());
            }
        }
    }
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .find(|line| {
            !line.starts_with("Weather report:")
                && !line.starts_with("Location:")
                && line.chars().any(char::is_alphabetic)
                && !line.contains('│')
                && !line.contains('┌')
                && !line.contains('└')
                && !line.contains('°')
        })
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn strip_ansi(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            stripped.push(ch);
        }
    }
    stripped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wttr_text_extracts_summary_and_strips_ansi() {
        let report = WeatherReport::from_wttr_text(
            "\u{1b}[38;5;226mWeather report: Bussum, North Holland, NL\u{1b}[0m\nSunny\n+22(25) °C\n",
        );

        assert_eq!(
            report.summary().location_name(),
            Some("Bussum, North Holland, NL")
        );
        assert_eq!(report.summary().condition(), "Sunny");
        assert_eq!(report.summary().temperature(), "+22(25) °C");
        assert!(!report.raw().contains('\u{1b}'));
    }

    #[test]
    fn indicator_refreshes_summary_from_hourly_weather_on_tick() {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let current_time = format!("{}T{:02}:00", now.date(), now.time().hour());
        let hourly = HourlyWeather::new([(current_time, "14 °C".to_string(), "Rain".to_string())]);
        let report = WeatherReport {
            raw: "raw".to_string(),
            summary: WeatherSummary::new("30 °C", "Sunny"),
            hourly: Some(hourly),
        };
        let mut indicator = WeatherIndicator::<()>::new().report(report);

        let tick = indicator.tick(Duration::from_secs(60), AnimationSettings::default());

        assert!(tick.changed);
        assert_eq!(indicator.report.unwrap().summary().temperature(), "14 °C");
    }

    #[test]
    fn indicator_marks_refresh_needed_when_hourly_weather_expires() {
        let yesterday = OffsetDateTime::now_local()
            .unwrap_or_else(|_| OffsetDateTime::now_utc())
            .saturating_sub(time::Duration::days(1));
        let expired_time = format!("{}T23:00", yesterday.date());
        let hourly = HourlyWeather::new([(expired_time, "14 °C".to_string(), "Rain".to_string())]);
        let report = WeatherReport::from_parts("raw", WeatherSummary::new("14 °C", "Rain"))
            .with_hourly(hourly);
        let mut indicator = WeatherIndicator::<()>::new().report(report);

        let tick = indicator.tick(Duration::from_secs(60), AnimationSettings::default());

        assert!(tick.changed);
        assert!(indicator.refresh_needed());
    }

    #[test]
    fn indicator_without_action_does_not_register_focus_target() {
        let mut indicator = WeatherIndicator::<()>::new();
        let mut ctx = LayoutCtx::new();

        indicator.layout(Rect::new(0, 0, 20, 1), &mut ctx);

        assert!(ctx.focus_targets().is_empty());
    }

    #[test]
    fn indicator_with_action_registers_focus_target() {
        let mut indicator = WeatherIndicator::new().on_open(|| ());
        let mut ctx = LayoutCtx::new();

        indicator.layout(Rect::new(0, 0, 20, 1), &mut ctx);

        let target = ctx.focus_targets().first().unwrap();
        assert_eq!(target.id.as_str(), WEATHER_FOCUS);
        assert!(target.tab_stop);
    }
}
