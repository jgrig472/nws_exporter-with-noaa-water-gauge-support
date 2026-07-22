# Changelog

## v0.9.2 - 2026-07-22

* Revamp the [Grafana NWS Station dashboard](ext/dashboard.json) from Grafana 8.4.3 schema to
  the modern `dashboard.grafana.app/v2` format targeting Grafana 13, consistent with the buoy
  dashboard.
* Add a **Current Conditions** stat row showing all six key readings at a glance — Temperature,
  Feels Like (Wind Chill), Dewpoint, Relative Humidity, Pressure, and Visibility — with
  color-coded threshold backgrounds (blue → green → yellow → red).
* Surface three metrics that were **missing** from the old dashboard: Dewpoint, Barometric
  Pressure, and Elevation.
* Convert all values to human-readable units: temperature/dewpoint/wind-chill from raw Celsius
  to **°F**, barometric pressure from Pascals to **hPa**, and visibility from meters to
  **statute miles**.
* Replace the time-series panels with modern Grafana 13 styling — smooth interpolation, gradient
  fills, multi-series tooltip, and per-metric accent colors.
* Update the Station variable to use short **ICAO codes** (e.g. `KBOS`, `KORD`) in the dropdown
  instead of raw `https://api.weather.gov/stations/…` URLs.

## v0.9.1 - 2026-07-21

* Add a `region` label to `nws_buoy_latitude_degrees` and `nws_buoy_longitude_degrees` metrics,
  automatically classified from each buoy's coordinates: **Great Lakes**, **Gulf of Mexico**,
  **Pacific Ocean**, **Caribbean Sea**, or **Atlantic Ocean**.
* Add a **"Body of Water"** multi-select variable to the [Grafana buoy dashboard](ext/buoy-dashboard.json)
  that filters the buoy selector to only stations in the chosen region(s). Selecting "Great Lakes"
  shows only Lake Michigan/Erie/Superior/etc. buoys; selecting "All" restores the full list.

## v0.9.0 - 2026-06-25

* Add `nws_buoy_latitude_degrees` and `nws_buoy_longitude_degrees` metrics with the buoy or
  coastal station's own coordinates, parsed from the NDBC station metadata table.
* Add `nws_buoy_coops_latitude_degrees` and `nws_buoy_coops_longitude_degrees` metrics (labeled
  with `coops_station`) giving the coordinates of the CO-OPS tide station matched to a buoy, if
  any.
* Add a "Location" row to the [Grafana buoy dashboard](ext/buoy-dashboard.json) with a Geomap
  panel plotting the buoy and its matched CO-OPS tide station, so you can visually confirm the
  auto-match is sensible. The map automatically pans and zooms to fit whichever markers are
  present whenever the buoy selector changes.

## v0.8.0 - 2026-06-23

* Automatically match each `--buoy` station to the nearest NOAA CO-OPS tide station (within
  `--coops-max-distance-nmi`, default 50 nautical miles), using published station coordinates.
  When matched, `nws_buoy_wind_direction_degrees`/`wind_speed_mps`/`wind_gust_mps`,
  `pressure_hpa`, `air_temperature_degrees`/`water_temperature_degrees`, and `tide_feet` are
  sourced from CO-OPS instead of NDBC whenever CO-OPS has a reading, falling back to NDBC
  otherwise. Buoys with no nearby CO-OPS station are unaffected.
* Add `nws_buoy_next_high_tide_feet`, `nws_buoy_next_high_tide_timestamp_seconds`,
  `nws_buoy_next_low_tide_feet`, and `nws_buoy_next_low_tide_timestamp_seconds` metrics, giving
  the predicted tide schedule for buoys matched to a CO-OPS tide station (NDBC has no
  equivalent).
* Add a `--buoy-tide-station BUOY_ID=COOPS_STATION_ID` flag to override or supply a CO-OPS tide
  station pairing manually, and `--coops-api-url`/`--coops-station-list-url`/
  `--coops-max-distance-nmi` flags to configure the new CO-OPS integration.
* Add a "Tide Predictions" row to the [Grafana buoy dashboard](ext/buoy-dashboard.json) with a
  next-high/low tide readout and an hours-until-next-tide-event countdown graph.

## v0.7.1 - 2026-06-22

* Convert the [Grafana buoy dashboard](ext/buoy-dashboard.json)'s Current Wave Height panel from
  meters to feet, so it displays consistently with the dashboard's other length panels (e.g. Tide).

## v0.7.0 - 2026-06-21

* Add a `buoy_name` label to all `nws_buoy_*` metrics with the station's friendly name (e.g.
  `Waukegan Buoy, IL`), looked up from the NDBC station metadata table; empty for stations not
  found in that table.
* Add a `--buoy-station-table-url` flag for overriding the NDBC station metadata table URL used
  for that lookup.
* Update the [Grafana buoy dashboard](ext/buoy-dashboard.json) station selector to show friendly
  station names instead of raw IDs, and convert the air/water temperature and dewpoint panels
  from Celsius to Fahrenheit.

## v0.6.1 - 2026-06-21

* Document buoy support (the new `--buoy` flag and `nws_buoy_*` metrics) in the Docker Hub
  README, keeping the published image description in sync with v0.6.0.

## v0.6.0 - 2026-06-21

* Add support for NOAA NDBC buoy and coastal weather station observations via the new `--buoy`
  flag (repeatable, combinable with `--gauge` and NWS stations), fetched from NDBC's `realtime2`
  data feed.
* Add `nws_buoy_*` Prometheus metrics for wind speed/gust/direction, wave height/period/direction,
  pressure (and tendency), air/water temperature, dewpoint, visibility, and tide.
* Add [Grafana dashboard](ext/buoy-dashboard.json) for visualizing buoy metrics, including a wind
  compass and wind rose panel.
* Add [ext/README.md](ext/README.md) documenting how to install the community Grafana plugins
  used by the new buoy dashboard.
* Update `docker-compose.yml` example to also demonstrate the new `--buoy` flag.

## v0.5.1 - 2023-10-21

* Dependency updates. #23
* Remove dependency on openssl. #22

## v0.5.0 - 2023-10-15

* Switch to Axum web framework. #19
* Build Docker images and binaries for each release. #20

## v0.4.0 - 2022-03-13

* Change station IDs to be a required argument (previously specified using `--station`) and
  add support for specifying multiple station IDs to collect metrics for.
* Add [Grafana dashboard](ext/dashboard.json) for visualizing metrics.

## v0.3.0 - 2022-02-05

* Emit station metadata as labels for the `nws_station` metric. #8
* Documentation improvements. #7 #9

## v0.2.0 - 2022-02-04

* Documentation.

## v0.1.0 - 2022-02-01

* Initial release.
