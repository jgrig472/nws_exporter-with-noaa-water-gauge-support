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

use crate::water_client::{FloodCategories, WaterGauge};
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::registry::Registry;
use std::sync::atomic::AtomicU64;

// Sentinel value the NOAA API uses when a measurement is unavailable
const NOAA_SENTINEL: f64 = -999.0;

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
struct WaterGaugeLabels {
    gauge: String,
    gauge_name: String,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
struct WaterGaugeInfoLabels {
    gauge: String,
    gauge_name: String,
    state: String,
}

/// Holder for Prometheus metrics tracking NOAA water gauge stage, flow, and flood thresholds.
///
/// All metrics use the prefix `nws_water_` and carry a `gauge` label set to the gauge LID
/// (e.g. `DSPI2`) and a `gauge_name` label set to the gauge's human-readable name as reported
/// by NOAA (e.g. `Des Plaines River`). Metrics are updated on every call to `update()`.
///
/// Registered metrics:
/// - `nws_water_gauge` - gauge metadata (always 1), with `gauge_name` and `state` labels
/// - `nws_water_stage_feet` - current observed water stage in feet
/// - `nws_water_flow_kcfs` - current observed flow in kcfs
/// - `nws_water_action_stage_feet` - action stage threshold in feet
/// - `nws_water_minor_flood_stage_feet` - minor flood stage threshold in feet
/// - `nws_water_moderate_flood_stage_feet` - moderate flood stage threshold in feet
/// - `nws_water_major_flood_stage_feet` - major flood stage threshold in feet
pub struct WaterLevelMetrics {
    gauge_info: Family<WaterGaugeInfoLabels, Gauge<f64, AtomicU64>>,
    stage: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
    flow: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
    action_stage: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
    minor_flood_stage: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
    moderate_flood_stage: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
    major_flood_stage: Family<WaterGaugeLabels, Gauge<f64, AtomicU64>>,
}

impl WaterLevelMetrics {
    /// Create a new `WaterLevelMetrics` and register each metric with the provided `Registry`.
    pub fn new(reg: &mut Registry) -> Self {
        let gauge_info = Family::<WaterGaugeInfoLabels, Gauge<f64, AtomicU64>>::default();
        let stage = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();
        let flow = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();
        let action_stage = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();
        let minor_flood_stage = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();
        let moderate_flood_stage = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();
        let major_flood_stage = Family::<WaterGaugeLabels, Gauge<f64, AtomicU64>>::default();

        reg.register("nws_water_gauge", "Water gauge metadata", gauge_info.clone());
        reg.register(
            "nws_water_stage_feet",
            "Current observed water stage in feet",
            stage.clone(),
        );
        reg.register(
            "nws_water_flow_kcfs",
            "Current observed flow in kcfs (thousands of cubic feet per second)",
            flow.clone(),
        );
        reg.register(
            "nws_water_action_stage_feet",
            "Action stage threshold in feet",
            action_stage.clone(),
        );
        reg.register(
            "nws_water_minor_flood_stage_feet",
            "Minor flood stage threshold in feet",
            minor_flood_stage.clone(),
        );
        reg.register(
            "nws_water_moderate_flood_stage_feet",
            "Moderate flood stage threshold in feet",
            moderate_flood_stage.clone(),
        );
        reg.register(
            "nws_water_major_flood_stage_feet",
            "Major flood stage threshold in feet",
            major_flood_stage.clone(),
        );

        Self {
            gauge_info,
            stage,
            flow,
            action_stage,
            minor_flood_stage,
            moderate_flood_stage,
            major_flood_stage,
        }
    }

    /// Update all metrics from the full gauge response. Called both on initialization and
    /// on every refresh cycle, since the NOAA API returns current readings and thresholds
    /// in a single response.
    pub fn update(&self, gauge: &WaterGauge) {
        let info_labels = WaterGaugeInfoLabels {
            gauge: gauge.lid.clone(),
            gauge_name: gauge.name.clone(),
            state: gauge.state.as_ref().map(|s| s.abbreviation.clone()).unwrap_or_default(),
        };
        self.gauge_info.get_or_create(&info_labels).set(1.0);

        let labels = WaterGaugeLabels {
            gauge: gauge.lid.clone(),
            gauge_name: gauge.name.clone(),
        };

        if let Some(status) = &gauge.status {
            if let Some(obs) = &status.observed {
                if let Some(v) = obs.primary.filter(|v| *v > NOAA_SENTINEL) {
                    self.stage.get_or_create(&labels).set(v);
                }
                if let Some(v) = obs.secondary.filter(|v| *v > NOAA_SENTINEL) {
                    self.flow.get_or_create(&labels).set(v);
                }
            }
        }

        if let Some(flood) = &gauge.flood {
            if let Some(cats) = &flood.categories {
                self.set_thresholds(&labels, cats);
            }
        }
    }

    fn set_thresholds(&self, labels: &WaterGaugeLabels, cats: &FloodCategories) {
        if let Some(t) = &cats.action {
            if let Some(v) = t.stage.filter(|v| *v > NOAA_SENTINEL) {
                self.action_stage.get_or_create(labels).set(v);
            }
        }
        if let Some(t) = &cats.minor {
            if let Some(v) = t.stage.filter(|v| *v > NOAA_SENTINEL) {
                self.minor_flood_stage.get_or_create(labels).set(v);
            }
        }
        if let Some(t) = &cats.moderate {
            if let Some(v) = t.stage.filter(|v| *v > NOAA_SENTINEL) {
                self.moderate_flood_stage.get_or_create(labels).set(v);
            }
        }
        if let Some(t) = &cats.major {
            if let Some(v) = t.stage.filter(|v| *v > NOAA_SENTINEL) {
                self.major_flood_stage.get_or_create(labels).set(v);
            }
        }
    }
}
