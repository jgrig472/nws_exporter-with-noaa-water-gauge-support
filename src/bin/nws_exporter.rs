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

use axum::routing::get;
use axum::Router;
use clap::Parser;
use nws_exporter::buoy_client::{BuoyClient, BuoyClientError};
use nws_exporter::buoy_metrics::BuoyMetrics;
use nws_exporter::client::{ClientError, NwsClient};
use nws_exporter::http::RequestState;
use nws_exporter::metrics::ForecastMetrics;
use nws_exporter::water_client::{WaterClientError, WaterGaugeClient};
use nws_exporter::water_metrics::WaterLevelMetrics;
use prometheus_client::registry::Registry;
use reqwest::Client;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::process;
use std::sync::Arc;
use std::time::Duration;
use tower_http::trace::TraceLayer;
use tracing::{Instrument, Level};

const DEFAULT_LOG_LEVEL: Level = Level::INFO;
const DEFAULT_BIND_ADDR: ([u8; 4], u16) = ([0, 0, 0, 0], 9782);
const DEFAULT_REFERSH_SECS: u64 = 300;
const DEFAULT_TIMEOUT_MILLIS: u64 = 5000;
const DEFAULT_API_URL: &str = "https://api.weather.gov/";
const DEFAULT_WATER_API_URL: &str = "https://api.water.noaa.gov/nwps/v1/";
const DEFAULT_BUOY_API_URL: &str = "https://www.ndbc.noaa.gov/data/realtime2/";
const DEFAULT_BUOY_STATION_TABLE_URL: &str = "https://www.ndbc.noaa.gov/data/stations/station_table.txt";

/// Export National Weather Service forecasts, NOAA water gauge levels, and NOAA buoy
/// observations as Prometheus metrics
#[derive(Debug, Parser)]
#[clap(name = "nws_exporter", version = clap::crate_version!())]
struct NwsExporterApplication {
    /// NWS weather station ID to fetch forecasts for. Must be specified at least once and
    /// may be used multiple times (separated by spaces) to fetch forecasts for multiple NWS
    /// stations
    #[arg(required_unless_present_any = ["gauge", "buoy"])]
    station: Vec<String>,

    /// NOAA water gauge ID to fetch water level data for (e.g. "dspi2" for the Des Plaines
    /// River at Joliet). May be used multiple times to monitor multiple gauges.
    /// See https://water.noaa.gov/ to find gauge IDs.
    #[arg(long = "gauge")]
    gauge: Vec<String>,

    /// NOAA NDBC buoy or coastal station ID to fetch observations for (e.g. "45186" for the
    /// Waukegan buoy on Lake Michigan). May be used multiple times to monitor multiple
    /// stations. See https://www.ndbc.noaa.gov/ to find station IDs.
    #[arg(long = "buoy")]
    buoy: Vec<String>,

    /// Base URL for the Weather.gov API
    #[arg(long, default_value_t = DEFAULT_API_URL.into())]
    api_url: String,

    /// Base URL for the NOAA National Water Prediction Service API
    #[arg(long, default_value_t = DEFAULT_WATER_API_URL.into())]
    water_api_url: String,

    /// Base URL for the NOAA NDBC realtime data feed
    #[arg(long, default_value_t = DEFAULT_BUOY_API_URL.into())]
    buoy_api_url: String,

    /// URL for the NOAA NDBC station metadata table, used to look up friendly names for
    /// `--buoy` stations
    #[arg(long, default_value_t = DEFAULT_BUOY_STATION_TABLE_URL.into())]
    buoy_station_table_url: String,

    /// Logging verbosity. Allowed values are 'trace', 'debug', 'info', 'warn', and 'error'
    /// (case insensitive)
    #[arg(long, default_value_t = DEFAULT_LOG_LEVEL)]
    log_level: Level,

    /// Fetch weather forecasts from the Weather.gov API at this interval, in seconds
    #[arg(long, default_value_t = DEFAULT_REFERSH_SECS)]
    refresh_secs: u64,

