# nws_exporter

![build status](https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support/actions/workflows/rust.yml/badge.svg)
[![docs.rs](https://docs.rs/nws_exporter/badge.svg)](https://docs.rs/nws_exporter/)
[![crates.io](https://img.shields.io/crates/v/nws_exporter.svg)](https://crates.io/crates/nws_exporter/)
[![Docker Hub](https://img.shields.io/docker/v/jasona1246/nws_exporter-with-noaa-water-gauge-support?label=docker%20hub)](https://hub.docker.com/r/jasona1246/nws_exporter-with-noaa-water-gauge-support)

Prometheus metrics exporter for api.weather.gov, NOAA water gauges, and NOAA buoys

## Features

`nws_exporter` fetches weather information for a particular [NWS station] using the [api.weather.gov] API and emits
it as Prometheus metrics. Users must pick a particular station to fetch weather information from. The following
metrics are emitted when available (not all fields are available for all stations).

* `nws_station{station=$STATION, station_id=$STATION_ID, station_name=$STATION_NAME}` - Station metadata
* `nws_elevation_meters{station=$STATION}` - Elevation of the station, in meters.
* `nws_temperature_degrees{station=$STATION}` - Temperature, in degrees celsius.
* `nws_dewpoint_degrees{station=$STATION}` - Dewpoint, in degrees celsius.
* `nws_barometric_pressure_pascals{station=$STATION}` - Barometric pressure, in pascals.
* `nws_visibility_meters{station=$STATION}` - Visibility, in meters.
* `nws_relative_humidity{station=$STATION}` - Relative humidity (0-100).
* `nws_wind_chill_degrees{station=$STATION}` - Temperature with wind chill, in degrees celsius.

`nws_exporter` can also fetch water stage and flow information for one or more NOAA water gauges using the
[NOAA National Water Prediction Service API], independently of (or alongside) NWS weather stations. The
following metrics are emitted when available (not all fields are available for all gauges).

* `nws_water_gauge{gauge=$GAUGE, gauge_name=$GAUGE_NAME, state=$STATE}` - Water gauge metadata.
* `nws_water_stage_feet{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Current observed water stage, in feet.
* `nws_water_flow_kcfs{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Current observed flow, in kcfs (thousands of cubic feet per second).
* `nws_water_action_stage_feet{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Action stage threshold, in feet.
* `nws_water_minor_flood_stage_feet{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Minor flood stage threshold, in feet.
* `nws_water_moderate_flood_stage_feet{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Moderate flood stage threshold, in feet.
* `nws_water_major_flood_stage_feet{gauge=$GAUGE, gauge_name=$GAUGE_NAME}` - Major flood stage threshold, in feet.

`$GAUGE_NAME` is the human-readable name NOAA reports for the gauge (e.g. `Des Plaines River`).

`nws_exporter` can also fetch observations for one or more [NOAA NDBC] buoys or coastal weather stations,
independently of (or alongside) NWS weather stations and water gauges. The following metrics are emitted
when available (not all fields are available for all stations). Every buoy metric also carries a
`buoy_name` label with the station's friendly name (e.g. `Waukegan Buoy, IL`), looked up from the NDBC
station metadata table; it is empty for stations not found in that table.

* `nws_buoy_station{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Buoy or coastal station metadata.
* `nws_buoy_wind_direction_degrees{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Wind direction, in degrees clockwise from true north.
* `nws_buoy_wind_speed_mps{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Wind speed, in meters per second.
* `nws_buoy_wind_gust_mps{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Peak wind gust speed, in meters per second.
* `nws_buoy_wave_height_meters{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Significant wave height, in meters.
* `nws_buoy_dominant_wave_period_seconds{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Dominant wave period, in seconds.
* `nws_buoy_average_wave_period_seconds{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Average wave period, in seconds.
* `nws_buoy_wave_direction_degrees{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Mean wave direction, in degrees clockwise from true north.
* `nws_buoy_pressure_hpa{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Sea level pressure, in hectopascals.
* `nws_buoy_pressure_tendency_hpa{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Pressure tendency over the last 3 hours, in hectopascals.
* `nws_buoy_air_temperature_degrees{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Air temperature, in degrees celsius.
* `nws_buoy_water_temperature_degrees{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Water temperature, in degrees celsius.
* `nws_buoy_dewpoint_degrees{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Dewpoint, in degrees celsius.
* `nws_buoy_visibility_nmi{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Visibility, in nautical miles.
* `nws_buoy_tide_feet{buoy=$BUOY,buoy_name=$BUOY_NAME}` - Water level above or below mean lower low water, in feet.

[NWS station]: https://www.weather.gov/documentation/services-web-api#/default/obs_stations
[api.weather.gov]: https://www.weather.gov/documentation/services-web-api
[NOAA National Water Prediction Service API]: https://water.noaa.gov/
[NOAA NDBC]: https://www.ndbc.noaa.gov/

## Install

There are multiple ways to install `nws_exporter` listed below.

### Binaries

Binaries are published for GNU/Linux (x86_64), Windows (x86_64), and MacOS (x86_64 and aarch64)
for [each release](https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support/releases).

### Docker

Docker images for GNU/Linux (amd64) are published for [each release](https://hub.docker.com/r/jasona1246/nws_exporter-with-noaa-water-gauge-support).

### Cargo

`nws_exporter` along with its dependencies can be downloaded and built from source using the
Rust `cargo` tool. Note that this requires you have a Rust toolchain installed.

To install:

```
cargo install nws_exporter
```

To uninstall:

```
cargo uninstall nws_exporter
```

### Source

`nws_exporter` along with its dependencies can be built from the latest sources on Github using
the Rust `cargo` tool. Note that this requires you have Git and a Rust toolchain installed.

Get the sources:

```
git clone https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support.git && cd nws_exporter-with-noaa-water-gauge-support
```

Install from local sources:

```
cargo install --path .
```

To uninstall:

```
cargo uninstall nws_exporter
```

## Usage

### Picking a station

In order to export NWS forecast information, `nws_exporter` needs to be told which NWS station to request
information for. You can get a list of the available stations in your state by using the API itself. An
example of this using `curl` is below.

```text
curl -sS 'https://api.weather.gov/stations?state=MA' | jq | less
```

This command lists all available stations in the state of Massachusetts. The `properties.stationIdentifier`
field for each station is the ID that you should use with `nws_exporter`. For example `KBOS` is the ID for
the station at Logan Airport in Boston.

You can then run `nws_exporter` for this station as demonstrated below.

```text
./nws_exporter KBOS
```

### Picking a water gauge

In order to export NOAA water gauge information, `nws_exporter` needs to be told which gauge(s) to request
information for. Gauge IDs (also called LIDs) can be found by searching for your river or location on
[water.noaa.gov] and reading the ID out of the gauge's URL. For example `dspi2` is the ID for the gauge on
the Des Plaines River at Joliet, IL.

You can then run `nws_exporter` for this gauge using the `--gauge` flag, which may be repeated to monitor
multiple gauges at once.

```text
./nws_exporter --gauge dspi2
```

Water gauges can also be combined with one or more NWS weather stations in a single invocation.

```text
./nws_exporter KBOS --gauge dspi2 --gauge dspi3
```

[water.noaa.gov]: https://water.noaa.gov/

### Picking a buoy or coastal station

In order to export NOAA NDBC observations, `nws_exporter` needs to be told which buoy or coastal station(s)
to request information for. Station IDs can be found by searching [ndbc.noaa.gov] for your area and reading
the ID out of the station's page. For example `45186` is the ID for the Waukegan buoy on Lake Michigan.

You can then run `nws_exporter` for this station using the `--buoy` flag, which may be repeated to monitor
multiple stations at once.

```text
./nws_exporter --buoy 45186
```

Buoys can also be combined with one or more NWS weather stations and water gauges in a single invocation.

```text
./nws_exporter KBOS --gauge dspi2 --buoy 45186 --buoy sdbc1
```

[ndbc.noaa.gov]: https://www.ndbc.noaa.gov/

### Run

You can run `nws_exporter` as a Systemd service using the [provided unit file](ext/nws_exporter.service). This
unit file  assumes that you have copied the resulting `nws_exporter` binary to `/usr/local/bin/nws_exporter`.
Make sure to edit the unit file to use a station near you that you picked in the previous step.

```text
sudo cp target/release/nws_exporter /usr/local/bin/nws_exporter
sudo cp ext/nws_exporter.service /etc/systemd/system/nws_exporter.service
sudo sed -i 's/KBOS/YOUR_STATION/' /etc/systemd/system/nws_exporter.service
sudo systemctl daemon-reload
sudo systemctl enable nws_exporter.service
sudo systemctl start nws_exporter.serivce
```

### Prometheus

Prometheus metrics are exposed on port `9782` at `/metrics`. Once `nws_exporter`
is running, configure scrapes of it by your Prometheus server. Add the host running
`nws_exporter` as a target under the Prometheus `scrape_configs` section as described by
the example below.

```yaml
# Sample config for Prometheus.

global:
  scrape_interval:     15s
  evaluation_interval: 15s
  external_labels:
    monitor: 'my_prom'

scrape_configs:
- job_name: nws_exporter
  static_configs:
  - targets: ['example:9782']
```

## License

nws_exporter is available under the terms of the [GPL, version 3](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you shall be licensed as above, without any
additional terms or conditions.
