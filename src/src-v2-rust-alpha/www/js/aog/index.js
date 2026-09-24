'use strict';
(() => {
  const $ = id => document.getElementById(id);
  const state = {
    csrf: null,
    passwordChanging: false,
    sessionExpired: false,
    loginRedirect: null,
    cameras: [],
    camerasPending: null,
    cameraViews: {},
    cameraError: null,
    reportBusy: false,
    reportIncludeCurrent: true,
    settings: null,
    ph: null,
    phError: null,
    phRequest: 0,
    phBusy: false,
    calibrationRefreshing: false,
    calibrationPending: false,
    settingsRequest: 0,
    settingsBusy: false,
    statsRequest: 0,
    stats: null,
    statsConnected: null,
    dirtyForms: new Set(),
    formBaselines: new Map(),
    formConflicts: new Map(),
    activePage: null,
    outputsRequest: 0,
    outputActionRequest: 0,
    outputPending: new Map(),
    outputStopSequences: new Map(),
    outputActionNotice: '',
    outputsReceivedAt: 0,
    history: null,
    historyError: null,
    historyLoading: false,
    historyLoadedAt: null,
    historyRange: 24,
    historyRequest: 0,
    historyCharts: {},
    outputs: null,
    range: 24,
    chartPoints: [],
    chartCursor: 0,
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
  const settingsSections = {
    relays: 'setup-form',
    overflow: 'overflow-form',
    safety: 'pump-form',
    ph: 'ph-settings-form',
    calibration: 'calibration-form'
  };
  const text = (id, value) => {
    const element = $(id);
    if (element.textContent !== String(value)) element.textContent = value;
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
    {key: 'co2', label: 'CO₂', unit: 'ppm', decimals: 0, color: '#5b7845'},
    {key: 'tvoc', label: 'TVOC', unit: 'ppb', decimals: 0, color: '#83719a'},
    {key: 'pm25', label: 'PM2.5', unit: 'µg/m³', decimals: 1, color: '#a27a36'},
    {key: 'pm10', label: 'PM10', unit: 'µg/m³', decimals: 1, color: '#92704e'},
    {key: 'temp', label: 'Temperature', unit: '°C', decimals: 1, color: '#a56e48'},
    {key: 'hum', label: 'Humidity', unit: '%', decimals: 1, color: '#527f86'}
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
    // Polling should not repeatedly announce an unchanged connection status.
    if (element.textContent !== label) element.textContent = label;
    const className = `badge ${kind}`;
    if (element.className !== className) element.className = className;
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

  function globalError(value, source = '') {
    text('global-message', value);
    $('global-message').dataset.source = value ? source : '';
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
    const query = new URLSearchParams({[reason]: '1'});
    if (reason === 'expired') {
      const {page, section} = currentRoute();
      const view = section ? `${page}/${section}` : page;
      if (view !== 'overview') query.set('return', view);
      if (pendingSettingForms().length) query.set('unsaved', '1');
    }
    window.location.replace(`/login.html?${query}`);
  }

  function updateFormAvailability() {
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = !state.settings || !state.csrf || Boolean(state.loginRedirect)
        || Boolean(button.closest('form').dataset.saving)
        || Boolean(state.formConflicts.get(button.closest('form').id)?.size);
    });
    updateCalibrationWorkflow();
  }

  function updateInitialSettingsAvailability(failed = false) {
    const ready = Boolean(state.settings);
    for (const id of ['setup-form', 'overflow-form', 'schedule-form', 'pump-form', 'ph-settings-form']) {
      $(id).querySelectorAll('input, select, textarea').forEach(field => {
        if (!ready && !field.disabled) {
          field.dataset.awaitingSettings = 'true';
          field.disabled = true;
        } else if (ready && field.dataset.awaitingSettings) {
          delete field.dataset.awaitingSettings;
          field.disabled = false;
        }
      });
    }
    document.querySelectorAll('[data-settings-loading]').forEach(notice => {
      notice.hidden = ready;
      notice.className = `notice ${failed ? 'warning' : 'neutral'}`;
      const value = failed ? 'Saved settings are unavailable. Use Refresh to load them before editing.' : 'Loading saved settings. Editing becomes available once they load.';
      if (notice.textContent !== value) notice.textContent = value;
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
      const body = await response.json().catch(error => {
        if (error.name === 'AbortError') throw error;
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

  function setNavigationOpen(open) {
    const toggle = $('navigation-toggle');
    if (!toggle) return;
    toggle.setAttribute('aria-expanded', String(open));
    updateNavigationLabel();
    document.querySelector('.sidebar').classList.toggle('is-open', open);
    document.querySelector('.sidebar').scrollTop = 0;
  }

  function currentRoute() {
    const [name, section] = location.hash.slice(1).split('/');
    const page = Object.prototype.hasOwnProperty.call(pageCopy, name) ? name : 'overview';
    const validSection = page === 'settings' ? Object.prototype.hasOwnProperty.call(settingsSections, section)
      : page === 'history' ? historyMetrics.some(metric => metric.key === section)
      : page === 'overview' && section === 'ph';
    return {page, section: validSection ? section : null};
  }

  function focusSettingsSection(section) {
    const heading = $(settingsSections[section]).querySelector('h2');
    heading.focus({preventScroll: true});
    heading.scrollIntoView({block: 'start', behavior: 'instant'});
  }

  function focusHistorySelection() {
    // Keep sensor and range controls in view when opening a single chart.
    $('page-title').focus({preventScroll: true});
    window.scrollTo({top: 0, behavior: 'instant'});
  }

  function focusPhHistory() {
    $('ph-history-title').focus({preventScroll: true});
    $('ph-history-title').scrollIntoView({block: 'start', behavior: 'instant'});
  }

  function navigate() {
    const {page, section} = currentRoute();
    const changed = state.activePage !== null && (state.activePage !== page || state.activeSection !== section);
    state.activePage = page;
    state.activeSection = section;
    setNavigationOpen(false);
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
    document.querySelectorAll('[data-settings-section]').forEach(link => {
      if (link.dataset.settingsSection === section) link.setAttribute('aria-current', 'location');
      else link.removeAttribute('aria-current');
    });
    if (page === 'settings' && section) {
      focusSettingsSection(section);
    } else if (page === 'overview' && section === 'ph') {
      focusPhHistory();
    } else if (page === 'history' && (section || changed)) {
      focusHistorySelection();
    } else if (changed) {
      $('page-title').focus({preventScroll: true});
      window.scrollTo({top: 0, behavior: 'instant'});
    }
    if (page === 'overview') renderChart();
    if (page === 'history') renderHistory();
    syncCameras();
    if (state.csrf) refreshVisiblePage();
  }

  function settingsFormValues({setup, schedules, pump, ph}) {
    return {
      'setup-form': {
        'setup-name-input': setup.setup_name,
        ...Object.fromEntries(setup.relay_labels.map((value, index) => [`relay-label-${index + 1}`, value])),
        ...Object.fromEntries(setup.relay_roles.map((value, index) => [`relay-role-${index + 1}`, value]))
      },
      'overflow-form': {
        ...Object.fromEntries(setup.overflow.sensor_labels.map((value, index) => [`overflow-label-${index + 1}`, value])),
        ...Object.fromEntries(waterRoles.map(role => [`overflow-${role}`, selectionValue(setup.overflow[role])]))
      },
      'schedule-form': {
        'lighting-enabled': schedules.lighting.enabled,
        'lighting-start': timeText(schedules.lighting.start_minute),
        'lighting-end': timeText(schedules.lighting.end_minute),
        'watering-enabled': schedules.watering.enabled,
        'watering-start': timeText(schedules.watering.start_minute),
        'watering-end': timeText(schedules.watering.end_minute),
        'watering-interval': String(schedules.watering.every_minutes),
        'watering-duration': String(schedules.watering.run_seconds),
        'watering-pump': schedules.watering.pump
      },
      'pump-form': {
        'safety-enabled': pump.safety_gpio_pin !== null,
        'safety-pin': String(pump.safety_gpio_pin === null ? 17 : pump.safety_gpio_pin)
      },
      'ph-settings-form': {
        'ph-enabled': ph.enabled,
        'ph-alerts': ph.alert_enabled,
        ...Object.fromEntries([
          ['ph-min', 'optimal_min'], ['ph-max', 'optimal_max'],
          ['ph-critical-min', 'critical_min'], ['ph-critical-max', 'critical_max'],
          ['ph-interval', 'monitoring_interval_seconds'], ['ph-history-size', 'history_size']
        ].map(([id, key]) => [id, String(ph[key])]))
      }
    };
  }

  function fieldMatchesSaved(id, saved) {
    const input = $(id);
    if (typeof saved === 'boolean') return input.checked === saved;
    if (id === 'safety-pin' && !$('safety-enabled').checked) return true;
    if (input.type === 'number') return input.value !== '' && Number(input.value) === Number(saved);
    return input.value.trim() === String(saved).trim();
  }

  function setSavedField(id, value) {
    if (typeof value === 'boolean') $(id).checked = value;
    else $(id).value = value;
    if (id.startsWith('relay-role-')) $(id).dataset.previousRole = value;
  }

  function settingsFieldLabel(id) {
    const explicit = {'lighting-enabled': 'Lighting schedule', 'watering-enabled': 'Watering schedule'};
    const field = $(id);
    if (explicit[id]) return explicit[id];
    if (field.getAttribute('aria-label')) return field.getAttribute('aria-label');
    const label = field.labels?.[0]?.cloneNode(true);
    label?.querySelectorAll('input, select, textarea').forEach(input => input.remove());
    return label?.textContent.replace(/\s+/g, ' ').trim() || id;
  }

  function settingsFieldText(id, value) {
    if (typeof value === 'boolean') return value ? 'Enabled' : 'Disabled';
    const field = $(id);
    if (field.tagName === 'SELECT') return [...field.options].find(option => option.value === value)?.textContent || String(value);
    return String(value).trim() || 'Empty';
  }

  function safetyMatchesSaved(values) {
    return fieldMatchesSaved('safety-enabled', values['safety-enabled'])
      && fieldMatchesSaved('safety-pin', values['safety-pin']);
  }

  function renderFormConflicts(id) {
    const form = $(id);
    const conflicts = state.formConflicts.get(id);
    let panel = $(`${id}-conflicts`);
    if (!panel && !conflicts?.size) return;
    if (!panel) {
      panel = document.createElement('div');
      panel.id = `${id}-conflicts`;
      panel.className = 'form-conflicts';
      panel.setAttribute('role', 'group');
      panel.setAttribute('aria-labelledby', `${id}-conflicts-title`);
      const title = document.createElement('strong');
      title.id = `${id}-conflicts-title`;
      title.textContent = 'Review changed device values';
      const guidance = document.createElement('p');
      guidance.textContent = 'Device settings changed while you were editing. Choose which values to keep before saving. Your other edits are preserved.';
      const status = document.createElement('p');
      status.id = `${id}-conflicts-status`;
      status.className = 'sr-only';
      status.setAttribute('role', 'status');
      const list = document.createElement('div');
      list.className = 'conflict-fields';
      panel.append(title, guidance, status, list);
      const heading = form.querySelector(':scope > .panel-heading');
      if (heading) heading.after(panel);
      else form.prepend(panel);
    }
    const list = panel.querySelector('.conflict-fields');
    let removedFocus;
    for (const row of [...list.children]) {
      if (!conflicts?.has(row.dataset.conflictField)) {
        if (row.contains(document.activeElement)) removedFocus = row.dataset.conflictField;
        row.remove();
      }
    }
    for (const fieldId of conflicts?.keys() || []) {
      const safety = id === 'pump-form' && fieldId === 'safety-pin';
      const fieldLabel = safety ? 'Safety switch' : settingsFieldLabel(fieldId);
      let row = list.querySelector(`[data-conflict-field="${fieldId}"]`);
      if (!row) {
        row = document.createElement('div');
        row.className = 'conflict-field';
        row.dataset.conflictField = fieldId;
        const title = document.createElement('strong');
        title.textContent = fieldLabel;
        const values = document.createElement('dl');
        for (const [key, label] of [['saved', 'On device'], ['draft', 'Your edit']]) {
          const group = document.createElement('div');
          const term = document.createElement('dt');
          term.textContent = label;
          const value = document.createElement('dd');
          value.dataset.conflictValue = key;
          group.append(term, value);
          values.append(group);
        }
        const actions = document.createElement('div');
        actions.className = 'conflict-actions';
        for (const choice of ['saved', 'draft']) {
          const button = document.createElement('button');
          button.type = 'button';
          button.className = 'button secondary';
          button.dataset.conflictChoice = choice;
          button.textContent = choice === 'saved' ? 'Use device value' : 'Keep my edit';
          button.setAttribute('aria-label', `${button.textContent} for ${fieldLabel}`);
          button.addEventListener('click', () => {
            if (form.dataset.saving || !state.formConflicts.get(id)?.has(fieldId)) return;
            if (choice === 'saved') {
              if (safety) setSavedField('safety-enabled', state.formBaselines.get(id)['safety-enabled']);
              setSavedField(fieldId, state.formBaselines.get(id)[fieldId]);
            }
            state.formConflicts.get(id).delete(fieldId);
            clearFormErrors(form);
            const result = form.querySelector('.form-result');
            if (result) message(result.id, '');
            updateFormState(id);
            previewSchedules();
            previewAssignments();
            previewOverflow();
            $('safety-pin-field').hidden = !$('safety-enabled').checked;
            $(safety && !$('safety-enabled').checked ? 'safety-enabled' : fieldId).focus({preventScroll: true});
          });
          actions.append(button);
        }
        row.append(title, values, actions);
        list.append(row);
      }
      const input = $(fieldId);
      const baseline = state.formBaselines.get(id);
      const values = safety ? {
        saved: baseline['safety-enabled'] ? `GPIO ${baseline['safety-pin']}` : 'Disabled',
        draft: $('safety-enabled').checked ? `GPIO ${input.value || 'not set'}` : 'Disabled'
      } : {saved: baseline[fieldId], draft: input.type === 'checkbox' ? input.checked : input.value};
      for (const [key, value] of Object.entries(values)) {
        const target = row.querySelector(`[data-conflict-value="${key}"]`);
        const display = settingsFieldText(fieldId, value);
        if (target.textContent !== display) target.textContent = display;
      }
      row.querySelectorAll('button').forEach(button => { button.disabled = Boolean(form.dataset.saving); });
    }
    panel.hidden = !conflicts?.size;
    text(`${id}-conflicts-status`, conflicts?.size ? `${conflicts.size} ${conflicts.size === 1 ? 'field needs' : 'fields need'} review before saving.` : 'All changed device values reviewed.');
    if (removedFocus && !form.closest('.page').hidden) {
      $(removedFocus === 'safety-pin' && !$('safety-enabled').checked ? 'safety-enabled' : removedFocus).focus({preventScroll: true});
    }
  }

  function updateFormState(id) {
    const baseline = state.formBaselines.get(id);
    if (!baseline) return;
    const conflicts = state.formConflicts.get(id);
    for (const field of conflicts?.keys() || []) {
      if (id === 'pump-form' ? safetyMatchesSaved(baseline) : fieldMatchesSaved(field, baseline[field])) conflicts.delete(field);
    }
    const dirty = Object.entries(baseline).some(([field, saved]) => !fieldMatchesSaved(field, saved));
    if (dirty) state.dirtyForms.add(id);
    else state.dirtyForms.delete(id);
    const form = $(id);
    form.classList.toggle('is-dirty', dirty);
    const section = Object.keys(settingsSections).find(key => settingsSections[key] === id);
    const sectionLink = document.querySelector(`[data-settings-section="${section}"]`);
    if (sectionLink) {
      sectionLink.classList.toggle('has-changes', dirty);
      sectionLink.querySelector('.section-edit-status').textContent = dirty ? ' · Unsaved changes' : '';
    }
    const feedback = document.querySelector(`[data-form-state="${id}"]`);
    const label = form.dataset.saving ? 'Saving changes…' : conflicts?.size ? 'Review changed device values' : dirty ? 'Unsaved changes' : 'All changes saved';
    if (feedback && feedback.textContent !== label) feedback.textContent = label;
    const discard = document.querySelector(`[data-discard-form="${id}"]`);
    if (discard) {
      discard.hidden = !dirty;
      discard.disabled = Boolean(form.dataset.saving);
    }
    renderFormConflicts(id);
    form.querySelector('[type=submit]').disabled = !state.csrf || Boolean(state.loginRedirect)
      || Boolean(form.dataset.saving) || Boolean(conflicts?.size);
    updateDraftSummary();
  }

  function pendingSettingForms() {
    return [...state.formBaselines.keys()].filter(id => state.dirtyForms.has(id));
  }

  function updateNavigationLabel() {
    const toggle = $('navigation-toggle');
    const label = toggle.getAttribute('aria-expanded') === 'true' ? 'Close' : 'Menu';
    const count = pendingSettingForms().length;
    text('navigation-toggle', count ? `${label} · ${count}` : label);
    toggle.setAttribute('aria-label', count ? `${label}, ${count} ${count === 1 ? 'section has' : 'sections have'} unsaved changes` : label);
  }

  function updateDraftSummary() {
    const pending = pendingSettingForms();
    $('draft-summary').hidden = !pending.length;
    const conflicts = [...state.formConflicts.values()].some(fields => fields.size);
    text('draft-summary-status', pending.length ? `${pending.length} ${pending.length === 1 ? 'section has' : 'sections have'} changes to review. ${conflicts ? 'Some device values changed while you were editing; review them before saving.' : 'Saved settings remain active until you save.'}` : '');
    document.querySelectorAll('[data-review-form]').forEach(link => {
      link.hidden = !pending.includes(link.dataset.reviewForm);
    });
    for (const page of ['settings', 'schedules']) {
      const count = pending.filter(id => page === 'schedules' ? id === 'schedule-form' : id !== 'schedule-form').length;
      $(`${page}-draft-count`).hidden = !count;
      text(`${page}-draft-count`, count);
      text(`${page}-draft-label`, count ? ` · ${count} ${count === 1 ? 'section has' : 'sections have'} unsaved changes` : '');
    }
    updateNavigationLabel();
  }

  function showSettings(settings, hydrate = true, acknowledgedForm = null) {
    const firstLoad = !state.settings;
    state.settings = settings;
    text('setup-name', settings.setup.setup_name);
    text('sidebar-setup-name', settings.setup.setup_name);
    text('device-address', location.origin);
    text('device-version', settings.version);
    text('command-access', settings.network.command_token_configured ? `Port ${settings.network.command_port} · token required` : `Port ${settings.network.command_port} · local commands only`);
    showClock(settings.device_time);
    const {setup, ph} = settings;
    for (const [id, values] of Object.entries(settingsFormValues(settings))) {
      const dirty = state.dirtyForms.has(id);
      const previous = state.formBaselines.get(id);
      const ownSaveAcknowledgment = id === acknowledgedForm;
      if (dirty && previous && !ownSaveAcknowledgment) {
        const conflicts = state.formConflicts.get(id) || new Map();
        if (id === 'pump-form') {
          // Enabled and pin describe one saved switch. An external disable
          // must not hide and erase an unsaved pin edit.
          if (Object.keys(values).some(field => values[field] !== previous[field]) && !safetyMatchesSaved(values)) conflicts.set('safety-pin', true);
        } else {
          const edited = new Set(Object.keys(values).filter(field => !fieldMatchesSaved(field, previous[field])));
          for (const [field, saved] of Object.entries(values)) {
            if (!edited.has(field)) setSavedField(field, saved);
            else if (saved !== previous[field] && !fieldMatchesSaved(field, saved)) conflicts.set(field, true);
          }
        }
        state.formConflicts.set(id, conflicts);
      }
      state.formBaselines.set(id, values);
      // A save response must preserve edits made while its request was pending.
      // Other clean forms can follow the confirmed device settings immediately.
      if (!dirty && (hydrate || !$(id).dataset.saving)) {
        for (const [field, value] of Object.entries(values)) {
          if (typeof value === 'boolean') $(field).checked = value;
          else $(field).value = value;
        }
        if (id === 'setup-form') setup.relay_roles.forEach((role, index) => {
          $(`relay-role-${index + 1}`).dataset.previousRole = role;
        });
      }
      updateFormState(id);
    }
    $('safety-pin-field').hidden = !$('safety-enabled').checked;
    setup.overflow.sensor_labels.forEach((label, i) => text(`tank${i + 1}-name`, label));
    text('pump-limit-summary', 'All relays support manual Start/Stop without runtime caps, cooldowns, or operating-hour restrictions. Multiple pumps can run together.');
    text('ph-target', `Target ${ph.optimal_min.toFixed(1)}–${ph.optimal_max.toFixed(1)}`);
    renderSchedules();
    previewSchedules();
    previewAssignments();
    previewOverflow();
    renderOutputs();
    renderChart();
    updateInitialSettingsAvailability();
    updateFormAvailability();
    if (firstLoad && $('global-message').dataset.source === 'settings-load') globalError('');
    if (firstLoad && currentRoute().page === 'settings' && currentRoute().section) focusSettingsSection(currentRoute().section);
  }

  function showClock(stamp) {
    // RFC 3339 device timestamps may include fractional seconds or a Z offset.
    // Keep the Pi's local time, rather than converting schedules to browser time.
    const match = typeof stamp === 'string' && stamp.match(/^(\d{4}-\d{2}-\d{2})T(\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$/);
    if (!match || !Number.isFinite(Date.parse(stamp))) {
      text('device-time', 'Device time unavailable. Schedule times follow the Pi’s clock.');
      text('footer-clock', 'Device time unavailable');
      return;
    }
    const offset = match[3] === 'Z' ? '+00:00' : match[3];
    const display = `${match[1]} ${match[2]} (UTC${offset})`;
    text('device-time', `Times follow the Pi’s clock: ${display}`);
    text('footer-clock', `Device time · ${match[2].slice(0, 5)} UTC${offset}`);
  }

  function renderSchedules() {
    if (!state.settings) return;
    const {
      lighting,
      watering
    } = state.settings.schedules;
    const available = !outputsUnavailable();
    const paused = available && state.outputs.automation_paused;
    const windowText = schedule => schedule.start_minute === schedule.end_minute ? 'All day · 24 hours' : `${timeText(schedule.start_minute)}–${timeText(schedule.end_minute)}${schedule.start_minute > schedule.end_minute ? ' · overnight' : ''}`;
    text('lighting-summary', lighting.enabled ? windowText(lighting) : 'No automatic lighting period');
    text('watering-summary', watering.enabled ? `${roleNames[watering.pump]} · ${watering.run_seconds}s every ${watering.every_minutes} min · ${windowText(watering)}` : 'No automatic water pump cycle');
    for (const [id, schedule] of [['lighting-state', lighting], ['watering-state', watering]]) {
      badge($(id), !schedule.enabled ? 'Disabled' : !available ? 'Unavailable' : paused ? 'Paused' : 'Scheduled', schedule.enabled && available && !paused ? 'good' : 'neutral');
    }
    const remaining = state.outputs && state.outputs.next_watering_in_seconds;
    const elapsed = Math.max(0, Math.floor((performance.now() - state.outputsReceivedAt) / 1000));
    const wait = finite(remaining) ? Math.max(0, remaining - elapsed) : null;
    text('next-watering', !available ? 'Saved schedules are shown. Live schedule status is unavailable; refresh to check the device.' : paused ? 'Schedules are paused. Resume when you are ready.' : watering.enabled && finite(wait) ? `Next watering check in ${duration(wait)}. Window and safety conditions must allow a run.` : 'Schedules run on the device when your browser is closed.');
  }

  function previewSchedules() {
    const waterRole = $('watering-pump').value;
    const start = minutes($('lighting-start').value),
      end = minutes($('lighting-end').value);
    const enabled = $('lighting-enabled').checked;
    const validLighting = finite(start) && finite(end);
    const from = start / 1440 * 100,
      to = end / 1440 * 100;
    $('lighting-preview').style.background = !enabled || !validLighting ? '#edf1e7' : start === end ? '#a9c77c' : start < end ? `linear-gradient(to right,#edf1e7 ${from}%,#a9c77c ${from}%,#a9c77c ${to}%,#edf1e7 ${to}%)` : `linear-gradient(to right,#a9c77c ${to}%,#edf1e7 ${to}%,#edf1e7 ${from}%,#a9c77c ${from}%)`;
    const hours = start === end ? 24 : ((end - start + 1440) % 1440) / 60;
    text('lighting-preview-text', !enabled ? 'Lights + Air schedule is disabled. Manual Start/Stop remains available.' : !validLighting ? 'Choose both on and off times to preview the lighting period.' : start === end ? 'All day: 24 hours of lights and air. Equal on and off times keep both on.' : `${hours.toFixed(1)} hours of lights and air per day${start > end ? ', crossing midnight' : ''}. Both switch together.`);
    const waterStart = minutes($('watering-start').value),
      waterEnd = minutes($('watering-end').value);
    const validWatering = finite(waterStart) && finite(waterEnd) && $('watering-interval').validity.valid && $('watering-duration').validity.valid;
    const waterWindow = waterStart === waterEnd ? 'all day (equal window times)' : `${timeText(waterStart)}–${timeText(waterEnd)}${waterStart > waterEnd ? ', crossing midnight' : ''}`;
    text('watering-preview-text', !$('watering-enabled').checked ? 'Water pump schedule is disabled. Choose the pump and interval, then enable it when ready.' : !validWatering ? 'Choose both window times and enter a repeat interval and run duration of at least 1 to preview the water cycle.' : `${roleNames[waterRole]}: run for ${$('watering-duration').value} seconds every ${$('watering-interval').value} minutes, ${waterWindow}. The first run waits one interval. Runs wait for the previous run to finish and for window and safety conditions to allow pumping.`);
    const dirty = state.dirtyForms.has('schedule-form');
    text('schedule-preview-state', dirty ? 'Preview of unsaved changes · Save schedules to apply.' : 'Preview of saved schedules');
    for (const [prefix, role] of [['lighting', 'lights_air'], ['watering', waterRole]]) {
      const channel = state.settings && relayFor(role);
      text(`${prefix}-relay`, `${roleNames[role]} · ${channel ? `Relay ${channel}` : 'No relay assigned'}`);
      $(`${prefix}-readiness`).hidden = !state.settings || Boolean(channel);
      text(`${prefix}-readiness-text`, `Assign and save a ${roleNames[role]} relay in Setup before enabling this schedule. `);
      const fields = prefix === 'watering' ? ['watering-enabled', 'watering-pump'] : ['lighting-enabled'];
      for (const id of fields) {
        const field = $(id);
        const descriptions = new Set((field.getAttribute('aria-describedby') || '').split(' ').filter(value => value && value !== `${prefix}-readiness`));
        if (state.settings && !channel) descriptions.add(`${prefix}-readiness`);
        if (descriptions.size) field.setAttribute('aria-describedby', [...descriptions].join(' '));
        else field.removeAttribute('aria-describedby');
      }
    }
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
    state.stats = data;
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
    state.statsConnected = true;
    updateConnection();
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
    const serverTime = finite(state.ph?.server_time) ? state.ph.server_time : Date.now() / 1000;
    const stale = status.last_reading && serverTime - status.last_reading > freshSeconds;
    if (status.enabled && hasReading && stale) badge($('ph-badge'), 'Stale', 'warning');
    text('ph-detail', !status.enabled ? 'History sampling is paused. Live readings and calibration remain available.' : stale ? 'This reading is stale. Check the sensor connection; the chart shows saved history.' : points === 0 ? 'Save a known buffer in Setup to normalize the reading. A second distinct buffer calibrates the slope.' : points === 1 ? 'Single-point normalization is active. Add a second distinct buffer to calibrate the slope. The chart uses your current calibration.' : 'Live readings and the chart use your current calibration. The shaded band shows your target range.');
  }

  function showPh(data) {
    if (!data.status || typeof data.status !== 'object' || Array.isArray(data.status)
      || !Array.isArray(data.history) || !data.calibration || typeof data.calibration !== 'object' || Array.isArray(data.calibration)) {
      throw new Error('The device returned incomplete pH status.');
    }
    state.phError = null;
    state.ph = data;
    showPhStatus(data.status);
    renderChart();
    if (data.error) text('ph-detail', data.error);
    text('ph-sample-time', data.history.length ? `Last chart sample ${dateText(data.history[data.history.length - 1].timestamp)}` : 'No chart sample yet');
    text('calibration-live-raw', finite(data.raw_value) ? data.raw_value.toFixed(3) : 'Unavailable');
    text('calibration-live-ph', formatNumber(data.status.current_ph));
    const warning = [...new Set([data.error, data.calibration_warning].filter(Boolean))].join(' ');
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
    updateCalibrationWorkflow();
    renderAttention();
  }

  function updateCalibrationWorkflow() {
    const form = $('calibration-form');
    const buffer = Number($('buffer-ph').value);
    const live = $('calibration-use-live').checked;
    const raw = live ? state.phError ? null : state.ph?.raw_value : $('calibration-raw').value === '' ? null : Number($('calibration-raw').value);
    const hasReading = finite(raw);
    const saving = Boolean(form.dataset.saving);
    const busy = state.calibrationPending || state.phBusy || saving;
    const refreshing = state.calibrationRefreshing;
    const saved = state.ph?.calibration?.[`point_${buffer}`];
    const count = [4, 7, 10].filter(point => state.ph?.calibration?.[`point_${point}`]).length;
    if (live && !hasReading) $('calibration-ready').checked = false;
    const ready = $('calibration-ready').checked;
    text('calibration-source-detail', live ? 'The display updates automatically. The device takes a fresh raw reading when you save.' : 'Enter the raw reading measured in this buffer. Use current live reading to copy a fresh value, then confirm it has settled.');
    text('calibration-plan', !state.ph ? 'Loading saved calibration points…' : saved ? `Saving replaces the saved pH ${buffer} point. Other buffer points stay in place; you do not need to remove this point first.` : count === 0 ? `Save pH ${buffer} to normalize the reading. Then add a second distinct buffer to calibrate the slope.` : `Add the pH ${buffer} point alongside your ${count} saved buffer${count === 1 ? '' : 's'}.`);
    text('calibration-ready-label', live ? `The probe is stable in the pH ${buffer} buffer.` : `I measured this raw value with the probe stable in pH ${buffer} buffer.`);
    text('calibration-readiness', busy ? 'Applying the calibration change…' : refreshing ? 'Checking the live sensor reading…' : !hasReading ? live ? 'A fresh raw reading is unavailable. Check the probe connection, or uncheck live reading to enter a measured average.' : 'Enter a raw sensor reading from the selected buffer.' : !ready ? `Let the reading settle in pH ${buffer} buffer, then confirm stability above.` : `Ready to ${saved ? 'replace' : 'save'} pH ${buffer} using ${live ? 'a fresh sensor reading' : 'your entered raw value'}.`);
    for (const id of ['buffer-ph', 'calibration-use-live', 'calibration-raw', 'calibration-ready']) $(id).disabled = busy || refreshing || (id === 'calibration-ready' && !hasReading);
    const button = $('calibration-submit');
    button.disabled = busy || refreshing || !hasReading || !ready || !state.settings || !state.csrf || Boolean(state.loginRedirect);
    if (!saving) button.textContent = state.phBusy ? 'Applying…' : `${saved ? 'Replace' : 'Save'} pH ${buffer} point`;
    text('use-raw-reading', refreshing ? 'Checking reading…' : live ? 'Refresh sensor reading' : 'Use current live reading');
    $('use-raw-reading').disabled = busy || refreshing || !state.csrf || Boolean(state.loginRedirect);
    document.querySelectorAll('[data-remove-calibration]').forEach(remove => { remove.disabled = busy || refreshing || !state.csrf || Boolean(state.loginRedirect); });
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

  function nearestChartPoint(points, stamp) {
    if (!finite(stamp)) return Math.max(0, points.length - 1);
    return points.reduce((nearest, point, index) => Math.abs(point.timestamp - stamp) < Math.abs(points[nearest].timestamp - stamp) ? index : nearest, 0);
  }

  function chartPointerStamp(svg, layout, event) {
    // SVG can be scaled and letterboxed inside a narrow card. Transform the
    // pointer into its viewBox instead of treating screen pixels as plot units.
    const matrix = svg.getScreenCTM();
    if (!matrix) return layout.end;
    const position = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
    return layout.start + (position.x - layout.left) / (layout.right - layout.left) * (layout.end - layout.start);
  }

  function addChartInspection(svg, layout, color) {
    const marker = svgNode('g', {class: 'chart-inspection', visibility: 'hidden', 'aria-hidden': 'true', 'pointer-events': 'none'});
    marker.append(svgNode('line', {y1: layout.top, y2: layout.bottom, stroke: color, 'stroke-opacity': .55, 'stroke-dasharray': '3 4'}));
    marker.append(svgNode('circle', {r: 4.5, fill: '#fff', stroke: color, 'stroke-width': 2.5}));
    svg.append(marker);
  }

  function showChartInspection(svg, layout, point, value) {
    const marker = svg.querySelector('.chart-inspection');
    if (!marker) return;
    const x = layout.left + (point.timestamp - layout.start) / (layout.end - layout.start) * (layout.right - layout.left);
    marker.querySelector('line').setAttribute('x1', x);
    marker.querySelector('line').setAttribute('x2', x);
    marker.querySelector('circle').setAttribute('cx', x);
    marker.querySelector('circle').setAttribute('cy', layout.y(value));
    marker.setAttribute('visibility', 'visible');
  }

  function hideChartInspection(svg, tooltip) {
    tooltip.hidden = true;
    svg.querySelector('.chart-inspection')?.setAttribute('visibility', 'hidden');
  }

  function syncChartControls(svg, points, cursor, description = '', announce = false) {
    const group = svg.closest('.chart-interaction');
    if (!group) return;
    for (const button of group.querySelectorAll('[data-chart-step]')) {
      button.disabled = !points.length;
      const atBoundary = Number(button.dataset.chartStep) < 0 ? cursor === 0 : cursor === points.length - 1;
      button.setAttribute('aria-disabled', String(!points.length || atBoundary));
    }
    const label = points.length ? `Reading ${cursor + 1} of ${points.length} shown · ${description}` : 'No readings to inspect.';
    const reading = group.querySelector('.chart-reading');
    if (reading.textContent !== label) reading.textContent = label;
    // Hover is visual inspection. Announce only intentional keyboard/button
    // stepping, not every pointer movement or background redraw.
    const status = group.querySelector('.chart-announcement');
    if (announce && status.textContent !== label) status.textContent = label;
  }

  function phReadingDescription(point) {
    const timestamp = new Date(point.timestamp * 1000).toLocaleString([], {month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit'});
    return `${timestamp} · pH ${point.ph_value.toFixed(2)}`;
  }

  function bindChartInspection(svg, tooltip, inspect) {
    const group = svg.closest('.chart-interaction');
    svg.addEventListener('focus', inspect);
    svg.addEventListener('pointermove', inspect);
    svg.addEventListener('pointerdown', event => {
      svg.focus({preventScroll: true});
      inspect(event);
    });
    svg.addEventListener('pointerleave', () => {
      if (!group.contains(document.activeElement)) hideChartInspection(svg, tooltip);
    });
    group.addEventListener('focusout', event => {
      if (!group.contains(event.relatedTarget)) hideChartInspection(svg, tooltip);
    });
    group.addEventListener('keydown', event => {
      if (event.key === 'Escape') hideChartInspection(svg, tooltip);
    });
    group.querySelectorAll('[data-chart-step]').forEach(button => {
      button.addEventListener('focus', () => inspect({type: 'controlfocus'}));
      button.addEventListener('click', () => {
        if (button.disabled || button.getAttribute('aria-disabled') === 'true') return;
        inspect({type: 'keydown', key: Number(button.dataset.chartStep) < 0 ? 'ArrowLeft' : 'ArrowRight'});
      });
    });
    svg.addEventListener('keydown', event => {
      if (event.key === 'Escape') hideChartInspection(svg, tooltip);
      if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) {
        event.preventDefault();
        inspect(event);
      }
    });
  }

  function renderChart() {
    const svg = $('ph-chart-svg');
    const previousStamp = state.chartPoints[state.chartCursor]?.timestamp;
    const inspecting = !$('chart-tooltip').hidden;
    svg.replaceChildren();
    if (state.phError) {
      hideChartInspection(svg, $('chart-tooltip'));
      text('ph-chart-message', state.phError);
      $('ph-chart-message').hidden = false;
      svg.setAttribute('hidden', '');
      state.chartPoints = [];
      syncChartControls(svg, [], 0);
      return;
    }
    if (!state.ph) return;
    const all = (state.ph.history || []).filter(point => finite(point.timestamp));
    const end = state.ph.server_time || Date.now() / 1000,
      start = end - state.range * 3600;
    const windowPoints = all.filter(point => point.timestamp >= start && point.timestamp <= end);
    const points = windowPoints.filter(point => finite(point.ph_value) && point.ph_value >= 0 && point.ph_value <= 14);
    state.chartPoints = points;
    state.chartCursor = nearestChartPoint(points, previousStamp);
    $('ph-chart-message').hidden = points.length > 0;
    svg.toggleAttribute('hidden', points.length === 0);
    if (!points.length) {
      hideChartInspection(svg, $('chart-tooltip'));
      syncChartControls(svg, [], 0);
      text('ph-chart-message', windowPoints.length ? 'Saved readings are outside pH 0–14 with this calibration. Recheck the saved buffer points in Setup.' : all.length ? 'No pH readings in this time range. Try a longer range.' : 'No pH history yet. Readings appear after the sensor reports a valid sample.');
      return;
    }
    const width = Math.max(svg.getBoundingClientRect().width, 220),
      height = Math.max(svg.getBoundingClientRect().height, 180);
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
      end,
      top,
      bottom,
      y
    };
    const config = state.settings ? state.settings.ph : state.ph.config;
    svg.append(svgNode('rect', {
      x: left,
      y: y(config.optimal_max),
      width: right - left,
      height: y(config.optimal_min) - y(config.optimal_max),
      fill: '#eef2df'
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
        fill: '#5c6f62',
        'font-size': 11,
        'text-anchor': 'end'
      }, String(value)));
    }
    const divisions = width < 450 ? 2 : 4;
    for (let i = 0; i <= divisions; i++) {
      const stamp = start + (end - start) * i / divisions;
      svg.append(svgNode('text', {
        x: x(stamp),
        y: height - 7,
        fill: '#5c6f62',
        'font-size': 11,
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
      if (!connected && (!next || !finite(next.ph_value) || next.ph_value < 0 || next.ph_value > 14 || next.timestamp - point.timestamp > gapSeconds)) {
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
    addChartInspection(svg, state.chartLayout, '#507943');
    svg.setAttribute('aria-label', `${points.length} pH readings in the last ${state.range} hours. Latest value ${last.ph_value.toFixed(2)}. Use arrow keys to inspect samples.`);
    syncChartControls(svg, points, state.chartCursor, phReadingDescription(points[state.chartCursor]));
    if (inspecting) chartTooltip();
  }

  function chartTooltip(event) {
    if (!state.chartPoints.length || !state.chartLayout) return;
    if (event?.type === 'keydown') {
      if (event.key === 'Home') state.chartCursor = 0;
      else if (event.key === 'End') state.chartCursor = state.chartPoints.length - 1;
      else state.chartCursor = Math.max(0, Math.min(state.chartPoints.length - 1, state.chartCursor + (event.key === 'ArrowLeft' ? -1 : 1)));
    } else if (event?.type.startsWith('pointer')) {
      state.chartCursor = nearestChartPoint(state.chartPoints, chartPointerStamp($('ph-chart-svg'), state.chartLayout, event));
    }
    const point = state.chartPoints[state.chartCursor];
    const description = phReadingDescription(point);
    text('chart-tooltip', description);
    $('chart-tooltip').hidden = false;
    showChartInspection($('ph-chart-svg'), state.chartLayout, point, point.ph_value);
    syncChartControls($('ph-chart-svg'), state.chartPoints, state.chartCursor, description, event?.type === 'keydown');
  }

  function historyNumber(value, metric) {
    return finite(value) ? value.toLocaleString([], {maximumFractionDigits: metric.decimals}) : '—';
  }

  function historyReadingDescription(point, metric) {
    return `${dateText(point.timestamp)} · ${historyNumber(point.value, metric)} ${metric.unit}`;
  }

  function historyRangeLabel(hours = state.historyRange) {
    return hours === 168 ? 'Last 7 days' : hours === 1 ? 'Last hour' : `Last ${hours} hours`;
  }

  function validHistorySnapshot(data, hours) {
    if (!data || data.range_hours !== hours || !finite(data.server_time)
      || !Array.isArray(data.series) || data.series.length !== historyMetrics.length
      || (data.error !== null && typeof data.error !== 'string')) return false;
    const validPoint = point => point && finite(point.timestamp) && (point.value === null || finite(point.value));
    return historyMetrics.every(metric => {
      const matches = data.series.filter(series => series?.key === metric.key);
      if (matches.length !== 1) return false;
      const series = matches[0];
      if (typeof series.available !== 'boolean' || !Number.isInteger(series.count) || series.count < 0
        || !Array.isArray(series.points) || !series.points.every(validPoint)
        || (series.latest !== null && (!validPoint(series.latest) || !finite(series.latest.value)))) return false;
      const summaries = [series.minimum, series.maximum, series.average];
      return series.count > 0
        ? summaries.every(finite) && series.minimum <= series.maximum && series.points.some(point => finite(point.value))
        : summaries.every(value => value === null) && series.points.every(point => point.value === null);
    });
  }

  function makeHistoryCards() {
    const all = document.createElement('a');
    all.href = '#history';
    all.dataset.historySensor = '';
    all.textContent = 'All sensors';
    $('history-sensors').append(all);
    for (const metric of historyMetrics) {
      const link = document.createElement('a');
      link.href = `#history/${metric.key}`;
      link.dataset.historySensor = metric.key;
      link.textContent = metric.label;
      $('history-sensors').append(link);
      const card = document.createElement('article');
      card.id = `history-card-${metric.key}`;
      card.setAttribute('aria-labelledby', `history-title-${metric.key}`);
      card.className = 'panel history-card';
      card.style.setProperty('--sensor-color', metric.color);
      card.innerHTML = `<div class="panel-heading"><div><span class="eyebrow">${metric.unit}</span><h2>${metric.label} over time</h2></div><span id="history-status-${metric.key}" class="badge neutral">Waiting</span></div><div class="history-latest"><strong id="history-value-${metric.key}">—</strong><span>Last recorded · ${metric.unit}</span></div><div class="chart-interaction"><div class="chart-wrap"><svg id="history-chart-${metric.key}" class="sensor-chart-svg" viewBox="0 0 500 230" role="img" tabindex="0" aria-label="${metric.label} readings over time"></svg><div id="history-empty-${metric.key}" class="chart-empty">Loading history…</div><output id="history-tooltip-${metric.key}" class="chart-tooltip" aria-hidden="true" aria-live="off" hidden></output></div><div class="chart-step-buttons"><button type="button" class="button quiet" data-chart-step="-1" aria-label="Previous ${metric.label} reading" aria-describedby="history-reading-${metric.key}" disabled>← Previous</button><button type="button" class="button quiet" data-chart-step="1" aria-label="Next ${metric.label} reading" aria-describedby="history-reading-${metric.key}" disabled>Next →</button></div><output id="history-reading-${metric.key}" class="chart-reading" aria-live="off">No readings to inspect.</output><span class="sr-only chart-announcement" role="status"></span></div><dl class="history-stats"><div><dt>Minimum</dt><dd id="history-min-${metric.key}">—</dd></div><div><dt>Average</dt><dd id="history-average-${metric.key}">—</dd></div><div><dt>Maximum</dt><dd id="history-max-${metric.key}">—</dd></div></dl><p id="history-detail-${metric.key}" class="helper">Waiting for readings</p>`;
      $('history-cards').append(card);
      card.querySelector('h2').id = `history-title-${metric.key}`;
      card.querySelector('h2').tabIndex = -1;
      const chart = $(`history-chart-${metric.key}`);
      chart.setAttribute('aria-describedby', `ph-chart-help history-reading-${metric.key}`);
      bindChartInspection(chart, $(`history-tooltip-${metric.key}`), event => historyTooltip(metric, event));
    }
    $('history-refresh').addEventListener('click', () => {
      if (!state.historyLoading) pollHistory(false);
    });
  }

  function renderHistory() {
    const route = currentRoute();
    const selected = route.page === 'history' ? route.section : null;
    $('history-cards').classList.toggle('single-sensor', Boolean(selected));
    document.querySelectorAll('[data-history-sensor]').forEach(link => {
      if (link.dataset.historySensor === (selected || '')) link.setAttribute('aria-current', 'location');
      else link.removeAttribute('aria-current');
    });
    const metricLabel = historyMetrics.find(metric => metric.key === selected)?.label;
    text('history-selection', `${metricLabel || 'All six sensors'} · ${historyRangeLabel()}`);
    // Never label another range's samples with the newly selected period.
    const history = state.history?.range_hours === state.historyRange ? state.history : null;
    const error = state.historyError || history?.error;
    text('history-error', error || '');
    $('history-error').hidden = !error;
    $('history-cards').setAttribute('aria-busy', String(state.historyLoading));
    $('history-refresh').setAttribute('aria-disabled', String(state.historyLoading));
    $('history-refresh').setAttribute('aria-busy', String(state.historyLoading));
    text('history-refresh', state.historyLoading ? 'Refreshing…' : error ? 'Retry history' : 'Refresh history');
    const loadedAt = state.historyLoadedAt ? new Date(state.historyLoadedAt).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit', second: '2-digit'}) : '';
    text('history-load-status', state.historyLoading
      ? `${history ? 'Updating' : 'Loading'} ${historyRangeLabel()}…${history ? ' Showing the last loaded history while this request completes.' : ''}`
      : history ? `${error ? 'Last loaded' : 'Loaded'} ${historyRangeLabel()}${loadedAt ? ` at ${loadedAt}` : ''}.`
      : error ? 'History could not be loaded. Retry history or choose another range.' : 'Waiting for saved history…');
    for (const metric of historyMetrics) {
      const key = metric.key;
      $(`history-card-${key}`).hidden = Boolean(selected && selected !== key);
      if (selected && selected !== key) {
        hideChartInspection($(`history-chart-${key}`), $(`history-tooltip-${key}`));
        continue;
      }
      const series = history && history.series.find(series => series.key === key);
      const svg = $(`history-chart-${key}`);
      const chartFocused = svg.closest('.chart-interaction').contains(document.activeElement);
      const previousChart = state.historyCharts[key];
      const previousStamp = previousChart?.points[previousChart.cursor]?.timestamp;
      const inspecting = !$(`history-tooltip-${key}`).hidden;
      svg.replaceChildren();
      state.historyCharts[key] = null;
      const points = series ? series.points.filter(point => finite(point.timestamp) && finite(point.value)) : [];
      const latest = series && series.latest;
      text(`history-value-${key}`, historyNumber(latest && latest.value, metric));
      let status = state.historyLoading ? series ? 'Updating' : 'Loading' : !series ? 'Waiting' : series.available ? 'Recording' : 'No fresh reading';
      if (error) status = !points.length ? 'Unavailable' : state.historyError ? 'Last loaded' : 'History warning';
      badge($(`history-status-${key}`), status, error ? 'warning' : !state.historyLoading && series && series.available ? 'good' : 'neutral');
      text(`history-min-${key}`, historyNumber(series && series.minimum, metric));
      text(`history-average-${key}`, historyNumber(series && series.average, metric));
      text(`history-max-${key}`, historyNumber(series && series.maximum, metric));
      text(`history-detail-${key}`, series ? `${series.count.toLocaleString()} readings in this range. ${latest ? `Last sample ${dateText(latest.timestamp)}.` : 'No valid sample recorded yet.'}` : error ? 'No saved readings loaded for this range.' : `Loading saved readings for ${historyRangeLabel().toLowerCase()}.`);
      $(`history-empty-${key}`).hidden = points.length > 0;
      svg.toggleAttribute('hidden', points.length === 0);
      if (!points.length) {
        hideChartInspection(svg, $(`history-tooltip-${key}`));
        syncChartControls(svg, [], 0);
        text(`history-empty-${key}`, error || (!series ? `Loading ${historyRangeLabel().toLowerCase()}…` : latest ? 'No readings in this time range. Try a longer range.' : 'No history yet. Recording starts when fresh sensor readings are available. Check the sensor connection if no readings appear.'));
        if (chartFocused && !$('page-history').hidden) $(`history-title-${key}`).focus({preventScroll: true});
        continue;
      }
      const width = Math.max(svg.getBoundingClientRect().width, 220);
      const height = Math.max(svg.getBoundingClientRect().height, 220);
      const left = 48, right = width - 12, top = 15, bottom = height - 28;
      const end = history.server_time, start = end - history.range_hours * 3600;
      const padding = Math.max((series.maximum - series.minimum) * 0.12, Math.abs(series.maximum) * 0.02, 1);
      const low = metric.key === 'temp' ? series.minimum - padding : Math.max(0, series.minimum - padding);
      const high = metric.key === 'hum' ? Math.min(100, series.maximum + padding) : series.maximum + padding;
      const x = stamp => left + (stamp - start) / (end - start) * (right - left);
      const y = value => bottom - (value - low) / (high - low) * (bottom - top);
      svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
      state.historyCharts[key] = {points, start, end, left, right, top, bottom, y, cursor: nearestChartPoint(points, previousStamp)};
      for (let step = 0; step <= 4; step++) {
        const value = low + (high - low) * step / 4;
        svg.append(svgNode('line', {x1: left, x2: right, y1: y(value), y2: y(value), stroke: '#e5eadf', 'stroke-dasharray': '3 4'}));
        svg.append(svgNode('text', {x: left - 8, y: y(value) + 4, fill: '#5c6f62', 'font-size': 11, 'text-anchor': 'end'}, value.toLocaleString([], {notation: Math.abs(value) >= 10000 ? 'compact' : 'standard', maximumFractionDigits: 1})));
      }
      const divisions = width < 400 ? 2 : 4;
      for (let step = 0; step <= divisions; step++) {
        const stamp = start + (end - start) * step / divisions;
        const label = history.range_hours > 24 ? new Date(stamp * 1000).toLocaleDateString([], {month: 'short', day: 'numeric'}) : new Date(stamp * 1000).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'});
        svg.append(svgNode('text', {x: x(stamp), y: height - 7, fill: '#5c6f62', 'font-size': 11, 'text-anchor': step === 0 ? 'start' : step === divisions ? 'end' : 'middle'}, label));
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
      addChartInspection(svg, state.historyCharts[key], metric.color);
      svg.setAttribute('aria-label', `${metric.label}: ${series.count} readings in the last ${history.range_hours} hours. Minimum ${historyNumber(series.minimum, metric)}, average ${historyNumber(series.average, metric)}, maximum ${historyNumber(series.maximum, metric)} ${metric.unit}. Use arrow keys to inspect samples.`);
      syncChartControls(svg, points, state.historyCharts[key].cursor, historyReadingDescription(points[state.historyCharts[key].cursor], metric));
      if (inspecting) historyTooltip(metric);
    }
  }

  function historyTooltip(metric, event) {
    const chart = state.historyCharts[metric.key];
    if (!chart || !chart.points.length) return;
    if (event?.type === 'keydown') {
      if (event.key === 'Home') chart.cursor = 0;
      else if (event.key === 'End') chart.cursor = chart.points.length - 1;
      else chart.cursor = Math.max(0, Math.min(chart.points.length - 1, chart.cursor + (event.key === 'ArrowLeft' ? -1 : 1)));
    } else if (event?.type.startsWith('pointer')) {
      chart.cursor = nearestChartPoint(chart.points, chartPointerStamp($(`history-chart-${metric.key}`), chart, event));
    }
    const point = chart.points[chart.cursor];
    const description = historyReadingDescription(point, metric);
    text(`history-tooltip-${metric.key}`, description);
    $(`history-tooltip-${metric.key}`).hidden = false;
    showChartInspection($(`history-chart-${metric.key}`), chart, point, point.value);
    syncChartControls($(`history-chart-${metric.key}`), chart.points, chart.cursor, description, event?.type === 'keydown');
  }

  async function pollHistory(schedule = true) {
    let request;
    try {
      if (state.passwordChanging || state.loginRedirect) return;
      request = ++state.historyRequest;
      const hours = state.historyRange;
      state.historyLoading = true;
      state.historyError = null;
      renderHistory();
      const history = await api(`/api/history?hours=${hours}`);
      if (request !== state.historyRequest) return;
      if (history.range_hours !== hours) throw new Error('The device returned a different history range. Retry history.');
      if (!validHistorySnapshot(history, hours)) throw new Error('The device returned incomplete sensor history. Retry history.');
      state.history = history;
      state.historyLoadedAt = Date.now();
      state.historyError = null;
    } catch (error) {
      if (request === state.historyRequest) {
        state.historyError = `${error.message} ${state.history?.range_hours === state.historyRange ? 'Showing the last loaded history.' : 'Use Retry history to try again.'}`;
      }
    } finally {
      if (request === state.historyRequest) {
        state.historyLoading = false;
        renderHistory();
      }
      if (schedule) setTimeout(() => pollHistory(), 60000);
    }
  }

  function makeOutputCards() {
    for (let channel = 1; channel <= 4; channel++) {
      const card = document.createElement('article');
      card.id = `output-card-${channel}`;
      card.className = 'output-card';
      card.innerHTML = `<div class="output-top"><span class="relay-identifier"><svg class="icon" aria-hidden="true"><use href="/img/icons.svg#power"/></svg><span class="eyebrow">RELAY ${channel}</span></span><span class="badge neutral" id="output-state-${channel}" role="status" aria-label="Relay ${channel} status">Unknown</span></div><h2 id="output-name-${channel}">Relay ${channel}</h2><p class="output-role" id="output-role-${channel}"></p><p class="helper" id="output-overflow-${channel}"></p><p class="output-timing" id="output-timing-${channel}">Waiting for the relay board</p><div class="output-actions"><button type="button" class="button primary" data-output="${channel}" aria-describedby="output-timing-${channel} output-overflow-${channel}" disabled>Connecting…</button></div>`;
      $('output-controls').append(card);
    }
  }

  function validOutputSnapshot(data) {
    return Array.isArray(data?.outputs) && data.outputs.length === 4
      && data.outputs.every(output => output && (output.on === null || typeof output.on === 'boolean'))
      && [1, 2, 3, 4].every(channel => data.outputs.filter(output => output.channel === channel).length === 1);
  }

  function outputsUnavailable() {
    const data = state.outputs;
    return !data || Boolean(data.error) || !Array.isArray(data.outputs) || data.outputs.length !== 4 || data.outputs.some(output => !output || typeof output.on !== 'boolean');
  }

  function updateConnection() {
    // The environment dot represents sensor requests; the header also reports
    // partial failures in equipment status without hiding healthy readings.
    document.body.dataset.connection = state.statsConnected === true ? 'connected' : state.statsConnected === false ? 'offline' : 'connecting';
    if (state.statsConnected === false) badge($('connection'), '● Offline', 'warning');
    else if (state.statsConnected !== true) badge($('connection'), 'Connecting', 'neutral');
    else if (!state.outputs) badge($('connection'), '● Checking equipment', 'neutral');
    else if (outputsUnavailable()) badge($('connection'), '● Check equipment', 'warning');
    else badge($('connection'), '● Connected', 'good');
    renderAttention();
  }

  function renderAttention() {
    const issues = [];
    const add = (key, title, detail, href, action) => issues.push({key, title, detail, href, action});
    const outputs = state.outputs;
    if (outputs?.stop_pending) {
      add('equipment', 'Outputs are still stopping', 'The device is retrying shutdown. Check equipment status and the relay board connection.', '#outputs', 'Check outputs');
    } else if (outputs && outputsUnavailable()) {
      add('equipment', 'Equipment status is unavailable', 'Current relay states could not be confirmed. Stop controls remain available while the dashboard reconnects.', '#outputs', 'Check outputs');
    } else if (outputs?.pump_block_reason) {
      add('equipment', 'Pumps are blocked', outputs.pump_block_reason, '#outputs', 'Review pump controls');
    }
    if (state.statsConnected === false) {
      add('environment', 'Environment readings are unavailable', 'Current sensor readings could not be refreshed. The dashboard retries automatically; recorded history remains separate.', '#history', 'Review sensor history');
    } else if (state.stats) {
      const missing = historyMetrics.filter(metric => !Number.isFinite(Number.parseFloat(state.stats[metric.key])));
      if (missing.length) add('environment', 'Missing environment readings', `${missing.map(metric => metric.label).join(', ')}: no fresh reading. Check the sensor connections.`, '#history', 'Review sensor history');
      const sensors = [1, 2].map(index => {
        const value = String(state.stats[`t${index}_ovf`] || '').trim();
        if (['NONE', 'NORMAL'].includes(value) && !state.stats.overflow_error) return null;
        const label = state.settings?.setup.overflow.sensor_labels[index - 1] || `Overflow sensor ${index}`;
        return `${label}: ${value === 'OVERFLOW' ? 'triggered' : 'unavailable'}`;
      }).filter(Boolean);
      if (sensors.length) add('overflow', 'Overflow sensors need attention', `${sensors.join(' · ')}. Review which pumps use these stops.`, '#settings/overflow', 'Review overflow stops');
    }
    const ph = state.ph;
    if ((ph?.status?.enabled ?? state.settings?.ph.enabled) !== false) {
      if (state.phError) {
        add('ph', 'pH monitoring is unavailable', 'The latest pH response could not be refreshed. The dashboard is retrying automatically.', '#overview/ph', 'Review pH history');
      } else if (ph) {
        const status = ph.status;
        const serverTime = finite(ph.server_time) ? ph.server_time : Date.now() / 1000;
        const freshness = Math.max(15, (state.settings?.ph.monitoring_interval_seconds || 60) * 2);
        const stale = status.last_reading && serverTime - status.last_reading > freshness;
        if (ph.error || !finite(status.current_ph) || stale) {
          add('ph', stale ? 'The pH reading is stale' : !finite(status.current_ph) ? 'The pH reading is unavailable' : 'pH monitoring needs attention', ph.error || 'Check the probe connection. Saved chart samples do not confirm a current reading.', '#overview/ph', 'Review pH history');
        } else if (['Warning', 'Critical'].includes(status.alert_level)) {
          const config = ph.config || state.settings?.ph;
          add('ph', 'pH is outside your target', `Current pH ${formatNumber(status.current_ph)}${config ? ` · your target ${config.optimal_min.toFixed(1)}–${config.optimal_max.toFixed(1)}` : ''}. Review the reading and calibration.`, '#overview/ph', 'Review pH history');
        }
      }
    }
    const priority = ['equipment', 'overflow', 'ph', 'environment'];
    issues.sort((a, b) => priority.indexOf(a.key) - priority.indexOf(b.key));
    const panel = $('overview-attention');
    const list = $('attention-items');
    const panelFocused = panel.contains(document.activeElement);
    let removedFocus = false;
    for (const item of [...list.children]) {
      if (!issues.some(issue => issue.key === item.dataset.issue)) {
        removedFocus ||= item.contains(document.activeElement);
        item.remove();
      }
    }
    // Keep existing links in place so polling does not reset keyboard focus.
    for (const issue of issues) {
      let item = list.querySelector(`[data-issue="${issue.key}"]`);
      if (!item) {
        item = document.createElement('li');
        item.dataset.issue = issue.key;
        item.append(document.createElement('strong'), document.createElement('p'), document.createElement('a'));
        item.lastElementChild.className = 'text-link';
        item.lastElementChild.addEventListener('click', event => {
          if (event.currentTarget.hash === location.hash && location.hash === '#overview/ph') {
            event.preventDefault();
            focusPhHistory();
          }
        });
        const next = [...list.children].find(row => priority.indexOf(row.dataset.issue) > priority.indexOf(issue.key));
        list.insertBefore(item, next || null);
      }
      const [title, detail, link] = item.children;
      if (title.textContent !== issue.title) title.textContent = issue.title;
      if (detail.textContent !== issue.detail) detail.textContent = issue.detail;
      link.setAttribute('href', issue.href);
      if (link.textContent !== `${issue.action} →`) link.textContent = `${issue.action} →`;
    }
    panel.hidden = !issues.length;
    text('attention-count', `${issues.length} ${issues.length === 1 ? 'item' : 'items'}`);
    text('attention-status', issues.length ? `${issues.length} ${issues.length === 1 ? 'item needs' : 'items need'} attention: ${issues.map(issue => issue.title).join('. ')}.` : 'Previously reported issues have cleared.');
    if ((removedFocus || (!issues.length && panelFocused)) && !panel.closest('.page').hidden) $(issues.length ? 'attention-title' : 'page-title').focus({preventScroll: true});
  }

  function renderOutputs() {
    updateConnection();
    if (!state.settings) return;
    state.settings.setup.relay_labels.forEach((label, i) => {
      text(`output-name-${i+1}`, label);
      text(`output-role-${i+1}`, roleNames[state.settings.setup.relay_roles[i]]);
      const role = state.settings.setup.relay_roles[i];
      const overflow = state.settings.setup.overflow;
      text(`output-overflow-${i+1}`, waterRoles.includes(role) ? `Overflow stops: ${sensorNames(overflow[role], overflow.sensor_labels)}` : role === 'lights_air' ? 'Lights and air switch together on this relay.' : 'Choose a function in Setup.');
    });
    const data = state.outputs || {
      outputs: [1, 2, 3, 4].map(channel => ({channel, on: null})),
      last_event: 'Waiting for the relay board'
    };
    const unavailable = outputsUnavailable();
    const elapsed = Math.max(0, Math.floor((performance.now() - state.outputsReceivedAt) / 1000));
    $('paused-banner').hidden = !data.automation_paused;
    if ($('paused-banner-message')) text('paused-banner-message', unavailable ? 'Schedules were last reported paused. Current automation state is unavailable.' : 'Schedules are paused. Outputs will stay under manual control until you resume.');
    const stopping = state.outputStopSequences.size > 0;
    const pendingActions = [...state.outputPending.values()];
    const allOff = !unavailable && data.outputs.every(output => output.on === false);
    $('pause-schedules').disabled = (allOff && data.automation_paused && !data.stop_pending && !pendingActions.some(action => action.energizing)) || Boolean($('pause-schedules').dataset.busy);
    document.querySelectorAll('.resume-schedules').forEach(button => {
      button.disabled = stopping || pendingActions.some(action => action.kind === 'resume') || Boolean(button.dataset.busy);
      button.textContent = pendingActions.some(action => action.kind === 'resume') ? 'Resuming…' : 'Resume schedules';
    });
    text('stop-all', state.outputStopSequences.has('all') ? '■ Stopping…' : '■ Stop all');
    const waiting = [...state.outputStopSequences.values()].some(sequence => sequence.phase === 'waiting');
    const checking = [...state.outputStopSequences.values()].some(sequence => sequence.phase === 'checking');
    const actionFeedback = stopping ? waiting ? 'Stop requested. Waiting for earlier Start or Resume requests, then sending Stop again.' : checking ? 'Stop sent. Checking the relay state…' : 'Sending Stop to the device…' : state.outputActionNotice;
    text('output-action-feedback', actionFeedback);
    $('output-action-feedback').hidden = !actionFeedback;
    $('outputs-error').hidden = !(data.error || data.pump_block_reason || data.stop_pending);
    text('outputs-error', data.error || (data.stop_pending ? 'Shutdown is retrying. Check the relay board connection.' : data.pump_block_reason || ''));
    text('water-safety-note', unavailable ? state.outputs ? 'Pump state and overflow checks could not be confirmed. Stop controls remain available.' : 'Waiting for pump state and overflow checks from the device.' : data.pump_block_reason || 'Assigned overflow checks allow pumping. Each pump can run independently with manual Start/Stop.');
    $('water-safety-note').classList.toggle('warning', Boolean(state.outputs && unavailable) || Boolean(data.pump_block_reason));
    text('last-event', unavailable && state.outputs ? `Current relay state unavailable.${data.last_event ? ` Last reported event: ${data.last_event}` : ''}` : data.last_event || 'No relay events yet');
    $('output-summary').replaceChildren();
    for (const output of data.outputs) {
      const channel = output.channel,
        known = typeof output.on === 'boolean';
      const stopSequence = state.outputStopSequences.get('all') || state.outputStopSequences.get(channel);
      const action = pendingActions.findLast(action => !action.channel || action.channel === channel);
      const pending = stopSequence ? 'Stopping…' : action?.label;
      const label = pending || (!known ? 'Unknown' : output.on ? 'On' : 'Off');
      const kind = !known && state.outputs ? 'warning' : output.on ? 'good' : 'neutral';
      badge($(`output-state-${channel}`), label, kind);
      $(`output-card-${channel}`).classList.toggle('is-on', output.on === true);
      const role = state.settings.setup.relay_roles[channel - 1];
      let detail;
      if (!known) detail = state.outputs ? 'Relay state is unavailable. Stop remains available.' : 'Waiting for the relay board';
      else if (unavailable) detail = 'Equipment status needs attention. Timers cannot be confirmed.';
      else if (output.on) detail = finite(output.remaining_seconds) ? `Stops in ${duration(Math.max(0, output.remaining_seconds - elapsed))}` : output.schedule_controlled ? 'On for the scheduled lighting period' : 'Running until you press Stop · no timer';
      else detail = 'Manual Start/Stop · no timer or cooldown';
      if (known && output.blocked_reason) detail = output.blocked_reason;
      text(`output-timing-${channel}`, detail);
      const button = document.querySelector(`[data-output="${channel}"]`);
      const starting = pendingActions.some(action => action.kind === 'start' && action.channel === channel);
      const stop = output.on || !known || starting;
      button.dataset.action = stop ? 'stop' : 'start';
      button.textContent = stopSequence ? 'Stopping…' : !state.outputs ? 'Connecting…' : stop ? 'Stop' : 'Start';
      button.className = `button ${stop ? 'danger-outline' : 'primary'}`;
      button.setAttribute('aria-label', `${stop ? 'Stop' : 'Start'} ${state.settings.setup.relay_labels[channel - 1]}`);
      button.disabled = !state.outputs || Boolean(stopSequence) || (!starting && Boolean(button.dataset.busy)) || (!stop && (stopping || state.outputPending.size > 0 || data.stop_pending || Boolean(output.blocked_reason) || role === 'unassigned' || output.role !== role));
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
    if (!state.settings) updateInitialSettingsAvailability();
    try {
      const settings = await api('/api/settings');
      if (request === state.settingsRequest) showSettings(settings, hydrate);
    } catch (error) {
      if (!state.settings && request === state.settingsRequest) updateInitialSettingsAvailability(true);
      throw error;
    }
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
    const page = currentRoute().page;
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
        for (const id of ['temperature-value', 'humidity-value', 'co2-value']) text(id, '—');
        for (const id of ['pm25-value', 'pm10-value', 'tvoc-value']) text(id, 'Unavailable');
        for (const index of [1, 2]) {
          badge($(`tank${index}-status`), 'Unavailable', 'warning');
          text(`tank${index}-detail`, 'Cannot confirm the current sensor state');
        }
        state.statsConnected = false;
        updateConnection();
        text('updated-at', 'Readings unavailable — retrying');
      }
    } finally {
      if (schedule) setTimeout(() => pollStats(), document.hidden ? 15000 : 5000);
    }
  }
  async function pollPh(schedule = true, afterCalibration = false) {
    // Keep the explicit capture or mutation readback authoritative. Background
    // and focus refreshes must not invalidate it by advancing phRequest.
    if (state.calibrationRefreshing || (state.calibrationPending && !afterCalibration)) {
      if (schedule) setTimeout(() => pollPh(), 5000);
      return;
    }
    const request = ++state.phRequest;
    try {
      if (state.phBusy || state.passwordChanging || state.loginRedirect) return;
      const ph = await api('/api/ph');
      if (request === state.phRequest && !state.phBusy) showPh(ph);
    } catch (error) {
      if (request === state.phRequest && !state.phBusy) {
        state.phError = error.message;
        renderAttention();
        text('ph-value', '—');
        badge($('ph-badge'), 'Unavailable', 'warning');
        text('ph-trend', 'Trend unavailable');
        text('ph-detail', error.message);
        text('ph-sample-time', 'Live reading unavailable');
        text('calibration-live-raw', 'Unavailable');
        text('calibration-live-ph', '—');
        text('calibration-warning', error.message);
        $('calibration-warning').hidden = false;
        if ($('calibration-use-live').checked) {
          $('calibration-raw').value = '';
          $('calibration-ready').checked = false;
        }
        updateCalibrationWorkflow();
        renderChart();
      }
    } finally {
      if (schedule) setTimeout(() => pollPh(), document.hidden ? 60000 : 5000);
    }
  }
  async function pollOutputs(schedule = true, afterStop = false) {
    if (state.outputStopSequences.size && !afterStop) {
      if (schedule) setTimeout(() => pollOutputs(), document.hidden ? 15000 : 1000);
      return;
    }
    const request = ++state.outputsRequest;
    try {
      if (state.outputPending.size || state.passwordChanging || state.loginRedirect) return;
      const outputs = await api('/api/outputs');
      if (request !== state.outputsRequest || state.outputPending.size) return;
      if (!validOutputSnapshot(outputs)) {
        throw new Error('The device returned incomplete relay status.');
      }
      state.outputs = outputs;
      state.outputsReceivedAt = performance.now();
      text('outputs-updated', outputsUnavailable() ? 'Equipment status unavailable · retrying automatically' : `Live relay state · updated ${new Date().toLocaleTimeString()}`);
      renderOutputs();
    } catch (error) {
      if (request !== state.outputsRequest || state.outputPending.size) return;
      const previous = state.outputs;
      state.outputs = {
        ...previous,
        error: error.message,
        outputs: [1, 2, 3, 4].map(channel => ({
          ...previous?.outputs?.find(output => output.channel === channel),
          channel,
          on: null,
          remaining_seconds: null
        })),
        next_watering_in_seconds: null
      };
      renderOutputs();
      text('outputs-updated', 'Connection lost · relay state is unknown · retrying automatically');
      text('outputs-error', error.message);
      $('outputs-error').hidden = false;
    } finally {
      if (schedule) setTimeout(() => pollOutputs(), document.hidden ? 15000 : currentRoute().page === 'outputs' ? 1000 : 3000);
    }
  }
  function makeCameraCards() {
    for (const [id, label, source] of [['ribbon', 'Raspberry Pi camera', 'Ribbon cable'], ['usb', 'USB webcam', 'USB connection']]) {
      const card = document.createElement('article');
      card.className = 'panel camera-card';
      card.innerHTML = `<div class="panel-heading"><div><span class="eyebrow">${source}</span><h2>${label}</h2></div><span class="badge neutral" id="camera-status-${id}" role="status" aria-label="${label} status">Connecting</span></div><div class="camera-viewport" id="camera-viewport-${id}"><p class="camera-placeholder" id="camera-placeholder-${id}">Waiting for the device…</p></div><p class="helper camera-detail" id="camera-detail-${id}">Checking camera availability</p><div class="camera-actions"><button type="button" class="button secondary" id="camera-toggle-${id}">Pause view</button><button type="button" class="button quiet" id="camera-retry-${id}" aria-controls="camera-viewport-${id}" aria-describedby="camera-detail-${id}">Retry view</button><button type="button" class="button quiet" id="camera-fullscreen-${id}">Full screen</button></div>`;
      $('camera-feeds').append(card);
      state.cameraViews[id] = {image: null, paused: false, retrying: false, retryAfter: 0, connectedAt: 0, streamError: false};
      $(`camera-toggle-${id}`).setAttribute('aria-controls', `camera-viewport-${id}`);
      $(`camera-fullscreen-${id}`).setAttribute('aria-label', `Full screen: ${label}`);
      $(`camera-toggle-${id}`).addEventListener('click', () => {
        const view = state.cameraViews[id];
        view.paused = !view.paused;
        view.retryAfter = 0;
        view.streamError = false;
        syncCameras();
        if (!view.paused) pollCameras(false);
      });
      $(`camera-retry-${id}`).addEventListener('click', () => retryCamera(id));
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
  async function retryCamera(id) {
    const view = state.cameraViews[id];
    if (view.paused || view.retrying) return;
    view.retrying = true;
    view.streamError = false;
    disconnectCamera(view);
    syncCameras();
    try {
      await pollCameras(false);
    } finally {
      view.retrying = false;
      view.retryAfter = 0;
      // Pausing during the status check must keep this view disconnected.
      syncCameras();
    }
  }
  function syncCameras() {
    const watching = state.csrf && !document.hidden && currentRoute().page === 'cameras';
    for (const [id, view] of Object.entries(state.cameraViews)) {
      const camera = state.cameras.find(item => item.id === id);
      const available = camera && camera.detected;
      const show = watching && !view.paused && !view.retrying && available && !state.cameraError;
      if (!show || Date.now() - view.connectedAt > 240000) disconnectCamera(view);
      if (show && !view.image && Date.now() >= view.retryAfter) {
        const image = new Image();
        image.alt = camera.label;
        image.className = 'camera-stream';
        view.image = image;
        view.connectedAt = Date.now();
        // The device's Live flag describes the shared encoder. This browser
        // must also have received an image before its own view can be Live.
        // MJPEG may expose dimensions before firing a load event, so status
        // polling checks both; failed or stalled connections are retried.
        const reconnect = () => {
          if (view.image !== image) return;
          disconnectCamera(view);
          view.retryAfter = Date.now() + 3000;
          view.streamError = true;
          syncCameras();
        };
        image.onerror = reconnect;
        image.onload = () => { if (view.image === image) syncCameras(); };
        image.src = `${camera.stream_url}?connection=${Date.now()}`;
        $(`camera-viewport-${id}`).prepend(image);
      }
      if (view.image && camera && (!camera.live || !view.image.naturalWidth) && Date.now() - view.connectedAt > 20000) {
        disconnectCamera(view);
        view.retryAfter = Date.now() + 3000;
        view.streamError = true;
      }
      const toggle = $(`camera-toggle-${id}`);
      toggle.textContent = view.paused ? 'Resume view' : 'Pause view';
      toggle.setAttribute('aria-pressed', String(view.paused));
      toggle.setAttribute('aria-label', `${view.paused ? 'Resume view' : 'Pause view'}: ${id === 'usb' ? 'USB webcam' : 'Raspberry Pi camera'}`);
      const retry = $(`camera-retry-${id}`);
      const retryLabel = view.retrying ? 'Checking…' : available ? 'Retry view' : 'Check again';
      retry.textContent = retryLabel;
      retry.setAttribute('aria-label', `${retryLabel}: ${id === 'usb' ? 'USB webcam' : 'Raspberry Pi camera'}`);
      retry.disabled = view.paused || view.retrying;
      const live = Boolean(show && view.image?.naturalWidth > 0 && camera.live);
      if (live) view.streamError = false;
      $(`camera-fullscreen-${id}`).disabled = !live;
      const placeholder = $(`camera-placeholder-${id}`);
      placeholder.hidden = live;
      const missingHelp = id === 'usb' ? 'USB webcam not detected. Check its USB connection and camera compatibility.' : 'Ribbon camera not detected. Check the camera model, cable, and boot configuration.';
      const detail = view.paused ? 'This view is paused. Resume view to reconnect.' : view.retrying ? 'Checking camera availability before reconnecting…' : state.cameraError ? `${state.cameraError} Retrying automatically, or choose ${retryLabel} to check now.` : !camera ? 'Checking camera availability…' : !available ? `${camera.error || missingHelp} Choose Check again after reconnecting the camera.` : view.streamError ? 'The live view was interrupted. Reconnecting automatically, or choose Retry view to try now.' : camera.error || (live ? `Live · ${camera.width} × ${camera.height} · up to ${camera.fps} fps · last frame ${new Date(camera.timestamp_ms).toLocaleTimeString()}` : camera.live ? 'Waiting for this browser to receive its first frame…' : 'Starting the camera. The view reconnects automatically.');
      text(`camera-detail-${id}`, detail);
      const label = view.paused ? 'Paused' : view.retrying ? 'Checking' : state.cameraError ? 'Offline' : !camera ? 'Connecting' : !available ? 'Not detected' : view.streamError ? 'Reconnecting' : live ? 'Live' : camera.error ? 'Reconnecting' : 'Starting';
      badge($(`camera-status-${id}`), label, label === 'Live' ? 'good' : ['Offline', 'Not detected', 'Reconnecting'].includes(label) ? 'warning' : 'neutral');
      placeholder.textContent = view.paused ? 'View paused' : view.retrying ? 'Checking camera…' : state.cameraError ? 'Connection lost · retrying…' : !available ? 'Camera unavailable' : view.streamError || camera.error ? 'Reconnecting…' : 'Waiting for live image…';
    }
  }
  async function pollCameras(schedule = true) {
    try {
      if (!state.csrf || state.passwordChanging || state.loginRedirect || document.hidden || currentRoute().page !== 'cameras') return;
      // Focus, automatic polling, and per-view retries can share one status
      // request without discarding the answer a pending retry is waiting for.
      if (!state.camerasPending) {
        state.camerasPending = (async () => {
          try {
            const cameras = await api('/api/cameras');
            if (!Array.isArray(cameras) || ['ribbon', 'usb'].some(id => cameras.filter(camera => camera?.id === id).length !== 1) || cameras.some(camera => !camera || typeof camera.id !== 'string' || typeof camera.detected !== 'boolean' || typeof camera.live !== 'boolean' || camera.detected && (typeof camera.stream_url !== 'string' || !camera.stream_url))) {
              throw new Error('Camera status is unavailable.');
            }
            state.cameras = cameras;
            state.cameraError = null;
            $('cameras-error').hidden = true;
          } catch (error) {
            state.cameraError = error.message;
            text('cameras-error', error.message);
            $('cameras-error').hidden = false;
          } finally {
            state.camerasPending = null;
            syncCameras();
          }
        })();
      }
      await state.camerasPending;
    } finally {
      if (schedule) setTimeout(() => pollCameras(), document.hidden ? 15000 : 3000);
    }
  }

  function reportRangeChanged() {
    const range = $('report-range').value;
    $('report-custom-range').hidden = range !== 'custom';
    $('report-current').disabled = range === 'current';
    $('report-current').checked = range === 'current' || state.reportIncludeCurrent;
    for (const id of ['report-start', 'report-end']) {
      $(id).disabled = range !== 'custom';
      $(id).required = range === 'custom';
      $(id).setCustomValidity('');
      $(id).removeAttribute('aria-invalid');
    }
    reportSelectionChanged();
  }
  function reportSelectionSnapshot(kind = 'csv') {
    const range = $('report-range');
    const options = {
      range: range.value,
      current: $('report-current').checked,
      start: range.value === 'custom' ? $('report-start').value : null,
      end: range.value === 'custom' ? $('report-end').value : null
    };
    let period = range.selectedOptions[0].textContent;
    if (range.value === 'custom') {
      const start = new Date(options.start);
      const end = new Date(options.end);
      const format = {dateStyle: 'medium', timeStyle: 'short'};
      if (finite(start.getTime()) && finite(end.getTime()) && end > start) period = `${start.toLocaleString([], format)} to ${end.toLocaleString([], format)}`;
    }
    const snapshot = range.value === 'current' ? '' : options.current ? ' · includes current readings' : ' · historical readings only';
    // Titles and notes belong to HTML reports; raw downloads contain data only.
    if (kind === 'html') {
      options.title = $('report-title').value.trim() || 'Air quality report';
      options.notes = $('report-notes').value.trim();
    }
    return {key: JSON.stringify(options), label: `${kind === 'html' ? `“${options.title}” · ` : ''}${period}${snapshot}`};
  }
  function reportSelectionChanged() {
    if (!state.reportBusy) {
      message('report-result', '');
      message('export-result', '');
    }
    if ($('report-selection-summary')) text('report-selection-summary', `CSV and JSON use: ${reportSelectionSnapshot().label}.`);
  }
  function reportFieldError(id, value) {
    const input = $(id);
    input.setCustomValidity(value);
    input.setAttribute('aria-invalid', 'true');
    input.reportValidity();
    throw new Error(value);
  }
  function reportParameters() {
    const range = $('report-range').value;
    const params = new URLSearchParams();
    for (const id of ['report-start', 'report-end']) {
      $(id).setCustomValidity('');
      $(id).removeAttribute('aria-invalid');
    }
    if (range === 'current') params.set('scope', 'current');
    else if (range === 'custom') {
      const start = new Date($('report-start').value).getTime() / 1000;
      const end = new Date($('report-end').value).getTime() / 1000;
      if (!finite(start)) reportFieldError('report-start', 'Choose a start date and time.');
      if (!finite(end)) reportFieldError('report-end', 'Choose an end date and time.');
      if (start < 0 || start >= Date.now() / 1000) reportFieldError('report-start', 'Choose a start time in the past, on or after January 1, 1970.');
      if (start >= end) reportFieldError('report-end', 'The end time must be after the start time.');
      if (end - start > 7 * 86400) reportFieldError('report-end', 'Choose a period of up to 7 days.');
      if (end > Date.now() / 1000 + 60) reportFieldError('report-end', 'The end time must be in the past.');
      params.set('start', String(Math.floor(start)));
      params.set('end', String(Math.floor(end)));
    } else params.set('hours', range);
    params.set('current', $('report-current').checked ? '1' : '0');
    params.set('title', $('report-title').value.trim() || 'Air quality report');
    params.set('notes', $('report-notes').value.trim());
    return params;
  }
  async function downloadExport(kind, button, preview = false) {
    if (state.reportBusy) return;
    const result = kind === 'html' ? 'report-result' : 'export-result';
    let params;
    let submitted;
    let previewWindow;
    try {
      params = reportParameters();
      submitted = reportSelectionSnapshot(kind);
      if (preview) {
        // Open in the click handler to preserve the browser's user activation.
        previewWindow = window.open('about:blank', '_blank');
        if (!previewWindow) throw new Error('Your browser blocked the report tab. Allow pop-ups for this device, or choose Download report.');
        previewWindow.opener = null;
        previewWindow.document.title = 'Preparing your report · AOG';
        previewWindow.document.body.textContent = 'Preparing your air quality report…';
      }
    } catch (error) {
      if (previewWindow && !previewWindow.closed) previewWindow.close();
      message(result, error.message, true);
      return;
    }
    const originalLabel = button.textContent;
    const abort = new AbortController();
    const timeout = setTimeout(() => abort.abort(), 30000);
    state.reportBusy = true;
    const actions = ['report-preview', 'report-html', 'export-csv', 'export-json'].map($);
    actions.forEach(action => { action.disabled = true; });
    button.setAttribute('aria-busy', 'true');
    button.textContent = 'Preparing…';
    message(result, `${preview ? 'Preparing your report in a new tab…' : 'Preparing your download…'} Selected: ${submitted.label}. You can edit options for your next export.`);
    const finishMessage = (value, error = false) => {
      // Read without validation: a newer unfinished custom period must not
      // steal focus or change the outcome of an already submitted request.
      const changed = reportSelectionSnapshot(kind).key !== submitted.key;
      const next = changed ? error
        ? ' Your options changed during preparation and are preserved for your next attempt.'
        : ' Your options changed during preparation. This file uses the earlier selection; export again to include your current choices.' : '';
      message(result, `${value} ${error ? 'Requested' : 'Prepared'} for: ${submitted.label}.${next}`, error);
    };
    try {
      const endpoint = kind === 'html' ? 'report' : 'export';
      params.set('format', kind);
      if (!preview) params.set('download', '1');
      const response = await fetch(`/api/air-quality/${endpoint}?${params}`, {credentials: 'same-origin', cache: 'no-store', signal: abort.signal});
      if (response.status === 401) {
        redirectToLogin();
        throw new Error('Your session expired. Sign in again.');
      }
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body?.error || `The ${preview ? 'report' : 'download'} could not be prepared (${response.status}). Try again.`);
      }
      const contentType = (response.headers.get('Content-Type') || '').split(';')[0].trim().toLowerCase();
      const expectedType = {html: 'text/html', csv: 'text/csv', json: 'application/json'}[kind];
      if (contentType !== expectedType) throw new Error('The device returned an unexpected file. Try again.');
      const blob = await response.blob();
      if (!blob.size) throw new Error('The device returned an empty file. Try again.');
      const url = URL.createObjectURL(blob);
      if (preview) {
        if (previewWindow.closed) {
          URL.revokeObjectURL(url);
          throw new Error('The report tab was closed. Choose Open report to try again.');
        }
        previewWindow.location.replace(url);
        // Keep the standalone report URL valid for printing and reloading.
        const release = setInterval(() => {
          if (!previewWindow.closed) return;
          URL.revokeObjectURL(url);
          clearInterval(release);
        }, 10000);
        finishMessage('Report opened in a new tab. Use Print / Save as PDF there to save a PDF copy.');
      } else {
        const link = document.createElement('a');
        link.href = url;
        link.download = (response.headers.get('Content-Disposition') || '').match(/filename="([^"]+)"/)?.[1] || `aog-air-quality.${kind}`;
        document.body.append(link);
        link.click();
        link.remove();
        setTimeout(() => URL.revokeObjectURL(url), 10000);
        finishMessage(`Your ${kind.toUpperCase()} download is ready.`);
      }
    } catch (error) {
      if (previewWindow && !previewWindow.closed) previewWindow.close();
      finishMessage(error.name === 'AbortError' ? `The ${preview ? 'report' : 'download'} took too long. Try again.` : error instanceof TypeError ? 'Could not reach the device. Check your connection and try again.' : error.message, true);
    } finally {
      clearTimeout(timeout);
      state.reportBusy = false;
      actions.forEach(action => { action.disabled = false; });
      button.removeAttribute('aria-busy');
      button.textContent = originalLabel;
    }
  }
  function prepareReportControls() {
    const localInputTime = date => new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
    $('report-start').value = localInputTime(new Date(Date.now() - 86400000));
    $('report-end').value = localInputTime(new Date());
    state.reportIncludeCurrent = $('report-current').checked;
    $('report-range').addEventListener('change', reportRangeChanged);
    $('report-current').addEventListener('change', () => {
      state.reportIncludeCurrent = $('report-current').checked;
      reportSelectionChanged();
    });
    for (const id of ['report-start', 'report-end', 'report-title', 'report-notes']) {
      $(id).addEventListener('input', () => {
        const related = ['report-start', 'report-end'].includes(id) ? ['report-start', 'report-end'] : [id];
        related.forEach(field => {
          $(field).setCustomValidity('');
          $(field).removeAttribute('aria-invalid');
        });
        reportSelectionChanged();
      });
    }
    $('history-to-reports').addEventListener('click', () => {
      $('report-range').value = String(state.historyRange);
      reportRangeChanged();
    });
    $('report-preview').addEventListener('click', event => downloadExport('html', event.currentTarget, true));
    for (const [id, kind] of [['export-csv', 'csv'], ['export-json', 'json'], ['report-html', 'html']]) {
      $(id).addEventListener('click', event => downloadExport(kind, event.currentTarget));
    }
    reportRangeChanged();
  }

  function invalidFields(message, fields) {
    const error = new Error(message);
    error.fields = fields;
    return error;
  }

  function clearFormErrors(form) {
    form.querySelectorAll('[data-form-error]').forEach(field => {
      const descriptions = (field.getAttribute('aria-describedby') || '').split(' ').filter(id => id && id !== field.dataset.formError);
      if (descriptions.length) field.setAttribute('aria-describedby', descriptions.join(' '));
      else field.removeAttribute('aria-describedby');
      field.removeAttribute('aria-invalid');
      delete field.dataset.formError;
    });
  }

  function validateForm(id) {
    if (state.formConflicts.get(id)?.size) throw new Error('Review changed device values before saving this section.');
    const emptyNames = [...$(id).querySelectorAll('input[required]:not([type])')].filter(field => !field.value.trim());
    if (emptyNames.length) throw invalidFields('Enter a name or label using at least one visible character.', emptyNames.map(field => field.id));
    if (id === 'setup-form') {
      const fields = [1, 2, 3, 4].map(channel => $(`relay-role-${channel}`));
      const duplicate = fields.find((field, index) => field.value !== 'unassigned' && fields.slice(0, index).some(previous => previous.value === field.value));
      if (duplicate) throw invalidFields(`${roleNames[duplicate.value]} is assigned more than once. Choose a different function or leave a relay unassigned.`, fields.filter(field => field.value === duplicate.value).map(field => field.id));
    }
    if (id === 'schedule-form') {
      if ($('lighting-enabled').checked && !relayFor('lights_air')) throw invalidFields('Assign and save a Lights + Air relay in Setup, or disable its schedule.', ['lighting-enabled']);
      const waterRole = $('watering-pump').value;
      if ($('watering-enabled').checked && !relayFor(waterRole)) throw invalidFields(`Assign and save a ${roleNames[waterRole]} relay in Setup, or disable its schedule.`, ['watering-pump']);
    }
    if (id === 'ph-settings-form') {
      const fields = ['ph-critical-min', 'ph-min', 'ph-max', 'ph-critical-max'];
      for (let index = 1; index < fields.length; index++) {
        if (Number($(fields[index - 1]).value) >= Number($(fields[index]).value)) {
          throw invalidFields('pH limits must increase: critical low < target minimum < target maximum < critical high.', [fields[index - 1], fields[index]]);
        }
      }
    }
    if (id === 'password-form') {
      if ($('new-password').value !== $('confirm-password').value) throw invalidFields('New passwords do not match.', ['confirm-password']);
      const length = Array.from($('new-password').value).length;
      if (length < 12 || length > 128) throw invalidFields('Use 12–128 characters for your new password.', ['new-password']);
    }
  }

  function form(id, result, handler) {
    $(id).addEventListener('submit', async event => {
      event.preventDefault();
      if ($(id).dataset.saving) return;
      $(id).dataset.saving = 'true';
      updateFormState(id);
      const version = $(id).dataset.editVersion;
      const submittedRoute = location.hash;
      const button = $(id).querySelector('[type=submit]');
      const label = button.textContent;
      button.disabled = true;
      button.textContent = id === 'calibration-form' ? 'Applying…' : 'Saving…';
      $(id).setAttribute('aria-busy', 'true');
      clearFormErrors($(id));
      message(result, '');
      try {
        if (!state.settings || !state.csrf) throw new Error('Wait for the device to connect, then try again.');
        validateForm(id);
        const reply = await handler();
        if ($(id).dataset.editVersion === version) state.dirtyForms.delete(id);
        showSettings(state.settings, true);
        if (reply) message(result, state.formBaselines.has(id) && state.dirtyForms.has(id) ? 'Earlier changes saved. Your newer edits are still unsaved.' : reply);
      } catch (error) {
        message(result, error.message, true);
        // Do not move focus away from a newer draft or a different page while
        // an asynchronous save completes in the background.
        const focusStillHere = $(id).contains(document.activeElement) || document.activeElement === document.body;
        if ($(id).dataset.editVersion === version && location.hash === submittedRoute && focusStillHere && !$(id).closest('.page').hidden) {
          const fields = (error.fields || []).map($);
          fields.forEach(field => {
            field.setAttribute('aria-invalid', 'true');
            field.dataset.formError = result;
            const descriptions = new Set((field.getAttribute('aria-describedby') || '').split(' ').filter(Boolean));
            descriptions.add(result);
            field.setAttribute('aria-describedby', [...descriptions].join(' '));
          });
          const target = fields[0] || $(result);
          target.focus({preventScroll: true});
          target.scrollIntoView({block: 'center', behavior: 'instant'});
        }
      } finally {
        delete $(id).dataset.saving;
        $(id).removeAttribute('aria-busy');
        updateFormAvailability();
        updateFormState(id);
        button.textContent = label;
        updateCalibrationWorkflow();
      }
    });
  }
  async function saveSettings(patch, formId) {
    if (state.settingsBusy) throw new Error('Another settings change is being saved. Try again when it finishes.');
    state.settingsBusy = true;
    ++state.settingsRequest;
    try {
      const settings = await api('/api/settings', {body: patch});
      showSettings(settings, false, formId);
    } finally {
      ++state.settingsRequest;
      state.settingsBusy = false;
    }
    await Promise.all([pollOutputs(false), pollPh(false)]);
    return 'Saved. Changes are active on the device.';
  }
  async function sendOutputAction(path, body = {}) {
    const request = ++state.outputActionRequest;
    const kind = path.endsWith('/start') ? 'start' : path.endsWith('/resume') ? 'resume' : 'stop';
    let settle;
    const action = {kind, channel: body.channel || null, energizing: kind !== 'stop', label: kind === 'start' ? 'Starting…' : kind === 'resume' ? 'Applying…' : 'Stopping…', settled: new Promise(resolve => { settle = resolve; })};
    let confirmed = false;
    ++state.outputsRequest;
    state.outputPending.set(request, action);
    renderOutputs();
    try {
      const outputs = await api(path, {body});
      if (!validOutputSnapshot(outputs)) throw new Error('The device returned incomplete relay status.');
      confirmed = true;
      ++state.outputsRequest;
      if (request === state.outputActionRequest) {
        state.outputs = outputs;
        state.outputsReceivedAt = performance.now();
        text('outputs-updated', `Device confirmed · ${new Date().toLocaleTimeString()}`);
        renderOutputs();
      }
      return outputs;
    } catch (error) {
      error.outputAction = action;
      throw error;
    } finally {
      state.outputPending.delete(request);
      settle({confirmed});
      renderOutputs();
    }
  }

  async function outputAction(path, body = {}) {
    const energizing = path.endsWith('/start') || path.endsWith('/resume');
    if (energizing && state.outputStopSequences.size) throw new Error('Wait for the requested stop and relay check to finish.');
    if (energizing) {
      state.outputActionNotice = '';
      try {
        const outputs = await sendOutputAction(path, body);
        if (!state.outputStopSequences.size) flash(outputs.last_event);
        return outputs;
      } catch (error) {
        // The stop sequence reports an uncertain earlier command with the
        // final stop result, instead of an old Start error replacing it.
        if (!error.outputAction?.stopRequested) throw error;
      } finally {
        await pollOutputs(false);
      }
      return;
    }
    const key = body.channel || 'all';
    if (state.outputStopSequences.has(key)) return state.outputStopSequences.get(key).promise;
    state.outputActionNotice = '';
    const earlier = [...state.outputPending.values()].filter(action => action.energizing && (key === 'all' || !action.channel || action.channel === key));
    earlier.forEach(action => { action.stopRequested = true; });
    const sequence = {phase: 'sending', promise: null};
    state.outputStopSequences.set(key, sequence);
    sequence.promise = (async () => {
      let result;
      let failure;
      try {
        // Send Stop immediately, then repeat it once older energizing requests
        // settle. Aborting a browser request cannot cancel a device command.
        try { result = await sendOutputAction(path, body); } catch (error) { failure = error; }
        if (earlier.length) {
          sequence.phase = 'waiting';
          renderOutputs();
          const outcomes = await Promise.all(earlier.map(action => action.settled));
          if (outcomes.some(outcome => !outcome.confirmed)) state.outputActionNotice = 'Stop was requested after an earlier Start or Resume could not be confirmed. A timed-out request may still reach the device; check the live relay states and use Stop again if needed.';
          sequence.phase = 'sending';
          try { result = await sendOutputAction(path, body); failure = null; } catch (error) { failure = error; }
        }
        if (failure) state.outputActionNotice = 'The stop request could not be confirmed. Check the live relay states and try Stop again.';
        sequence.phase = 'checking';
        renderOutputs();
        await pollOutputs(false, true);
        if (failure) throw failure;
        const stopped = !outputsUnavailable() && state.outputs.automation_paused && (key === 'all' ? state.outputs.outputs.every(output => output.on === false) : state.outputs.outputs.find(output => output.channel === key)?.on === false);
        if (!stopped && !state.outputActionNotice) state.outputActionNotice = 'Stop was sent, but the latest relay state has not confirmed the requested stop. Check the live state and try Stop again if needed.';
        if (!state.outputActionNotice) flash(result.last_event);
        return result;
      } finally {
        state.outputStopSequences.delete(key);
        renderOutputs();
      }
    })();
    return sequence.promise;
  }
  async function calibrationAction(path, body) {
    if (state.phBusy || state.calibrationPending) throw new Error('A calibration change is already being applied.');
    state.calibrationPending = true;
    state.phBusy = true;
    ++state.phRequest;
    updateCalibrationWorkflow();
    try {
      const snapshot = await api(path, {body});
      ++state.phRequest;
      showPh(snapshot);
      flash(snapshot.message);
      return snapshot.message;
    } finally {
      state.phBusy = false;
      try {
        await pollPh(false, true);
      } finally {
        state.calibrationPending = false;
        updateCalibrationWorkflow();
      }
    }
  }
  async function actionButton(button, action) {
    if (button.dataset.busy) return;
    button.dataset.busy = 'true';
    button.disabled = true;
    globalError('');
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
    const edited = event => {
      if (event.target.id === 'show-passwords') return;
      state.dirtyForms.add(element.id);
      element.dataset.editVersion = String(Number(element.dataset.editVersion || 0) + 1);
      updateFormState(element.id);
      clearFormErrors(element);
      const result = element.querySelector('.form-result');
      if (result) result.textContent = '';
    };
    element.addEventListener('input', edited);
    element.addEventListener('change', edited);
  });
  document.querySelector('.skip-link').addEventListener('click', event => {
    event.preventDefault();
    setNavigationOpen(false);
    $('main').focus({preventScroll: true});
    $('main').scrollIntoView({block: 'start', behavior: 'instant'});
  });
  $('navigation-toggle')?.addEventListener('click', () => {
    setNavigationOpen($('navigation-toggle').getAttribute('aria-expanded') !== 'true');
  });
  window.matchMedia('(max-width: 780px)').addEventListener('change', event => {
    const focusedNavigation = $('primary-navigation')?.contains(document.activeElement)
      || document.querySelector('.sidebar-bottom')?.contains(document.activeElement);
    setNavigationOpen(false);
    if (event.matches && focusedNavigation) $('navigation-toggle')?.focus();
    else if (!event.matches && document.activeElement === $('navigation-toggle')) document.querySelector('[data-page][aria-current="page"]')?.focus();
  });
  document.querySelector('.sidebar').addEventListener('click', event => {
    const link = event.target.closest('a[href^="#"]');
    if (!link) return;
    setNavigationOpen(false);
    if (link.hash === location.hash || (link.hash === '#overview' && !location.hash)) {
      event.preventDefault();
      $('page-title').focus({preventScroll: true});
      window.scrollTo({top: 0, behavior: 'instant'});
    }
  });
  document.querySelectorAll('[data-settings-section]').forEach(link => link.addEventListener('click', event => {
    if (link.hash !== location.hash) return;
    event.preventDefault();
    focusSettingsSection(link.dataset.settingsSection);
  }));
  document.querySelectorAll('[data-review-form]').forEach(link => link.addEventListener('click', event => {
    if (link.hash !== location.hash) return;
    event.preventDefault();
    const {page, section} = currentRoute();
    if (page === 'settings' && section) focusSettingsSection(section);
    else {
      $('page-title').focus({preventScroll: true});
      window.scrollTo({top: 0, behavior: 'instant'});
    }
  }));
  document.querySelectorAll('[data-history-sensor]').forEach(link => link.addEventListener('click', event => {
    if (link.hash !== location.hash) return;
    event.preventDefault();
    focusHistorySelection();
  }));
  document.querySelectorAll('a[href="#overview/ph"]').forEach(link => link.addEventListener('click', event => {
    if (link.hash !== location.hash) return;
    event.preventDefault();
    focusPhHistory();
  }));
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && $('navigation-toggle')?.getAttribute('aria-expanded') === 'true') {
      setNavigationOpen(false);
      $('navigation-toggle').focus();
    }
  });
  document.querySelectorAll('[data-discard-form]').forEach(button => button.addEventListener('click', () => {
    const id = button.dataset.discardForm;
    if (!state.settings || $(id).dataset.saving) return;
    state.dirtyForms.delete(id);
    state.formConflicts.delete(id);
    clearFormErrors($(id));
    showSettings(state.settings, true);
    const result = $(id).querySelector('.form-result');
    if (result) result.textContent = '';
    $(id).querySelector('[type=submit]').focus({preventScroll: true});
  }));
  window.addEventListener('beforeunload', event => {
    if (!state.loginRedirect && pendingSettingForms().length) {
      event.preventDefault();
      event.returnValue = '';
    }
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
    if (!document.hidden && state.outputs && ['outputs', 'schedules'].includes(currentRoute().page)) renderOutputs();
  }, 1000);
  document.querySelectorAll('[data-history-range]').forEach(button => button.addEventListener('click', () => {
    if (state.historyRange === Number(button.dataset.historyRange)) return;
    state.historyRange = Number(button.dataset.historyRange);
    state.history = null;
    state.historyError = null;
    state.historyLoadedAt = null;
    document.querySelectorAll('[data-history-range]').forEach(item => item.setAttribute('aria-pressed', String(item === button)));
    pollHistory(false);
  }));
  bindChartInspection($('ph-chart-svg'), $('chart-tooltip'), chartTooltip);
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
    const channel = Number(button.dataset.output);
    globalError('');
    outputAction(`/api/outputs/${button.dataset.action}`, {channel}).catch(error => globalError(error.message));
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
  }, 'schedule-form'));
  form('setup-form', 'setup-result', async () => {
    const roles = [1, 2, 3, 4].map(index => $(`relay-role-${index}`).value);
    const changed = roles.some((role, index) => role !== state.settings.setup.relay_roles[index]);
    const reply = await saveSettings({
      setup: {
        setup_name: $('setup-name-input').value.trim(),
        relay_labels: [1, 2, 3, 4].map(index => $(`relay-label-${index}`).value.trim()),
        relay_roles: roles
      }
    }, 'setup-form');
    await pollOutputs(false);
    return changed ? 'Assignments saved. Schedules are paused; resume them when you are ready.' : reply;
  });
  form('pump-form', 'pump-result', () => saveSettings({
    pump: {
      ...state.settings.pump,
      safety_gpio_pin: $('safety-enabled').checked ? Number($('safety-pin').value) : null
    }
  }, 'pump-form'));
  form('overflow-form', 'overflow-result', async () => {
    const overflow = {
      sensor_labels: [1, 2].map(index => $(`overflow-label-${index}`).value.trim()),
      ...Object.fromEntries(waterRoles.map(role => [role, sensorSelection($(`overflow-${role}`).value)]))
    };
    await saveSettings({setup: {...state.settings.setup, overflow}}, 'overflow-form');
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
    }, 'ph-settings-form');
    await pollPh(false);
    return reply;
  });
  $('use-raw-reading').addEventListener('click', async () => {
    if (state.phBusy || state.calibrationPending || state.calibrationRefreshing || $('calibration-form').dataset.saving) return;
    state.calibrationRefreshing = true;
    updateCalibrationWorkflow();
    message('calibration-result', '');
    const request = ++state.phRequest;
    try {
      const data = await api('/api/ph');
      if (state.phBusy || request !== state.phRequest) return;
      showPh(data);
      if (!finite(data.raw_value)) throw new Error(data.error || 'A fresh raw reading is unavailable. Check the probe connection.');
      $('calibration-raw').value = data.raw_value;
      $('calibration-ready').checked = false;
      message('calibration-result', $('calibration-use-live').checked ? 'Fresh reading loaded. Confirm the probe has settled before saving.' : 'Current raw reading copied. Confirm it was measured in the selected buffer before saving.');
    } catch (error) {
      if (request === state.phRequest) {
        state.phError = error.message;
        text('calibration-live-raw', 'Unavailable');
        text('calibration-live-ph', '—');
        text('calibration-warning', error.message);
        $('calibration-warning').hidden = false;
        if ($('calibration-use-live').checked) $('calibration-raw').value = '';
        message('calibration-result', error.message, true);
      }
    } finally {
      state.calibrationRefreshing = false;
      updateCalibrationWorkflow();
    }
  });
  $('calibration-use-live').addEventListener('change', () => {
    $('calibration-raw').readOnly = $('calibration-use-live').checked;
    $('calibration-ready').checked = false;
    updateCalibrationWorkflow();
    if ($('calibration-use-live').checked) pollPh(false);
  });
  $('buffer-ph').addEventListener('change', () => {
    $('calibration-ready').checked = false;
    updateCalibrationWorkflow();
  });
  $('calibration-raw').addEventListener('input', () => {
    $('calibration-ready').checked = false;
    updateCalibrationWorkflow();
  });
  $('calibration-ready').addEventListener('change', updateCalibrationWorkflow);
  document.querySelectorAll('[data-remove-calibration]').forEach(button => button.addEventListener('click', async () => {
    if (state.phBusy || state.calibrationPending || state.calibrationRefreshing || $('calibration-form').dataset.saving) return;
    const route = location.hash;
    button.textContent = 'Removing…';
    button.setAttribute('aria-busy', 'true');
    message('calibration-result', '');
    let removed = false;
    try {
      const reply = await calibrationAction('/api/ph/calibration/remove', {buffer_ph: Number(button.dataset.removeCalibration)});
      $('calibration-ready').checked = false;
      message('calibration-result', reply);
      removed = true;
    } catch (error) {
      message('calibration-result', error.message, true);
    } finally {
      button.textContent = 'Remove';
      button.removeAttribute('aria-busy');
      updateCalibrationWorkflow();
      if (removed && location.hash === route && [button, document.body].includes(document.activeElement)) $('buffer-ph').focus({preventScroll: true});
    }
  }));
  form('calibration-form', 'calibration-result', async () => {
    const body = {buffer_ph: Number($('buffer-ph').value)};
    if ($('calibration-use-live').checked && (state.phError || !finite(state.ph?.raw_value))) throw invalidFields('A fresh raw reading is unavailable. Refresh the sensor reading before saving.', ['calibration-raw']);
    if (!$('calibration-use-live').checked) body.raw_value = Number($('calibration-raw').value);
    const reply = await calibrationAction('/api/ph/calibrate', body);
    $('calibration-ready').checked = false;
    return reply;
  });
  form('password-form', 'password-result', async () => {
    const body = {
      current_password: $('current-password').value,
      new_password: $('new-password').value,
      confirm_password: $('confirm-password').value
    };
    const controls = ['current-password', 'new-password', 'confirm-password', 'show-passwords'].map(id => ({field: $(id), disabled: $(id).disabled}));
    // The server applies one Unicode-aware strength policy to web and console
    // changes. Do not reject valid non-ASCII letters with an ASCII-only regex.
    state.passwordChanging = true;
    state.sessionExpired = false;
    controls.forEach(({field}) => { field.disabled = true; });
    try {
      await api('/api/account/password', {body});
      $('password-form').reset();
      redirectToLogin('changed');
      return 'Password changed. Sign in again.';
    } finally {
      controls.forEach(({field, disabled}) => { field.disabled = disabled; });
      state.passwordChanging = false;
      if (state.sessionExpired && !state.loginRedirect) redirectToLogin();
    }
  });
  $('refresh-button').addEventListener('click', event => actionButton(event.currentTarget, async () => {
    globalError('');
    if (!state.csrf) {
      state.csrf = (await api('/api/session')).csrf_token;
    }
    const results = await Promise.allSettled([loadSettings(), pollStats(false), pollPh(false), pollHistory(false), pollOutputs(false), pollCameras(false)]);
    updateFormAvailability();
    // A settings failure must not prevent an explicit retry of live readings.
    if (results[0].status === 'rejected') throw results[0].reason;
  }));
  async function boot() {
    updateInitialSettingsAvailability();
    document.querySelectorAll('form [type=submit]').forEach(button => {
      button.disabled = true;
    });
    try {
      const session = await api('/api/session');
      state.csrf = session.csrf_token;
      await loadSettings();
      updateFormAvailability();
    } catch (error) {
      if (!state.settings) updateInitialSettingsAvailability(true);
      globalError(`Could not load setup: ${error.message} Use Refresh to try again.`, 'settings-load');
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