    /// Timeout for fetching weather forecasts from the Weather.gov API, in milliseconds
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_MILLIS)]
    timeout_millis: u64,

    /// Address to bind to. By default, nws_exporter will bind to public address since
    /// the purpose is to expose metrics to an external system (Prometheus or another
    /// agent for ingestion)
    #[arg(long, default_value_t = DEFAULT_BIND_ADDR.into())]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let opts = NwsExporterApplication::parse();
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(opts.log_level)
            .finish(),
    )
    .expect("failed to set tracing subscriber");

    let timeout = Duration::from_millis(opts.timeout_millis);
    let http_client = Client::builder().timeout(timeout).build().unwrap_or_else(|e| {
        tracing::error!(message = "unable to initialize HTTP client", error = %e);
        process::exit(1)
    });

    let mut registry = <Registry>::default();

    // Weather station update task (only when stations are provided)
    if !opts.station.is_empty() {
        let client = NwsClient::new(http_client.clone(), &opts.api_url).unwrap_or_else(|e| {
            tracing::error!(message = "unable to initialize NWS client", error = %e);
            process::exit(1)
        });

        let metrics = ForecastMetrics::new(&mut registry);
        let update = WeatherUpdateTask::new(opts.station, metrics, client, Duration::from_secs(opts.refresh_secs));

        if let Err(e) = update.initialize().await {
            tracing::error!(message = "failed to fetch initial station information", error = %e);
            process::exit(1);
        }

        tokio::spawn(update.run());
    }

    // Water gauge update task (only when gauges are provided)
    if !opts.gauge.is_empty() {
        let water_client = WaterGaugeClient::new(http_client.clone(), &opts.water_api_url).unwrap_or_else(|e| {
            tracing::error!(message = "unable to initialize water gauge client", error = %e);
            process::exit(1)
        });

        let water_metrics = WaterLevelMetrics::new(&mut registry);
        let water_update = WaterUpdateTask::new(
            opts.gauge,
            water_metrics,
            water_client,
            Duration::from_secs(opts.refresh_secs),
        );

        if let Err(e) = water_update.initialize().await {
            tracing::error!(message = "failed to fetch initial water gauge information", error = %e);
            process::exit(1);
        }

        tokio::spawn(water_update.run());
    }

    // Buoy update task (only when buoys are provided)
    if !opts.buoy.is_empty() {
        let buoy_client = BuoyClient::new(http_client.clone(), &opts.buoy_api_url, &opts.buoy_station_table_url)
            .unwrap_or_else(|e| {
                tracing::error!(message = "unable to initialize buoy client", error = %e);
                process::exit(1)
            });

        let buoy_names = buoy_client.station_names().await.unwrap_or_else(|e| {
            tracing::warn!(message = "failed to fetch buoy station names, buoy_name label will be empty", error = %e);
            HashMap::new()
        });

        let buoy_metrics = BuoyMetrics::new(&mut registry);
        let buoy_update = BuoyUpdateTask::new(
            opts.buoy,
            buoy_metrics,
            buoy_client,
            buoy_names,
            Duration::from_secs(opts.refresh_secs),
        );

        if let Err(e) = buoy_update.initialize().await {
            tracing::error!(message = "failed to fetch initial buoy information", error = %e);
            process::exit(1);
        }

        tokio::spawn(buoy_update.run());
    }

    let state = Arc::new(RequestState { registry });
    let app = Router::new()
        .route("/metrics", get(nws_exporter::http::text_metrics_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());

    let server = axum::Server::try_bind(&opts.bind)
        .map(|s| {
            s.serve(app.into_make_service()).with_graceful_shutdown(async {
                tokio::select! {
                    _ = sigterm() => {}
                    _ = sigint() => {}
                }
            })
        })
        .unwrap_or_else(|e| {
            tracing::error!(message = "error starting server", address = %opts.bind, err = %e);
            process::exit(1)
        });

    tracing::info!(message = "starting server", address = %opts.bind);
    server.await.unwrap();

    tracing::info!("server shutdown");
    Ok(())
}

async fn sigint() -> io::Result<()> {
    tokio::signal::ctrl_c().await
}

#[cfg(unix)]
async fn sigterm() -> io::Result<()> {
    use tokio::signal::unix::{self, SignalKind};
    unix::signal(SignalKind::terminate())?.recv().await;
    Ok(())
}

#[cfg(not(unix))]
async fn sigterm() -> io::Result<()> {
    std::future::pending::<io::Result<()>>().await
}

/// Task for periodically updating forecast metrics for multiple NWS weather stations.
struct WeatherUpdateTask {
    stations: Vec<String>,
    metrics: ForecastMetrics,
    client: NwsClient,
    interval: Duration,
}

impl WeatherUpdateTask {
    fn new(stations: Vec<String>, metrics: ForecastMetrics, client: NwsClient, interval: Duration) -> Self {
        Self {
            stations,
            metrics,
            client,
            interval,
        }
    }

