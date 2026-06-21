// nws_exporter - Prometheus metrics exporter for api.weather.gov
//
// Copyright 2022 Nick Pillitteri
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

use reqwest::header::USER_AGENT;
use reqwest::{Client, Response, StatusCode, Url};
use std::error;
use std::fmt;

// NDBC uses this string for any column that doesn't have a value for a given observation
const NDBC_MISSING: &str = "MM";

/// Error resulting from setup of or calls to a `BuoyClient` instance.
#[derive(Debug)]
pub enum BuoyClientError {
    Internal(reqwest::Error),
    Initialization(String),
    InvalidStation(String),
    Unexpected(StatusCode, Url),
    Parse(String),
}

impl fmt::Display for BuoyClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(e) => write!(f, "{}", e),
            Self::Initialization(msg) => write!(f, "initialization error: {}", msg),
            Self::InvalidStation(s) => write!(f, "invalid station {}", s),
            Self::Unexpected(status, url) => write!(f, "unexpected status {} for {}", status, url),
            Self::Parse(msg) => write!(f, "unable to parse observation: {}", msg),
        }
    }
}

impl error::Error for BuoyClientError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Internal(e) => Some(e),
            _ => None,
        }
    }
}

/// Client for fetching NOAA NDBC buoy and coastal station observations.
///
/// Data is fetched from the NDBC "realtime2" plain text feed for a station (e.g.
/// `https://www.ndbc.noaa.gov/data/realtime2/45186.txt`), which contains the most recent
/// observations for that station, one per line, most recent first.
#[derive(Debug)]
pub struct BuoyClient {
    client: Client,
    base_url: Url,
}

impl BuoyClient {
    const USER_AGENT: &'static str = "nws_exporter/0.6.0 (https://github.com/56quarters/nws_exporter)";

    /// Create a new `BuoyClient` from the provided reqwest client and base URL for the NDBC
    /// realtime2 data feed (typically `https://www.ndbc.noaa.gov/data/realtime2/`).
    pub fn new(client: Client, base_url: &str) -> Result<Self, BuoyClientError> {
        Ok(BuoyClient {
            client,
            base_url: base_url
                .parse()
                .map_err(|e| BuoyClientError::Initialization(format!("cannot parse {}: {}", base_url, e)))?,
        })
    }

    /// Fetch the most recent observation for the given buoy or coastal station ID.
    pub async fn observation(&self, station_id: &str) -> Result<BuoyObservation, BuoyClientError> {
        let url = self.observation_url(station_id);
        tracing::debug!(message = "fetching buoy observation", url = %url);

        let res = self.make_request(station_id, url).await?;
        let body = res.text().await.map_err(BuoyClientError::Internal)?;
        parse_latest_observation(station_id, &body)
    }

    async fn make_request<S: Into<String>>(&self, station_id: S, url: Url) -> Result<Response, BuoyClientError> {
        let res = self
            .client
            .get(url.clone())
            .header(USER_AGENT, Self::USER_AGENT)
            .send()
            .await
            .map_err(BuoyClientError::Internal)?;

        let status = res.status();
        if status == StatusCode::OK {
            Ok(res)
        } else if status == StatusCode::NOT_FOUND {
            Err(BuoyClientError::InvalidStation(station_id.into()))
        } else {
            Err(BuoyClientError::Unexpected(status, url))
        }
    }

    fn observation_url(&self, station_id: &str) -> Url {
        self.base_url
            .join(&format!("{}.txt", station_id.to_uppercase()))
            .expect("unable to build observation URL")
    }
}

