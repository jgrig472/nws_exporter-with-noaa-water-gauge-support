# ext/

This directory contains example Grafana dashboards and a Systemd unit file for `nws_exporter`.

* `dashboard.json` - NWS weather station dashboard.
* `water-dashboard.json` - NOAA water gauge dashboard.
* `buoy-dashboard.json` - NOAA buoy/coastal station dashboard.
* `nws_exporter.service` - Systemd unit file for running `nws_exporter` as a service.

## Buoy dashboard plugin requirements

`buoy-dashboard.json` uses two community Grafana panel plugins to visualize wind direction and
speed, since no built-in Grafana panel represents that data well:

* [Compass panel](https://grafana.com/grafana/plugins/oceandatatools-compass-panel/) - `oceandatatools-compass-panel`
* [Operato Windrose](https://grafana.com/grafana/plugins/operato-windrose-panel/) - `operato-windrose-panel`

Both are unsigned community plugins, so Grafana won't load them until you install them *and*
explicitly allow unsigned plugins. Pick one of the two options below depending on how you run
Grafana.

### Option 1: Docker Compose environment variable

If you run Grafana via Docker Compose, install the plugins and allow them as unsigned in one step
by adding the following environment variables to your Grafana service:

```yaml
services:
  grafana:
    image: grafana/grafana:latest
    environment:
      - GF_INSTALL_PLUGINS=oceandatatools-compass-panel,operato-windrose-panel
      - GF_PLUGINS_ALLOW_LOADING_UNSIGNED_PLUGINS=oceandatatools-compass-panel,operato-windrose-panel
    ports:
      - 3000:3000
    restart: always
```

`GF_INSTALL_PLUGINS` installs the plugins on container start; `GF_PLUGINS_ALLOW_LOADING_UNSIGNED_PLUGINS`
tells Grafana to load them even though they're not signed.

### Option 2: Install via grafana-cli

If you run Grafana directly (a binary install or your own container image), install each plugin
with `grafana-cli`:

```text
grafana-cli plugins install oceandatatools-compass-panel
grafana-cli plugins install operato-windrose-panel
```

Then edit your `grafana.ini` to allow them to load unsigned:

```ini
[plugins]
allow_loading_unsigned_plugins = oceandatatools-compass-panel,operato-windrose-panel
```

Restart Grafana after either option so the plugins are picked up, then import `buoy-dashboard.json`
under Dashboards -> New -> Import.
