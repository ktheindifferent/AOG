'use strict';
(() => {
  const $ = id => document.getElementById(id);
  const state = {
    csrf: null,
    passwordChanging: false,
    sessionExpired: false,
    loginRedirect: null,
    cameras: [],
    camerasRequest: 0,
    cameraViews: {},
    cameraError: null,
    settings: null,
    ph: null,
    phError: null,
    phRequest: 0,
    phBusy: false,
    settingsRequest: 0,
    settingsBusy: false,
    statsRequest: 0,
    dirtyForms: new Set(),
    outputsRequest: 0,
    outputActionRequest: 0,
    outputPending: new Map(),
    outputsReceivedAt: 0,
    history: null,
    historyError: null,
    historyRange: 24,
    historyRequest: 0,
    historyCharts: {},
    outputs: null,
    range: 24,
    chartPoints: [],
    refreshing: false
  };
  const pageCopy = {
    overview: ['Grow overview', 'Your environment, water, and equipment in one place.'],
    history: ['Sensor history', 'Follow air quality, temperature, and humidity over time.'],
    cameras: ['Live cameras', 'A window into your grow, from both cameras.'],
    reports: ['Reports & exports', 'Capture your air quality story and keep the underlying data.'],
    schedules: ['Lighting & watering', 'Build a daily rhythm for your grow.'],
    outputs: ['Equipment controls', 'Start and stop each relay independently.'],
    settings: ['Setup settings', 'Tune your environment and make this setup your own.'],
    account: ['Your account', 'Manage your password and device access.']
  };
  const text = (id, value) => {
    $(id).textContent = value;
  };
  const finite = value => typeof value === 'number' && Number.isFinite(value);
  const roleNames = {
    unassigned: 'Unassigned',
    lights_air: 'Lights + Air',
    filling: 'Filling',
    draining: 'Draining',
    tank_cycle: 'Tank-to-tank'
  };
  const waterRoles = ['filling', 'tank_cycle', 'draining'];
  const historyMetrics = [
    {key: 'co2', label: 'CO₂', unit: 'ppm', decimals: 0, color: '#507943'},
    {key: 'tvoc', label: 'TVOC', unit: 'ppb', decimals: 0, color: '#8163a0'},
    {key: 'pm25', label: 'PM2.5', unit: 'µg/m³', decimals: 1, color: '#ae7434'},
    {key: 'pm10', label: 'PM10', unit: 'µg/m³', decimals: 1, color: '#8a6546'},
    {key: 'temp', label: 'Temperature', unit: '°C', decimals: 1, color: '#a95545'},
    {key: 'hum', label: 'Humidity', unit: '%', decimals: 1, color: '#407e94'}
  ];
  const sensorSelection = value => [value === 'both' || value === '1', value === 'both' || value === '2'];
  const selectionValue = sensors => sensors[0] ? sensors[1] ? 'both' : '1' : sensors[1] ? '2' : 'none';
  const sensorNames = (sensors, labels) => sensors.map((selected, index) => selected ? `${labels[index]} (sensor ${index + 1})` : null).filter(Boolean).join(' + ') || 'No overflow stop';
  const relayFor = role => state.settings.setup.relay_roles.indexOf(role) + 1;
  const formatNumber = value => finite(value) ? value.toFixed(2) : '—';
  const timeText = minute => `${String(Math.floor(minute / 60)).padStart(2, '0')}:${String(minute % 60).padStart(2, '0')}`;
  const minutes = value => {
    const [hour, minute] = value.split(':').map(Number);
    return hour * 60 + minute;
  };
  const dateText = seconds => new Date(seconds * 1000).toLocaleString([], {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit'
  });
  const duration = seconds => seconds >= 3600 ? `${Math.floor(seconds / 3600)}h ${Math.floor(seconds % 3600 / 60)}m` : seconds >= 60 ? `${Math.floor(seconds / 60)}m ${seconds % 60}s` : `${seconds}s`;

  function badge(element, label, kind = 'neutral') {
    element.textContent = label;
    element.className = `badge ${kind}`;
  }

  function message(id, value, error = false) {
    text(id, value);
    $(id).classList.toggle('error', error);
  }

  function flash(value) {
    text('toast', value);
    $('toast').hidden = false;
    clearTimeout(flash.timer);
    flash.timer = setTimeout(() => {
      $('toast').hidden = true;
    }, 4500);
  }

  function globalError(value) {
    text('global-message', value);
    $('global-message').hidden = !value;
    $('global-message').classList.toggle('bad', Boolean(value));
  }
  function redirectToLogin(reason = 'expired') {
    // Background requests can receive 401 after a password change has already
    // succeeded. They must not replace its confirmation with an expiry notice.
    if (state.loginRedirect === 'changed') return;
    if (reason === 'expired' && state.passwordChanging) {
      state.sessionExpired = true;
      return;
    }
    if (state.loginRedirect === reason) return;
    state.loginRedirect = reason;
    state.csrf = null;
    window.location.replace(`/login.html?${reason}=1`);
  }

  function updateFormAvailability() {
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = !state.settings || !state.csrf || Boolean(state.loginRedirect)
        || Boolean(button.closest('form').dataset.saving);
    });
  }

  async function api(path, options = {}) {
    const abort = new AbortController();
    const timeout = setTimeout(() => abort.abort(), 10000);
    try {
      const headers = {
        Accept: 'application/json'
      };
      if (options.body !== undefined) {
        headers['Content-Type'] = 'application/json';
        headers['X-AOG-CSRF'] = state.csrf || '';
      }
      const response = await fetch(path, {
        method: options.body === undefined ? 'GET' : 'POST',
        credentials: 'same-origin',
        cache: 'no-store',
        headers,
        body: options.body === undefined ? undefined : JSON.stringify(options.body),
        signal: abort.signal
      });
      const body = await response.json().catch(() => {
        if (response.ok) throw new Error('The device returned an invalid response. Try Refresh.');
        return {};
      });
      if (response.status === 401) {
        redirectToLogin();
        throw new Error('Your session expired. Sign in again.');
      }
      if (!response.ok) throw new Error(body?.error || `Request failed (${response.status})`);
      if (!body || typeof body !== 'object') throw new Error('The device returned an invalid response. Try Refresh.');
      return body;
    } catch (error) {
      if (error.name === 'AbortError') throw new Error('The device took too long to respond. Try again.');
      throw error;
    } finally {
      clearTimeout(timeout);
    }
  }

  function navigate() {
    const page = Object.prototype.hasOwnProperty.call(pageCopy, location.hash.slice(1)) ? location.hash.slice(1) : 'overview';
    document.querySelectorAll('.page').forEach(section => {
      section.hidden = section.id !== `page-${page}`;
    });
    document.querySelectorAll('[data-page]').forEach(link => {
      if (link.dataset.page === page) link.setAttribute('aria-current', 'page');
      else link.removeAttribute('aria-current');
    });
    text('page-title', pageCopy[page][0]);
    text('page-description', pageCopy[page][1]);
    document.title = `AOG · ${pageCopy[page][0]}`;
    if (page === 'overview') renderChart();
    if (page === 'history') renderHistory();
    syncCameras();
    if (state.csrf) refreshVisiblePage();
  }

  function showSettings(settings, hydrate = true) {
    state.settings = settings;
    text('setup-name', settings.setup.setup_name);
    text('sidebar-setup-name', settings.setup.setup_name);
    text('device-address', location.origin);
    text('device-version', settings.version);
    text('command-access', settings.network.command_token_configured ? `Port ${settings.network.command_port} · token required` : `Port ${settings.network.command_port} · local commands only`);
    showClock(settings.device_time);
    const {
      setup,
      schedules,
      pump,
      ph
    } = settings;
    if (hydrate && !state.dirtyForms.has('setup-form')) {
      $('setup-name-input').value = setup.setup_name;
      setup.relay_labels.forEach((label, i) => {
        $(`relay-label-${i + 1}`).value = label;
        $(`relay-role-${i + 1}`).value = setup.relay_roles[i];
        $(`relay-role-${i + 1}`).dataset.previousRole = setup.relay_roles[i];
      });
    }
    if (hydrate && !state.dirtyForms.has('overflow-form')) {
      setup.overflow.sensor_labels.forEach((label, i) => {
        $(`overflow-label-${i + 1}`).value = label;
      });
      waterRoles.forEach(role => {
        $(`overflow-${role}`).value = selectionValue(setup.overflow[role]);
      });
    }
    if (hydrate && !state.dirtyForms.has('schedule-form')) {
      $('lighting-enabled').checked = schedules.lighting.enabled;
      $('lighting-start').value = timeText(schedules.lighting.start_minute);
      $('lighting-end').value = timeText(schedules.lighting.end_minute);
      $('watering-enabled').checked = schedules.watering.enabled;
      $('watering-start').value = timeText(schedules.watering.start_minute);
      $('watering-end').value = timeText(schedules.watering.end_minute);
      $('watering-interval').value = schedules.watering.every_minutes;
      $('watering-duration').value = schedules.watering.run_seconds;
      $('watering-pump').value = schedules.watering.pump;
    }
    if (hydrate && !state.dirtyForms.has('pump-form')) {
      $('safety-enabled').checked = pump.safety_gpio_pin !== null;
      $('safety-pin').value = pump.safety_gpio_pin === null ? 17 : pump.safety_gpio_pin;
      $('safety-pin-field').hidden = !$('safety-enabled').checked;
    }
    if (hydrate && !state.dirtyForms.has('ph-settings-form')) {
      $('ph-enabled').checked = ph.enabled;
      $('ph-alerts').checked = ph.alert_enabled;
      [
        ['ph-min', 'optimal_min'],
        ['ph-max', 'optimal_max'],
        ['ph-critical-min', 'critical_min'],
        ['ph-critical-max', 'critical_max'],
        ['ph-interval', 'monitoring_interval_seconds'],
        ['ph-history-size', 'history_size']
      ].forEach(([id, key]) => {
        $(id).value = ph[key];
      });
    }
    setup.overflow.sensor_labels.forEach((label, i) => text(`tank${i + 1}-name`, label));
    text('pump-limit-summary', 'All relays support manual Start/Stop without runtime caps, cooldowns, or operating-hour restrictions. Multiple pumps can run together.');
    text('ph-target', `Target ${ph.optimal_min.toFixed(1)}–${ph.optimal_max.toFixed(1)}`);
    renderSchedules();
    previewSchedules();
    previewAssignments();
    previewOverflow();
    renderOutputs();
    renderChart();
    updateFormAvailability();
  }

  function showClock(stamp) {
    if (!stamp) return;
    const display = stamp.slice(0, 19).replace('T', ' ') + ' (UTC' + stamp.slice(19) + ')';
    text('device-time', `Times follow the Pi’s clock: ${display}`);
    text('footer-clock', `Device time · ${stamp.slice(11, 16)} UTC${stamp.slice(19)}`);
  }

  function renderSchedules() {
    if (!state.settings) return;
    const {
      lighting,
      watering
    } = state.settings.schedules;
    const paused = Boolean(state.outputs && state.outputs.automation_paused);
    for (const [id, role] of [
        ['lighting-relay', 'lights_air'],
        ['watering-relay', watering.pump]
      ]) {
      const channel = relayFor(role);
      text(id, `${roleNames[role]} · ${channel ? `Relay ${channel}` : 'No relay assigned'}`);
    }
    text('lighting-summary', lighting.enabled ? `${timeText(lighting.start_minute)}–${timeText(lighting.end_minute)}${lighting.start_minute > lighting.end_minute ? ' · overnight' : ''}` : 'No automatic lighting period');
    text('watering-summary', watering.enabled ? `${roleNames[watering.pump]} · ${watering.run_seconds}s every ${watering.every_minutes} min · ${timeText(watering.start_minute)}–${timeText(watering.end_minute)}` : 'No automatic water pump cycle');
    badge($('lighting-state'), lighting.enabled ? paused ? 'Paused' : 'Scheduled' : 'Disabled', lighting.enabled && !paused ? 'good' : 'neutral');
    badge($('watering-state'), watering.enabled ? paused ? 'Paused' : 'Scheduled' : 'Disabled', watering.enabled && !paused ? 'good' : 'neutral');
    const remaining = state.outputs && state.outputs.next_watering_in_seconds;
    const elapsed = Math.max(0, Math.floor((performance.now() - state.outputsReceivedAt) / 1000));
    const wait = finite(remaining) ? Math.max(0, remaining - elapsed) : null;
    text('next-watering', paused ? 'Schedules are paused. Resume when you are ready.' : watering.enabled && finite(wait) ? `Next watering check in ${duration(wait)}. Window and safety conditions must allow a run.` : 'Schedules run on the device when your browser is closed.');
  }

  function previewSchedules() {
    const waterRole = $('watering-pump').value;
    const start = minutes($('lighting-start').value),
      end = minutes($('lighting-end').value);
    const enabled = $('lighting-enabled').checked;
    const from = start / 1440 * 100,
      to = end / 1440 * 100;
    $('lighting-preview').style.background = !enabled ? '#edf1e7' : start < end ? `linear-gradient(to right,#edf1e7 ${from}%,#a9c77c ${from}%,#a9c77c ${to}%,#edf1e7 ${to}%)` : `linear-gradient(to right,#a9c77c ${to}%,#edf1e7 ${to}%,#edf1e7 ${from}%,#a9c77c ${from}%)`;
    const hours = start === end ? 24 : ((end - start + 1440) % 1440) / 60;
    text('lighting-preview-text', enabled ? `${hours.toFixed(1)} hours of lights and air per day${start > end ? ', crossing midnight' : ''}. Both switch together.` : 'Lights + Air schedule is disabled. Manual Start/Stop remains available.');
    text('watering-preview-text', $('watering-enabled').checked ? `${roleNames[waterRole]}: run for ${$('watering-duration').value || '—'} seconds every ${$('watering-interval').value || '—'} minutes. The first run waits one interval; blocked runs retry once conditions allow.` : 'Water pump schedule is disabled. Choose the pump and interval, then enable it when ready.');
  }

  function previewAssignments() {
    const roles = [1, 2, 3, 4].map(channel => $(`relay-role-${channel}`).value);
    text('relay-assignment-summary', ['lights_air', 'draining', 'filling', 'tank_cycle'].map(role => {
      const channels = roles.map((assigned, index) => assigned === role ? index + 1 : null).filter(Boolean);
      return `${roleNames[role]} → ${channels.length ? `Relay ${channels.join(', ')}` : 'Unassigned'}`;
    }).join(' · '));
  }

  function previewOverflow() {
    const labels = [1, 2].map(index => $(`overflow-label-${index}`).value.trim() || `Sensor ${index}`);
    text('overflow-assignment-summary', waterRoles.map(role => `${roleNames[role]} → ${sensorNames(sensorSelection($(`overflow-${role}`).value), labels)}`).join(' · '));
    $('overflow-assignment-summary').classList.toggle('warning', waterRoles.some(role => $(`overflow-${role}`).value === 'none'));
  }

  function metric(id, raw, suffix = '') {
    const value = typeof raw === 'string' ? Number.parseFloat(raw) : raw;
    const element = $(id);
    element.replaceChildren();
    if (!finite(value)) {
      element.textContent = '—';
      return;
    }
    element.append(document.createTextNode(Number.isInteger(value) ? value.toLocaleString() : value.toFixed(1)));
    if (suffix) {
      const unit = document.createElement('small');
      unit.textContent = ` ${suffix}`;
      element.append(unit);
    }
  }

  function showStats(data) {
    metric('temperature-value', data.temp, '°C');
    metric('humidity-value', data.hum, '%');
    metric('co2-value', data.co2);
    text('pm25-value', data.pm25 === 'N/A' ? 'Unavailable' : data.pm25);
    text('pm10-value', data.pm10 === 'N/A' ? 'Unavailable' : data.pm10);
    text('tvoc-value', data.tvoc === 'N/A' ? 'Unavailable' : data.tvoc);
    for (const index of [1, 2]) {
      const value = data[`t${index}_ovf`];
      const normal = ['NONE', 'NORMAL'].includes((value || '').trim());
      const error = data.overflow_error || !['NONE', 'NORMAL', 'OVERFLOW'].includes((value || '').trim());
      badge($(`tank${index}-status`), error ? 'Unavailable' : normal ? 'Normal' : 'Overflow', error || !normal ? 'warning' : 'good');
      text(`tank${index}-detail`, error ? 'Sensor data needs attention' : normal ? 'Overflow sensor clear' : 'Overflow sensor triggered');
    }
    showClock(data.device_time);
    badge($('connection'), '● Connected', 'good');
    text('updated-at', `Updated ${new Date().toLocaleTimeString([], {hour:'2-digit', minute:'2-digit', second:'2-digit'})}`);
  }

  function showPhStatus(status) {
    text('ph-value', formatNumber(status.current_ph));
    const hasReading = finite(status.current_ph);
    badge($('ph-badge'), !status.enabled ? 'Paused' : !hasReading ? 'No reading' : status.alert_level, status.enabled && hasReading ? status.alert_level === 'Normal' ? 'good' : status.alert_level === 'Warning' ? 'warning' : 'bad' : 'neutral');
    text('ph-trend', !status.enabled ? 'Monitoring paused' : !hasReading ? 'Waiting for readings' : status.reading_count < 2 ? 'Collecting trend data' : `${status.trend === 'Rising' ? '↗' : status.trend === 'Falling' ? '↘' : '→'} ${status.trend} · last hour`);
    const points = status.calibration_points || 0;
    const calibrationLabel = points >= 2 ? `${points}-point calibrated` : points === 1 ? '1-point normalized' : 'Not calibrated';
    badge($('ph-calibration-badge'), calibrationLabel, points ? 'good' : 'warning');
    badge($('calibration-mode'), calibrationLabel, points ? 'good' : 'warning');
    text('ph-sample-time', status.last_reading ? `Last sample ${dateText(status.last_reading)}` : 'No sample yet');
    const freshSeconds = state.settings ? Math.max(15, state.settings.ph.monitoring_interval_seconds * 2) : 120;
    const stale = status.last_reading && Date.now() / 1000 - status.last_reading > freshSeconds;
    text('ph-detail', !status.enabled ? 'History sampling is paused. Live readings and calibration remain available.' : stale ? 'This reading is stale. Check the sensor connection; the chart shows saved history.' : points === 0 ? 'Save a known buffer in Setup to normalize the reading. A second distinct buffer calibrates the slope.' : points === 1 ? 'Single-point normalization is active. Add a second distinct buffer to calibrate the slope. The chart uses your current calibration.' : 'Live readings and the chart use your current calibration. The shaded band shows your target range.');
  }

  function showPh(data) {
    state.phError = null;
    state.ph = data;
    showPhStatus(data.status);
    renderChart();
    if (data.error) text('ph-detail', data.error);
    text('ph-sample-time', data.history.length ? `Last chart sample ${dateText(data.history[data.history.length - 1].timestamp)}` : 'No chart sample yet');
    text('calibration-live-raw', finite(data.raw_value) ? data.raw_value.toFixed(3) : 'Unavailable');
    text('calibration-live-ph', formatNumber(data.status.current_ph));
    const warning = data.calibration_warning || '';
    text('calibration-warning', warning);
    $('calibration-warning').hidden = !warning;
    if ($('calibration-use-live').checked) $('calibration-raw').value = finite(data.raw_value) ? data.raw_value : '';
    for (const point of [4, 7, 10]) {
      const saved = data.calibration[`point_${point}`];
      $(`calibration-point-${point}`).classList.toggle('done', Boolean(saved));
      text(`calibration-point-value-${point}`, saved ? `Raw ${saved.raw_value.toFixed(3)}` : 'Not set');
      text(`calibration-point-time-${point}`, saved ? dateText(saved.timestamp) : '');
      $(`remove-calibration-${point}`).hidden = !saved;
      $(`remove-calibration-${point}`).disabled = state.phBusy;
    }
  }

  function makeCalibrationCards() {
    for (const point of [4, 7, 10]) {
      const card = document.createElement('div');
      card.id = `calibration-point-${point}`;
      card.className = 'calibration-point';
      card.innerHTML = `<strong>pH ${point}</strong><span id="calibration-point-value-${point}">Not set</span><small id="calibration-point-time-${point}"></small><button type="button" id="remove-calibration-${point}" class="text-button" data-remove-calibration="${point}" aria-label="Remove pH ${point} calibration point" hidden>Remove</button>`;
      $('calibration-points').append(card);
    }
  }

  function svgNode(name, attributes, label) {
    const node = document.createElementNS('http://www.w3.org/2000/svg', name);
    Object.entries(attributes).forEach(([key, value]) => node.setAttribute(key, value));
    if (label !== undefined) node.textContent = label;
    return node;
  }

  function renderChart() {
    const svg = $('ph-chart-svg');
    svg.replaceChildren();
    if (state.phError) {
      text('ph-chart-message', state.phError);
      $('ph-chart-message').hidden = false;
      svg.setAttribute('hidden', '');
      state.chartPoints = [];
      return;
    }
    if (!state.ph) return;
    const all = (state.ph.history || []).filter(point => finite(point.timestamp));
    const end = state.ph.server_time || Date.now() / 1000,
      start = end - state.range * 3600;
    const windowPoints = all.filter(point => point.timestamp >= start && point.timestamp <= end);
    const points = windowPoints.filter(point => finite(point.ph_value) && point.ph_value >= 0 && point.ph_value <= 14);
    state.chartPoints = points;
    $('ph-chart-message').hidden = points.length > 0;
    svg.toggleAttribute('hidden', points.length === 0);
    if (!points.length) {
      text('ph-chart-message', windowPoints.length ? 'Saved readings are outside pH 0–14 with this calibration. Recheck the saved buffer points in Setup.' : all.length ? 'No pH readings in this time range. Try a longer range.' : 'No pH history yet. Readings appear after the sensor reports a valid sample.');
      return;
    }
    const width = Math.max(svg.getBoundingClientRect().width, 280),
      height = 230;
    svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
    const left = 31,
      right = width - 12,
      top = 15,
      bottom = height - 28;
    const x = stamp => left + (stamp - start) / (end - start) * (right - left);
    const y = value => bottom - Math.max(0, Math.min(14, value)) / 14 * (bottom - top);
    state.chartLayout = {
      left,
      right,
      start,
      end
    };
    const config = state.settings ? state.settings.ph : state.ph.config;
    svg.append(svgNode('rect', {
      x: left,
      y: y(config.optimal_max),
      width: right - left,
      height: y(config.optimal_min) - y(config.optimal_max),
      fill: '#edf3e2'
    }));
    for (const value of [0, 4, 7, 10, 14]) {
      svg.append(svgNode('line', {
        x1: left,
        y1: y(value),
        x2: right,
        y2: y(value),
        stroke: '#e5eadf',
        'stroke-dasharray': '3 4'
      }));
      svg.append(svgNode('text', {
        x: 19,
        y: y(value) + 4,
        fill: '#84907e',
        'font-size': 10,
        'text-anchor': 'end'
      }, String(value)));
    }
    const divisions = width < 450 ? 2 : 4;
    for (let i = 0; i <= divisions; i++) {
      const stamp = start + (end - start) * i / divisions;
      svg.append(svgNode('text', {
        x: x(stamp),
        y: height - 7,
        fill: '#84907e',
        'font-size': 10,
        'text-anchor': i === 0 ? 'start' : i === divisions ? 'end' : 'middle'
      }, new Date(stamp * 1000).toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit'
      })));
    }
    let previous = null;
    const path = [];
    const gapSeconds = Math.max(120, config.monitoring_interval_seconds * 2);
    for (let index = 0; index < windowPoints.length; index++) {
      const point = windowPoints[index];
      if (!finite(point.ph_value) || point.ph_value < 0 || point.ph_value > 14) { previous = null; continue; }
      const connected = previous && point.timestamp - previous.timestamp <= gapSeconds;
      path.push(`${connected ? 'L' : 'M'}${x(point.timestamp)},${y(point.ph_value)}`);
      const next = windowPoints[index + 1];
      if (!connected && (!next || !finite(next.ph_value) || next.timestamp - point.timestamp > gapSeconds)) {
        svg.append(svgNode('circle', {cx: x(point.timestamp), cy: y(point.ph_value), r: 2.5, fill: '#507943'}));
      }
      previous = point;
    }
    svg.append(svgNode('path', {
      d: path.join(' '),
      fill: 'none',
      stroke: '#507943',
      'stroke-width': 2,
      'stroke-linejoin': 'round',
      'stroke-linecap': 'round'
    }));
    const last = points[points.length - 1];
    svg.append(svgNode('circle', {
      cx: x(last.timestamp),
      cy: y(last.ph_value),
      r: 3.5,
      fill: '#507943',
      stroke: '#fff',
      'stroke-width': 2
    }));
    svg.setAttribute('aria-label', `${points.length} pH readings in the last ${state.range} hours. Latest value ${last.ph_value.toFixed(2)}.`);
  }

  function chartTooltip(event) {
    if (!state.chartPoints.length || !state.chartLayout) return;
    const layout = state.chartLayout,
      position = event.clientX - $('ph-chart-svg').getBoundingClientRect().left;
    const stamp = layout.start + (position - layout.left) / (layout.right - layout.left) * (layout.end - layout.start);
    const point = state.chartPoints.reduce((nearest, value) => Math.abs(value.timestamp - stamp) < Math.abs(nearest.timestamp - stamp) ? value : nearest);
    text('chart-tooltip', `${dateText(point.timestamp)} · pH ${point.ph_value.toFixed(2)}`);
    $('chart-tooltip').hidden = false;
  }

  function historyNumber(value, metric) {
    return finite(value) ? value.toLocaleString([], {maximumFractionDigits: metric.decimals}) : '—';
  }

  function makeHistoryCards() {
    for (const metric of historyMetrics) {
      const card = document.createElement('article');
      card.className = 'panel history-card';
      card.innerHTML = `<div class="panel-heading"><div><span class="eyebrow">${metric.unit}</span><h2>${metric.label} over time</h2></div><span id="history-status-${metric.key}" class="badge neutral">Waiting</span></div><div class="history-latest"><strong id="history-value-${metric.key}">—</strong><span>Last recorded · ${metric.unit}</span></div><div class="chart-wrap"><svg id="history-chart-${metric.key}" class="sensor-chart-svg" viewBox="0 0 500 230" role="img" tabindex="0" aria-label="${metric.label} readings over time"></svg><div id="history-empty-${metric.key}" class="chart-empty">Loading history…</div><output id="history-tooltip-${metric.key}" class="chart-tooltip" hidden></output></div><dl class="history-stats"><div><dt>Minimum</dt><dd id="history-min-${metric.key}">—</dd></div><div><dt>Average</dt><dd id="history-average-${metric.key}">—</dd></div><div><dt>Maximum</dt><dd id="history-max-${metric.key}">—</dd></div></dl><p id="history-detail-${metric.key}" class="helper">Waiting for readings</p>`;
      $('history-cards').append(card);
      const chart = $(`history-chart-${metric.key}`);
      chart.addEventListener('pointermove', event => historyTooltip(metric, event));
      chart.addEventListener('pointerdown', event => historyTooltip(metric, event));
      chart.addEventListener('pointerleave', () => { $(`history-tooltip-${metric.key}`).hidden = true; });
      chart.addEventListener('blur', () => { $(`history-tooltip-${metric.key}`).hidden = true; });
      chart.addEventListener('keydown', event => {
        if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
          event.preventDefault();
          historyTooltip(metric, event);
        }
      });
    }
  }

  function renderHistory() {
    const error = state.historyError || (state.history && state.history.error);
    text('history-error', error || '');
    $('history-error').hidden = !error;
    for (const metric of historyMetrics) {
      const key = metric.key;
      const series = state.history && state.history.series.find(series => series.key === key);
      const svg = $(`history-chart-${key}`);
      svg.replaceChildren();
      state.historyCharts[key] = null;
      $(`history-tooltip-${key}`).hidden = true;
      const points = series ? series.points.filter(point => finite(point.timestamp) && finite(point.value)) : [];
      const latest = series && series.latest;
      text(`history-value-${key}`, historyNumber(latest && latest.value, metric));
      badge($(`history-status-${key}`), state.historyError ? 'Unavailable' : !series ? 'Waiting' : series.available ? 'Recording' : 'No fresh reading', series && series.available && !state.historyError ? 'good' : 'neutral');
      text(`history-min-${key}`, historyNumber(series && series.minimum, metric));
      text(`history-average-${key}`, historyNumber(series && series.average, metric));
      text(`history-max-${key}`, historyNumber(series && series.maximum, metric));
      text(`history-detail-${key}`, series ? `${series.count.toLocaleString()} readings in this range. ${latest ? `Last sample ${dateText(latest.timestamp)}.` : 'No valid sample recorded yet.'}` : 'Waiting for saved readings');
      $(`history-empty-${key}`).hidden = points.length > 0;
      svg.toggleAttribute('hidden', points.length === 0);
      if (!points.length) {
        text(`history-empty-${key}`, state.historyError || (!series ? 'Loading history…' : latest ? 'No readings in this time range. Try a longer range.' : 'No history yet. A fresh sensor reading will appear here after the next sample.'));
        continue;
      }
      const width = Math.max(svg.getBoundingClientRect().width, 220);
      const height = Math.max(svg.getBoundingClientRect().height, 220);
      const left = 48, right = width - 12, top = 15, bottom = height - 28;
      const end = state.history.server_time, start = end - state.history.range_hours * 3600;
      const padding = Math.max((series.maximum - series.minimum) * 0.12, Math.abs(series.maximum) * 0.02, 1);
      const low = metric.key === 'temp' ? series.minimum - padding : Math.max(0, series.minimum - padding);
      const high = metric.key === 'hum' ? Math.min(100, series.maximum + padding) : series.maximum + padding;
      const x = stamp => left + (stamp - start) / (end - start) * (right - left);
      const y = value => bottom - (value - low) / (high - low) * (bottom - top);
      svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
      state.historyCharts[key] = {points, start, end, left, right, cursor: points.length - 1};
      for (let step = 0; step <= 4; step++) {
        const value = low + (high - low) * step / 4;
        svg.append(svgNode('line', {x1: left, x2: right, y1: y(value), y2: y(value), stroke: '#e5eadf', 'stroke-dasharray': '3 4'}));
        svg.append(svgNode('text', {x: left - 8, y: y(value) + 4, fill: '#84907e', 'font-size': 10, 'text-anchor': 'end'}, value.toLocaleString([], {notation: Math.abs(value) >= 10000 ? 'compact' : 'standard', maximumFractionDigits: 1})));
      }
      const divisions = width < 400 ? 2 : 4;
      for (let step = 0; step <= divisions; step++) {
        const stamp = start + (end - start) * step / divisions;
        const label = state.history.range_hours > 24 ? new Date(stamp * 1000).toLocaleDateString([], {month: 'short', day: 'numeric'}) : new Date(stamp * 1000).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'});
        svg.append(svgNode('text', {x: x(stamp), y: height - 7, fill: '#84907e', 'font-size': 10, 'text-anchor': step === 0 ? 'start' : step === divisions ? 'end' : 'middle'}, label));
      }
      let connected = false;
      const path = [];
      for (let index = 0; index < series.points.length; index++) {
        const point = series.points[index];
        if (!finite(point.value) || !finite(point.timestamp)) { connected = false; continue; }
        path.push(`${connected ? 'L' : 'M'}${x(point.timestamp)},${y(point.value)}`);
        if (!connected && (index === series.points.length - 1 || !finite(series.points[index + 1].value))) {
          svg.append(svgNode('circle', {cx: x(point.timestamp), cy: y(point.value), r: 2.5, fill: metric.color}));
        }
        connected = true;
      }
      svg.append(svgNode('path', {d: path.join(' '), fill: 'none', stroke: metric.color, 'stroke-width': 2, 'stroke-linejoin': 'round', 'stroke-linecap': 'round'}));
      const last = points[points.length - 1];
      svg.append(svgNode('circle', {cx: x(last.timestamp), cy: y(last.value), r: 3.5, fill: metric.color, stroke: '#fff', 'stroke-width': 2}));
      svg.setAttribute('aria-label', `${metric.label}: ${series.count} readings in the last ${state.history.range_hours} hours. Minimum ${historyNumber(series.minimum, metric)}, average ${historyNumber(series.average, metric)}, maximum ${historyNumber(series.maximum, metric)} ${metric.unit}. Use arrow keys to inspect samples.`);
    }
  }

  function historyTooltip(metric, event) {
    const chart = state.historyCharts[metric.key];
    if (!chart || !chart.points.length) return;
    if (event.type === 'keydown') {
      if (event.key === 'Home') chart.cursor = 0;
      else if (event.key === 'End') chart.cursor = chart.points.length - 1;
      else chart.cursor = Math.max(0, Math.min(chart.points.length - 1, chart.cursor + (event.key === 'ArrowLeft' ? -1 : 1)));
    } else {
      const position = event.clientX - $(`history-chart-${metric.key}`).getBoundingClientRect().left;
      const stamp = chart.start + (position - chart.left) / (chart.right - chart.left) * (chart.end - chart.start);
      chart.cursor = chart.points.reduce((nearest, point, index) => Math.abs(point.timestamp - stamp) < Math.abs(chart.points[nearest].timestamp - stamp) ? index : nearest, 0);
    }
    const point = chart.points[chart.cursor];
    text(`history-tooltip-${metric.key}`, `${dateText(point.timestamp)} · ${historyNumber(point.value, metric)} ${metric.unit}`);
    $(`history-tooltip-${metric.key}`).hidden = false;
  }

  async function pollHistory(schedule = true) {
    const request = ++state.historyRequest;
    try {
      if (state.passwordChanging || state.loginRedirect) return;
      const history = await api(`/api/history?hours=${state.historyRange}`);
      if (request !== state.historyRequest) return;
      if (!Array.isArray(history.series)) throw new Error('Sensor history response is unavailable. Try Refresh.');
      state.history = history;
      state.historyError = null;
      renderHistory();
    } catch (error) {
      if (request === state.historyRequest) {
        state.historyError = `${error.message} ${state.history ? 'Showing the last loaded history.' : 'Use Refresh to try again.'}`;
        renderHistory();
      }
    } finally {
      if (schedule) setTimeout(() => pollHistory(), 60000);
    }
  }

  function makeOutputCards() {
    for (let channel = 1; channel <= 4; channel++) {
      const card = document.createElement('article');
      card.id = `output-card-${channel}`;
      card.className = 'output-card';
      card.innerHTML = `<div class="output-top"><span class="eyebrow">RELAY ${channel}</span><span class="badge neutral" id="output-state-${channel}">Unknown</span></div><h2 id="output-name-${channel}"></h2><p class="output-role" id="output-role-${channel}"></p><p class="helper" id="output-overflow-${channel}"></p><p class="output-timing" id="output-timing-${channel}">Waiting for the relay board</p><div class="output-actions"><button type="button" class="button primary" data-output="${channel}" disabled>Connecting…</button></div>`;
      $('output-controls').append(card);
    }
  }

  function renderOutputs() {
    if (!state.settings) return;
    state.settings.setup.relay_labels.forEach((label, i) => {
      text(`output-name-${i+1}`, label);
      text(`output-role-${i+1}`, roleNames[state.settings.setup.relay_roles[i]]);
      const role = state.settings.setup.relay_roles[i];
      const overflow = state.settings.setup.overflow;
      text(`output-overflow-${i+1}`, waterRoles.includes(role) ? `Overflow stops: ${sensorNames(overflow[role], overflow.sensor_labels)}` : role === 'lights_air' ? 'Lights and air switch together on this relay.' : 'Choose a function in Setup.');
    });
    if (!state.outputs) return;
    const data = state.outputs;
    const elapsed = Math.max(0, Math.floor((performance.now() - state.outputsReceivedAt) / 1000));
    $('paused-banner').hidden = !data.automation_paused;
    $('pause-schedules').disabled = data.automation_paused || Boolean($('pause-schedules').dataset.busy);
    $('outputs-error').hidden = !(data.error || data.pump_block_reason || data.stop_pending);
    text('outputs-error', data.error || (data.stop_pending ? 'Shutdown is retrying. Check the relay board connection.' : data.pump_block_reason || ''));
    text('water-safety-note', data.pump_block_reason || 'Assigned overflow checks allow pumping. Each pump can run independently with manual Start/Stop.');
    $('water-safety-note').classList.toggle('warning', Boolean(data.pump_block_reason));
    text('last-event', data.last_event);
    $('output-summary').replaceChildren();
    for (const output of data.outputs) {
      const channel = output.channel,
        known = typeof output.on === 'boolean';
      const pending = state.outputPending.get('all') || state.outputPending.get(channel);
      const label = pending || (!known ? 'Unknown' : output.on ? 'On' : 'Off');
      const kind = output.on ? 'good' : 'neutral';
      badge($(`output-state-${channel}`), label, kind);
      $(`output-card-${channel}`).classList.toggle('is-on', output.on === true);
      const role = state.settings.setup.relay_roles[channel - 1];
      let detail;
      if (!known) detail = 'Relay state is unavailable';
      else if (output.on) detail = finite(output.remaining_seconds) ? `Stops in ${duration(Math.max(0, output.remaining_seconds - elapsed))}` : output.schedule_controlled ? 'On for the scheduled lighting period' : 'Running until you press Stop · no timer';
      else detail = 'Manual Start/Stop · no timer or cooldown';
      if (output.blocked_reason) detail = output.blocked_reason;
      text(`output-timing-${channel}`, detail);
      const button = document.querySelector(`[data-output="${channel}"]`);
      const stop = output.on || !known;
      button.dataset.action = stop ? 'stop' : 'start';
      button.textContent = pending || (stop ? 'Stop' : 'Start');
      button.className = `button ${stop ? 'danger-outline' : 'primary'}`;
      button.setAttribute('aria-label', `${stop ? 'Stop' : 'Start'} ${state.settings.setup.relay_labels[channel - 1]}`);
      button.disabled = Boolean(pending) || Boolean(button.dataset.busy) || (!stop && (state.outputPending.size > 0 || data.stop_pending || Boolean(output.blocked_reason) || role === 'unassigned' || output.role !== role));
      const chip = document.createElement('div');
      chip.className = 'output-chip';
      const name = document.createElement('strong');
      name.textContent = state.settings.setup.relay_labels[channel - 1];
      const status = document.createElement('span');
      badge(status, label, kind);
      chip.append(name, status);
      $('output-summary').append(chip);
    }
    renderSchedules();
  }
  async function loadSettings(hydrate = true) {
    if (state.settingsBusy || state.passwordChanging || state.loginRedirect) return;
    const request = ++state.settingsRequest;
    const settings = await api('/api/settings');
    if (request === state.settingsRequest) showSettings(settings, hydrate);
  }
  async function pollSettings() {
    try {
      if (!document.hidden) await loadSettings();
    } catch (error) {
      /* Other live requests report connection failures without discarding edits. */
    } finally {
      setTimeout(pollSettings, 15000);
    }
  }
  function refreshVisiblePage() {
    syncCameras();
    if (!state.csrf || document.hidden) return;
    const page = location.hash.slice(1) || 'overview';
    loadSettings().catch(() => {});
    pollStats(false);
    pollOutputs(false);
    if (page === 'overview' || page === 'settings') pollPh(false);
    if (page === 'history') pollHistory(false);
    if (page === 'cameras') pollCameras(false);
  }
  async function pollStats(schedule = true) {
    const request = ++state.statsRequest;
    try {
      if (state.passwordChanging || state.loginRedirect) return;
      const stats = await api('/api/stats');
      if (request === state.statsRequest) showStats(stats);
    } catch (error) {
      if (request === state.statsRequest) {
        for (const id of ['temperature-value', 'humidity-value', 'co2-value', 'pm25-value', 'pm10-value', 'tvoc-value']) text(id, 'Unavailable');
        for (const index of [1, 2]) {
          badge($(`tank${index}-status`), 'Unavailable', 'warning');
          text(`tank${index}-detail`, 'Cannot confirm the current sensor state');
        }
        badge($('connection'), '● Offline', 'warning');
        text('updated-at', 'Readings unavailable — retrying');
      }
    } finally {
      if (schedule) setTimeout(() => pollStats(), document.hidden ? 15000 : 5000);
    }
  }
  async function pollPh(schedule = true) {
    const request = ++state.phRequest;
    try {
      if (state.phBusy || state.passwordChanging || state.loginRedirect) return;
      const ph = await api('/api/ph');
      if (request === state.phRequest && !state.phBusy) showPh(ph);
    } catch (error) {
      if (request === state.phRequest && !state.phBusy) {
        state.phError = error.message;
        text('ph-value', '—');
        badge($('ph-badge'), 'Unavailable', 'warning');
        text('ph-trend', 'Trend unavailable');
        renderChart();
      }
    } finally {
      if (schedule) setTimeout(() => pollPh(), document.hidden ? 60000 : 5000);
    }
  }
  async function pollOutputs(schedule = true) {
    const request = ++state.outputsRequest;
    try {
      if (state.outputPending.size || state.passwordChanging || state.loginRedirect) return;
      const outputs = await api('/api/outputs');
      if (request !== state.outputsRequest || state.outputPending.size) return;
      state.outputs = outputs;
      state.outputsReceivedAt = performance.now();
      text('outputs-updated', `Live relay state · updated ${new Date().toLocaleTimeString()}`);
      renderOutputs();
    } catch (error) {
      if (request !== state.outputsRequest || state.outputPending.size) return;
      if (state.outputs) {
        state.outputs = {
          ...state.outputs,
          error: error.message,
          outputs: state.outputs.outputs.map(output => ({
            ...output,
            on: null
          }))
        };
        renderOutputs();
      }
      text('outputs-updated', 'Connection lost · relay state is unknown · retrying automatically');
      text('outputs-error', error.message);
      $('outputs-error').hidden = false;
    } finally {
      if (schedule) setTimeout(() => pollOutputs(), document.hidden ? 15000 : location.hash === '#outputs' ? 1000 : 3000);
    }
  }
  function makeCameraCards() {
    for (const [id, label, source] of [['ribbon', 'Raspberry Pi camera', 'Ribbon cable'], ['usb', 'USB webcam', 'USB connection']]) {
      const card = document.createElement('article');
      card.className = 'panel camera-card';
      card.innerHTML = `<div class="panel-heading"><div><span class="eyebrow">${source}</span><h2>${label}</h2></div><span class="badge neutral" id="camera-status-${id}">Connecting</span></div><div class="camera-viewport" id="camera-viewport-${id}"><p class="camera-placeholder" id="camera-placeholder-${id}">Waiting for the device…</p></div><p class="helper camera-detail" id="camera-detail-${id}" role="status">Checking camera availability</p><div class="camera-actions"><button type="button" class="button secondary" id="camera-toggle-${id}">Pause view</button><button type="button" class="button quiet" id="camera-fullscreen-${id}">Full screen</button></div>`;
      $('camera-feeds').append(card);
      state.cameraViews[id] = {image: null, paused: false, retryAfter: 0, connectedAt: 0};
      $(`camera-toggle-${id}`).addEventListener('click', () => {
        const view = state.cameraViews[id];
        view.paused = !view.paused;
        view.retryAfter = 0;
        syncCameras();
        if (!view.paused) pollCameras(false);
      });
      $(`camera-fullscreen-${id}`).addEventListener('click', () => {
        const viewport = $(`camera-viewport-${id}`);
        if (viewport.requestFullscreen) viewport.requestFullscreen().catch(error => flash(error.message));
        else flash('Full screen is unavailable in this browser.');
      });
    }
  }
  function disconnectCamera(view) {
    if (!view.image) return;
    const image = view.image;
    view.image = null;
    image.onload = null;
    image.onerror = null;
    image.removeAttribute('src');
    image.remove();
  }
  function syncCameras() {
    const watching = state.csrf && !document.hidden && location.hash === '#cameras';
    for (const [id, view] of Object.entries(state.cameraViews)) {
      const camera = state.cameras.find(item => item.id === id);
      const available = camera && camera.detected;
      const show = watching && !view.paused && available;
      if (!show || Date.now() - view.connectedAt > 240000) disconnectCamera(view);
      $(`camera-toggle-${id}`).textContent = view.paused ? 'Resume view' : 'Pause view';
      $(`camera-fullscreen-${id}`).disabled = !show;
      const placeholder = $(`camera-placeholder-${id}`);
      placeholder.hidden = Boolean(show && view.image && camera.live);
      let detail = view.paused ? 'This view is paused.' : !camera ? 'Checking camera availability…' : !available ? (camera.error || 'Ribbon camera not detected. Check the camera model, cable, and boot configuration.') : camera.error || (camera.live ? `Live · ${camera.width} × ${camera.height} · up to ${camera.fps} fps · last frame ${new Date(camera.timestamp_ms).toLocaleTimeString()}` : 'Starting the camera. The view reconnects automatically.');
      if (state.cameraError) detail = state.cameraError;
      text(`camera-detail-${id}`, detail);
      const label = view.paused ? 'Paused' : !camera ? 'Connecting' : !available ? 'Not detected' : state.cameraError ? 'Offline' : camera.live && view.image ? 'Live' : camera.error ? 'Reconnecting' : 'Starting';
      badge($(`camera-status-${id}`), label, label === 'Live' ? 'good' : camera && (camera.error || !available) ? 'warning' : 'neutral');
      placeholder.textContent = view.paused ? 'View paused' : !available ? 'Camera unavailable' : camera && camera.error ? 'Reconnecting…' : 'Starting live view…';
      if (show && !view.image && Date.now() >= view.retryAfter) {
        const image = new Image();
        image.alt = camera.label;
        image.className = 'camera-stream';
        view.image = image;
        view.connectedAt = Date.now();
        // MJPEG load events differ between browsers. Device frame timestamps
        // drive the Live indicator; connection-end events trigger reconnection.
        const reconnect = () => {
          if (view.image !== image) return;
          disconnectCamera(view);
          view.retryAfter = Date.now() + 3000;
        };
        image.onerror = reconnect;
        image.onload = () => { /* Status polling renews bounded streams. */ };
        image.src = `${camera.stream_url}?connection=${Date.now()}`;
        $(`camera-viewport-${id}`).prepend(image);
      }
      if (view.image && camera && !camera.live && Date.now() - view.connectedAt > 20000) {
        disconnectCamera(view);
        view.retryAfter = Date.now() + 3000;
      }
    }
  }
  async function pollCameras(schedule = true) {
    const request = ++state.camerasRequest;
    try {
      if (!state.csrf || state.passwordChanging || state.loginRedirect || document.hidden || location.hash !== '#cameras') return;
      const cameras = await api('/api/cameras');
      if (request !== state.camerasRequest) return;
      state.cameras = cameras;
      state.cameraError = null;
      $('cameras-error').hidden = true;
      syncCameras();
    } catch (error) {
      if (request !== state.camerasRequest) return;
      state.cameraError = error.message;
      text('cameras-error', error.message);
      $('cameras-error').hidden = false;
      for (const view of Object.values(state.cameraViews)) disconnectCamera(view);
    } finally {
      if (schedule) setTimeout(() => pollCameras(), document.hidden ? 15000 : 3000);
    }
  }

  function reportRangeChanged() {
    const range = $('report-range').value;
    $('report-custom-range').hidden = range !== 'custom';
    $('report-current').disabled = range === 'current';
    if (range === 'current') $('report-current').checked = true;
    message('report-result', '');
    message('export-result', '');
  }
  function reportParameters() {
    const range = $('report-range').value;
    const params = new URLSearchParams();
    if (range === 'current') params.set('scope', 'current');
    else if (range === 'custom') {
      const start = new Date($('report-start').value).getTime() / 1000;
      const end = new Date($('report-end').value).getTime() / 1000;
      if (!finite(start) || !finite(end) || start >= end) throw new Error('Choose a start and end time, with the end after the start.');
      if (end - start > 7 * 86400) throw new Error('Choose a period of up to 7 days.');
      if (end > Date.now() / 1000 + 60) throw new Error('The end time must be in the past.');
      params.set('start', String(Math.floor(start)));
      params.set('end', String(Math.floor(end)));
    } else params.set('hours', range);
    params.set('current', $('report-current').checked ? '1' : '0');
    params.set('title', $('report-title').value.trim() || 'Air quality report');
    params.set('notes', $('report-notes').value.trim());
    return params;
  }
  async function downloadExport(kind, button) {
    const result = kind === 'html' ? 'report-result' : 'export-result';
    const originalLabel = button.textContent;
    const abort = new AbortController();
    const timeout = setTimeout(() => abort.abort(), 30000);
    button.disabled = true;
    button.textContent = 'Preparing…';
    message(result, 'Preparing your download…');
    try {
      const params = reportParameters();
      const endpoint = kind === 'html' ? 'report' : 'export';
      params.set('format', kind);
      params.set('download', '1');
      const response = await fetch(`/api/air-quality/${endpoint}?${params}`, {credentials: 'same-origin', cache: 'no-store', signal: abort.signal});
      if (response.status === 401) { redirectToLogin(); return; }
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body.error || `Download failed (${response.status})`);
      }
      const blob = await response.blob();
      const link = document.createElement('a');
      const url = URL.createObjectURL(blob);
      link.href = url;
      link.download = (response.headers.get('Content-Disposition') || '').match(/filename="([^"]+)"/)?.[1] || `aog-air-quality.${kind}`;
      document.body.append(link);
      link.click();
      link.remove();
      setTimeout(() => URL.revokeObjectURL(url), 10000);
      message(result, `Your ${kind.toUpperCase()} download is ready.`);
    } catch (error) {
      message(result, error.name === 'AbortError' ? 'The download took too long. Try again.' : error.message, true);
    } finally {
      clearTimeout(timeout);
      button.disabled = false;
      button.textContent = originalLabel;
    }
  }
  function prepareReportControls() {
    const localInputTime = date => new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
    $('report-start').value = localInputTime(new Date(Date.now() - 86400000));
    $('report-end').value = localInputTime(new Date());
    $('report-range').addEventListener('change', reportRangeChanged);
    $('history-to-reports').addEventListener('click', () => {
      $('report-range').value = String(state.historyRange);
      reportRangeChanged();
    });
    $('report-preview').addEventListener('click', () => {
      try {
        const params = reportParameters();
        const link = document.createElement('a');
        link.href = `/api/air-quality/report?${params}`;
        link.target = '_blank';
        link.rel = 'noopener';
        document.body.append(link);
        link.click();
        link.remove();
        message('report-result', 'Report opened in a new tab. Use Print / Save as PDF there to save a PDF copy.');
      } catch (error) { message('report-result', error.message, true); }
    });
    for (const [id, kind] of [['export-csv', 'csv'], ['export-json', 'json'], ['report-html', 'html']]) {
      $(id).addEventListener('click', event => downloadExport(kind, event.currentTarget));
    }
  }

  function form(id, result, handler) {
    $(id).addEventListener('submit', async event => {
      event.preventDefault();
      if ($(id).dataset.saving) return;
      $(id).dataset.saving = 'true';
      const version = $(id).dataset.editVersion;
      const button = $(id).querySelector('[type=submit]');
      const label = button.textContent;
      button.disabled = true;
      button.textContent = id === 'calibration-form' ? 'Applying…' : 'Saving…';
      $(id).setAttribute('aria-busy', 'true');
      message(result, '');
      try {
        if (!state.settings || !state.csrf) throw new Error('Wait for the device to connect, then try again.');
        const reply = await handler();
        if ($(id).dataset.editVersion === version) state.dirtyForms.delete(id);
        showSettings(state.settings, true);
        if (reply) message(result, reply);
      } catch (error) {
        message(result, error.message, true);
      } finally {
        delete $(id).dataset.saving;
        $(id).removeAttribute('aria-busy');
        updateFormAvailability();
        button.textContent = label;
      }
    });
  }
  async function saveSettings(patch) {
    if (state.settingsBusy) throw new Error('Another settings change is being saved. Try again when it finishes.');
    state.settingsBusy = true;
    ++state.settingsRequest;
    try {
      const settings = await api('/api/settings', {body: patch});
      showSettings(settings, false);
    } finally {
      ++state.settingsRequest;
      state.settingsBusy = false;
    }
    await pollOutputs(false);
    await pollPh(false);
    return 'Saved. Changes are active on the device.';
  }
  async function outputAction(path, body = {}) {
    const key = body.channel || 'all';
    if (state.outputPending.has(key)) return;
    const request = ++state.outputActionRequest;
    ++state.outputsRequest;
    state.outputPending.set(key, path.endsWith('/start') ? 'Starting…' : path.endsWith('/resume') ? 'Applying…' : 'Stopping…');
    renderOutputs();
    try {
      const outputs = await api(path, {body});
      ++state.outputsRequest;
      if (request === state.outputActionRequest) {
        state.outputs = outputs;
        state.outputsReceivedAt = performance.now();
        text('outputs-updated', `Device confirmed · ${new Date().toLocaleTimeString()}`);
        renderOutputs();
        flash(outputs.last_event);
      }
      return outputs;
    } finally {
      state.outputPending.delete(key);
      renderOutputs();
      // A fresh read also resolves timeouts where the command reached the Pi
      // but its response did not reach the browser.
      await pollOutputs(false);
    }
  }
  async function calibrationAction(path, body) {
    if (state.phBusy) throw new Error('A calibration change is already being applied.');
    state.phBusy = true;
    ++state.phRequest;
    document.querySelectorAll('[data-remove-calibration]').forEach(button => { button.disabled = true; });
    try {
      const snapshot = await api(path, {body});
      ++state.phRequest;
      showPh(snapshot);
      flash(snapshot.message);
      return snapshot.message;
    } finally {
      state.phBusy = false;
      document.querySelectorAll('[data-remove-calibration]').forEach(button => { button.disabled = false; });
      await pollPh(false);
    }
  }
  async function actionButton(button, action) {
    if (button.dataset.busy) return;
    button.dataset.busy = 'true';
    button.disabled = true;
    try {
      await action();
    } catch (error) {
      globalError(error.message);
    } finally {
      delete button.dataset.busy;
      button.disabled = false;
      renderOutputs();
    }
  }
  makeCameraCards();
  prepareReportControls();
  makeOutputCards();
  makeHistoryCards();
  makeCalibrationCards();
  navigate();
  document.querySelectorAll('form').forEach(element => {
    const edited = () => {
      state.dirtyForms.add(element.id);
      element.dataset.editVersion = String(Number(element.dataset.editVersion || 0) + 1);
    };
    element.addEventListener('input', edited);
    element.addEventListener('change', edited);
  });
  window.addEventListener('hashchange', navigate);
  window.addEventListener('resize', renderChart);
  window.addEventListener('resize', renderHistory);
  document.addEventListener('visibilitychange', refreshVisiblePage);
  window.addEventListener('focus', refreshVisiblePage);
  window.addEventListener('pagehide', () => {
    for (const view of Object.values(state.cameraViews)) disconnectCamera(view);
  });
  setInterval(() => {
    if (!document.hidden && state.outputs && (location.hash === '#outputs' || location.hash === '#schedules')) renderOutputs();
  }, 1000);
  document.querySelectorAll('[data-history-range]').forEach(button => button.addEventListener('click', () => {
    state.historyRange = Number(button.dataset.historyRange);
    state.history = null;
    state.historyError = null;
    document.querySelectorAll('[data-history-range]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
    renderHistory();
    pollHistory(false);
  }));
  $('ph-chart-svg').addEventListener('pointermove', chartTooltip);
  $('ph-chart-svg').addEventListener('pointerleave', () => {
    $('chart-tooltip').hidden = true;
  });
  document.querySelectorAll('[data-range]').forEach(button => button.addEventListener('click', () => {
    state.range = Number(button.dataset.range);
    document.querySelectorAll('[data-range]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
    renderChart();
  }));
  $('schedule-form').addEventListener('input', previewSchedules);
  $('overflow-form').addEventListener('input', previewOverflow);
  for (const channel of [1, 2, 3, 4]) {
    $(`relay-role-${channel}`).addEventListener('change', event => {
      const label = $(`relay-label-${channel}`);
      const previous = event.target.dataset.previousRole;
      if ([roleNames[previous], 'Lights', 'Air', 'Lights & air', 'Fill pump', 'Drain pump', 'Tank-to-tank pump', 'Auxiliary pump', `Relay ${channel}`].includes(label.value)) {
        label.value = event.target.value === 'unassigned' ? `Relay ${channel}` : roleNames[event.target.value];
      }
      event.target.dataset.previousRole = event.target.value;
      previewAssignments();
    });
  }
  $('safety-enabled').addEventListener('change', () => {
    $('safety-pin-field').hidden = !$('safety-enabled').checked;
  });
  $('show-passwords').addEventListener('change', () => {
    ['current-password', 'new-password', 'confirm-password'].forEach(id => {
      $(id).type = $('show-passwords').checked ? 'text' : 'password';
    });
  });
  $('stop-all').addEventListener('click', event => actionButton(event.currentTarget, () => outputAction('/api/outputs/stop-all')));
  $('pause-schedules').addEventListener('click', event => actionButton(event.currentTarget, () => outputAction('/api/schedules/pause')));
  document.querySelectorAll('.resume-schedules').forEach(button => button.addEventListener('click', event => actionButton(event.currentTarget, () => outputAction('/api/schedules/resume'))));
  document.querySelectorAll('[data-output]').forEach(button => button.addEventListener('click', () => {
    const channel = Number(button.dataset.output),
      output = state.outputs.outputs.find(item => item.channel === channel);
    globalError('');
    if (output.on || output.on === null) {
      return actionButton(button, () => outputAction('/api/outputs/stop', {channel}));
    }
    return actionButton(button, () => outputAction('/api/outputs/start', {channel}));
  }));
  form('schedule-form', 'schedule-result', () => saveSettings({
    schedules: {
      lighting: {
        enabled: $('lighting-enabled').checked,
        start_minute: minutes($('lighting-start').value),
        end_minute: minutes($('lighting-end').value)
      },
      watering: {
        pump: $('watering-pump').value,
        enabled: $('watering-enabled').checked,
        start_minute: minutes($('watering-start').value),
        end_minute: minutes($('watering-end').value),
        every_minutes: Number($('watering-interval').value),
        run_seconds: Number($('watering-duration').value)
      }
    }
  }));
  form('setup-form', 'setup-result', async () => {
    const roles = [1, 2, 3, 4].map(index => $(`relay-role-${index}`).value);
    const assigned = roles.filter(role => role !== 'unassigned');
    if (new Set(assigned).size !== assigned.length) throw new Error('Assign each function to only one relay. Unused relays can remain unassigned.');
    const changed = roles.some((role, index) => role !== state.settings.setup.relay_roles[index]);
    const reply = await saveSettings({
      setup: {
        setup_name: $('setup-name-input').value.trim(),
        relay_labels: [1, 2, 3, 4].map(index => $(`relay-label-${index}`).value.trim()),
        relay_roles: roles
      }
    });
    await pollOutputs(false);
    return changed ? 'Assignments saved. Schedules are paused; resume them when you are ready.' : reply;
  });
  form('pump-form', 'pump-result', () => saveSettings({
    pump: {
      ...state.settings.pump,
      safety_gpio_pin: $('safety-enabled').checked ? Number($('safety-pin').value) : null
    }
  }));
  form('overflow-form', 'overflow-result', async () => {
    const overflow = {
      sensor_labels: [1, 2].map(index => $(`overflow-label-${index}`).value.trim()),
      ...Object.fromEntries(waterRoles.map(role => [role, sensorSelection($(`overflow-${role}`).value)]))
    };
    await saveSettings({setup: {...state.settings.setup, overflow}});
    await pollOutputs(false);
    return 'Overflow assignments saved. Schedules are paused; resume when ready.';
  });
  form('ph-settings-form', 'ph-settings-result', async () => {
    const reply = await saveSettings({
      ph: {
        ...state.settings.ph,
        enabled: $('ph-enabled').checked,
        alert_enabled: $('ph-alerts').checked,
        optimal_min: Number($('ph-min').value),
        optimal_max: Number($('ph-max').value),
        critical_min: Number($('ph-critical-min').value),
        critical_max: Number($('ph-critical-max').value),
        monitoring_interval_seconds: Number($('ph-interval').value),
        history_size: Number($('ph-history-size').value)
      }
    });
    await pollPh(false);
    return reply;
  });
  $('use-raw-reading').addEventListener('click', event => actionButton(event.currentTarget, async () => {
    if (state.phBusy) return;
    const request = ++state.phRequest;
    const data = await api('/api/ph');
    if (state.phBusy || request !== state.phRequest) return;
    showPh(data);
    if (!finite(data.raw_value)) throw new Error('A fresh raw reading is unavailable. Check the probe connection.');
    $('calibration-raw').value = data.raw_value;
  }));
  $('calibration-use-live').addEventListener('change', () => {
    $('calibration-raw').readOnly = $('calibration-use-live').checked;
    $('calibration-ready').checked = false;
    if ($('calibration-use-live').checked) pollPh(false);
  });
  $('buffer-ph').addEventListener('change', () => { $('calibration-ready').checked = false; });
  document.querySelectorAll('[data-remove-calibration]').forEach(button => button.addEventListener('click', event => actionButton(event.currentTarget, async () => {
    const reply = await calibrationAction('/api/ph/calibration/remove', {buffer_ph: Number(button.dataset.removeCalibration)});
    $('calibration-ready').checked = false;
    message('calibration-result', reply);
  })));
  form('calibration-form', 'calibration-result', async () => {
    const body = {buffer_ph: Number($('buffer-ph').value)};
    if (!$('calibration-use-live').checked) body.raw_value = Number($('calibration-raw').value);
    const reply = await calibrationAction('/api/ph/calibrate', body);
    $('calibration-ready').checked = false;
    return reply;
  });
  form('password-form', 'password-result', async () => {
    if ($('new-password').value !== $('confirm-password').value) throw new Error('New passwords do not match.');
    const password = $('new-password').value;
    const length = Array.from(password).length;
    if (length < 12 || length > 128) throw new Error('Use 12–128 characters for your new password.');
    // The server applies one Unicode-aware strength policy to web and console
    // changes. Do not reject valid non-ASCII letters with an ASCII-only regex.
    state.passwordChanging = true;
    state.sessionExpired = false;
    try {
      await api('/api/account/password', {
        body: {
          current_password: $('current-password').value,
          new_password: password,
          confirm_password: $('confirm-password').value
        }
      });
      $('password-form').reset();
      redirectToLogin('changed');
      return 'Password changed. Sign in again.';
    } finally {
      state.passwordChanging = false;
      if (state.sessionExpired && !state.loginRedirect) redirectToLogin();
    }
  });
  $('refresh-button').addEventListener('click', event => actionButton(event.currentTarget, async () => {
    globalError('');
    if (!state.csrf) {
      state.csrf = (await api('/api/session')).csrf_token;
    }
    await loadSettings(!state.settings);
    updateFormAvailability();
    await Promise.all([pollStats(false), pollPh(false), pollHistory(false), pollOutputs(false), pollCameras(false)]);
  }));
  async function boot() {
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = true;
    });
    try {
      const session = await api('/api/session');
      state.csrf = session.csrf_token;
      await loadSettings();
      updateFormAvailability();
    } catch (error) {
      globalError(`Could not load setup: ${error.message} Use Refresh to try again.`);
    }
    pollStats();
    pollPh();
    pollHistory();
    pollOutputs();
    pollCameras();
    setTimeout(pollSettings, 15000);
  }
  boot();
})();