/// Parse the most recent observation out of the body of an NDBC realtime2 text response.
///
/// The response is made up of two header lines (column names and units, both prefixed with
/// `#`) followed by one line per observation, most recent first.
fn parse_latest_observation(station_id: &str, body: &str) -> Result<BuoyObservation, BuoyClientError> {
    let data_line = body
        .lines()
        .find(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .ok_or_else(|| BuoyClientError::Parse(format!("no observations found for station {}", station_id)))?;

    let fields: Vec<&str> = data_line.split_whitespace().collect();
    if fields.len() < 17 {
        return Err(BuoyClientError::Parse(format!(
            "expected at least 17 columns, found {}",
            fields.len()
        )));
    }

    let year = fields[0];
    let month = fields[1];
    let day = fields[2];
    let hour = fields[3];
    let minute = fields[4];

    Ok(BuoyObservation {
        station_id: station_id.to_uppercase(),
        timestamp: format!("{}-{}-{}T{}:{}:00Z", year, month, day, hour, minute),
        wind_direction_degrees: ndbc_field(fields[5]),
        wind_speed_mps: ndbc_field(fields[6]),
        wind_gust_mps: ndbc_field(fields[7]),
        wave_height_meters: ndbc_field(fields[8]),
        dominant_wave_period_secs: ndbc_field(fields[9]),
        average_wave_period_secs: ndbc_field(fields[10]),
        wave_direction_degrees: ndbc_field(fields[11]),
        pressure_hpa: ndbc_field(fields[12]),
        air_temp_celsius: ndbc_field(fields[13]),
        water_temp_celsius: ndbc_field(fields[14]),
        dewpoint_celsius: ndbc_field(fields[15]),
        visibility_nmi: ndbc_field(fields[16]),
        pressure_tendency_hpa: fields.get(17).copied().and_then(ndbc_field),
        tide_feet: fields.get(18).copied().and_then(ndbc_field),
    })
}

/// Parse a single NDBC column, treating the "MM" (missing measurement) sentinel as `None`.
fn ndbc_field(raw: &str) -> Option<f64> {
    if raw == NDBC_MISSING {
        None
    } else {
        raw.parse().ok()
    }
}

/// A single observation from a NOAA NDBC buoy or coastal weather station.
#[derive(Debug, Clone, PartialEq)]
pub struct BuoyObservation {
    /// Buoy or station ID (e.g. "45186")
    pub station_id: String,
    /// UTC timestamp of the observation, formatted as an RFC 3339 string
    pub timestamp: String,
    pub wind_direction_degrees: Option<f64>,
    pub wind_speed_mps: Option<f64>,
    pub wind_gust_mps: Option<f64>,
    pub wave_height_meters: Option<f64>,
    pub dominant_wave_period_secs: Option<f64>,
    pub average_wave_period_secs: Option<f64>,
    pub wave_direction_degrees: Option<f64>,
    pub pressure_hpa: Option<f64>,
    pub air_temp_celsius: Option<f64>,
    pub water_temp_celsius: Option<f64>,
    pub dewpoint_celsius: Option<f64>,
    pub visibility_nmi: Option<f64>,
    pub pressure_tendency_hpa: Option<f64>,
    pub tide_feet: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const WAUKEGAN_45186: &str = "\
#YY  MM DD hh mm WDIR WSPD GST  WVHT   DPD   APD MWD   PRES  ATMP  WTMP  DEWP  VIS PTDY  TIDE
#yr  mo dy hr mn degT m/s  m/s     m   sec   sec degT   hPa  degC  degC  degC  nmi  hPa    ft
2026 06 20 20 10 300  5.0  6.0   0.2    MM    MM  17 1014.1  19.9  11.9    MM   MM   MM    MM
2026 06 20 20 00 290  5.0  8.0   0.2    MM    MM   6 1014.2  20.5  11.8    MM   MM -0.5    MM
";

    const SDBC1: &str = "\
#YY  MM DD hh mm WDIR WSPD GST  WVHT   DPD   APD MWD   PRES  ATMP  WTMP  DEWP  VIS PTDY  TIDE
#yr  mo dy hr mn degT m/s  m/s     m   sec   sec degT   hPa  degC  degC  degC  nmi  hPa    ft
2026 06 20 19 54  MM   MM   MM    MM    MM    MM  MM 1015.6    MM  20.0    MM   MM   MM    MM
";

    #[test]
    fn test_parse_latest_observation_full_fields() {
        let obs = parse_latest_observation("45186", WAUKEGAN_45186).unwrap();
        assert_eq!(obs.station_id, "45186");
        assert_eq!(obs.timestamp, "2026-06-20T20:10:00Z");
        assert_eq!(obs.wind_direction_degrees, Some(300.0));
        assert_eq!(obs.wind_speed_mps, Some(5.0));
        assert_eq!(obs.wind_gust_mps, Some(6.0));
        assert_eq!(obs.wave_height_meters, Some(0.2));
        assert_eq!(obs.dominant_wave_period_secs, None);
        assert_eq!(obs.average_wave_period_secs, None);
        assert_eq!(obs.wave_direction_degrees, Some(17.0));
        assert_eq!(obs.pressure_hpa, Some(1014.1));
        assert_eq!(obs.air_temp_celsius, Some(19.9));
        assert_eq!(obs.water_temp_celsius, Some(11.9));
        assert_eq!(obs.dewpoint_celsius, None);
        assert_eq!(obs.visibility_nmi, None);
        assert_eq!(obs.pressure_tendency_hpa, None);
        assert_eq!(obs.tide_feet, None);
    }

    #[test]
    fn test_parse_latest_observation_mostly_missing() {
        let obs = parse_latest_observation("sdbc1", SDBC1).unwrap();
        assert_eq!(obs.station_id, "SDBC1");
        assert_eq!(obs.wind_direction_degrees, None);
        assert_eq!(obs.pressure_hpa, Some(1015.6));
        assert_eq!(obs.water_temp_celsius, Some(20.0));
        assert_eq!(obs.air_temp_celsius, None);
    }

    #[test]
    fn test_parse_latest_observation_no_data_lines() {
        let body = "#YY  MM DD hh mm WDIR WSPD GST  WVHT   DPD   APD MWD   PRES  ATMP  WTMP  DEWP  VIS PTDY  TIDE\n#yr  mo dy hr mn degT m/s  m/s     m   sec   sec degT   hPa  degC  degC  degC  nmi  hPa    ft\n";
        let err = parse_latest_observation("45186", body).unwrap_err();
        assert!(matches!(err, BuoyClientError::Parse(_)));
    }

    #[test]
    fn test_observation_url_uppercases_station() {
        let client = BuoyClient::new(Client::new(), "https://www.ndbc.noaa.gov/data/realtime2/").unwrap();
        let url = client.observation_url("sdbc1");
        assert_eq!(url.as_str(), "https://www.ndbc.noaa.gov/data/realtime2/SDBC1.txt");
    }
}
