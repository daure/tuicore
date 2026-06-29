use std::{sync::mpsc, thread, time::Duration};

use reqwest::header::ACCEPT;

use super::weather_indicator::WeatherReport;

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_LATITUDE: f64 = 52.2773656;
const DEFAULT_LONGITUDE: f64 = 5.1630646;
const DEFAULT_LOCATION: &str = "Bussum, North Holland, NL";
const USER_AGENT: &str = "tuicore-weather/0.1";

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherProviderConfig {
    enabled: bool,
    latitude: f64,
    longitude: f64,
    location: String,
    refresh_interval: Duration,
    timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeatherFetchError {
    message: String,
}

pub(crate) type WeatherFetchReceiver = mpsc::Receiver<Result<WeatherReport, WeatherFetchError>>;

impl WeatherProviderConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn coordinates(mut self, latitude: f64, longitude: f64) -> Self {
        self.latitude = latitude;
        self.longitude = longitude;
        self
    }

    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = location.into();
        self
    }

    pub fn refresh_interval(mut self, refresh_interval: Duration) -> Self {
        self.refresh_interval = refresh_interval;
        self
    }

    pub fn refresh_interval_value(&self) -> Duration {
        self.refresh_interval
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Default for WeatherProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            latitude: DEFAULT_LATITUDE,
            longitude: DEFAULT_LONGITUDE,
            location: DEFAULT_LOCATION.to_string(),
            refresh_interval: DEFAULT_REFRESH_INTERVAL,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl std::fmt::Display for WeatherFetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WeatherFetchError {}

impl WeatherFetchError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) fn spawn_weather_fetch(config: WeatherProviderConfig) -> WeatherFetchReceiver {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let result = fetch_weather_report(&config);
        let _ = tx.send(result);
    });
    rx
}

fn fetch_weather_report(
    config: &WeatherProviderConfig,
) -> Result<WeatherReport, WeatherFetchError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(config.timeout)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| WeatherFetchError::new(format!("weather client failed: {error}")))?;
    let json = client
        .get(open_meteo_url(config))
        .header(ACCEPT, "application/json")
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.text())
        .map_err(|error| WeatherFetchError::new(format!("weather fetch failed: {error}")))?;
    let json = json_weather_response(json)?;
    WeatherReport::from_open_meteo_json(config.location.clone(), json)
        .map_err(|error| WeatherFetchError::new(format!("weather parse failed: {error}")))
}

fn json_weather_response(text: String) -> Result<String, WeatherFetchError> {
    let trimmed = text.trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("<!doctype") || lower.starts_with("<html") {
        return Err(WeatherFetchError::new(
            "weather provider returned HTML instead of Open-Meteo JSON",
        ));
    }
    Ok(text)
}

fn open_meteo_url(config: &WeatherProviderConfig) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,precipitation,precipitation_probability,visibility&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone=auto&forecast_days=7",
        config.latitude, config.longitude
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_is_disabled_until_explicitly_enabled() {
        let config = WeatherProviderConfig::default();
        let url = open_meteo_url(&config);

        assert!(!config.is_enabled());
        assert!(url.starts_with("https://api.open-meteo.com/v1/forecast?"));
        assert!(url.contains("forecast_days=7"));
        assert!(url.contains("daily=weather_code,temperature_2m_max,temperature_2m_min"));
    }

    #[test]
    fn provider_can_be_explicitly_enabled() {
        let config = WeatherProviderConfig::new().enabled(true);

        assert!(config.is_enabled());
    }

    #[test]
    fn provider_coordinates_are_used_in_open_meteo_url() {
        let config = WeatherProviderConfig::new()
            .coordinates(40.7128, -74.006)
            .location("New York, US");
        let url = open_meteo_url(&config);

        assert!(url.contains("latitude=40.7128"));
        assert!(url.contains("longitude=-74.006"));
    }

    #[test]
    fn html_weather_response_is_rejected_before_parsing() {
        let error = json_weather_response("<!DOCTYPE html><html></html>".to_string())
            .expect_err("html should not be treated as weather json");

        assert!(error.message().contains("returned HTML"));
    }
}
