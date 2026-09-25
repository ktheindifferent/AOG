"""Review simultaneous settings edits against a simulated device only."""
import json
import os

from playwright.sync_api import expect, sync_playwright
from web_ui_test import Device, open_page, navigate, assert_fits
from web_ui_attention_test import refresh


def check_settings_conflicts(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    try:
        navigate(page, 'settings')
        # Keep multiple forms visible for the overlapping edit/save scenarios.
        page.click('[data-settings-view=all]')
        page.fill('#setup-name-input', 'My grow room')
        device.settings['setup']['relay_labels'][2] = 'Updated fill label'
        refresh(page)
        expect(page.locator('#setup-name-input')).to_have_value('My grow room')
        expect(page.locator('#relay-label-3')).to_have_value('Updated fill label')
        expect(page.locator('#setup-form-conflicts')).not_to_be_visible()

        # Another writer changed the field being edited, not just an untouched field.
        device.settings['setup']['setup_name'] = 'Name saved on the device'
        refresh(page)
        row = page.locator('[data-conflict-field="setup-name-input"]')
        expect(row).to_be_visible()
        expect(row.locator('[data-conflict-value=saved]')).to_have_text('Name saved on the device')
        expect(row.locator('[data-conflict-value=draft]')).to_have_text('My grow room')
        expect(page.locator('#setup-form [type=submit]')).to_be_disabled()
        expect(page.locator('#draft-summary-status')).to_contain_text('device values changed')
        page.evaluate("document.querySelector('#setup-form').requestSubmit()")
        expect(page.locator('#setup-result')).to_contain_text('Review changed device values')
        assert not device.requests, 'Unreviewed conflicts must not issue writes'
        for width in [1440, 390, 320]:
            page.set_viewport_size({'width': width, 'height': 1080 if width == 1440 else 844})
            assert_fits(page)
            page.screenshot(path=f'/tmp/aog-ui-settings-review-{width}.png', full_page=True)

        # A value accepted explicitly is not repeatedly challenged by polling.
        row.locator('[data-conflict-choice=draft]').click()
        expect(page.locator('#setup-name-input')).to_be_focused()
        expect(page.locator('#setup-form-conflicts')).not_to_be_visible()
        expect(page.locator('#setup-form [type=submit]')).to_be_enabled()
        expect(page.locator('#setup-result')).to_have_text('')
        refresh(page)
        expect(page.locator('#setup-form-conflicts')).not_to_be_visible()
        device.settings['setup']['setup_name'] = 'A newer device name'
        refresh(page)
        expect(row.locator('[data-conflict-value=saved]')).to_have_text('A newer device name')
        row.locator('[data-conflict-choice=saved]').focus()
        device.settings['setup']['setup_name'] = 'The latest device name'
        refresh(page, keep_focus=True)
        expect(row.locator('[data-conflict-choice=saved]')).to_be_focused()
        expect(row.locator('[data-conflict-value=saved]')).to_have_text('The latest device name')
        row.locator('[data-conflict-choice=saved]').click()
        expect(page.locator('#setup-name-input')).to_have_value('The latest device name')
        expect(page.locator('#setup-form-conflicts')).not_to_be_visible()
        expect(page.locator('[data-form-state="setup-form"]')).to_have_text('All changes saved')

        # Converging to a numerically equivalent value resolves its own review.
        page.fill('#ph-min', '6.7')
        device.settings['ph']['optimal_min'] = 6.8
        refresh(page)
        expect(page.locator('[data-conflict-field="ph-min"]')).to_be_visible()
        page.fill('#ph-min', '6.800')
        expect(page.locator('#ph-settings-form-conflicts')).not_to_be_visible()
        expect(page.locator('#ph-settings-form [type=submit]')).to_be_enabled()
        expect(page.locator('[data-form-state="ph-settings-form"]')).to_have_text('All changes saved')

        # Polling that observes the user's exact draft also clears a review,
        # returning focus from the removed choice to its associated field.
        page.fill('#setup-name-input', 'Converged name')
        device.settings['setup']['setup_name'] = 'Other name'
        refresh(page)
        row.locator('[data-conflict-choice=draft]').focus()
        device.settings['setup']['setup_name'] = 'Converged name'
        refresh(page, keep_focus=True)
        expect(page.locator('#setup-form-conflicts')).not_to_be_visible()
        expect(page.locator('#setup-name-input')).to_be_focused()

        navigate(page, 'schedules')
        page.fill('#watering-interval', '15')
        device.settings['schedules']['watering'].update(every_minutes=30, run_seconds=60)
        refresh(page)
        expect(page.locator('#watering-interval')).to_have_value('15')
        expect(page.locator('#watering-duration')).to_have_value('60')
        expect(page.locator('#schedule-form [type=submit]')).to_be_disabled()
        page.click('[data-conflict-field="watering-interval"] [data-conflict-choice=draft]')
        page.click('#schedule-form [type=submit]')
        expect(page.locator('#schedule-result')).to_contain_text('Saved')
        assert device.settings['schedules']['watering']['every_minutes'] == 15
        assert device.settings['schedules']['watering']['run_seconds'] == 60
        expect(page.locator('#schedule-form-conflicts')).not_to_be_visible()
        assert len(device.requests) == 1 and device.requests[0][0] == '/api/settings'

        # Discard uses the newest device version and clears all review state.
        page.fill('#watering-interval', '45')
        device.settings['schedules']['watering']['every_minutes'] = 90
        refresh(page)
        expect(page.locator('#schedule-form-conflicts')).to_be_visible()
        page.click('[data-discard-form="schedule-form"]')
        expect(page.locator('#watering-interval')).to_have_value('90')
        expect(page.locator('#schedule-form-conflicts')).not_to_be_visible()
        expect(page.locator('#draft-summary')).not_to_be_visible()

        # The safety switch's enabled state and pin are one setting. A remote
        # disable must not hide or erase a local edit to the pin.
        navigate(page, 'settings')
        device.settings['pump']['safety_gpio_pin'] = 17
        refresh(page)
        page.fill('#safety-pin', '23')
        device.settings['pump']['safety_gpio_pin'] = None
        refresh(page)
        safety = page.locator('[data-conflict-field="safety-pin"]')
        expect(page.locator('#safety-enabled')).to_be_checked()
        expect(page.locator('#safety-pin')).to_have_value('23')
        expect(safety.locator('[data-conflict-value=saved]')).to_have_text('Disabled')
        expect(safety.locator('[data-conflict-value=draft]')).to_have_text('GPIO 23')
        safety.locator('[data-conflict-choice=draft]').click()
        refresh(page)
        expect(page.locator('#pump-form-conflicts')).not_to_be_visible()
        expect(page.locator('#safety-pin')).to_have_value('23')
        device.settings['pump']['safety_gpio_pin'] = 18
        refresh(page)
        safety.locator('[data-conflict-choice=saved]').click()
        expect(page.locator('#safety-enabled')).to_be_checked()
        expect(page.locator('#safety-pin')).to_have_value('18')

        # Conversely, keep a local disable when another client changes the pin.
        page.uncheck('#safety-enabled')
        device.settings['pump']['safety_gpio_pin'] = 24
        refresh(page)
        expect(safety.locator('[data-conflict-value=draft]')).to_have_text('Disabled')
        expect(safety.locator('[data-conflict-value=saved]')).to_have_text('GPIO 24')
        safety.locator('[data-conflict-choice=draft]').click()
        expect(page.locator('#safety-enabled')).not_to_be_checked()
        expect(page.locator('#safety-enabled')).to_be_focused()
        device.settings['pump']['safety_gpio_pin'] = 25
        refresh(page)
        safety.locator('[data-conflict-choice=saved]').click()
        expect(page.locator('#safety-enabled')).to_be_checked()
        expect(page.locator('#safety-pin')).to_have_value('25')
        expect(page.locator('#draft-summary')).not_to_be_visible()
        assert len(device.requests) == 1, 'Review choices must not save settings'

        # After the POST acknowledgement, output readback may still be pending.
        # A refresh during that interval must review a different client's save
        # against edits typed after our submission.
        held_outputs = []
        def hold_output_readback(route):
            held_outputs.append(route)
        context.route('https://aog.test/api/outputs', hold_output_readback)
        page.fill('#setup-name-input', 'Submitted name')
        page.click('#setup-form [type=submit]')
        expect(page.locator('#setup-name')).to_have_text('Submitted name')
        expect(page.locator('#setup-form')).to_have_attribute('aria-busy', 'true')
        page.fill('#setup-name-input', 'New draft after submitting')
        device.settings['setup']['setup_name'] = 'Saved from another browser'
        # Do not await refresh completion: its output readback is held too.
        page.click('#refresh-button')
        expect(row.locator('[data-conflict-value=saved]')).to_have_text('Saved from another browser')
        expect(row.locator('[data-conflict-value=draft]')).to_have_text('New draft after submitting')
        expect(row.locator('[data-conflict-choice=draft]')).to_be_disabled()
        context.unroute('https://aog.test/api/outputs', hold_output_readback)
        for route in held_outputs:
            route.fulfill(status=200, content_type='application/json', body=json.dumps(device.output_status()))
        expect(page.locator('#setup-form')).not_to_have_attribute('aria-busy', 'true')
        expect(page.locator('#refresh-button')).to_be_enabled()
        expect(page.locator('#setup-form [type=submit]')).to_be_disabled()
        expect(page.locator('#setup-name-input')).to_have_value('New draft after submitting')
        expect(row.locator('[data-conflict-choice=draft]')).to_be_enabled()
        assert len(device.requests) == 2, 'Only the explicitly submitted saves should write'
        row.locator('[data-conflict-choice=saved]').click()

        # An unrelated form can save while the first form awaits readback.
        # Its response is not an acknowledgement of the first form's draft.
        held_outputs.clear()
        context.route('https://aog.test/api/outputs', hold_output_readback)
        page.fill('#setup-name-input', 'Another submitted name')
        page.click('#setup-form [type=submit]')
        expect(page.locator('#setup-name')).to_have_text('Another submitted name')
        expect(page.locator('#setup-form')).to_have_attribute('aria-busy', 'true')
        page.fill('#setup-name-input', 'Draft during overlapping saves')
        device.settings['setup']['setup_name'] = 'Another browser changed it again'
        page.fill('#safety-pin', '26')
        page.click('#pump-form [type=submit]')
        expect(row.locator('[data-conflict-value=saved]')).to_have_text('Another browser changed it again')
        expect(row.locator('[data-conflict-value=draft]')).to_have_text('Draft during overlapping saves')
        context.unroute('https://aog.test/api/outputs', hold_output_readback)
        for route in held_outputs:
            route.fulfill(status=200, content_type='application/json', body=json.dumps(device.output_status()))
        expect(page.locator('#setup-form')).not_to_have_attribute('aria-busy', 'true')
        expect(page.locator('#pump-form')).not_to_have_attribute('aria-busy', 'true')
        expect(page.locator('#setup-form [type=submit]')).to_be_disabled()
        expect(page.locator('#setup-name-input')).to_have_value('Draft during overlapping saves')
        expect(page.locator('#safety-pin')).to_have_value('26')
        assert len(device.requests) == 4
        assert not errors, errors
    finally:
        context.close()


if __name__ == '__main__':
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
                                    headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_settings_conflicts(browser)
            print('PASS: settings merge, explicit conflict review, convergence, save and discard', flush=True)
        finally:
            browser.close()
