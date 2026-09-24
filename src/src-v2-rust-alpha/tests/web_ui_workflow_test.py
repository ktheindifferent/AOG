"""Focused workflow regressions using intercepted device APIs only.

Import the checks into the shared browser run, or run this file independently.
No request reaches a device and no real relay or schedule is changed.
"""
import copy
import json
import os

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, open_page, navigate, assert_fits


def refresh(page):
    page.click('#refresh-button')
    expect(page.locator('#refresh-button')).to_be_enabled()


def check_output_recovery(browser):
    device = Device()
    device.settings['schedules']['lighting']['enabled'] = True
    device.settings['schedules']['watering']['enabled'] = True
    device.outputs['outputs'][2].update(on=True, remaining_seconds=120)
    device.outputs['next_watering_in_seconds'] = 180
    device.outputs['last_event'] = 'Fill pump started for two minutes'
    context, page, errors = open_page(browser, device)
    mode = {'value': 'healthy'}

    def output_reply(route):
        if route.request.method != 'GET' or mode['value'] == 'healthy':
            route.fallback()
            return
        if mode['value'] == 'error':
            route.fulfill(status=503, content_type='application/json',
                          body='{"error":"Relay connection temporarily unavailable"}')
            return
        data = copy.deepcopy(device.output_status())
        if mode['value'] == 'unknown':
            # The API can reply successfully while board readback is unknown.
            for output in data['outputs']:
                output['on'] = None
        elif mode['value'] == 'incomplete':
            data['outputs'] = data['outputs'][:2]
        route.fulfill(status=200, content_type='application/json', body=json.dumps(data))

    context.route('https://aog.test/api/outputs', output_reply)
    try:
        expect(page.locator('#water-safety-note')).to_contain_text('allow pumping')
        expect(page.locator('#next-watering')).to_contain_text('Next watering check in')
        navigate(page, 'outputs')
        expect(page.locator('#output-state-3')).to_have_text('On')
        expect(page.locator('#output-timing-3')).to_contain_text('Stops in')

        mode['value'] = 'error'
        refresh(page)
        expect(page.locator('#connection')).to_contain_text('Check equipment')
        expect(page.locator('body')).to_have_attribute('data-connection', 'connected')
        expect(page.locator('#temperature-value')).to_contain_text('24.5')
        expect(page.locator('#output-summary .badge')).to_have_text(['Unknown'] * 4)
        expect(page.locator('#water-safety-note')).to_contain_text('could not be confirmed')
        expect(page.locator('#water-safety-note')).not_to_contain_text('allow pumping')
        expect(page.locator('#next-watering')).to_contain_text('Live schedule status is unavailable')
        expect(page.locator('#lighting-state')).to_have_text('Unavailable')
        expect(page.locator('#watering-state')).to_have_text('Unavailable')
        expect(page.locator('#last-event')).to_contain_text('Last reported event: Fill pump started')
        assert 'Stops in' not in page.locator('#output-controls').inner_text()
        for channel in range(1, 5):
            expect(page.locator(f'#output-state-{channel}')).to_have_text('Unknown')
            expect(page.locator(f'[data-output="{channel}"]')).to_have_text('Stop')
            expect(page.locator(f'[data-output="{channel}"]')).to_be_enabled()
        expect(page.locator('#stop-all')).to_be_enabled()

        # Healthy relay status cannot disguise a failed environment request.
        device.stats_error = True
        mode['value'] = 'healthy'
        refresh(page)
        expect(page.locator('#connection')).to_contain_text('Offline')
        expect(page.locator('body')).to_have_attribute('data-connection', 'offline')
        expect(page.locator('#output-state-3')).to_have_text('On')
        expect(page.locator('#output-timing-3')).to_contain_text('Stops in')
        device.stats_error = False
        refresh(page)
        expect(page.locator('#connection')).to_contain_text('Connected')
        expect(page.locator('#water-safety-note')).to_contain_text('allow pumping')
        expect(page.locator('#next-watering')).to_contain_text('Next watering check in')

        # Null readback and incomplete responses must not leave cached On states.
        for failure in ['unknown', 'incomplete']:
            mode['value'] = failure
            refresh(page)
            expect(page.locator('#connection')).to_contain_text('Check equipment')
            expect(page.locator('#output-summary .badge')).to_have_text(['Unknown'] * 4)
            expect(page.locator('#water-safety-note')).to_contain_text('could not be confirmed')
            expect(page.locator('#next-watering')).not_to_contain_text('Next watering check in')
            assert 'Stops in' not in page.locator('#output-controls').inner_text()

        mode['value'] = 'healthy'
        device.outputs['automation_paused'] = True
        refresh(page)
        expect(page.locator('#paused-banner-message')).to_contain_text('Schedules are paused.')
        navigate(page, 'schedules')
        # The manual fill pump is still on even though automation is paused.
        expect(page.locator('#pause-schedules')).to_be_enabled()
        mode['value'] = 'error'
        refresh(page)
        expect(page.locator('#paused-banner-message')).to_contain_text('last reported paused')
        expect(page.locator('#pause-schedules')).to_be_enabled()
        expect(page.locator('#paused-banner .resume-schedules')).to_be_enabled()
        assert not device.requests, 'Status and recovery checks must not issue control commands'

        # A fresh application load with no successful output response still
        # supplies four unknown relays and usable Stop controls.
        page.reload()
        expect(page.locator('#connection')).to_contain_text('Check equipment')
        expect(page.locator('#output-summary .badge')).to_have_text(['Unknown'] * 4)
        expect(page.locator('#last-event')).to_contain_text('Current relay state unavailable')
        navigate(page, 'outputs')
        for channel in range(1, 5):
            expect(page.locator(f'[data-output="{channel}"]')).to_have_text('Stop')
            expect(page.locator(f'[data-output="{channel}"]')).to_be_enabled()
        expect(page.locator('#stop-all')).to_be_enabled()
        page.click('[data-output="1"]')
        expect(page.locator('[data-output="1"]')).to_be_enabled()
        assert device.requests == [('/api/outputs/stop', {'channel': 1})], device.requests

        mode['value'] = 'healthy'
        refresh(page)
        expect(page.locator('#connection')).to_contain_text('Connected')
        expect(page.locator('#output-state-1')).to_have_text('Off')
        expect(page.locator('#outputs-error')).not_to_be_visible()
        page.set_viewport_size({'width': 390, 'height': 844})
        assert_fits(page)
        assert not errors, errors
    finally:
        context.close()


