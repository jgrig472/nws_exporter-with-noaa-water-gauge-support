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

use crate::buoy_client::BuoyObservation;
use crate::coops_client::CoOpsObservation;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
struct BuoyLabels {
    buoy: String,
    buoy_name: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
struct CoOpsLocationLabels {
    buoy: String,
    buoy_name: String,
    coops_station: String,
}

/// Holder for Prometheus metrics tracking NOAA NDBC buoy and coastal station observations.
///
/// All metrics use the prefix `nws_buoy_` and carry a `buoy` label set to the station ID
/// (e.g. `45186`) and a `buoy_name` label set to the station's friendly name as reported by
/// NDBC (e.g. `Waukegan Buoy, IL`), if known. Metrics are updated on every call to `update()`.
///
/// Registered metrics:
/// - `nws_buoy_station` - station metadata (always 1)
/// - `nws_buoy_wind_direction_degrees` - wind direction, in degrees clockwise from true north
/// - `nws_buoy_wind_speed_mps` - wind speed, in meters per second
/// - `nws_buoy_wind_gust_mps` - peak wind gust, in meters per second
/// - `nws_buoy_wave_height_meters` - significant wave height, in meters
/// - `nws_buoy_dominant_wave_period_seconds` - dominant wave period, in seconds
/// - `nws_buoy_average_wave_period_seconds` - average wave period, in seconds
/// - `nws_buoy_wave_direction_degrees` - mean wave direction, in degrees clockwise from true north
/// - `nws_buoy_pressure_hpa` - sea level pressure, in hectopascals
/// - `nws_buoy_pressure_tendency_hpa` - pressure tendency over the last 3 hours, in hectopascals
/// - `nws_buoy_air_temperature_degrees` - air temperature, in degrees celsius
/// - `nws_buoy_water_temperature_degrees` - water temperature, in degrees celsius
/// - `nws_buoy_dewpoint_degrees` - dewpoint, in degrees celsius
/// - `nws_buoy_visibility_nmi` - visibility, in nautical miles
/// - `nws_buoy_tide_feet` - water level above or below mean lower low water, in feet
/// - `nws_buoy_next_high_tide_feet` - predicted height of the next high tide, in feet
/// - `nws_buoy_next_high_tide_timestamp_seconds` - predicted time of the next high tide, as a unix timestamp
/// - `nws_buoy_next_low_tide_feet` - predicted height of the next low tide, in feet
/// - `nws_buoy_next_low_tide_timestamp_seconds` - predicted time of the next low tide, as a unix timestamp
/// - `nws_buoy_latitude_degrees` - latitude of the buoy or coastal station, in decimal degrees
/// - `nws_buoy_longitude_degrees` - longitude of the buoy or coastal station, in decimal degrees
/// - `nws_buoy_coops_latitude_degrees` - latitude of the matched CO-OPS tide station, in decimal degrees
/// - `nws_buoy_coops_longitude_degrees` - longitude of the matched CO-OPS tide station, in decimal degrees
///
/// For buoys matched to a nearby NOAA CO-OPS tide station (see `--buoy-tide-station` and
/// `--coops-max-distance-nmi`), `wind_direction`/`wind_speed`/`wind_gust`, `pressure`,
/// `air_temp`, `water_temp`, and `tide` are sourced from CO-OPS instead of NDBC whenever CO-OPS
/// has a reading, via `apply_coops()`. The four tide-prediction metrics have no NDBC equivalent
/// and are only ever populated via `apply_coops()`.
pub struct BuoyMetrics {
    station: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    wind_direction: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    wind_speed: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    wind_gust: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    wave_height: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    dominant_wave_period: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    average_wave_period: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    wave_direction: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    pressure: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    pressure_tendency: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    air_temp: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    water_temp: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    dewpoint: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    visibility: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    tide: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    next_high_tide_feet: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    next_high_tide_timestamp: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    next_low_tide_feet: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    next_low_tide_timestamp: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    latitude: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    longitude: Family<BuoyLabels, Gauge<f64, AtomicU64>>,
    coops_latitude: Family<CoOpsLocationLabels, Gauge<f64, AtomicU64>>,
    coops_longitude: Family<CoOpsLocationLabels, Gauge<f64, AtomicU64>>,
}

impl BuoyMetrics {
    /// Create a new `BuoyMetrics` and register each metric with the provided `Registry`.
    pub fn new(reg: &mut Registry) -> Self {
        let station = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let wind_direction = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let wind_speed = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let wind_gust = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let wave_height = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let dominant_wave_period = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let average_wave_period = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let wave_direction = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let pressure = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let pressure_tendency = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let air_temp = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let water_temp = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let dewpoint = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let visibility = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let tide = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let next_high_tide_feet = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let next_high_tide_timestamp = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let next_low_tide_feet = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let next_low_tide_timestamp = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let latitude = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let longitude = Family::<BuoyLabels, Gauge<f64, AtomicU64>>::default();
        let coops_latitude = Family::<CoOpsLocationLabels, Gauge<f64, AtomicU64>>::default();
        let coops_longitude = Family::<CoOpsLocationLabels, Gauge<f64, AtomicU64>>::default();

        reg.register("nws_buoy_station", "Buoy or coastal station metadata", station.clone());
        reg.register(
            "nws_buoy_wind_direction_degrees",
            "Wind direction, in degrees clockwise from true north",
            wind_direction.clone(),
        );
        reg.register(
            "nws_buoy_wind_speed_mps",
            "Wind speed, in meters per second",
            wind_speed.clone(),
        );
        reg.register(
            "nws_buoy_wind_gust_mps",
            "Peak wind gust speed, in meters per second",
            wind_gust.clone(),
        );
        reg.register(
            "nws_buoy_wave_height_meters",
            "Significant wave height, in meters",
            wave_height.clone(),
        );
        reg.register(
            "nws_buoy_dominant_wave_period_seconds",
            "Dominant wave period, in seconds",
            dominant_wave_period.clone(),
        );
        reg.register(
            "nws_buoy_average_wave_period_seconds",
            "Average wave period, in seconds",
            average_wave_period.clone(),
        );
        reg.register(
            "nws_buoy_wave_direction_degrees",
            "Mean wave direction, in degrees clockwise from true north",
            wave_direction.clone(),
        );
        reg.register(
            "nws_buoy_pressure_hpa",
            "Sea level pressure, in hectopascals",
            pressure.clone(),
        );
        reg.register(
            "nws_buoy_pressure_tendency_hpa",
            "Pressure tendency over the last 3 hours, in hectopascals",
            pressure_tendency.clone(),
        );
        reg.register(
            "nws_buoy_air_temperature_degrees",
            "Air temperature, in degrees celsius",
            air_temp.clone(),
        );
        reg.register(
            "nws_buoy_water_temperature_degrees",
            "Water temperature, in degrees celsius",
            water_temp.clone(),
        );
        reg.register(
            "nws_buoy_dewpoint_degrees",
            "Dewpoint, in degrees celsius",
            dewpoint.clone(),
        );
        reg.register(
            "nws_buoy_visibility_nmi",
            "Visibility, in nautical miles",
            visibility.clone(),
        );
        reg.register(
            "nws_buoy_tide_feet",
            "Water level above or below mean lower low water, in feet",
            tide.clone(),
        );
        reg.register(
            "nws_buoy_next_high_tide_feet",
            "Predicted height of the next high tide, in feet",
            next_high_tide_feet.clone(),
        );
        reg.register(
            "nws_buoy_next_high_tide_timestamp_seconds",
            "Predicted time of the next high tide, as a unix timestamp",
            next_high_tide_timestamp.clone(),
        );
        reg.register(
            "nws_buoy_next_low_tide_feet",
            "Predicted height of the next low tide, in feet",
            next_low_tide_feet.clone(),
        );
        reg.register(
            "nws_buoy_next_low_tide_timestamp_seconds",
            "Predicted time of the next low tide, as a unix timestamp",
            next_low_tide_timestamp.clone(),
        );
        reg.register(
            "nws_buoy_latitude_degrees",
            "Latitude of the buoy or coastal station, in decimal degrees",
            latitude.clone(),
        );
        reg.register(
            "nws_buoy_longitude_degrees",
            "Longitude of the buoy or coastal station, in decimal degrees",
            longitude.clone(),
        );
        reg.register(
            "nws_buoy_coops_latitude_degrees",
            "Latitude of the matched CO-OPS tide station, in decimal degrees",
            coops_latitude.clone(),
        );
        reg.register(
            "nws_buoy_coops_longitude_degrees",
            "Longitude of the matched CO-OPS tide station, in decimal degrees",
            coops_longitude.clone(),
        );

        Self {
            station,
            wind_direction,
            wind_speed,
            wind_gust,
            wave_height,
            dominant_wave_period,
            average_wave_period,
            wave_direction,
            pressure,
            pressure_tendency,
            air_temp,
            water_temp,
            dewpoint,
            visibility,
            tide,
            next_high_tide_feet,
            next_high_tide_timestamp,
            next_low_tide_feet,
            next_low_tide_timestamp,
            latitude,
            longitude,
            coops_latitude,
            coops_longitude,
        }
    }

