"""Cross-page draft visibility, using only the intercepted test device."""
import json
import os

from playwright.sync_api import expect, sync_playwright
from web_ui_test import Device, open_page, navigate, assert_fits


def check_initial_settings_load(browser):
    device = Device()
    device.settings['setup']['setup_name'] = 'Saved grow setup'
    device.settings['schedules']['lighting'].update(enabled=True, start_minute=1320, end_minute=360)
    device.settings['schedules']['watering'].update(
        enabled=True, pump='tank_cycle', start_minute=1380, end_minute=300,
        every_minutes=17, run_seconds=75)
    device.settings['pump']['safety_gpio_pin'] = 23
    context = browser.new_context(viewport={'width': 390, 'height': 844})
    context.route('https://aog.test/**', device.route)
    pending = []
    mode = {'value': 'hold'}

    def settings_reply(route):
        if route.request.method != 'GET':
            route.fallback()
        elif mode['value'] == 'hold':
            pending.append(route)
        elif mode['value'] == 'error':
            route.fulfill(status=503, content_type='application/json',
                          body='{"error":"Saved setup temporarily unavailable"}')
        else:
            route.fallback()

    context.route('https://aog.test/api/settings', settings_reply)
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    guarded_forms = ['setup-form', 'overflow-form', 'schedule-form', 'pump-form', 'ph-settings-form']

    def assert_guarded():
        for form in guarded_forms:
            fields = page.locator(f'#{form} input, #{form} select, #{form} textarea')
            assert fields.evaluate_all('fields => fields.length > 0 && fields.every(field => field.disabled)'), form
            expect(page.locator(f'#{form} [type=submit]')).to_be_disabled()
        expect(page.locator('#draft-summary')).not_to_be_visible()

    def release_settings():
        assert pending, 'Expected an intercepted settings request'
        mode['value'] = 'healthy'
        while pending:
            pending.pop().fulfill(status=200, content_type='application/json', body=json.dumps(device.settings))

    def assert_saved_values():
        expect(page.locator('#lighting-enabled')).to_be_checked()
        expect(page.locator('#lighting-start')).to_have_value('22:00')
        expect(page.locator('#lighting-end')).to_have_value('06:00')
        expect(page.locator('#watering-enabled')).to_be_checked()
        expect(page.locator('#watering-pump')).to_have_value('tank_cycle')
        expect(page.locator('#watering-start')).to_have_value('23:00')
        expect(page.locator('#watering-end')).to_have_value('05:00')
        expect(page.locator('#watering-interval')).to_have_value('17')
        expect(page.locator('#watering-duration')).to_have_value('75')
        expect(page.locator('#setup-name-input')).to_have_value('Saved grow setup')
        expect(page.locator('#safety-pin')).to_have_value('23')
        expect(page.locator('#watering-interval')).to_be_enabled()
        expect(page.locator('#schedule-form [type=submit]')).to_be_enabled()
        expect(page.locator('[data-settings-loading]:visible')).to_have_count(0)
        expect(page.locator('#draft-summary')).not_to_be_visible()

    try:
        with page.expect_request('https://aog.test/api/settings'):
            page.goto('https://aog.test/index.html#schedules')
        expect(page.locator('#page-schedules [data-settings-loading]')).to_contain_text('Loading saved settings')
        assert_guarded()
        release_settings()
        assert_saved_values()
        expect(page.locator('#connection')).to_contain_text('Connected')
        assert_fits(page)

        # A failed first read retains the guard and exposes a usable retry.
        mode['value'] = 'error'
        page.reload()
        expect(page.locator('#page-schedules [data-settings-loading]')).to_contain_text('Saved settings are unavailable')
        expect(page.locator('#global-message')).to_contain_text('Saved setup temporarily unavailable')
        assert_guarded()
        expect(page.locator('#refresh-button')).to_be_enabled()

        # Automatic recovery clears the original load error as well as the
        # local notice, without requiring a manual Refresh click.
        mode['value'] = 'hold'
        with page.expect_request('https://aog.test/api/settings'):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        assert_guarded()
        release_settings()
        assert_saved_values()
        expect(page.locator('#global-message')).not_to_be_visible()

        mode['value'] = 'error'
        page.reload()
        expect(page.locator('#page-schedules [data-settings-loading]')).to_contain_text('Saved settings are unavailable')
        mode['value'] = 'hold'
        with page.expect_request('https://aog.test/api/settings'):
            page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_disabled()
        expect(page.locator('#page-schedules [data-settings-loading]')).to_contain_text('Loading saved settings')
        assert_guarded()
        release_settings()
        expect(page.locator('#refresh-button')).to_be_enabled()
        assert_saved_values()
        expect(page.locator('#global-message')).not_to_be_visible()
        assert not device.requests, 'Loading or retrying settings must not submit HTML defaults'
        assert_fits(page)
        assert not errors, errors
    finally:
        context.close()


