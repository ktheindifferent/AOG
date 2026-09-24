"""Overview problem summaries and independent refreshes, with simulated APIs."""
import json
import os
from urllib.parse import urlparse

from playwright.sync_api import expect, sync_playwright
from web_ui_test import Device, NOW, open_page, navigate, assert_fits


class AttentionDevice(Device):
    def __init__(self):
        super().__init__()
        self.readings = {'temp': '24.50C', 'hum': '62.00%', 'co2': '820ppm',
                         'pm25': '4.2', 'pm10': '6.1', 'tvoc': '18ppb',
                         'water_temp_d6': '21.50C', 'water_temp_d8': '22.10C',
                         't1_ovf': 'NONE', 't2_ovf': 'NONE', 'overflow_error': False}
        self.ph_status = {}
        self.ph_message = None
        self.ph_invalid = False
        self.settings_error = False
        self.stats_reads = 0

    def ph(self):
        data = super().ph()
        data['status'].update(self.ph_status)
        data['status']['enabled'] = self.settings['ph']['enabled']
        data['error'] = self.ph_message
        return data

    def route(self, route):
        path = urlparse(route.request.url).path
        if path == '/api/ph' and self.ph_invalid:
            route.fulfill(status=200, content_type='application/json', body='{}')
            return
        if path == '/api/settings' and route.request.method == 'GET' and self.settings_error:
            route.fulfill(status=503, content_type='application/json',
                          body='{"error":"Saved settings temporarily unavailable"}')
            return
        if path == '/api/stats':
            self.stats_reads += 1
            if not self.stats_error:
                route.fulfill(status=200, content_type='application/json', body=json.dumps({
                    **self.readings, 'device_time': self.settings['device_time'],
                    'ph': '6.85', 'ph_status': None}))
                return
        super().route(route)


def refresh(page, keep_focus=False):
    expect(page.locator('#refresh-button')).to_be_enabled()
    if keep_focus:
        page.eval_on_selector('#refresh-button', 'button => button.click()')
    else:
        page.click('#refresh-button')
    expect(page.locator('#refresh-button')).to_be_enabled()


