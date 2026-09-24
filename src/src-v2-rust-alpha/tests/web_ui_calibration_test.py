"""Calibration workflow checks; every request is served by a simulated device."""
import copy
import json
import os
from urllib.parse import urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, NOW, open_page, assert_fits


class CalibrationDevice(Device):
    def __init__(self):
        super().__init__()
        self.live_raw = 6.85
        self.saved_points = {}

    def ph(self):
        response = super().ph()
        response['raw_value'] = self.live_raw
        response['error'] = None if self.live_raw is not None else 'pH sensor reading is missing or stale'
        response['calibration'].update({f'point_{point}': copy.deepcopy(self.saved_points.get(point)) for point in [4, 7, 10]})
        response['status']['calibration_points'] = len(self.saved_points)
        response['status']['calibration_valid'] = len(self.saved_points) >= 2
        if self.live_raw is None:
            response['status']['current_ph'] = None
            response['status']['last_reading'] = None
        return response

    def route(self, route):
        request = route.request
        path = urlparse(request.url).path
        if request.method != 'POST' or path not in ['/api/ph/calibrate', '/api/ph/calibration/remove']:
            return super().route(route)
        data = request.post_data_json
        assert request.headers.get('x-aog-csrf') == 'browser-test-token'
        self.requests.append((path, data))
        point = data['buffer_ph']
        if path.endswith('/remove'):
            self.saved_points.pop(point, None)
            message = 'Calibration point removed. Readings and charts have been updated.'
        else:
            raw = data.get('raw_value', self.live_raw)
            if raw is None or any(key != point and abs(saved['raw_value'] - raw) < .0001 for key, saved in self.saved_points.items()):
                route.fulfill(status=400, content_type='application/json', body=json.dumps({'error': 'Calibration points need different raw readings; place the probe in the selected buffer'}))
                return
            self.saved_points[point] = {'ph_value': point, 'raw_value': raw, 'temperature': 24.5, 'timestamp': NOW}
            message = 'Calibration saved and applied. Readings and charts now use the updated calibration.'
        response = self.ph()
        response['message'] = message
        route.fulfill(status=200, content_type='application/json', body=json.dumps(response))


def check_calibration(browser):
    device = CalibrationDevice()
    context, page, errors = open_page(browser, device, 390, 844, 'index.html#settings/calibration')
    try:
        save = page.locator('#calibration-submit')
        ready = page.locator('#calibration-ready')
        raw = page.locator('#calibration-raw')
        expect(save).to_have_text('Save pH 7 point')
        expect(save).to_be_disabled()
        expect(page.locator('#calibration-plan')).to_contain_text('normalize')
        ready.check()
        expect(save).to_be_enabled()

        # Live save omits raw_value so the device takes a fresh reading.
        held = []
        context.route('**/api/ph/calibrate', lambda route: held.append(route))
        save.click()
        expect(save).to_have_text('Applying…')
        expect(page.locator('#buffer-ph')).to_be_disabled()
        expect(raw).to_be_disabled()
        assert len(held) == 1
        assert held[0].request.post_data_json == {'buffer_ph': 7}
        held.pop().fallback()
        expect(save).to_have_text('Replace pH 7 point')
        expect(page.locator('#calibration-result')).to_contain_text('saved and applied')
        expect(page.locator('#calibration-mode')).to_have_text('1-point normalized')
        expect(ready).not_to_be_checked()
        expect(save).to_be_disabled()
        context.unroute('**/api/ph/calibrate')

        page.uncheck('#calibration-use-live')
        expect(page.locator('#use-raw-reading')).to_have_text('Use current live reading')
        raw.fill('6.95')
        ready.check()
        raw.fill('7.1')
        expect(ready).not_to_be_checked()
        expect(save).to_be_disabled()
        ready.check()
        save.click()
        expect(page.locator('#calibration-point-value-7')).to_have_text('Raw 7.100')
        assert device.requests[-1] == ('/api/ph/calibrate', {'buffer_ph': 7, 'raw_value': 7.1})
        assert len(device.saved_points) == 1, 'Replacing a buffer must not require a remove request'

        # Missing live readings block live saves but allow a measured average.
        device.live_raw = None
        page.check('#calibration-use-live')
        expect(page.locator('#calibration-warning')).to_contain_text('missing or stale')
        expect(raw).to_have_value('')
        expect(ready).to_be_disabled()
        expect(save).to_be_disabled()
        page.uncheck('#calibration-use-live')
        raw.fill('200')
        ready.check()
        expect(save).to_be_enabled()
        save.click()
        expect(page.locator('#calibration-point-value-7')).to_have_text('Raw 200.000')

        # Backend validation remains authoritative and preserves saved points.
        page.select_option('#buffer-ph', '4')
        expect(save).to_have_text('Save pH 4 point')
        ready.check()
        save.click()
        expect(page.locator('#calibration-result')).to_contain_text('different raw readings')
        assert set(device.saved_points) == {7}
        expect(page.locator('#calibration-point-value-7')).to_have_text('Raw 200.000')
        expect(page.locator('#calibration-point-value-4')).to_have_text('Not set')

        # Capture copies into manual mode and requires a new stability check.
        device.live_raw = 3.9
        context.route('**/api/ph', lambda route: held.append(route))
        with page.expect_request('**/api/ph'):
            page.click('#use-raw-reading')
        expect(page.locator('#use-raw-reading')).to_have_text('Checking reading…')
        assert len(held) == 1
        page.evaluate("window.dispatchEvent(new Event('focus'))")
        held.pop().fallback()
        expect(raw).to_have_value('3.9')
        expect(ready).not_to_be_checked()
        expect(page.locator('#calibration-result')).to_contain_text('copied')
        assert not held, 'Background refresh must not supersede a manual sensor capture'
        context.unroute('**/api/ph')
        ready.check()
        save.click()
        expect(page.locator('#calibration-mode')).to_have_text('2-point calibrated')

        remove = page.locator('#remove-calibration-4')
        context.route('**/api/ph/calibration/remove', lambda route: held.append(route))
        remove.click()
        expect(remove).to_have_text('Removing…')
        expect(remove).to_be_disabled()
        expect(save).to_be_disabled()
        assert len(held) == 1
        followup = []
        context.route('**/api/ph', lambda route: followup.append(route))
        held.pop().fallback()
        expect(remove).not_to_be_visible()
        expect(page.locator('#buffer-ph')).to_be_disabled()
        expect(raw).to_be_disabled()
        assert len(followup) == 1
        page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(page.locator('#buffer-ph')).to_be_disabled()
        assert len(followup) == 1, 'Background refresh must not supersede calibration readback'
        followup.pop().fallback()
        expect(page.locator('#calibration-mode')).to_have_text('1-point normalized')
        expect(page.locator('#calibration-result')).to_contain_text('point removed')
        expect(page.locator('#buffer-ph')).to_be_enabled()
        assert not followup
        assert set(device.saved_points) == {7}
        assert all(path.startswith('/api/ph/') for path, _ in device.requests)
        assert_fits(page)
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_calibration(browser)
        finally:
            browser.close()
    print('PASS: calibration readiness, live/manual save, direct replacement, stale reading recovery, validation, capture, and removal')


if __name__ == '__main__':
    run()
