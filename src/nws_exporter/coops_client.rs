// nws_exporter - Prometheus metrics exporter for api.weather.gov
//
// Copyright 2026 Jason Griggs
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use reqwest::header::USER_AGENT;
use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;

/// Mean radius of the earth, in nautical miles. Used for the haversine distance calculation in
/// `nearest_station()`.
const EARTH_RADIUS_NMI: f64 = 3440.065;

/// Error resulting from setup of a `CoOpsClient` instance.
///
/// Individual product fetches are best-effort (see `CoOpsClient::observation()`) and never
/// produce a hard error; this only covers construction-time failures.
#[derive(Debug)]
pub enum CoOpsClientError {
    Initialization(String),
}

impl std::fmt::Display for CoOpsClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initialization(msg) => write!(f, "initialization error: {}", msg),
        }
    }
}

impl std::error::Error for CoOpsClientError {}

/// Client for fetching NOAA CO-OPS Tides & Currents observations and tide predictions.
///
/// Data is fetched from the CO-OPS `datagetter` API (typically
/// `https://api.tidesandcurrents.noaa.gov/api/prod/datagetter`) and the CO-OPS station metadata
/// API (typically
/// `https://api.tidesandcurrents.noaa.gov/mdapi/prod/webapi/stations.json?type=tidepredictions`).
#[derive(Debug)]
pub struct CoOpsClient {
    client: Client,
    base_url: Url,
    station_list_url: Url,
}

impl CoOpsClient {
    const USER_AGENT: &'static str =
        "nws_exporter/0.8.0 (https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support)";

    /// Create a new `CoOpsClient` from the provided reqwest client, base URL for the CO-OPS
    /// `datagetter` API, and URL for the CO-OPS tide-prediction station metadata list.
    pub fn new(client: Client, base_url: &str, station_list_url: &str) -> Result<Self, CoOpsClientError> {
        Ok(CoOpsClient {
            client,
            base_url: base_url
                .parse()
                .map_err(|e| CoOpsClientError::Initialization(format!("cannot parse {}: {}", base_url, e)))?,
            station_list_url: station_list_url.parse().map_err(|e| {
                CoOpsClientError::Initialization(format!("cannot parse {}: {}", station_list_url, e))
            })?,
        })
    }

    /// Fetch the latest observation and tide schedule for a CO-OPS station. Every field is
    /// independently best-effort: a missing sensor, an unsupported product for that station, or
    /// a transient network failure all simply leave the corresponding field `None` rather than
    /// failing the whole call, since callers (`BuoyMetrics::apply_coops`) already fall back to
    /// NDBC data field-by-field.
    pub async fn observation(&self, station_id: &str) -> CoOpsObservation {
        let (water_level, predictions, wind, air_temp, water_temp, pressure) = tokio::join!(
            self.get(
                station_id,
                "water_level",
                "english",
                &[("datum", "MLLW"), ("date", "latest")]
            ),
            self.get(
                station_id,
                "predictions",
                "english",
                &[("datum", "MLLW"), ("interval", "hilo"), ("date", "today"), ("range", "48")]
            ),
            self.get(station_id, "wind", "metric", &[("date", "latest")]),
            self.get(station_id, "air_temperature", "metric", &[("date", "latest")]),
            self.get(station_id, "water_temperature", "metric", &[("date", "latest")]),
            self.get(station_id, "air_pressure", "metric", &[("date", "latest")]),
        );

        let (wind_direction_degrees, wind_speed_mps, wind_gust_mps) =
            wind.as_deref().map(parse_wind).unwrap_or((None, None, None));
        let (next_high, next_low) = predictions
            .as_deref()
            .map(|body| parse_predictions(body, Utc::now()))
            .unwrap_or((None, None));

        CoOpsObservation {
            station_id: station_id.to_string(),
            tide_feet: water_level.as_deref().and_then(parse_single_value),
            wind_direction_degrees,
            wind_speed_mps,
            wind_gust_mps,
            pressure_hpa: pressure.as_deref().and_then(parse_single_value),
            air_temp_celsius: air_temp.as_deref().and_then(parse_single_value),
            water_temp_celsius: water_temp.as_deref().and_then(parse_single_value),
            next_high_tide_unix: next_high.map(|(t, _)| t),
            next_high_tide_feet: next_high.map(|(_, v)| v),
            next_low_tide_unix: next_low.map(|(t, _)| t),
            next_low_tide_feet: next_low.map(|(_, v)| v),
        }
    }

