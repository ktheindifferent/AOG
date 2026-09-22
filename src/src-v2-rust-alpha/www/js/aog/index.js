'use strict';
(() => {
  const $ = id => document.getElementById(id);
  const state = {
    csrf: null,
    settings: null,
    ph: null,
    phError: null,
    outputs: null,
    range: 24,
    chartPoints: [],
    selectedOutput: null,
    refreshing: false
  };
  const pageCopy = {
    overview: ['Grow overview', 'Your environment, water, and equipment in one place.'],
    schedules: ['Lighting & watering', 'Build a daily rhythm for your grow.'],
    outputs: ['Equipment controls', 'See actual relay states and run equipment for a set time.'],
    settings: ['Setup settings', 'Tune your environment and make this setup your own.'],
    account: ['Your account', 'Manage your password and device access.']
  };
  const text = (id, value) => {
    $(id).textContent = value;
  };
  const finite = value => typeof value === 'number' && Number.isFinite(value);
  const roleNames = {
    unassigned: 'Unassigned',
    lights: 'Lights',
    air: 'Air',
    filling: 'Filling',
    draining: 'Draining'
  };
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
      const body = await response.json().catch(() => ({}));
      if (response.status === 401) {
        window.location.replace('/login.html?expired=1');
        throw new Error('Your session expired. Sign in again.');
      }
      if (!response.ok) throw new Error(body.error || `Request failed (${response.status})`);
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
    if (hydrate) {
      $('setup-name-input').value = setup.setup_name;
      setup.relay_labels.forEach((label, i) => {
        $(`relay-label-${i + 1}`).value = label;
        $(`relay-role-${i + 1}`).value = setup.relay_roles[i];
        $(`relay-role-${i + 1}`).dataset.previousRole = setup.relay_roles[i];
      });
      $('lighting-enabled').checked = schedules.lighting.enabled;
      $('lighting-start').value = timeText(schedules.lighting.start_minute);
      $('lighting-end').value = timeText(schedules.lighting.end_minute);
      $('watering-enabled').checked = schedules.watering.enabled;
      $('watering-start').value = timeText(schedules.watering.start_minute);
      $('watering-end').value = timeText(schedules.watering.end_minute);
      $('watering-interval').value = schedules.watering.every_minutes;
      $('watering-duration').value = schedules.watering.run_seconds;
      $('pump-runtime').value = pump.pump_runtime_limit_seconds;
      $('pump-cooldown').value = pump.pump_cooldown_seconds;
      $('safety-enabled').checked = pump.safety_gpio_pin !== null;
      $('safety-pin').value = pump.safety_gpio_pin === null ? 17 : pump.safety_gpio_pin;
      $('safety-pin-field').hidden = !$('safety-enabled').checked;
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
    text('pump-limit-summary', `Configured maximum: ${pump.pump_runtime_limit_seconds} seconds. Cooldown: ${pump.pump_cooldown_seconds} seconds.${pump.photo_cycle_enabled ? ` Additional pump operating window: ${timeText(pump.photo_cycle_start_hour * 60)}–${timeText(pump.photo_cycle_end_hour * 60)}.` : ''}`);
    text('ph-target', `Target ${ph.optimal_min.toFixed(1)}–${ph.optimal_max.toFixed(1)}`);
    renderSchedules();
    previewSchedules();
    previewAssignments();
    renderOutputs();
    renderChart();
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
        ['lighting-relay', 'lights'],
        ['watering-relay', 'filling']
      ]) {
      const channel = relayFor(role);
      text(id, `${roleNames[role]} · ${channel ? `Relay ${channel}` : 'No relay assigned'}`);
    }
    text('lighting-summary', lighting.enabled ? `${timeText(lighting.start_minute)}–${timeText(lighting.end_minute)}${lighting.start_minute > lighting.end_minute ? ' · overnight' : ''}` : 'No automatic lighting period');
    text('watering-summary', watering.enabled ? `${watering.run_seconds}s every ${watering.every_minutes} min · ${timeText(watering.start_minute)}–${timeText(watering.end_minute)}` : 'No automatic watering cycle');
    badge($('lighting-state'), lighting.enabled ? paused ? 'Paused' : 'Scheduled' : 'Disabled', lighting.enabled && !paused ? 'good' : 'neutral');
    badge($('watering-state'), watering.enabled ? paused ? 'Paused' : 'Scheduled' : 'Disabled', watering.enabled && !paused ? 'good' : 'neutral');
    const wait = state.outputs && state.outputs.next_watering_in_seconds;
    text('next-watering', paused ? 'Schedules are paused. Resume when you are ready.' : watering.enabled && finite(wait) ? `Next watering check in ${duration(wait)}. Window and safety conditions must allow a run.` : 'Schedules run on the device when your browser is closed.');
  }

  function previewSchedules() {
    $('watering-duration').max = $('watering-enabled').checked && state.settings ? Math.min(300, state.settings.pump.pump_runtime_limit_seconds) : 300;
    const start = minutes($('lighting-start').value),
      end = minutes($('lighting-end').value);
    const enabled = $('lighting-enabled').checked;
    const from = start / 1440 * 100,
      to = end / 1440 * 100;
    $('lighting-preview').style.background = !enabled ? '#edf1e7' : start < end ? `linear-gradient(to right,#edf1e7 ${from}%,#a9c77c ${from}%,#a9c77c ${to}%,#edf1e7 ${to}%)` : `linear-gradient(to right,#a9c77c ${to}%,#edf1e7 ${to}%,#edf1e7 ${from}%,#a9c77c ${from}%)`;
    const hours = ((end - start + 1440) % 1440) / 60;
    text('lighting-preview-text', enabled ? `${hours.toFixed(1)} hours of light per day${start > end ? ', crossing midnight' : ''}.` : 'Lighting schedule is disabled. Manual timed runs remain available.');
    text('watering-preview-text', $('watering-enabled').checked ? `Run for ${$('watering-duration').value || '—'} seconds every ${$('watering-interval').value || '—'} minutes. The first run waits one interval; blocked runs retry once conditions allow.` : 'Watering schedule is disabled. Configure the interval and enable it when ready.');
  }

  function previewAssignments() {
    const roles = [1, 2, 3, 4].map(channel => $(`relay-role-${channel}`).value);
    text('relay-assignment-summary', ['lights', 'air', 'filling', 'draining'].map(role => {
      const channels = roles.map((assigned, index) => assigned === role ? index + 1 : null).filter(Boolean);
      return `${roleNames[role]} → ${channels.length ? `Relay ${channels.join(', ')}` : 'Unassigned'}`;
    }).join(' · '));
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
    if (data.ph_status) showPhStatus(data.ph_status);
    showClock(data.device_time);
    badge($('connection'), '● Connected', 'good');
    text('updated-at', `Updated ${new Date().toLocaleTimeString([], {hour:'2-digit', minute:'2-digit', second:'2-digit'})}`);
  }

  function showPhStatus(status) {
    text('ph-value', formatNumber(status.current_ph));
    const hasReading = finite(status.current_ph);
    badge($('ph-badge'), !status.enabled ? 'Paused' : !hasReading ? 'No reading' : status.alert_level, status.enabled && hasReading ? status.alert_level === 'Normal' ? 'good' : status.alert_level === 'Warning' ? 'warning' : 'bad' : 'neutral');
    text('ph-trend', !status.enabled ? 'Monitoring paused' : !hasReading ? 'Waiting for readings' : status.reading_count < 2 ? 'Collecting trend data' : `${status.trend === 'Rising' ? '↗' : status.trend === 'Falling' ? '↘' : '→'} ${status.trend} · last hour`);
    badge($('ph-calibration-badge'), status.calibration_valid ? 'Calibrated' : 'Not calibrated', status.calibration_valid ? 'good' : 'warning');
    text('ph-sample-time', status.last_reading ? `Last sample ${dateText(status.last_reading)}` : 'No sample yet');
    const freshSeconds = state.settings ? Math.max(15, state.settings.ph.monitoring_interval_seconds * 2) : 120;
    const stale = status.last_reading && Date.now() / 1000 - status.last_reading > freshSeconds;
    text('ph-detail', !status.enabled ? 'Monitoring is paused. Enable it in Setup settings to collect new samples.' : stale ? 'This reading is stale. Check the sensor connection; the chart shows saved history.' : !status.calibration_valid ? 'The probe has not completed a two-point calibration. Calibrate it in Setup settings before relying on its pH values.' : 'The shaded band shows your configured target range.');
  }

  function showPh(data) {
    state.phError = null;
    state.ph = data;
    showPhStatus(data.status);
    renderChart();
    if (data.error) text('ph-detail', data.error);
    $('calibration-points').replaceChildren();
    for (const point of [4, 7, 10]) {
      const saved = data.calibration[`point_${point}`];
      const element = document.createElement('div');
      element.className = `calibration-point${saved ? ' done' : ''}`;
      const title = document.createElement('strong');
      title.textContent = `pH ${point}`;
      const detail = document.createElement('span');
      detail.textContent = saved ? `Raw ${saved.raw_value}` : 'Not set';
      element.append(title, detail);
      $('calibration-points').append(element);
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
    const all = (state.ph.history || []).filter(point => finite(point.ph_value) && finite(point.timestamp));
    const end = state.ph.server_time || Date.now() / 1000,
      start = end - state.range * 3600;
    const points = all.filter(point => point.timestamp >= start && point.timestamp <= end);
    state.chartPoints = points;
    $('ph-chart-message').hidden = points.length > 0;
    svg.toggleAttribute('hidden', points.length === 0);
    if (!points.length) {
      text('ph-chart-message', all.length ? 'No pH readings in this time range. Try a longer range.' : 'No pH history yet. Readings appear after the sensor reports a valid sample.');
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
    svg.append(svgNode('polyline', {
      points: points.map(point => `${x(point.timestamp)},${y(point.ph_value)}`).join(' '),
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

  function makeOutputCards() {
    for (let channel = 1; channel <= 4; channel++) {
      const card = document.createElement('article');
      card.id = `output-card-${channel}`;
      card.className = 'output-card';
      card.innerHTML = `<div class="output-top"><span class="eyebrow">RELAY ${channel}</span><span class="badge neutral" id="output-state-${channel}">Unknown</span></div><h2 id="output-name-${channel}"></h2><p class="output-role" id="output-role-${channel}"></p><p class="output-timing" id="output-timing-${channel}">Waiting for the relay board</p><div class="output-actions"><button type="button" class="button primary" data-start="${channel}" disabled>Start timed run</button><button type="button" class="button secondary" data-stop="${channel}">Stop</button></div>`;
      $('output-controls').append(card);
    }
  }

  function renderOutputs() {
    if (!state.settings) return;
    state.settings.setup.relay_labels.forEach((label, i) => {
      text(`output-name-${i+1}`, label);
      text(`output-role-${i+1}`, roleNames[state.settings.setup.relay_roles[i]]);
    });
    if (!state.outputs) return;
    const data = state.outputs;
    $('paused-banner').hidden = !data.automation_paused;
    $('pause-schedules').disabled = data.automation_paused;
    $('outputs-error').hidden = !(data.error || data.pump_block_reason || data.stop_pending);
    text('outputs-error', data.error || (data.stop_pending ? 'Shutdown is retrying. Check the relay board connection.' : data.pump_block_reason || ''));
    text('water-safety-note', data.pump_block_reason || 'Tank readings are clear. Pump starts still respect runtime and cooldown limits.');
    $('water-safety-note').classList.toggle('warning', Boolean(data.pump_block_reason));
    text('last-event', data.last_event);
    $('output-summary').replaceChildren();
    for (const output of data.outputs) {
      const channel = output.channel,
        known = typeof output.on === 'boolean';
      const label = !known ? 'Unknown' : output.on ? 'On' : 'Off';
      const kind = output.on ? 'good' : 'neutral';
      badge($(`output-state-${channel}`), label, kind);
      $(`output-card-${channel}`).classList.toggle('is-on', output.on === true);
      let detail = !known ? 'Relay state is unavailable' : output.on ? finite(output.remaining_seconds) ? `Stops in ${duration(output.remaining_seconds)}` : 'On outside a dashboard timed run' : output.cooldown_seconds > 0 ? `Cooldown: ${duration(output.cooldown_seconds)} remaining` : `Maximum timed run: ${duration(output.maximum_seconds)}`;
      const role = state.settings.setup.relay_roles[channel - 1];
      const controlled = output.schedule_controlled;
      if (controlled) detail = output.on ? 'On for the scheduled lighting period' : 'Waiting for the lighting period';
      if (output.blocked_reason) detail = output.blocked_reason;
      text(`output-timing-${channel}`, detail);
      document.querySelector(`[data-start="${channel}"]`).disabled = !known || output.on || data.stop_pending || controlled || Boolean(output.blocked_reason) || output.cooldown_seconds > 0 || role === 'unassigned' || output.role !== role;
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
    showSettings(await api('/api/settings'), hydrate);
  }
  async function pollStats(schedule = true) {
    try {
      showStats(await api('/api/stats'));
    } catch (error) {
      badge($('connection'), '● Offline', 'warning');
      text('updated-at', 'Readings unavailable — retrying');
    } finally {
      if (schedule) setTimeout(() => pollStats(), document.hidden ? 15000 : 5000);
    }
  }
  async function pollPh(schedule = true) {
    try {
      showPh(await api('/api/ph'));
    } catch (error) {
      state.phError = error.message;
      text('ph-trend', 'Trend unavailable');
      renderChart();
    } finally {
      if (schedule) setTimeout(() => pollPh(), document.hidden ? 60000 : 15000);
    }
  }
  async function pollOutputs(schedule = true) {
    try {
      state.outputs = await api('/api/outputs');
      renderOutputs();
    } catch (error) {
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
      document.querySelectorAll('[data-start]').forEach(button => {
        button.disabled = true;
      });
      text('outputs-error', error.message);
      $('outputs-error').hidden = false;
    } finally {
      if (schedule) setTimeout(() => pollOutputs(), document.hidden ? 15000 : 5000);
    }
  }
  async function pollCameras() {
    try {
      const cameras = await api('/api/cameras');
      $('cameras-panel').hidden = cameras.length === 0;
      for (const existing of Array.from($('camera-feeds').children)) {
        if (!cameras.some(camera => existing.dataset.camera === String(camera.id))) existing.remove();
      }
      for (const camera of cameras) {
        let figure = document.querySelector(`[data-camera="${camera.id}"]`);
        if (!figure) {
          figure = document.createElement('figure');
          figure.dataset.camera = String(camera.id);
          const image = document.createElement('img');
          image.alt = `Camera ${camera.id + 1}`;
          const caption = document.createElement('figcaption');
          figure.append(image, caption);
          $('camera-feeds').append(figure);
        }
        figure.querySelector('img').src = `${camera.url}?frame=${camera.timestamp}`;
        figure.querySelector('figcaption').textContent = `Camera ${camera.id + 1} · ${dateText(camera.timestamp)}`;
      }
    } catch (_) {
      /* Camera availability does not interrupt equipment controls. */
    } finally {
      setTimeout(pollCameras, document.hidden ? 30000 : 5000);
    }
  }

  function form(id, result, handler) {
    $(id).addEventListener('submit', async event => {
      event.preventDefault();
      const button = $(id).querySelector('[type=submit]');
      const label = button.textContent;
      button.disabled = true;
      button.textContent = 'Saving…';
      message(result, '');
      try {
        if (!state.settings || !state.csrf) throw new Error('Wait for the device to connect, then try again.');
        const reply = await handler();
        if (reply) message(result, reply);
      } catch (error) {
        message(result, error.message, true);
      } finally {
        button.disabled = false;
        button.textContent = label;
      }
    });
  }
  async function saveSettings(patch) {
    const settings = await api('/api/settings', {
      body: patch
    });
    showSettings(settings, false);
    return 'Saved. Changes are active on the device.';
  }
  async function outputAction(path, body = {}) {
    state.outputs = await api(path, {
      body
    });
    renderOutputs();
    flash(state.outputs.last_event);
    return state.outputs;
  }
  async function actionButton(button, action) {
    button.disabled = true;
    try {
      await action();
    } catch (error) {
      globalError(error.message);
    } finally {
      button.disabled = false;
    }
  }
  makeOutputCards();
  navigate();
  window.addEventListener('hashchange', navigate);
  window.addEventListener('resize', renderChart);
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
  for (const channel of [1, 2, 3, 4]) {
    $(`relay-role-${channel}`).addEventListener('change', event => {
      const label = $(`relay-label-${channel}`);
      const previous = event.target.dataset.previousRole;
      if ([roleNames[previous], 'Lights & air', 'Fill pump', 'Drain pump', 'Auxiliary pump', `Relay ${channel}`].includes(label.value)) {
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
  document.querySelectorAll('[data-stop]').forEach(button => button.addEventListener('click', event => actionButton(event.currentTarget, () => outputAction('/api/outputs/stop', {
    channel: Number(button.dataset.stop)
  }))));
  document.querySelectorAll('[data-start]').forEach(button => button.addEventListener('click', () => {
    const channel = Number(button.dataset.start),
      output = state.outputs.outputs.find(item => item.channel === channel);
    state.selectedOutput = channel;
    text('run-dialog-title', `Start ${state.settings.setup.relay_labels[channel-1]}?`);
    text('run-dialog-description', `This switches physical relay ${channel} on for ${roleNames[output.role]}. Maximum duration: ${duration(output.maximum_seconds)}.`);
    $('run-seconds').max = output.maximum_seconds;
    $('run-seconds').value = Math.min(['lights', 'air'].includes(output.role) ? 600 : 30, output.maximum_seconds);
    message('run-result', '');
    $('run-dialog').showModal();
  }));
  $('cancel-run').addEventListener('click', () => {
    $('run-dialog').close();
  });
  form('run-form', 'run-result', async () => {
    await outputAction('/api/outputs/start', {
      channel: state.selectedOutput,
      seconds: Number($('run-seconds').value)
    });
    $('run-dialog').close();
    return '';
  });
  form('schedule-form', 'schedule-result', () => saveSettings({
    schedules: {
      lighting: {
        enabled: $('lighting-enabled').checked,
        start_minute: minutes($('lighting-start').value),
        end_minute: minutes($('lighting-end').value)
      },
      watering: {
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
      continuous_mode: false,
      pump_runtime_limit_seconds: Number($('pump-runtime').value),
      pump_cooldown_seconds: Number($('pump-cooldown').value),
      safety_gpio_pin: $('safety-enabled').checked ? Number($('safety-pin').value) : null
    }
  }));
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
    const data = await api('/api/ph');
    showPh(data);
    if (!finite(data.raw_value)) throw new Error('A fresh raw reading is unavailable. Check the probe connection.');
    $('calibration-raw').value = data.raw_value;
  }));
  form('calibration-form', 'calibration-result', async () => {
    const reply = await api('/api/ph/calibrate', {
      body: {
        buffer_ph: Number($('buffer-ph').value),
        raw_value: Number($('calibration-raw').value)
      }
    });
    $('calibration-ready').checked = false;
    await pollPh(false);
    return reply.message;
  });
  form('password-form', 'password-result', async () => {
    if ($('new-password').value !== $('confirm-password').value) throw new Error('New passwords do not match.');
    const password = $('new-password').value;
    if (!/[A-Z]/.test(password) || !/[a-z]/.test(password) || !/[0-9]/.test(password) || !(/[^a-zA-Z0-9]/.test(password))) throw new Error('Include uppercase, lowercase, a number, and a symbol.');
    await api('/api/account/password', {
      body: {
        current_password: $('current-password').value,
        new_password: password,
        confirm_password: $('confirm-password').value
      }
    });
    $('password-form').reset();
    window.location.assign('/login.html?changed=1');
    return 'Password changed. Sign in again.';
  });
  $('refresh-button').addEventListener('click', event => actionButton(event.currentTarget, async () => {
    globalError('');
    if (!state.csrf) {
      state.csrf = (await api('/api/session')).csrf_token;
    }
    await loadSettings(!state.settings);
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = false;
    });
    await Promise.all([pollStats(false), pollPh(false), pollOutputs(false)]);
  }));
  async function boot() {
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = true;
    });
    try {
      const session = await api('/api/session');
      state.csrf = session.csrf_token;
      await loadSettings();
      document.querySelectorAll('form [type=submit]').forEach(button => {
        button.disabled = false;
      });
    } catch (error) {
      globalError(`Could not load setup: ${error.message} Use Refresh to try again.`);
    }
    pollStats();
    pollPh();
    pollOutputs();
    pollCameras();
  }
  boot();
})();
