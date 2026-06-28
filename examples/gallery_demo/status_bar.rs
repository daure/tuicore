use serde_json::json;
use time::OffsetDateTime;

use tuicore::WeatherReport;

const LOCATION: &str = "Bussum, North Holland, NL";

pub(crate) fn demo_weather_report() -> WeatherReport {
    WeatherReport::from_open_meteo_json(LOCATION, open_meteo_fixture())
        .expect("valid Open-Meteo gallery fixture")
}

fn open_meteo_fixture() -> String {
    let today = OffsetDateTime::now_local()
        .unwrap_or_else(|_| OffsetDateTime::now_utc())
        .date();
    let dates = (0..7)
        .map(|offset| {
            today
                .saturating_add(time::Duration::days(offset))
                .to_string()
        })
        .collect::<Vec<_>>();
    let daily_codes = [0, 61, 80, 3, 51, 2, 0];
    let highs = [30.0, 35.0, 33.0, 28.0, 24.0, 23.0, 22.0];
    let lows = [16.0, 26.0, 25.0, 20.0, 17.0, 15.0, 16.0];
    let mut time = Vec::new();
    let mut temperature_2m = Vec::new();
    let mut apparent_temperature = Vec::new();
    let mut weather_code = Vec::new();
    let mut wind_speed_10m = Vec::new();
    let mut precipitation = Vec::new();
    let mut precipitation_probability = Vec::new();
    let mut visibility = Vec::new();

    for (day, date) in dates.iter().enumerate() {
        for (slot, hour) in ["00:00", "06:00", "12:00", "18:00"].iter().enumerate() {
            time.push(format!("{date}T{hour}"));
            let temperature = lows[day] + ((highs[day] - lows[day]) * slot as f64 / 3.0);
            temperature_2m.push(temperature);
            apparent_temperature.push(temperature + if daily_codes[day] <= 3 { 1.5 } else { 0.5 });
            weather_code.push(match slot {
                0 => daily_codes[day],
                1 => daily_codes[day],
                2 => daily_codes[day].min(3),
                _ => daily_codes[day],
            });
            wind_speed_10m.push(8.0 + day as f64 + slot as f64 * 2.0);
            precipitation.push(if daily_codes[day] >= 50 {
                0.2 + slot as f64 * 0.1
            } else {
                0.0
            });
            precipitation_probability.push(if daily_codes[day] >= 50 {
                20.0 + slot as f64 * 8.0
            } else {
                slot as f64 * 2.0
            });
            visibility.push(if daily_codes[day] >= 50 {
                10000.0
            } else {
                50000.0
            });
        }
    }

    json!({
        "hourly": {
            "time": time,
            "temperature_2m": temperature_2m,
            "apparent_temperature": apparent_temperature,
            "weather_code": weather_code,
            "wind_speed_10m": wind_speed_10m,
            "precipitation": precipitation,
            "precipitation_probability": precipitation_probability,
            "visibility": visibility,
        },
        "daily": {
            "time": dates,
            "weather_code": daily_codes,
            "temperature_2m_max": highs,
            "temperature_2m_min": lows,
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_weather_report_renders_seven_forecast_days() {
        let report = demo_weather_report();

        assert_eq!(
            report
                .raw()
                .lines()
                .filter(|line| line.starts_with('┌'))
                .count(),
            7
        );
    }
}
