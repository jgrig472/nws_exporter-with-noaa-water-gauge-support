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
use serde::{Deserialize, Serialize};
use std::error;
use std::fmt;

/// Error resulting from setup of or calls to a `WaterGaugeClient` instance.
#[derive(Debug)]
pub enum WaterClientError {
    Internal(reqwest::Error),
    Initialization(String),
    InvalidGauge(String),
    Unexpected(StatusCode, Url),
}

impl fmt::Display for WaterClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Internal(e) => write!(f, "{}", e),
            Self::Initialization(msg) => write!(f, "initialization error: {}", msg),
            Self::InvalidGauge(g) => write!(f, "invalid gauge {}", g),
            Self::Unexpected(status, url) => write!(f, "unexpected status {} for {}", status, url),
        }
    }
}

impl error::Error for WaterClientError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Internal(e) => Some(e),
            _ => None,
        }
    }
}

/// Client for fetching NOAA water gauge data from the National Water Prediction Service API.
///
/// API base: `https://api.water.noaa.gov/nwps/v1/`
#[derive(Debug)]
pub struct WaterGaugeClient {
    client: Client,
    base_url: Url,
}

impl WaterGaugeClient {
    const USER_AGENT: &'static str =
        "nws_exporter/0.7.0 (https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support)";

    /// Create a new `WaterGaugeClient` from the provided reqwest client and base URL for the
    /// NOAA NWPS API (typically `https://api.water.noaa.gov/nwps/v1/`).
    pub fn new(client: Client, base_url: &str) -> Result<Self, WaterClientError> {
        Ok(WaterGaugeClient {
            client,
            base_url: base_url
                .parse()
                .map_err(|e| WaterClientError::Initialization(format!("cannot parse {}: {}", base_url, e)))?,
        })
    }

    /// Fetch full gauge data including current status and flood stage thresholds.
    pub async fn gauge(&self, gauge_id: &str) -> Result<WaterGauge, WaterClientError> {
        let url = self.gauge_url(gauge_id);
        tracing::debug!(message = "fetching water gauge data", url = %url);

        let res = self.make_request(gauge_id, url).await?;
        res.json::<WaterGauge>().await.map_err(WaterClientError::Internal)
    }

    async fn make_request<S: Into<String>>(&self, gauge_id: S, url: Url) -> Result<Response, WaterClientError> {
        let res = self
            .client
            .get(url.clone())
            .header(USER_AGENT, Self::USER_AGENT)
            .send()
            .await
            .map_err(WaterClientError::Internal)?;

        let status = res.status();
        if status == StatusCode::OK {
            Ok(res)
        } else if status == StatusCode::NOT_FOUND {
            Err(WaterClientError::InvalidGauge(gauge_id.into()))
        } else {
            Err(WaterClientError::Unexpected(status, url))
        }
    }

    fn gauge_url(&self, gauge_id: &str) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("unable to modify gauge URL path segments")
            .push("gauges")
            .push(gauge_id);
        url
    }
}

/// Full gauge response from the NOAA NWPS API, including current status and flood thresholds.
#[derive(Serialize, Deserialize, Debug)]
pub struct WaterGauge {
    /// Gauge location ID (e.g. "DSPI2")
    #[serde(alias = "lid")]
    pub lid: String,
    #[serde(alias = "name")]
    pub name: String,
    #[serde(alias = "state")]
    pub state: Option<GeoRef>,
    #[serde(alias = "county")]
    pub county: Option<String>,
    #[serde(alias = "latitude")]
    pub latitude: Option<f64>,
    #[serde(alias = "longitude")]
    pub longitude: Option<f64>,
    #[serde(alias = "status")]
    pub status: Option<GaugeStatus>,
    #[serde(alias = "flood")]
    pub flood: Option<FloodInfo>,
}

/// An abbreviated reference to a geographic entity (state, RFC, WFO) with a short code.
#[derive(Serialize, Deserialize, Debug)]
pub struct GeoRef {
    #[serde(alias = "abbreviation")]
    pub abbreviation: String,
    #[serde(alias = "name")]
    pub name: Option<String>,
}

/// Current observed and forecast status for a gauge.
#[derive(Serialize, Deserialize, Debug)]
pub struct GaugeStatus {
    #[serde(alias = "observed")]
    pub observed: Option<StatusReading>,
}

/// A single observed or forecast reading with stage (primary) and flow (secondary).
#[derive(Serialize, Deserialize, Debug)]
pub struct StatusReading {
    /// Stage in feet (use `primaryUnit` to confirm units)
    #[serde(alias = "primary")]
    pub primary: Option<f64>,
    #[serde(alias = "primaryUnit")]
    pub primary_unit: Option<String>,
    /// Flow (use `secondaryUnit` to confirm units; often kcfs)
    #[serde(alias = "secondary")]
    pub secondary: Option<f64>,
    #[serde(alias = "secondaryUnit")]
    pub secondary_unit: Option<String>,
    #[serde(alias = "floodCategory")]
    pub flood_category: Option<String>,
    #[serde(alias = "validTime")]
    pub valid_time: Option<String>,
}

/// Flood stage and flow threshold information.
#[derive(Serialize, Deserialize, Debug)]
pub struct FloodInfo {
    #[serde(alias = "stageUnits")]
    pub stage_units: Option<String>,
    #[serde(alias = "flowUnits")]
    pub flow_units: Option<String>,
    #[serde(alias = "categories")]
    pub categories: Option<FloodCategories>,
}

/// Flood stage/flow thresholds for each flood severity level.
#[derive(Serialize, Deserialize, Debug)]
pub struct FloodCategories {
    #[serde(alias = "action")]
    pub action: Option<FloodThreshold>,
    #[serde(alias = "minor")]
    pub minor: Option<FloodThreshold>,
    #[serde(alias = "moderate")]
    pub moderate: Option<FloodThreshold>,
    #[serde(alias = "major")]
    pub major: Option<FloodThreshold>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FloodThreshold {
    #[serde(alias = "stage")]
    pub stage: Option<f64>,
    #[serde(alias = "flow")]
    pub flow: Option<f64>,
}
