# AOG Rust v2 (alpha)

Rust v2 runs the Raspberry Pi controller, sensor monitoring, HTTPS dashboard,
lighting and watering schedules, and four configurable relay outputs. This is
the actively developed implementation; hardware assignments must match the
connected equipment.

## Wiring and pin maps

See the root README's [component overview](../../README.md#component-connections)
and [complete pin mapping](../../README.md#pin-mapping), with downloadable
[system illustration](../../media/diagrams/aog-system-overview.png) and
[exact pin-map image](../../media/diagrams/aog-pin-map.svg).

| Board | Connections in the checked-in firmware |
| --- | --- |
| Sensor-kit Arduino (`SENSORKIT_MK1`) | DHT11 **D7**; analog CO₂ **A0**; CCS811 **A4/SDA**, **A5/SCL** on classic Nano |
| Separate overflow/pH Arduino (`DUAL_OVF_SENSOR`) | Tank 1 switch **D2 → GND**; Tank 2 switch **D4 → GND**; analog pH interface output **A0** |
| Raspberry Pi I²C header | Physical **1: 3V3**, **3: GPIO2/SDA**, **5: GPIO3/SCL**, **6: GND**; LCD **0x72**, relay board **0x25** |

Overflow contacts close to GND at a safe level and open on overflow. Both
Arduinos connect separately over USB; their A0 pins are unrelated. The pH probe
connects through its analog interface board. Read the full pin map for power,
logic-level, firmware, and relay-assignment details before changing wiring.

## Building on Debian, Ubuntu, or Raspberry Pi OS

With Rust installed, run these commands from this directory:

```sh
sh ./setup.sh
cargo build --locked
```

The setup script installs the native build dependencies, including `libv4l-dev`.
The camera dependency needs its `libv4l2.so` linker file; having only the runtime
library (`libv4l2.so.0`) is not sufficient. If the build reports
`unable to find library -lv4l2`, install `libv4l-dev` and rebuild.

## Access from another device

The dashboard defaults to HTTPS on `0.0.0.0:8443`, and the command API defaults
to HTTPS on `0.0.0.0:9443`. Open `https://<device-ip>:8443/` from a computer
on the same network. The generated certificate is self-signed, so the browser
will ask you to accept it. Log in as `admin` with the existing installation
password.

Existing installations keep the addresses saved in `/opt/aog/data.json`.
To enable LAN access, set both `https_bind_address` and
`command_api_bind_address` to `"0.0.0.0"` and restart AOG. The corresponding
`https_bind_port` and `command_api_bind_port` settings remain configurable.

Remote command requests require a nonempty `command_api_token` in that file
and an `Authorization: Bearer <token>` header. Without a configured token,
command requests are accepted only from loopback. The dashboard login uses
the admin password independently of this token.

Login sessions last one hour and are cleared by logout or a server restart.
Dashboard readings and live pH refresh every five seconds. Output states refresh
every second on Outputs and every three seconds on other visible pages, with
countdowns updating between requests. Settings refresh every fifteen seconds
without replacing unsaved form edits. Returning to a page or browser tab refreshes
its content, and actions immediately apply the device's response. Environmental
history is recorded and refreshed every minute. AOG
continues serving when stdin is closed and exits on SIGTERM or Ctrl-C.

The binary embeds `src/www.zip`; changes to `www/` must also be packaged into
that archive before rebuilding.

### Run on device startup

Install a tested build as a persistent service so the dashboard and schedules
return after the Pi restarts. Run from this directory:

```sh
cargo build --locked -j1
sudo install -m 755 target/debug/aog /usr/local/bin/aog-v2.new
sudo mv /usr/local/bin/aog-v2.new /usr/local/bin/aog-v2
sudo install -d /opt/aog
sudo install -m 644 deploy/aog-v2.service /etc/systemd/system/aog-v2.service
sudo systemctl daemon-reload
sudo systemctl enable aog-v2
sudo systemctl restart aog-v2
```

This runtime currently requires root for device setup and hardware access.
Inspect logs with `sudo journalctl -u aog-v2 -f`. Startup preserves the installed
HTTPS certificate and key instead of changing the device identity on every
restart.

## Dashboard controls

The dashboard works on desktop and mobile and provides:

- **Overview:** live sensor and overflow readings, available camera images,
  relay states, and a pH history chart with 1-hour, 6-hour, and 24-hour views.
  History is available before calibration; missing, stale, and failed readings
  have explicit status messages.
- **Sensor history:** CO₂ (ppm), TVOC (ppb), PM2.5 and PM10 (µg/m³), temperature
  (°C), and humidity (%), with shared 1-hour, 6-hour, 24-hour, and 7-day ranges.
  Each chart shows its latest recorded value, minimum, average, and maximum.
  Hover, tap, or use arrow keys on a focused chart to inspect recorded samples.
- **Schedules:** daily lighting and repeating watering, including overnight
  windows. Schedules use the Pi's local time, displayed on the page. Both are
  disabled by default. Lighting controls the shared **Lights + Air** relay;
  the repeating water schedule can run **Filling** or **Tank-to-tank**. Saving an enabled lighting schedule applies
  its current window immediately. Watering waits one interval after enabling,
  saving schedules, resuming, or restarting AOG. Changing the scheduled pump
  stops any scheduled run on the previous pump before applying the new target.
- **Outputs:** Draining uses **Start draining / Stop**, with no timer, cooldown,
  or operating-hour restriction. It keeps running when the browser closes;
  press **Stop** when finished. Other outputs use timed manual runs with actual
  relay readback, remaining runtime, cooldown, and stop controls. Lights + Air
  switch together for up to 24 hours per manual run. Configured overflow stops,
  the external safety input, emergency stop, and one-pump-at-a-time protection
  apply to every water pump, regardless of relay number. Unassigned relays cannot be started.
  Each relay has one button that switches between **Start** and **Stop**, with
  pending feedback until the device responds. **Stop all** remains available.
- **Setup:** setup name, output labels and assignments, sensor names and per-pump
  overflow stops, pump runtime/cooldown/safety limits,
  pH monitoring thresholds and interval, history retention, and buffer
  calibration. One buffer immediately normalizes pH; a second distinct buffer
  calibrates the slope. Individual points can be removed or replaced. Calibration
  and monitoring changes take effect without restarting.
- **Account:** change the admin password using the current password. A
  successful change signs out all sessions immediately; sign in with the new
  password. Forgotten passwords can still be reset locally as described below.

### Live pH calibration

Under **Setup → pH calibration**, leave **Use the live sensor reading when saving**
checked to capture a fresh reading on the Pi when you save. The raw value and
current pH update on screen. Uncheck it to enter a measured average manually.
Confirm the probe is stable in the selected buffer before saving.

A single point, such as pH 7, applies an offset immediately and is labeled
**1-point normalized**. It does not determine the probe's slope. A second distinct
buffer adds slope calibration; three points use separate pH 4–7 and 7–10 segments
so the third buffer is honored. Buffers with identical raw readings or an
inconsistent three-point order are rejected. Very close raw readings produce a
visible warning because they amplify noise. **Remove** on a point returns to the
remaining calibration, including single-point normalization.

Saving or removing a point immediately refreshes the live pH, takes a new history
sample when the sensor is available, and returns the updated chart and status.
The chart recalculates saved raw samples using the current calibration; original
raw samples remain intact on disk. Out-of-range results appear as gaps instead
of being clipped to 0 or 14. The existing temperature correction is referenced to
the saved neutral buffer's temperature (or another saved point when pH 7 is absent).
Changing thresholds and sampling settings also updates the page immediately.

`POST /api/ph/calibrate` accepts `buffer_ph` and an optional `raw_value`; omitting
the latter captures the live sensor reading. `POST /api/ph/calibration/remove`
accepts `buffer_ph` of `4`, `7`, or `10`. Both return the complete updated pH
snapshot and require the dashboard session and CSRF token.

### Sensor history

The Pi samples the six environmental readings once per minute independently of
browser connections. Up to seven days are retained in memory and in daily UTC
JSONL files under `/opt/aog/sensor_history/v1/`. Appending records avoids rewriting
the entire history on every sample. AOG reloads saved records on startup and
removes expired daily files automatically. Include this directory in backups
to retain environmental history when moving to another Pi.

Collection begins with installation of this version. Past readings cannot be
backfilled from the live sensor files. Missing, invalid, and stale values are
recorded as gaps, never as zero. Zero is a valid reading, including for TVOC.
The history sampler accepts environment files updated within 15 seconds and
particulate files updated within 120 seconds. Charts retain gaps during service
downtime and use the browser's local time zone. Wide time ranges reduce plotted
points while retaining peaks and troughs; statistics use all valid samples.

The authenticated endpoint `GET /api/history?hours=24` accepts `1`, `6`, `24`,
or `168` hours. Its response includes each metric's units, freshness, latest
recorded value, window statistics, and chart points. Storage failures are
reported on the page while in-memory history remains available. Environmental
retention is separate from the configurable pH sample interval and history size;
the pH chart remains on Overview.

### Assign the four relays

Under **Setup settings → Setup & relay assignments**, choose **Lights + Air**,
**Draining**, **Filling**, **Tank-to-tank**, or **Unassigned** for each physical relay. Each function
can be assigned once; multiple unused relays may remain unassigned. Labels are
separate from functions and do not determine the safety rules.

Use **Stop all** before changing assignments. The server verifies that every
relay is off, saves the assignments, and leaves schedules paused. Resume
schedules when the assignments match the connected equipment. Only one of
Filling, Draining, and Tank-to-tank can run at a time; Lights + Air can run alongside it.
Overflow checks follow the water-pump functions even when assigned to relay 1.
Cooldown and runtime limits apply to Filling and Tank-to-tank; Draining is untimed.

New installations default to Lights + Air on relay 1, Draining on relay 2,
Filling on relay 3, and Tank-to-tank on relay 4. Tank-to-tank moves water from the
55-gallon drum to the display tank above. Filling adds water to the drum.
Existing saved maps are preserved: legacy `lights` reads as `lights_air`, legacy
standalone `air` becomes unassigned, and an unassigned fourth relay requires an
explicit assignment. Existing water schedules keep targeting Filling until
changed in **Schedules → Scheduled pump**. Filling runs are capped at 300 seconds;
Tank-to-tank runs are capped at 600 seconds, subject to your lower configured
pump runtime limit. Draining has no runtime limit.

### Configure overflow stops

Under **Setup → Overflow sensor stops**, name sensor 1 (Arduino D2) and sensor 2
(D4) for their actual locations. Select either sensor, both, or **No overflow
stop** separately for Filling, Tank-to-tank, and Draining. Both sensors protect
each water pump by default. These settings follow functions, not relay numbers.

For example, if sensor 1 is in the display tank and sensor 2 is in the drum,
select sensor 1 for Tank-to-tank and sensor 2 for Filling. Either sensor can
protect multiple pumps. A selected sensor blocks starting and stops an active
run on overflow or missing/stale data. An unselected sensor's overflow does not
block that pump. Losing the shared sensor board stops all pumps using either
sensor. **No overflow stop** explicitly disables this protection for that pump.

Save these settings with all outputs off; schedules remain paused afterward.
Emergency stop, the external safety input, and one-pump-at-a-time checks continue
to apply independently. Runtime, cooldown, and the pump operating window apply
to Filling and Tank-to-tank. Output cards list each pump's selected
sensors and the particular sensor preventing a run.

Console and command API relay actions use the same controller and protections:
`relay on <1–4> [seconds]`, `relay off <1–4|all>`, and `relay status`.
For Draining, omit the duration: with the default map, use `relay on 2` and
`relay off 2`. Other outputs require an explicit duration. Likewise,
`POST /api/outputs/start` takes `{"channel":2}` for Draining and requires
`seconds` for timed outputs; a duration supplied for Draining is rejected.
Draining status reports `maximum_seconds` and `remaining_seconds` as `null`,
with `cooldown_seconds` always zero. Commands report failures instead of
silently retrying through a separate hardware control path.

Stopping any output pauses schedules; scheduled outputs then stop. **Stop all**
and **Pause schedules** stop all four outputs immediately. The pause survives
restarts until explicitly resumed. Schedule settings persist in
`/opt/aog/data.json`; pause state is in `/opt/aog/automation_paused`.

The server supervises timed runs even after the browser closes. Draining stays
on until stopped manually or by a configured safety input, emergency stop, or
service shutdown; it does not restart automatically. Scheduled
water runs require fresh, safe readings from their selected sensors and observe pump cooldown, any configured
operating window, the optional GPIO safety input, and emergency stop. Only one
pump can run at a time. A watering run also stops when its daily window closes.
Shutdown failures block new starts and are retried. Lighting and watering
schedules run only while AOG is running.

Settings, password, calibration, and output APIs require a dashboard session;
mutations also require its `X-AOG-CSRF` token. Settings responses omit password
hashes and command tokens. Configuration saves use atomic file replacement.

### Reset a forgotten admin password

Run these commands on the AOG device from this directory:

```sh
cargo build --locked -j1 --example reset_admin_password
sudo target/debug/examples/reset_admin_password
```

The utility prints a new password for `admin` and saves its hash to the existing
configuration without initializing hardware. Restart AOG to load the change
(for a systemd service named `aog-v2`, run `sudo systemctl restart aog-v2`).

## I²C display and relay board

The LCD uses address `0x72` and the quad relay board uses `0x25` on `/dev/i2c-1`.
On Raspberry Pi OS, enable I²C in `/boot/firmware/config.txt`:

```ini
dtparam=i2c_arm=on
dtparam=i2c_arm_baudrate=10000
```

Load `i2c-dev` at boot (for example, list it in
`/etc/modules-load.d/aog-i2c.conf`). Reboot after editing the boot settings.
The 10 kHz clock avoids intermittent SerLCD transfer failures observed on the
AOG Pi 4 at the default 100 kHz. Slowing the bus is a documented
[Raspberry Pi clock-stretching workaround](https://learn.adafruit.com/raspberry-pi-i2c-clock-stretching-fixes/change-the-clock-speed).
The LCD also spaces out commands and characters and retries connection failures.

Relay startup switches all four outputs off and verifies their status. Periodic
health checks only read status registers. Do not use the dependency's single-relay
`get_version()` function on a quad board: its register address toggles relay 4.

## Serial sensor diagnostics

AOG discovers `ttyUSB*` and `ttyACM*` ports and identifies Arduino kits from
complete `BEGIN` / `END` reports. It tries 9600 baud and legacy 1200 baud.
Arduino and SDS011 readers share port ownership, so they cannot consume each
other's serial data. Temporary `WouldBlock`, timeout, and interrupted reads
are retried without flooding the log.

Overflow monitoring starts with sensor-protected pumping blocked. Both readings must come
from a valid `DUAL_OVF_SENSOR` report before the communication fault clears.
Actual `OVERFLOW` readings block the functions assigned to that sensor. A disconnect, invalid overflow
report, or five seconds without a valid report restores the communication
fault, blocking pumps that use either sensor. Pump checks also reject missing,
invalid, or stale selected reading files. The legacy GPIO pump path continues
requiring both sensors; configurable routing applies to the four relay outputs.

With AOG stopped, build and run the sensor diagnostics tool:

```sh
cargo build --locked --example sensor_diagnostics
sudo target/debug/examples/sensor_diagnostics --seconds 60
```

This updates `/opt/aog/sensors` and prints readings for the requested duration.
It does not initialize pumps, relays, the LCD, or HTTP servers. The discovery
log includes the detected device, port, and baud rate. Allow a full discovery
cycle for every connected kit to appear.

The serial and overflow regression tests use temporary files and simulated
reads, including split reports, transient errors, disconnects, stale readings,
and invalid sensor data:

```sh
sh ./test-on-device.sh --lib
```

This helper isolates `/opt/aog` and localhost networking so tests cannot
overwrite live data or send commands to the running service. Dependencies are
fetched before entering the test network namespace.