    /// Fetch the list of CO-OPS stations that support tide predictions, for nearest-station
    /// matching against buoy coordinates. Best-effort: any failure logs a warning and returns
    /// an empty list, so callers simply find no match rather than crashing.
    pub async fn station_list(&self) -> Vec<CoOpsStation> {
        let res = match self
            .client
            .get(self.station_list_url.clone())
            .header(USER_AGENT, Self::USER_AGENT)
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!(message = "failed to fetch CO-OPS station list", error = %e);
                return Vec::new();
            }
        };

        let status = res.status();
        if status != StatusCode::OK {
            tracing::warn!(message = "unexpected status fetching CO-OPS station list", status = %status);
            return Vec::new();
        }

        let body = match res.text().await {
            Ok(body) => body,
            Err(e) => {
                tracing::warn!(message = "failed to read CO-OPS station list response", error = %e);
                return Vec::new();
            }
        };

        match serde_json::from_str::<StationListResponse>(&body) {
            Ok(parsed) => parsed
                .stations
                .into_iter()
                .map(|s| CoOpsStation {
                    id: s.id,
                    lat: s.lat,
                    lng: s.lng,
                })
                .collect(),
            Err(e) => {
                tracing::warn!(message = "failed to parse CO-OPS station list", error = %e);
                Vec::new()
            }
        }
    }

    /// Fetch a single CO-OPS `datagetter` product for a station and return the raw response
    /// body, or `None` on any transport/HTTP-level failure. CO-OPS returns HTTP 200 with an
    /// `{"error": ...}` body for unsupported product/station combinations rather than a 4xx, so
    /// that case is detected by the per-product parse functions, not here.
    async fn get(&self, station_id: &str, product: &str, units: &str, extra: &[(&str, &str)]) -> Option<String> {
        let mut url = self.base_url.clone();
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("station", station_id)
                .append_pair("product", product)
                .append_pair("units", units)
                .append_pair("time_zone", "gmt")
                .append_pair("format", "json")
                .append_pair("application", "nws_exporter");
            for (key, value) in extra {
                query.append_pair(key, value);
            }
        }

        tracing::debug!(message = "fetching CO-OPS product", url = %url);

        let res = match self.client.get(url.clone()).header(USER_AGENT, Self::USER_AGENT).send().await {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!(message = "failed to fetch CO-OPS product", product = %product, station = %station_id, error = %e);
                return None;
            }
        };

        let status = res.status();
        if status != StatusCode::OK {
            // CO-OPS returns plain 4xx (in addition to the 200+{"error":...} shape handled by
            // the per-product parse functions) for products a station simply doesn't support,
            // e.g. a tide-predictions-only subordinate station has no real-time sensors at all.
            // That's routine, not exceptional, so it's logged at debug rather than warn.
            tracing::debug!(message = "unexpected status fetching CO-OPS product", product = %product, station = %station_id, status = %status);
            return None;
        }

        match res.text().await {
            Ok(body) => Some(body),
            Err(e) => {
                tracing::warn!(message = "failed to read CO-OPS product response", product = %product, station = %station_id, error = %e);
                None
            }
        }
    }
}

/// Find the nearest CO-OPS station to the given coordinates, among `stations`, within
/// `max_nmi` nautical miles. Returns the matched station ID and its distance, or `None` if no
/// station is within range.
pub fn nearest_station(buoy_lat: f64, buoy_lon: f64, stations: &[CoOpsStation], max_nmi: f64) -> Option<(String, f64)> {
    stations
        .iter()
        .map(|s| (s.id.clone(), haversine_nmi(buoy_lat, buoy_lon, s.lat, s.lng)))
        .filter(|(_, distance)| *distance <= max_nmi)
        .min_by(|a, b| a.1.partial_cmp(&b.1).expect("distance is never NaN"))
}

