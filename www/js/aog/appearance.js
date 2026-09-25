'use strict';
(() => {
  const key = 'aog.appearance';
  const choices = ['system', 'light', 'dark'];
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  let choice = 'system';
  try {
    const stored = localStorage.getItem(key);
    if (choices.includes(stored)) choice = stored;
  } catch (_) { /* Storage can be unavailable; the controls still work. */ }

  function apply() {
    const theme = choice === 'system' ? system.matches ? 'dark' : 'light' : choice;
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.appearance = choice;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = theme === 'dark' ? '#111b16' : '#20372b';
    document.querySelectorAll('select[data-appearance]').forEach(control => { control.value = choice; });
    const detail = choice === 'system' ? `Following your system · ${theme} appearance`
      : `${theme === 'dark' ? 'Dark' : 'Light'} appearance · this browser only`;
    document.querySelectorAll('[data-appearance-status]').forEach(element => {
      if (element.textContent !== detail) element.textContent = detail;
    });
  }

  // Run before the stylesheets load to avoid a bright flash on dark screens.
  apply();
  system.addEventListener('change', () => { if (choice === 'system') apply(); });
  window.addEventListener('storage', event => {
    if (event.key !== key && event.key !== null) return;
    choice = choices.includes(event.newValue) ? event.newValue : 'system';
    apply();
  });
  document.addEventListener('DOMContentLoaded', () => {
    apply();
    document.querySelectorAll('select[data-appearance]').forEach(control => {
      control.addEventListener('change', () => {
        if (!choices.includes(control.value)) return;
        choice = control.value;
        try { localStorage.setItem(key, choice); } catch (_) { /* Keep the in-memory choice. */ }
        apply();
      });
    });
  });
})();
