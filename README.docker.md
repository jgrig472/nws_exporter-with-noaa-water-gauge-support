# nws_exporter

Prometheus metrics exporter for [api.weather.gov] weather stations and [NOAA water gauges].

Source and full documentation: https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support

[api.weather.gov]: https://www.weather.gov/documentation/services-web-api
[NOAA water gauges]: https://water.noaa.gov/

## Quick start

Pull the image:

```text
docker pull jasona1246/nws_exporter-with-noaa-water-gauge-support
```

Run it for an NWS weather station (e.g. `KBOS` for Logan Airport in Boston):

```text
docker run -p 9782:9782 jasona1246/nws_exporter-with-noaa-water-gauge-support KBOS
```

Run it for a NOAA water gauge instead (e.g. `dspi2` for the Des Plaines River at Joliet, IL):

```text
docker run -p 9782:9782 jasona1246/nws_exporter-with-noaa-water-gauge-support --gauge dspi2
```

Both can be combined, and `--gauge` may be repeated to monitor multiple gauges:

```text
docker run -p 9782:9782 jasona1246/nws_exporter-with-noaa-water-gauge-support KBOS --gauge dspi2 --gauge dspi3
```

Metrics are then available at `http://localhost:9782/metrics`.

Press Ctrl-C, or run `docker stop` from another terminal, to stop the container.

Logs are written to stdout/stderr and can be viewed with `docker logs`.

## Metrics emitted

Weather station metrics (prefixed `nws_`): station metadata, elevation, temperature, dewpoint, barometric
pressure, visibility, relative humidity, and wind chill.

Water gauge metrics (prefixed `nws_water_`): gauge metadata, current stage (feet), current flow (kcfs),
and action/minor/moderate/major flood stage thresholds (feet).

## [Prometheus] scrape config

```yaml
scrape_configs:
- job_name: nws_exporter
  static_configs:
  - targets: ['example:9782']
```

## License

GPL, version 3. See the [GitHub repository](https://github.com/jgrig472/nws_exporter-with-noaa-water-gauge-support) for source and license details.

[Prometheus]: https://prometheus.io/