/// Great-circle distance between two coordinates, in nautical miles.
fn haversine_nmi(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (lat1, lon1, lat2, lon2) = (lat1.to_radians(), lon1.to_radians(), lat2.to_radians(), lon2.to_radians());
    let dlat = lat2 - lat1;
    let dlon = lon2 - lon1;
    let a = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    EARTH_RADIUS_NMI * 2.0 * a.sqrt().asin()
}

/// A CO-OPS station that supports tide predictions (either a reference station with real-time
/// sensors, or a subordinate station with predictions only).
#[derive(Debug, Clone, PartialEq)]
pub struct CoOpsStation {
    pub id: String,
    pub lat: f64,
    pub lng: f64,
}

/// A CO-OPS observation: current readings (where the station has the relevant sensor) plus the
/// next upcoming high and low tide predictions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CoOpsObservation {
    pub station_id: String,
    pub tide_feet: Option<f64>,
    pub wind_direction_degrees: Option<f64>,
    pub wind_speed_mps: Option<f64>,
    pub wind_gust_mps: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub air_temp_celsius: Option<f64>,
    pub water_temp_celsius: Option<f64>,
    pub next_high_tide_unix: Option<i64>,
    pub next_high_tide_feet: Option<f64>,
    pub next_low_tide_unix: Option<i64>,
    pub next_low_tide_feet: Option<f64>,
}

#[derive(Deserialize)]
struct ErrorResponse {
    #[allow(dead_code)]
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[allow(dead_code)]
    message: String,
}

#[derive(Deserialize)]
struct ValueRow {
    v: String,
}

#[derive(Deserialize)]
struct ValueResponse {
    data: Vec<ValueRow>,
}

#[derive(Deserialize)]
struct WindRow {
    d: String,
    s: String,
    g: String,
}

#[derive(Deserialize)]
struct WindResponse {
    data: Vec<WindRow>,
}

#[derive(Deserialize)]
struct PredictionRow {
    t: String,
    v: String,
    r#type: String,
}

#[derive(Deserialize)]
struct PredictionsResponse {
    predictions: Vec<PredictionRow>,
}

#[derive(Deserialize)]
struct StationRow {
    id: String,
    lat: f64,
    lng: f64,
}

#[derive(Deserialize)]
struct StationListResponse {
    stations: Vec<StationRow>,
}

/// Parse a CO-OPS `{"data": [{"v": "..."}]}` response (used by `water_level`,
/// `air_temperature`, `water_temperature`, and `air_pressure`) into the most recent value.
/// Returns `None` for an `{"error": ...}` response, empty data, or any parse failure.
fn parse_single_value(body: &str) -> Option<f64> {
    if serde_json::from_str::<ErrorResponse>(body).is_ok() {
        return None;
    }
    let resp: ValueResponse = serde_json::from_str(body).ok()?;
    resp.data.first()?.v.parse().ok()
}

/// Parse a CO-OPS `wind` product response into `(direction_degrees, speed_mps, gust_mps)`.
fn parse_wind(body: &str) -> (Option<f64>, Option<f64>, Option<f64>) {
    if serde_json::from_str::<ErrorResponse>(body).is_ok() {
        return (None, None, None);
    }
    let resp: Result<WindResponse, _> = serde_json::from_str(body);
    let Some(row) = resp.ok().and_then(|r| r.data.into_iter().next()) else {
        return (None, None, None);
    };
    (row.d.parse().ok(), row.s.parse().ok(), row.g.parse().ok())
}

/// A predicted tide event: unix timestamp and height, in feet.
type TidePrediction = (i64, f64);