def check_schedule_previews(browser):
    device = Device()
    device.settings['schedules']['lighting'].update(enabled=True, start_minute=360, end_minute=360)
    device.settings['schedules']['watering'].update(enabled=True, start_minute=1320, end_minute=1320)
    saved_schedules = copy.deepcopy(device.settings['schedules'])
    context, page, errors = open_page(browser, device, width=390, height=844)
    try:
        expect(page.locator('#lighting-summary')).to_have_text('All day · 24 hours')
        expect(page.locator('#watering-summary')).to_contain_text('All day · 24 hours')
        navigate(page, 'schedules')
        expect(page.locator('#lighting-preview-text')).to_contain_text('All day: 24 hours')
        expect(page.locator('#watering-preview-text')).to_contain_text('all day (equal window times)')
        expect(page.locator('#schedule-preview-state')).to_have_text('Preview of saved schedules')

        page.fill('#lighting-start', '22:00')
        page.fill('#lighting-end', '06:00')
        page.fill('#watering-start', '20:00')
        page.fill('#watering-end', '04:00')
        expect(page.locator('#lighting-preview-text')).to_contain_text('8.0 hours')
        expect(page.locator('#lighting-preview-text')).to_contain_text('crossing midnight')
        expect(page.locator('#watering-preview-text')).to_contain_text('20:00–04:00, crossing midnight')
        expect(page.locator('#schedule-preview-state')).to_contain_text('Preview of unsaved changes')
        expect(page.locator('[data-form-state="schedule-form"]')).to_have_text('Unsaved changes')

        page.fill('#lighting-start', '')
        expect(page.locator('#lighting-preview-text')).to_contain_text('Choose both on and off times')
        assert 'NaN' not in page.locator('#lighting-preview-text').inner_text()
        page.fill('#lighting-start', '22:00')
        page.fill('#watering-start', '')
        expect(page.locator('#watering-preview-text')).to_contain_text('Choose both window times')
        page.fill('#watering-start', '20:00')
        page.fill('#watering-interval', '')
        expect(page.locator('#watering-preview-text')).to_contain_text('at least 1')
        page.fill('#watering-interval', '15')
        page.fill('#watering-duration', '90')
        page.select_option('#watering-pump', 'tank_cycle')
        expect(page.locator('#watering-relay')).to_contain_text('Tank-to-tank · Relay 4')
        expect(page.locator('#watering-preview-text')).to_contain_text('90 seconds every 15 minutes')

        # A settings refresh changes saved relay readiness while preserving the
        # unsaved schedule draft, including the selected pump and preview.
        device.settings['setup']['relay_roles'] = ['unassigned', 'draining', 'filling', 'unassigned']
        refresh(page)
        expect(page.locator('#lighting-readiness')).to_be_visible()
        expect(page.locator('#watering-readiness')).to_be_visible()
        expect(page.locator('#lighting-readiness-text')).to_contain_text('Assign and save a Lights + Air relay')
        expect(page.locator('#watering-readiness-text')).to_contain_text('Assign and save a Tank-to-tank relay')
        expect(page.locator('#watering-relay')).to_contain_text('No relay assigned')
        assert 'watering-readiness' in page.locator('#watering-pump').get_attribute('aria-describedby').split()
        assert 'lighting-readiness' in page.locator('#lighting-enabled').get_attribute('aria-describedby').split()
        expect(page.locator('#lighting-start')).to_have_value('22:00')
        expect(page.locator('#watering-interval')).to_have_value('15')
        expect(page.locator('#watering-pump')).to_have_value('tank_cycle')
        assert_fits(page)

        page.click('#schedule-form [type=submit]')
        expect(page.locator('#lighting-enabled')).to_be_focused()
        expect(page.locator('#lighting-enabled')).to_have_attribute('aria-invalid', 'true')
        expect(page.locator('#schedule-result')).to_contain_text('Assign and save a Lights + Air relay')
        assert not device.requests, 'A missing lighting relay must not submit settings'
        page.uncheck('#lighting-enabled')
        page.click('#schedule-form [type=submit]')
        expect(page.locator('#watering-pump')).to_be_focused()
        expect(page.locator('#watering-pump')).to_have_attribute('aria-invalid', 'true')
        expect(page.locator('#schedule-result')).to_contain_text('Assign and save a Tank-to-tank relay')
        assert not device.requests, 'A missing scheduled pump relay must not submit settings'
        page.check('#lighting-enabled')
        expect(page.locator('#lighting-start')).to_have_value('22:00')
        expect(page.locator('#watering-duration')).to_have_value('90')

        page.click('#watering-readiness a')
        expect(page).to_have_url('https://aog.test/index.html#settings/relays')
        expect(page.locator('#setup-form h2')).to_be_focused()
        page.go_back()
        expect(page.locator('#page-schedules')).to_be_visible()
        expect(page.locator('#lighting-start')).to_have_value('22:00')
        expect(page.locator('#watering-duration')).to_have_value('90')
        expect(page.locator('#watering-pump')).to_have_value('tank_cycle')
        expect(page.locator('#schedule-preview-state')).to_contain_text('Preview of unsaved changes')

        # Overview summaries remain the saved schedule while the editor has a draft.
        navigate(page, 'overview')
        expect(page.locator('#lighting-summary')).to_have_text('All day · 24 hours')
        expect(page.locator('#watering-summary')).to_contain_text('All day · 24 hours')
        navigate(page, 'schedules')
        expect(page.locator('#watering-preview-text')).to_contain_text('Tank-to-tank')
        assert not device.requests, 'Previewing schedules and following assignment links must not write settings'
        assert device.settings['schedules'] == saved_schedules
        assert_fits(page)
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
            headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_output_recovery(browser)
            check_schedule_previews(browser)
        finally:
            browser.close()
    print('PASS: partial equipment failure and recovery, stop availability, schedule previews, assignment links, and retained drafts')


if __name__ == '__main__':
    run()