    /// Update all metrics from the most recent observation for a buoy or coastal station.
    /// `name` is the station's friendly name (e.g. from `BuoyClient::station_names()`), or an
    /// empty string if not known.
    pub fn update(&self, obs: &BuoyObservation, name: &str) {
        let labels = BuoyLabels {
            buoy: obs.station_id.clone(),
            buoy_name: name.to_string(),
        };

        self.station.get_or_create(&labels).set(1.0);

        if let Some(v) = obs.wind_direction_degrees {
            self.wind_direction.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.wind_speed_mps {
            self.wind_speed.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.wind_gust_mps {
            self.wind_gust.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.wave_height_meters {
            self.wave_height.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.dominant_wave_period_secs {
            self.dominant_wave_period.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.average_wave_period_secs {
            self.average_wave_period.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.wave_direction_degrees {
            self.wave_direction.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.pressure_hpa {
            self.pressure.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.pressure_tendency_hpa {
            self.pressure_tendency.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.air_temp_celsius {
            self.air_temp.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.water_temp_celsius {
            self.water_temp.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.dewpoint_celsius {
            self.dewpoint.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.visibility_nmi {
            self.visibility.get_or_create(&labels).set(v);
        }
        if let Some(v) = obs.tide_feet {
            self.tide.get_or_create(&labels).set(v);
        }
    }

    /// Override wind, pressure, air/water temperature, and tide with higher-precision NOAA
    /// CO-OPS readings, and populate the next-high/low tide prediction metrics. Should be
    /// called after `update()` for buoys matched to a CO-OPS station; fields where `coops` has
    /// no reading are left as whatever `update()` already set from NDBC.
    pub fn apply_coops(&self, station_id: &str, name: &str, coops: &CoOpsObservation) {
        let labels = BuoyLabels {
            buoy: station_id.to_string(),
            buoy_name: name.to_string(),
        };

        if let Some(v) = coops.wind_direction_degrees {
            self.wind_direction.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.wind_speed_mps {
            self.wind_speed.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.wind_gust_mps {
            self.wind_gust.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.pressure_hpa {
            self.pressure.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.air_temp_celsius {
            self.air_temp.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.water_temp_celsius {
            self.water_temp.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.tide_feet {
            self.tide.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.next_high_tide_feet {
            self.next_high_tide_feet.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.next_high_tide_unix {
            self.next_high_tide_timestamp.get_or_create(&labels).set(v as f64);
        }
        if let Some(v) = coops.next_low_tide_feet {
            self.next_low_tide_feet.get_or_create(&labels).set(v);
        }
        if let Some(v) = coops.next_low_tide_unix {
            self.next_low_tide_timestamp.get_or_create(&labels).set(v as f64);
        }
    }

    /// Set the buoy or coastal station's own coordinates, as parsed from the NDBC station
    /// metadata table. Static for the lifetime of the process, so this is called once at
    /// startup rather than on every `update()`.
    pub fn set_location(&self, station_id: &str, name: &str, lat: f64, lon: f64) {
        let labels = BuoyLabels {
            buoy: station_id.to_string(),
            buoy_name: name.to_string(),
        };

        self.latitude.get_or_create(&labels).set(lat);
        self.longitude.get_or_create(&labels).set(lon);
    }

    /// Set the coordinates of the CO-OPS tide station matched to this buoy. Static for the
    /// lifetime of the process, so this is called once at startup rather than on every
    /// `apply_coops()`.
    pub fn set_coops_location(&self, station_id: &str, name: &str, coops_station: &str, lat: f64, lon: f64) {
        let labels = CoOpsLocationLabels {
            buoy: station_id.to_string(),
            buoy_name: name.to_string(),
            coops_station: coops_station.to_string(),
        };

        self.coops_latitude.get_or_create(&labels).set(lat);
        self.coops_longitude.get_or_create(&labels).set(lon);
    }
}
