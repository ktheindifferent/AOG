"""History loading, stale response rejection, and recovery with intercepted reads only."""
import json
import os
from urllib.parse import parse_qs, urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, NOW, open_page, assert_fits


class HeldHistoryDevice(Device):
    def __init__(self):
        super().__init__()
        self.history_requests = []

    def route(self, route):
        if urlparse(route.request.url).path == '/api/history':
            assert route.request.method == 'GET'
            hours = int(parse_qs(urlparse(route.request.url).query)['hours'][0])
            self.history_requests.append({'route': route, 'hours': hours, 'done': False})
            return
        super().route(route)

    def take(self, hours):
        requests = [request for request in self.history_requests if request['hours'] == hours and not request['done']]
        assert requests, f'No pending history request for {hours}h'
        return requests[-1]

    def reply(self, request, value=24, payload=None, status=200):
        if payload is None:
            payload = self.sensor_history(request['hours'])
            series = next(series for series in payload['series'] if series['key'] == 'temp')
            for point in series['points']:
                point['value'] = value
            series['latest']['value'] = value
            series.update(minimum=value, maximum=value, average=value)
        request['done'] = True
        request['route'].fulfill(status=status, content_type='application/json', body=json.dumps(payload))


def check_history_loading(browser):
    device = HeldHistoryDevice()
    context, page, errors = open_page(browser, device, path='index.html#history/temp')
    chart = page.locator('#history-chart-temp')
    refresh = page.locator('#history-refresh')
    status = page.locator('#history-load-status')

    def select_range(hours):
        with page.expect_request(f'https://aog.test/api/history?hours={hours}'):
            page.click(f'[data-history-range="{hours}"]')
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'true')
        return device.take(hours)

    def retry(hours):
        expect(refresh).to_have_attribute('aria-disabled', 'false')
        with page.expect_request(f'https://aog.test/api/history?hours={hours}'):
            refresh.click()
        expect(refresh).to_have_attribute('aria-disabled', 'true')
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'true')
        return device.take(hours)

    def loaded(hours, value):
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'false')
        expect(refresh).to_have_attribute('aria-disabled', 'false')
        expect(chart).to_have_attribute('aria-label', f'Temperature: {len(device.sensor_history(hours)["series"][4]["points"])} readings in the last {hours} hours. Minimum {value}, average {value}, maximum {value} °C. Use arrow keys to inspect samples.')
        expect(page.locator('#history-value-temp')).to_have_text(str(value))
        expect(chart.locator('path')).to_have_count(1)
        expect(page.locator('#history-error')).not_to_be_visible()

    try:
        # No earlier range's readings or chart controls should appear during
        # initial load or after selecting a different range.
        expect(status).to_contain_text('Loading Last 24 hours')
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'true')
        expect(page.locator('#history-value-temp')).to_have_text('—')
        expect(chart).not_to_be_visible()
        expect(page.get_by_role('button', name='Previous Temperature reading')).to_be_disabled()
        first = device.take(24)
        six = select_range(6)
        expect(status).to_contain_text('Loading Last 6 hours')
        device.reply(first, value=99)
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'true')
        expect(page.locator('#history-value-temp')).to_have_text('—')
        one = select_range(1)
        device.reply(one, value=11)
        loaded(1, 11)
        device.reply(six, value=66)
        expect(page.locator('#history-selection')).to_have_text('Temperature · Last hour')
        loaded(1, 11)

        # A late failure from an old range cannot overwrite the newest success.
        six = select_range(6)
        day = select_range(24)
        device.reply(day, value=25)
        loaded(24, 25)
        device.reply(six, payload={'error': 'Old range request failed'}, status=503)
        loaded(24, 25)
        expect(status).to_contain_text('Loaded Last 24 hours')

        request = retry(24)
        expect(status).to_contain_text('Updating Last 24 hours')
        expect(status).to_contain_text('Showing the last loaded history')
        expect(page.locator('#history-status-temp')).to_have_text('Updating')
        expect(chart.locator('path')).to_have_count(1)
        expect(refresh).to_be_focused()
        # Repeated keyboard activation remains inert while this request waits.
        count = len(device.history_requests)
        refresh.press('Enter')
        assert len(device.history_requests) == count
        device.reply(request, payload={'error': 'History storage temporarily unavailable'}, status=503)
        expect(page.locator('#history-error')).to_contain_text('Showing the last loaded history')
        expect(page.locator('#history-status-temp')).to_have_text('Last loaded')
        expect(chart.locator('path')).to_have_count(1)
        expect(refresh).to_have_text('Retry history')
        expect(refresh).to_be_focused()

        # Validate before replacing a usable snapshot: incorrect ranges and
        # malformed nested series must neither relabel data nor poison redraws.
        request = retry(24)
        device.reply(request, payload=device.sensor_history(6))
        expect(page.locator('#history-error')).to_contain_text('different history range')
        expect(page.locator('#history-value-temp')).to_have_text('25')
        expect(chart).to_have_attribute('aria-label', 'Temperature: 120 readings in the last 24 hours. Minimum 25, average 25, maximum 25 °C. Use arrow keys to inspect samples.')
        for malformed in ['missing-series', 'null-series', 'null-point', 'missing-summary']:
            payload = device.sensor_history(24)
            if malformed == 'missing-series':
                payload['series'].pop()
            elif malformed == 'null-series':
                payload['series'][0] = None
            elif malformed == 'null-point':
                payload['series'][0]['points'][0] = None
            else:
                payload['series'][0]['minimum'] = None
            request = retry(24)
            device.reply(request, payload=payload)
            expect(page.locator('#history-error')).to_contain_text('incomplete sensor history')
            expect(page.locator('#history-status-temp')).to_have_text('Last loaded')
            expect(page.locator('#history-value-temp')).to_have_text('25')
            page.set_viewport_size({'width': 390, 'height': 844})
            expect(chart.locator('path')).to_have_count(1)
            page.set_viewport_size({'width': 1440, 'height': 1080})
        request = retry(24)
        device.reply(request, value=27)
        loaded(24, 27)
        expect(refresh).to_be_focused()

        # Null points represent legitimate gaps and must remain acceptable.
        gap = device.sensor_history(24)
        gap['series'][4]['points'][50]['value'] = None
        request = retry(24)
        device.reply(request, payload=gap)
        expect(page.locator('#history-error')).not_to_be_visible()
        expect(page.locator('#history-status-temp')).to_have_text('Recording')

        # Empty history differs from an unavailable request. Give useful next
        # steps and return focus from a disappearing chart to its own heading.
        empty = device.sensor_history(24)
        for series in empty['series']:
            series.update(points=[], count=0, minimum=None, maximum=None, average=None, available=False)
            series['latest']['timestamp'] = NOW - 2 * 86400
        previous = page.get_by_role('button', name='Previous Temperature reading')
        previous.focus()
        with page.expect_request('https://aog.test/api/history?hours=24'):
            refresh.evaluate('button => button.click()')
        # Let the request interception callback run before taking the held
        # request, as the range-selection and retry helpers do above.
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'true')
        device.reply(device.take(24), payload=empty)
        expect(page.locator('#history-empty-temp')).to_contain_text('Try a longer range')
        expect(page.locator('#history-title-temp')).to_be_focused()
        expect(previous).to_be_disabled()
        expect(chart).not_to_be_visible()
        expect(page.locator('#history-error')).not_to_be_visible()
        for series in empty['series']:
            series['latest'] = None
        request = retry(24)
        device.reply(request, payload=empty)
        expect(page.locator('#history-empty-temp')).to_contain_text('Recording starts when fresh sensor readings are available')
        expect(page.locator('#history-empty-temp')).to_contain_text('Check the sensor connection')
        expect(page.locator('#history-value-temp')).to_have_text('—')

        # Failure after a range change cannot fall back to the old range.
        week = select_range(168)
        device.reply(week, payload={'error': 'History storage offline'}, status=503)
        expect(status).to_contain_text('Retry history or choose another range')
        expect(page.locator('#history-error')).not_to_contain_text('last loaded history')
        expect(page.locator('#history-value-temp')).to_have_text('—')
        expect(chart).not_to_be_visible()
        expect(page.locator('#history-detail-temp')).to_have_text('No saved readings loaded for this range.')
        page.set_viewport_size({'width': 320, 'height': 844})
        assert_fits(page)
        request = retry(168)
        device.reply(request, value=28)
        loaded(168, 28)
        expect(page.locator('#history-selection')).to_have_text('Temperature · Last 7 days')
        assert_fits(page)
        assert not device.requests, 'History checks must not write settings or operate equipment'
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
            headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_history_loading(browser)
        finally:
            browser.close()
    print('PASS: history loading, range races, malformed response recovery, retry focus, and empty states')


if __name__ == '__main__':
    run()
