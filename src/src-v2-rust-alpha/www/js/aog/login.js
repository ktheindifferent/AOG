'use strict';
(() => {
  const query = new URLSearchParams(location.search);
  const notice = document.getElementById('login-notice');
  if (query.has('changed') || query.has('expired')) {
    notice.textContent = query.has('changed') ? 'Password changed. Sign in with your new password.' : 'Your session expired. Sign in again to continue.';
    notice.hidden = false;
  }
  document.getElementById('show-password').addEventListener('change', event => {
    document.getElementById('input_password').type = event.target.checked ? 'text' : 'password';
  });
  document.getElementById('login-form').addEventListener('submit', async event => {
    event.preventDefault();
    const form = event.currentTarget,
      button = form.querySelector('button'),
      error = document.getElementById('login-error');
    if (form.dataset.saving) return;
    form.dataset.saving = 'true';
    button.disabled = true;
    button.textContent = 'Signing in…';
    error.textContent = '';
    const abort = new AbortController(),
      timer = setTimeout(() => abort.abort(), 15000);
    try {
      const response = await fetch('/authenticate', {
        method: 'POST',
        credentials: 'same-origin',
        body: new URLSearchParams(new FormData(form)),
        signal: abort.signal
      });
      if (!response.ok) {
        const body = await response.json().catch(() => ({}));
        throw new Error(body.error || (response.status === 401 ? 'Username or password is incorrect.' : 'Could not sign in. Try again.'));
      }
      location.assign('/index.html');
    } catch (failure) {
      error.textContent = failure.name === 'AbortError' ? 'The device did not respond. Try again.' : failure.message;
    } finally {
      clearTimeout(timer);
      delete form.dataset.saving;
      button.disabled = false;
      button.textContent = 'Sign in →';
    }
  });
})();
