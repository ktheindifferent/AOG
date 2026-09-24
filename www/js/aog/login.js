'use strict';
(() => {
  const query = new URLSearchParams(location.search);
  const notice = document.getElementById('login-notice');
  const form = document.getElementById('login-form');
  const username = document.getElementById('input_username');
  const password = document.getElementById('input_password');
  const revealPassword = document.getElementById('show-password');
  const capsLockHint = document.getElementById('caps-lock-hint');
  const button = form.querySelector('button[type="submit"]');
  const buttonLabel = button.innerHTML;
  const error = document.getElementById('login-error');
  // Return values name a known view, never a URL. Keep sign-in navigation on
  // this device and avoid retaining credentials or unfinished form contents.
  const returnViews = {
    overview: 'Grow overview', 'overview/ph': 'pH history',
    connections: 'Connections & firmware', 'history/water_temp_d6': 'Water D6 history', 'history/water_temp_d8': 'Water D8 history',
    history: 'Sensor history', 'history/co2': 'CO₂ history',
    'history/tvoc': 'TVOC history', 'history/pm25': 'PM2.5 history',
    'history/pm10': 'PM10 history', 'history/temp': 'Temperature history',
    'history/hum': 'Humidity history', cameras: 'Live cameras',
    reports: 'Reports & exports', schedules: 'Lighting & watering',
    outputs: 'Equipment controls', settings: 'Setup settings',
    'settings/relays': 'Relays & labels', 'settings/overflow': 'Overflow stops',
    'settings/safety': 'Pump safety', 'settings/ph': 'pH settings',
    'settings/calibration': 'Probe calibration', account: 'Your account'
  };
  // A bookmarked dashboard fragment survives the server's sign-in redirect.
  // An explicit return parameter takes precedence, including invalid values.
  const requestedView = query.get('return') ?? location.hash.slice(1);
  const returnView = !query.has('changed') && Object.prototype.hasOwnProperty.call(returnViews, requestedView) ? requestedView : null;
  const destination = `/index.html${returnView ? `#${returnView}` : ''}`;
  if (query.has('changed') || query.has('expired')) {
    notice.textContent = query.has('changed') ? 'Password changed. Sign in with your new password.' : returnView ? `Your session expired. Sign in again to return to ${returnViews[returnView]}.` : 'Your session expired. Sign in again to continue.';
    if (!query.has('changed') && query.has('expired') && query.get('unsaved') === '1') {
      notice.textContent += ' Your unsaved edits were not saved. Enter them again after signing in.';
    }
    notice.hidden = false;
  } else if (returnView) {
    notice.textContent = `Sign in to continue to ${returnViews[returnView]}.`;
    notice.hidden = false;
  }
  revealPassword.addEventListener('change', event => {
    password.type = event.target.checked ? 'text' : 'password';
  });
  const updateCapsLock = event => {
    if (typeof event.getModifierState === 'function') {
      const capsLockOn = event.getModifierState('CapsLock');
      capsLockHint.hidden = !capsLockOn;
      capsLockHint.textContent = capsLockOn ? 'Caps Lock is on.' : '';
    }
  };
  password.addEventListener('keydown', updateCapsLock);
  password.addEventListener('keyup', updateCapsLock);
  password.addEventListener('blur', () => {
    capsLockHint.hidden = true;
    capsLockHint.textContent = '';
  });
  const clearInvalidState = () => {
    username.removeAttribute('aria-invalid');
    password.removeAttribute('aria-invalid');
  };
  username.addEventListener('input', clearInvalidState);
  password.addEventListener('input', clearInvalidState);
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (form.dataset.saving) return;
    // Capture credentials before disabling inputs: disabled fields are omitted
    // from FormData. Keep the submitted values visible until the request ends.
    const credentials = new URLSearchParams(new FormData(form));
    const controls = [username, password, revealPassword, button].map(field => ({field, disabled: field.disabled}));
    form.dataset.saving = 'true';
    form.setAttribute('aria-busy', 'true');
    clearInvalidState();
    controls.forEach(({field}) => { field.disabled = true; });
    button.textContent = 'Signing in…';
    error.textContent = '';
    let signedIn = false;
    const abort = new AbortController(),
      timer = setTimeout(() => abort.abort(), 15000);
    try {
      const response = await fetch('/authenticate', {
        method: 'POST',
        credentials: 'same-origin',
        body: credentials,
        signal: abort.signal
      });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        if (response.status === 401) {
          username.setAttribute('aria-invalid', 'true');
          password.setAttribute('aria-invalid', 'true');
          error.textContent = 'Username or password is incorrect. Check your details and try again.';
        } else if (response.status >= 500) {
          error.textContent = 'The device could not complete sign-in. Try again in a moment.';
        } else {
          error.textContent = typeof body?.error === 'string' && body.error.trim()
            ? body.error : 'Could not sign in. Try again.';
        }
        return;
      }
      location.assign(destination);
      signedIn = true;
    } catch (failure) {
      error.textContent = failure.name === 'AbortError'
        ? 'The device took too long to respond. Check your connection and try again.'
        : 'Could not reach your device. Check your connection and try again.';
    } finally {
      clearTimeout(timer);
      if (!signedIn) {
        const focusStillHere = form.contains(document.activeElement) || document.activeElement === document.body;
        delete form.dataset.saving;
        form.setAttribute('aria-busy', 'false');
        controls.forEach(({field, disabled}) => { field.disabled = disabled; });
        button.innerHTML = buttonLabel;
        if (focusStillHere) error.focus();
      }
    }
  });
})();
