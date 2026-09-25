# AOG web dashboard

The dashboard is served by the Rust application from `/opt/aog/www`. The active
interface uses plain HTML, CSS, and JavaScript; it does not need an npm build or
an internet connection to load its assets.

## Active files

- `index.html`: Overview, connections, sensor history, cameras, reports, schedules, outputs,
  setup, and account screens.
- `css/aog.css`: shared layout, responsive styles, controls, and design tokens.
- `css/appearance.css` and `js/aog/appearance.js`: light/dark palettes, chart
  colors and the browser's appearance preference.
- `js/aog/index.js`: hash navigation, API requests, polling, charts, and forms.
- `login.html` and `js/aog/login.js`: device sign-in.
- `img/aog-mark.svg`, `img/icons.svg`, `img/grow-study.svg`, and
  `img/camera-off.svg`: the AOG mark, shared interface icons, sign-in
  illustration, and camera placeholder.
- `fonts/manrope-latin.woff2`: locally served variable typeface; its SIL Open
  Font License is included in `fonts/OFL-Manrope.txt`.

The unused SB Admin template, vendor assets, and Gulp files have been removed.
`hardware.js` and `hardware.css` implement the connection map, raw diagnostics,
probe settings, and firmware panel.

## Visual system

The dashboard uses warm white surfaces, forest green navigation, and lime
accents. The leaf-and-bubble mark is shared by the dashboard, sign-in page, and
favicon. Manrope is served by the device, with system fallbacks for characters
outside the bundled Latin font. No external font or icon requests are needed.

Sensor cards emphasize the reading and unit, with distinct temperature,
humidity, and air icons. Status labels always accompany color. Charts size
their SVG coordinates to the actual container so labels remain legible on
phones; sample controls share a row on wider screens and stack on small ones.
The sign-in illustration gives way to a compact brand header on mobile.
Controls retain visible keyboard focus, touch targets, reduced-motion support,
and native checkboxes in forced-colors mode.

The shared header identifies the current workspace, and phones have persistent
quick links to Overview, Connections, History and Outputs. Secondary water and
air measurements use labeled readout cards with direct history links. History
groups sensor and period selection above the charts. Setup has a persistent
section index on desktop; reports group period, context and download actions,
with raw exports before the report outline. Both schedule windows have
visual timelines, outputs summarize confirmed relay state, and camera views
can switch between side-by-side and stacked layouts. Account and sign-in share
the same typography, field spacing and local-device identity.

Setup opens one section at a time, with an **All sections** view for reviewing
multiple forms together. Section links retain their URLs, keyboard focus and
unsaved edits. History offers **Compare readings**, a compact view of the last
recorded values and period statistics, with a link into each full chart. Layout
choices last for the current browser page; they do not change device settings.
The report outline reflects the selected title, period, snapshot and notes.
Password requirements and confirmation feedback update while typing, while
the device remains responsible for validating password changes. Shared refresh
feedback and **Back to top** controls complete the page navigation.

Account and sign-in offer **System**, **Light** and **Dark** appearance. System
follows the operating system, including changes while the dashboard is open.
The preference is stored in this browser and synchronized between its tabs;
it never changes device settings or submits drafts. If browser storage is
unavailable, the choice still works until the page reloads. The initial palette
is applied before styles load, and chart colors change without replacing the
selected sample. All nine pages, diagnostics, forms and sign-in share the
palette. High-contrast system colors remain available.

Air-quality values separate the number and unit, with roomier reading rows on
phones. Missing readings have a quieter, explicitly unavailable presentation.
A failed live-readings request clears both water-temperature values along with
the air readings, so a previous temperature cannot look current during an
interruption. Saved history remains available.

## Atlas Gravity pH connection