def check_draft_summary(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    try:
        expect(page.locator('#draft-summary')).not_to_be_visible()
        navigate(page, 'settings')
        page.fill('#setup-name-input', 'Draft grow room')
        page.click('[data-settings-section=overflow]')
        page.fill('#overflow-label-1', 'Display tank draft')
        expect(page.locator('#draft-summary-status')).to_contain_text('2 sections have changes')
        expect(page.locator('#settings-draft-count')).to_have_text('2')
        expect(page.locator('#settings-draft-label')).to_contain_text('2 sections have unsaved changes')
        navigate(page, 'schedules')
        page.fill('#watering-interval', '45')
        expect(page.locator('#schedules-draft-count')).to_have_text('1')
        navigate(page, 'overview')
        expect(page.locator('#draft-summary-status')).to_contain_text('3 sections have changes')
        expect(page.locator('[data-review-form]:visible')).to_have_count(3)
        page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_enabled()
        expect(page.locator('#setup-name-input')).to_have_value('Draft grow room')
        expect(page.locator('#watering-interval')).to_have_value('45')
        expect(page.locator('[data-review-form]:visible')).to_have_count(3)
        assert not device.requests, 'Reviewing drafts must not apply them'
        page.screenshot(path='/tmp/aog-ui-draft-summary-desktop.png', full_page=True)

        for width in [390, 320]:
            page.set_viewport_size({'width': width, 'height': 844})
            expect(page.locator('#navigation-toggle')).to_have_text('Menu · 3')
            expect(page.locator('#navigation-toggle')).to_have_attribute('aria-label', 'Menu, 3 sections have unsaved changes')
            assert_fits(page)
            if width == 390:
                page.screenshot(path='/tmp/aog-ui-draft-summary-mobile.png')
            page.click('#navigation-toggle')
            expect(page.locator('#settings-draft-count')).to_be_visible()
            expect(page.locator('#schedules-draft-count')).to_be_visible()
            assert_fits(page)
            page.keyboard.press('Escape')

        page.click('[data-review-form=overflow-form]')
        expect(page).to_have_url('https://aog.test/index.html#settings/overflow')
        expect(page.locator('#settings-overflow-title')).to_be_focused()
        expect(page.locator('#overflow-label-1')).to_have_value('Display tank draft')
        page.click('[data-discard-form=overflow-form]')
        expect(page.locator('#draft-summary-status')).to_contain_text('2 sections have changes')
        expect(page.locator('[data-review-form=overflow-form]')).not_to_be_visible()
        page.click('[data-review-form=setup-form]')
        expect(page.locator('#settings-relays-title')).to_be_focused()
        # Selecting the already-open review link still returns focus to the form.
        page.locator('#setup-name-input').focus()
        page.click('[data-review-form=setup-form]')
        expect(page.locator('#settings-relays-title')).to_be_focused()

        held = []
        def hold_settings(route):
            if route.request.method == 'POST':
                held.append(route)
            else:
                route.fallback()
        context.route('https://aog.test/api/settings', hold_settings)
        page.fill('#setup-name-input', 'Saved grow room')
        page.click('#setup-form [type=submit]')
        expect(page.locator('#setup-form')).to_have_attribute('aria-busy', 'true')
        page.fill('#setup-name-input', 'A newer unsaved draft')
        assert len(held) == 1
        request = held.pop()
        device.settings['setup'].update(request.request.post_data_json['setup'])
        request.fulfill(status=200, content_type='application/json', body=json.dumps(device.settings))
        expect(page.locator('#setup-result')).to_contain_text('newer edits are still unsaved')
        expect(page.locator('#settings-draft-count')).to_have_text('1')
        expect(page.locator('#draft-summary-status')).to_contain_text('2 sections have changes')
        expect(page.locator('#setup-name-input')).to_have_value('A newer unsaved draft')

        # Reverting to the confirmed device value removes only that draft.
        page.fill('#setup-name-input', 'Saved grow room')
        expect(page.locator('#settings-draft-count')).not_to_be_visible()
        expect(page.locator('[data-review-form=setup-form]')).not_to_be_visible()
        expect(page.locator('#draft-summary-status')).to_contain_text('1 section has changes')
        page.click('[data-review-form=schedule-form]')
        expect(page.locator('#page-title')).to_be_focused()
        expect(page.locator('#watering-interval')).to_have_value('45')
        page.click('[data-discard-form=schedule-form]')
        expect(page.locator('#draft-summary')).not_to_be_visible()
        expect(page.locator('#schedules-draft-count')).not_to_be_visible()
        expect(page.locator('#navigation-toggle')).to_have_text('Menu')

        navigate(page, 'account')
        page.fill('#current-password', 'Never saved in the draft summary')
        expect(page.locator('#draft-summary')).not_to_be_visible()
        expect(page.locator('#navigation-toggle')).to_have_text('Menu')
        assert not errors, errors
    finally:
        context.close()


if __name__ == '__main__':
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True,
                                    args=['--no-sandbox', '--disable-dev-shm-usage'])
        check_draft_summary(browser)
        browser.close()
    print('PASS: cross-page draft summary, review links, saves, reverts, and mobile counts')