def check_attention_summary(browser):
    device = AttentionDevice()
    context, page, errors = open_page(browser, device)
    try:
        expect(page.locator('#overview-attention')).not_to_be_visible()
        device.readings.update(temp='N/A', tvoc='N/A', t1_ovf='OVERFLOW')
        device.outputs['pump_block_reason'] = 'Filling is blocked by Overflow sensor 1'
        device.ph_status.update(current_ph=8.9, alert_level='Critical')
        refresh(page)
        expect(page.locator('#attention-count')).to_have_text('4 items')
        expect(page.locator('#attention-items li').first).to_have_attribute('data-issue', 'equipment')
        expect(page.locator('[data-issue=equipment]')).to_contain_text('Filling is blocked')
        expect(page.locator('[data-issue=overflow]')).to_contain_text('Overflow sensor 1: triggered')
        expect(page.locator('[data-issue=environment]')).to_contain_text('Temperature')
        expect(page.locator('[data-issue=environment]')).to_contain_text('TVOC')
        expect(page.locator('[data-issue=ph]')).to_contain_text('8.90')
        # Connection and sensor health describe different facts.
        expect(page.locator('#connection')).to_contain_text('Connected')
        assert_fits(page)
        page.screenshot(path='/tmp/aog-ui-attention-desktop.png', full_page=True)
        for width in [390, 320]:
            page.set_viewport_size({'width': width, 'height': 844})
            assert_fits(page)
            assert page.locator('#overview-attention').bounding_box()['y'] < page.locator('.metrics').bounding_box()['y']
        page.screenshot(path='/tmp/aog-ui-attention-mobile.png', full_page=True)

        # Polling must retain a focused review link, even when its value changes.
        ph_link = page.locator('[data-issue=ph] a')
        ph_link.focus()
        device.ph_status['current_ph'] = 8.8
        refresh(page, keep_focus=True)
        expect(ph_link).to_be_focused()
        expect(page.locator('[data-issue=ph]')).to_contain_text('8.80')
        ph_link.click()
        expect(page).to_have_url('https://aog.test/index.html#overview/ph')
        expect(page.locator('#ph-history-title')).to_be_focused()
        ph_link.click()
        expect(page.locator('#ph-history-title')).to_be_focused()
        page.click('[data-issue=overflow] a')
        expect(page.locator('#settings-overflow-title')).to_be_focused()
        navigate(page, 'overview')

        # Missing/stale pH must not be described as a current high reading.
        device.ph_status.update(current_ph=None, last_reading=None)
        device.ph_message = 'Probe reading is missing or stale'
        refresh(page)
        expect(page.locator('[data-issue=ph]')).to_contain_text('reading is unavailable')
        expect(page.locator('[data-issue=ph]')).not_to_contain_text('outside your target')
        device.ph_status.update(current_ph=6.85, last_reading=NOW-600, alert_level='Normal')
        device.ph_message = None
        refresh(page)
        expect(page.locator('[data-issue=ph]')).to_contain_text('reading is stale')
        device.ph_error = True
        refresh(page)
        expect(page.locator('[data-issue=ph]')).to_contain_text('monitoring is unavailable')
        device.ph_error = False
        device.ph_invalid = True
        refresh(page)
        expect(page.locator('#ph-value')).to_have_text('—')
        expect(page.locator('#ph-detail')).to_contain_text('incomplete pH status')
        expect(page.locator('#connection')).to_contain_text('Connected')
        expect(page.locator('[data-issue=ph]')).to_contain_text('monitoring is unavailable')
        device.ph_invalid = False
        device.ph_status.update(current_ph=6.85, last_reading=NOW, alert_level='Normal')
        device.stats_error = True
        refresh(page)
        expect(page.locator('[data-issue=environment]')).to_contain_text('could not be refreshed')
        expect(page.locator('[data-issue=overflow]')).to_have_count(0)
        expect(page.locator('[data-issue=ph]')).to_have_count(0)
        device.outputs['stop_pending'] = True
        refresh(page)
        expect(page.locator('[data-issue=equipment]')).to_contain_text('still stopping')

        # A focused link resolves to an in-page heading instead of disappearing.
        page.locator('[data-issue=equipment] a').focus()
        device.outputs.update(stop_pending=False, pump_block_reason=None)
        refresh(page, keep_focus=True)
        expect(page.locator('#attention-title')).to_be_focused()
        device.stats_error = False
        device.readings.update(temp='0C', tvoc='0ppb', t1_ovf='NONE')
        refresh(page, keep_focus=True)
        expect(page.locator('#overview-attention')).not_to_be_visible()
        expect(page.locator('#page-title')).to_be_focused()
        assert not device.requests, 'Reviewing status must never send device commands'
        assert not errors, errors
    finally:
        context.close()


def check_refresh_and_device_clock(browser):
    device = AttentionDevice()
    device.settings['device_time'] = '2026-09-24T14:08:31.372832405-04:00'
    context, page, errors = open_page(browser, device)
    try:
        expect(page.locator('#footer-clock')).to_have_text('Device time · 14:08 UTC-04:00')
        navigate(page, 'schedules')
        expect(page.locator('#device-time')).to_contain_text('2026-09-24 14:08:31 (UTC-04:00)')
        for stamp, expected in [('2026-09-24T18:08:31.123Z', '18:08 UTC+00:00'),
                                ('2026-09-25T00:38:31+06:30', '00:38 UTC+06:30')]:
            device.settings['device_time'] = stamp
            refresh(page)
            expect(page.locator('#footer-clock')).to_have_text('Device time · ' + expected)
        device.settings['device_time'] = 'unavailable'
        refresh(page)
        expect(page.locator('#footer-clock')).to_have_text('Device time unavailable')

        navigate(page, 'settings')
        page.fill('#setup-name-input', 'A draft worth keeping')
        device.settings_error = True
        device.readings['temp'] = '27.5C'
        before = device.stats_reads
        refresh(page)
        expect(page.locator('#global-message')).to_contain_text('Saved settings temporarily unavailable')
        expect(page.locator('#temperature-value')).to_have_text('27.5 °C')
        assert device.stats_reads > before
        expect(page.locator('#setup-name-input')).to_have_value('A draft worth keeping')
        expect(page.locator('[data-form-state="setup-form"]')).to_have_text('Unsaved changes')
        device.settings_error = False
        refresh(page)
        expect(page.locator('#global-message')).not_to_be_visible()
        assert not device.requests
        assert not errors, errors
    finally:
        context.close()


if __name__ == '__main__':
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
                                    headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_attention_summary(browser)
            check_refresh_and_device_clock(browser)
            print('PASS: actionable attention summaries, recovery/focus, independent refresh, and device clock', flush=True)
        finally:
            browser.close()
