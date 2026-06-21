# Changelog

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