    async fn initialize(&self) -> Result<(), ClientError> {
        for id in self.stations.iter() {
            let station = self
                .client
                .station(id)
                .instrument(tracing::span!(Level::DEBUG, "nws_station"))
                .await?;
            self.metrics.station(&station);
        }

        Ok(())
    }

    async fn run(self) -> ! {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            let _ = interval.tick().await;
            for id in self.stations.iter() {
                match self
                    .client
                    .observation(id)
                    .instrument(tracing::span!(Level::DEBUG, "nws_observation"))
                    .await
                {
                    Ok(obs) => {
                        self.metrics.observation(&obs);
                        tracing::info!(message = "fetched new observation", station_id = %id, observation = %obs.id);
                    }
                    Err(e) => {
                        tracing::error!(message = "failed to fetch observation", station_id = %id, error = %e);
                    }
                }
            }
        }
    }
}

/// Task for periodically updating water level metrics for multiple NOAA water gauges.
struct WaterUpdateTask {
    gauges: Vec<String>,
    metrics: WaterLevelMetrics,
    client: WaterGaugeClient,
    interval: Duration,
}

impl WaterUpdateTask {
    fn new(gauges: Vec<String>, metrics: WaterLevelMetrics, client: WaterGaugeClient, interval: Duration) -> Self {
        Self {
            gauges,
            metrics,
            client,
            interval,
        }
    }

    /// Fetch gauge data once to validate gauge IDs and populate initial metrics.
    async fn initialize(&self) -> Result<(), WaterClientError> {
        for id in self.gauges.iter() {
            let gauge = self
                .client
                .gauge(id)
                .instrument(tracing::span!(Level::DEBUG, "water_gauge"))
                .await?;
            self.metrics.update(&gauge);
            tracing::info!(message = "initialized water gauge", gauge_id = %id, name = %gauge.name);
        }

        Ok(())
    }

    /// Periodically fetch the full gauge response (which includes current stage/flow) for all
    /// configured gauges and update metrics.
    async fn run(self) -> ! {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            let _ = interval.tick().await;
            for id in self.gauges.iter() {
                match self
                    .client
                    .gauge(id)
                    .instrument(tracing::span!(Level::DEBUG, "water_gauge"))
                    .await
                {
                    Ok(gauge) => {
                        self.metrics.update(&gauge);
                        tracing::info!(message = "fetched water gauge reading", gauge_id = %id);
                    }
                    Err(e) => {
                        tracing::error!(message = "failed to fetch water gauge reading", gauge_id = %id, error = %e);
                    }
                }
            }
        }
    }
}

/// Task for periodically updating buoy/coastal station metrics for multiple NOAA NDBC stations.
struct BuoyUpdateTask {
    buoys: Vec<String>,
    metrics: BuoyMetrics,
    client: BuoyClient,
    names: HashMap<String, String>,
    interval: Duration,
}

impl BuoyUpdateTask {
    fn new(
        buoys: Vec<String>,
        metrics: BuoyMetrics,
        client: BuoyClient,
        names: HashMap<String, String>,
        interval: Duration,
    ) -> Self {
        Self {
            buoys,
            metrics,
            client,
            names,
            interval,
        }
    }

    fn name_for(&self, station_id: &str) -> &str {
        self.names.get(station_id).map(String::as_str).unwrap_or("")
    }

    /// Fetch the latest observation once to validate buoy IDs and populate initial metrics.
    async fn initialize(&self) -> Result<(), BuoyClientError> {
        for id in self.buoys.iter() {
            let obs = self
                .client
                .observation(id)
                .instrument(tracing::span!(Level::DEBUG, "buoy_observation"))
                .await?;
            self.metrics.update(&obs, self.name_for(&obs.station_id));
            tracing::info!(message = "initialized buoy station", buoy_id = %id);
        }

        Ok(())
    }

    /// Periodically fetch the latest observation for all configured buoys and update metrics.
    async fn run(self) -> ! {
        let mut interval = tokio::time::interval(self.interval);

        loop {
            let _ = interval.tick().await;
            for id in self.buoys.iter() {
                match self
                    .client
                    .observation(id)
                    .instrument(tracing::span!(Level::DEBUG, "buoy_observation"))
                    .await
                {
                    Ok(obs) => {
                        self.metrics.update(&obs, self.name_for(&obs.station_id));
                        tracing::info!(message = "fetched buoy observation", buoy_id = %id);
                    }
                    Err(e) => {
                        tracing::error!(message = "failed to fetch buoy observation", buoy_id = %id, error = %e);
                    }
                }
            }
        }
    }
}