/// Parse a CO-OPS `predictions` (`interval=hilo`) response into the next upcoming high and low
/// tide, relative to `now`. Predictions at or before `now` are ignored; among the remaining rows
/// of each type, the earliest is kept. Returns `(None, None)` for an `{"error": ...}` response or
/// any parse failure.
fn parse_predictions(body: &str, now: DateTime<Utc>) -> (Option<TidePrediction>, Option<TidePrediction>) {
    if serde_json::from_str::<ErrorResponse>(body).is_ok() {
        return (None, None);
    }
    let Ok(resp) = serde_json::from_str::<PredictionsResponse>(body) else {
        return (None, None);
    };

    let mut next_high: Option<TidePrediction> = None;
    let mut next_low: Option<TidePrediction> = None;

    for row in &resp.predictions {
        let Ok(naive) = NaiveDateTime::parse_from_str(&row.t, "%Y-%m-%d %H:%M") else {
            continue;
        };
        let timestamp = Utc.from_utc_datetime(&naive);
        if timestamp < now {
            continue;
        }
        let Ok(height) = row.v.parse::<f64>() else {
            continue;
        };
        let unix = timestamp.timestamp();

        let slot = match row.r#type.as_str() {
            "H" => &mut next_high,
            "L" => &mut next_low,
            _ => continue,
        };
        if slot.is_none_or(|(t, _)| unix < t) {
            *slot = Some((unix, height));
        }
    }

    (next_high, next_low)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_single_value() {
        let body = r#"{"metadata":{"id":"8454000"},"data":[{"t":"2026-06-23 04:06", "v":"2.351", "s":"0.033", "f":"0,0,0,0", "q":"p"}]}"#;
        assert_eq!(parse_single_value(body), Some(2.351));
    }

    #[test]
    fn test_parse_single_value_error_response() {
        let body = r#"{"error":{"message":"No data was found. Please make sure the Datum input is valid."}}"#;
        assert_eq!(parse_single_value(body), None);
    }

    #[test]
    fn test_parse_wind() {
        let body = r#"{"data":[{"t":"2026-06-23 04:06", "s":"11.86", "d":"155.0", "dr":"SSE", "g":"14.19", "f":"0,0"}]}"#;
        assert_eq!(parse_wind(body), (Some(155.0), Some(11.86), Some(14.19)));
    }

    #[test]
    fn test_parse_wind_error_response() {
        let body = r#"{"error":{"message":"No data was found."}}"#;
        assert_eq!(parse_wind(body), (None, None, None));
    }

    #[test]
    fn test_parse_predictions_picks_next_after_now() {
        let body = r#"{ "predictions" : [
            {"t":"2026-06-23 02:48", "v":"1.346", "type":"L"},
            {"t":"2026-06-23 07:24", "v":"3.974", "type":"H"},
            {"t":"2026-06-23 12:30", "v":"0.915", "type":"L"},
            {"t":"2026-06-23 20:00", "v":"4.686", "type":"H"}
        ]}"#;
        let now = Utc.with_ymd_and_hms(2026, 6, 23, 5, 0, 0).unwrap();

        let (next_high, next_low) = parse_predictions(body, now);

        assert_eq!(
            next_high,
            Some((Utc.with_ymd_and_hms(2026, 6, 23, 7, 24, 0).unwrap().timestamp(), 3.974))
        );
        assert_eq!(
            next_low,
            Some((Utc.with_ymd_and_hms(2026, 6, 23, 12, 30, 0).unwrap().timestamp(), 0.915))
        );
    }

    #[test]
    fn test_parse_predictions_error_response() {
        let body = r#"{"error":{"message":"No Predictions data was found."}}"#;
        assert_eq!(parse_predictions(body, Utc::now()), (None, None));
    }

    #[test]
    fn test_nearest_station_within_range() {
        let stations = vec![
            CoOpsStation {
                id: "8443970".to_string(),
                lat: 42.3548,
                lng: -71.0534,
            },
            CoOpsStation {
                id: "8418150".to_string(),
                lat: 43.6564,
                lng: -70.2483,
            },
        ];

        // Boston buoy 44013, roughly 16nmi east of Boston Harbor.
        let result = nearest_station(42.346, -70.651, &stations, 50.0);
        assert_eq!(result.map(|(id, _)| id), Some("8443970".to_string()));
    }

    #[test]
    fn test_nearest_station_out_of_range() {
        let stations = vec![CoOpsStation {
            id: "8443970".to_string(),
            lat: 42.3548,
            lng: -71.0534,
        }];

        // Far offshore, well outside any reasonable radius.
        let result = nearest_station(35.0, -50.0, &stations, 50.0);
        assert_eq!(result, None);
    }
}