The supported analog board is the Atlas Scientific **Gravity pH Meter V2.1**.
With the Dual OVF Arduino powered off, connect **A → A0**, **+ → 5V** and
**− → GND**. This is the Arduino with the overflow switches on D2 and D4.
The probe remains attached to the Gravity board. See the
[manufacturer’s wiring diagram](https://files.atlas-scientific.com/Gravity-pH-datasheet.pdf#page=3).

In **Connections → Atlas Gravity pH · connection setup**, confirm the physical
wiring to enable measurements. A0 voltage alone cannot identify an attached
board or detect an unplugged probe. The input is disabled until explicitly
confirmed; raw ADC counts and voltage remain available. Disable the input
before disconnecting the hardware. Enabling archives the previous calibration
and starts a new measurement period; check known buffers and calibrate again.

Older pH history without source verification, and unconfirmed Arduino replay
records, are retained but excluded from live values, charts, averages and
adjustment suggestions. Confirmation never reinterprets earlier raw logs as
water measurements. The connection setting is stored in `ph_input.json`;
changes use the authenticated, CSRF-protected `/api/ph-input` endpoint.

## Interaction conventions

- Mobile navigation opens with **Menu**, closes after selection, and supports
  Escape. Page changes move focus to the heading and return to the top. The
  skip link focuses the current page without changing routes. The mobile menu
  stays within reach while scrolling; it can scroll in a short landscape view.
- Settings sections have direct links, such as `#settings/calibration` and
  `#settings/overflow`, with focused headings and browser Back/Forward support.
  Section links also identify unsaved edits. Invalid names, duplicate relay
  assignments, and out-of-order pH limits point to the fields that need fixing.
- Settings and schedule forms compare drafts with the device's last settings.
  Fields become editable after the first saved settings load, so a slow
  connection cannot cause untouched values to be replaced by page defaults.
  Unsaved edits survive polling and navigation between dashboard pages. Each
  form offers **Discard changes**; saving an older draft does not overwrite
  edits made while that save is in progress.
- Refreshed settings update untouched fields while preserving edited values.
  If the device changes a field you also edited, choose **Keep my edit** or
  **Use device value** before saving. Repeated polls do not reopen a reviewed
  value, but a later device change does. The safety switch's enabled state and
  GPIO pin are reviewed together; discarding adopts the latest device settings.
- A shared draft summary links to every settings section with unsaved changes,
  including schedules. Navigation counts keep those drafts visible when you
  move to another page; counts clear after saving, discarding, or reverting.
  Passwords and calibration actions are not included in this summary.
- After an expired session, sign-in returns to the same known dashboard view.
  If settings drafts were open, the sign-in page explains that they were not
  saved and must be entered again. Return navigation never accepts an external
  URL, and passwords or draft contents are not stored for this purpose.
- Calibration explains the selected buffer, whether saving adds or replaces a
  point, and what is needed before applying it. Changing an entered raw value
  clears the stability confirmation. Save, remove, and reading-capture actions
  show progress and errors inside the calibration form.
- A failed request must not leave a current-looking sensor reading. Connection
  failures are shown explicitly, pH freshness follows the device clock, and
  unavailable live calibration values are cleared.
- Overview groups current sensor and equipment problems in **Needs attention**,
  above the readings, with links to history, overflow settings, and outputs.
  Healthy connections remain distinct from missing sensor readings. Issues
  clear after recovery, and polling preserves focus on review links.
- Refresh retries readings even when saved settings cannot be fetched. The
  device clock accepts fractional timestamps and UTC offsets while preserving
  the Pi's local schedule times.
- Charts support pointer and touch input. Focus a chart to inspect samples with
  Left/Right, Home, and End; Escape dismisses the reading. A crosshair marks the
  selected sample, and polling or resizing preserves the selection.
  **Previous** and **Next** move one displayed sample at a time and show its
  timestamp and value below the chart. Sensor history counts describe the
  samples shown, which may be downsampled; raw exports retain every recorded
  sample in the selected period. Intentional stepping announces the selection
  without repeated announcements from hovering or background updates.
- Overview readings link directly to their histories. Choose a single sensor
  or all eight; `#history/temp` and the other sensor links support browser
  Back/Forward and keep the selected time range. `#overview/ph` focuses pH
  history. Export controls follow the charts, keeping readings within reach.
- Schedule previews distinguish saved values from drafts and explain all-day
  and overnight windows. Missing relay assignments link to Setup; incomplete
  values show guidance instead of invalid previews.
- Equipment connection failures show unknown relays and remove unconfirmed
  countdowns and pump status assurances. The header distinguishes an equipment
  problem from unavailable environment readings. Stop controls stay available.
- Stop can interrupt a pending Start or Resume. The browser sends it immediately
  and repeats it after earlier conflicting requests finish, then checks relay
  state. Pending and unconfirmed commands remain explicit; a browser timeout
  cannot prove a device command was cancelled. Pausing schedules also remains
  available when paused automation has manually running outputs to stop.
- Reports explain which range applies to raw downloads, validate custom dates,
  and show preparation/error feedback. Preview tabs are populated only after a
  successful response; blocked pop-ups offer the download alternative. Feedback
  identifies the submitted period and HTML report title, even when options
  change during preparation. New options stay editable for the next export,
  and completion explains when they were excluded from the file. Titles and
  notes apply to HTML reports, not raw CSV or JSON data.
- Camera connection failures clear the Live indicator and show reconnection
  feedback.
- API writes use the existing session and `X-AOG-CSRF` token. Manual output
  controls and Stop all retain their existing device behavior.
- Sign-in and password changes lock credential fields while submitting, keeping
  the visible password consistent with the submitted one. Failed requests
  restore editing without taking focus from another task. Bookmarked dashboard
  views return after sign-in through the same fixed route allowlist.

## Browser checks

From the repository root, use an environment with Python Playwright and a
Chromium executable:

```sh
python3 -m venv /tmp/aog-ui-venv
/tmp/aog-ui-venv/bin/pip install playwright
CHROMIUM=/usr/bin/chromium /tmp/aog-ui-venv/bin/python tests/web_ui_test.py
```

The tests intercept requests to `https://aog.test` and serve these files with a
simulated device. They do not connect to hardware or switch physical outputs.
On the controller Pi, run browser checks one at a time, separately from Rust
builds. A temporary directory on `/dev/shm` and a CPU limit for the test process
reduce disk contention with sensor collection; monitor live board reports
while verifying an installed system.
They cover dashboard interactions, form saves and recovery, keyboard controls,
deep links, single-sensor history, report downloads and previews, camera
reconnection, sign-in feedback, and responsive layouts. The companion
`tests/web_ui_workflow_test.py` covers schedule previews and equipment recovery;
`tests/web_ui_drafts_test.py`, `tests/web_ui_session_test.py`, and
`tests/web_ui_calibration_test.py` cover draft review, returning after sign-in,
and calibration interactions. These run with the main suite or independently.
`tests/web_ui_attention_test.py`, `tests/web_ui_output_actions_test.py`, and
`tests/web_ui_account_test.py` cover issue recovery, device time, delayed control
commands, credential submission, and bookmarked sign-in.
`tests/web_ui_settings_conflicts_test.py`, `tests/web_ui_chart_steps_test.py`,
and `tests/web_ui_report_options_test.py` cover refreshed settings conflicts,
sample stepping, and edits made while report requests are pending.
Screenshots are written to `/tmp`.

`tests/web_ui_workspace_test.py` checks the shared navigation, viewport changes,
retained drafts, schedule windows, output summaries, camera layout, export order
and reading units across desktop and 390/320px screens. Set `AOG_AXE_PATH` to a
local axe-core script to audit every page, including sign-in, for WCAG A/AA
violations during this check.

`tests/web_ui_refinement_test.py` checks focused settings navigation and retained
drafts, comparison-to-chart navigation and range recovery, report outlines,
Unicode password guidance and confirmation feedback. It also supports
`AOG_AXE_PATH` and writes screenshots to `AOG_UI_ARTIFACTS` when specified.

`tests/web_ui_appearance_test.py` covers both appearances across all pages,
system changes, reload and cross-tab persistence, unavailable browser storage,
draft preservation, sensor units and water-temperature failure/recovery. With
`AOG_AXE_PATH`, it audits dark pages and expanded diagnostics as well as the
new controls and unavailable states in light appearance.

A plain static file server can show the shell, but live readings and device
controls require the Rust API. Editing this source directory does not itself
update an installed copy under `/opt/aog/www`.

`tests/web_ui_ph_input_test.py` checks rejected pH readings, retained-history warnings, exact Gravity wiring, explicit enable/disable actions and contrast at desktop and phone sizes.
