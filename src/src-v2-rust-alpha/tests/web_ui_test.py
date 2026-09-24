"""Browser regression checks with simulated APIs; never switches device outputs.
Run with a Python environment containing playwright and Chromium installed.
"""
import base64
import copy
import json
import math
import mimetypes
import os
import time
from pathlib import Path
from urllib.parse import urlparse, parse_qs
from playwright.sync_api import sync_playwright, expect

# Allow the same request timeout as the UI plus rendering time on a Pi.
expect.set_options(timeout=15000)

ROOT = Path(__file__).resolve().parents[1] / 'www'
NOW = int(time.time())
SETTINGS = {
    'setup': {'setup_name': 'AOG Grow System', 'relay_labels': ['Lights + Air', 'Drain pump', 'Fill pump', 'Tank-to-tank pump'], 'relay_roles': ['lights_air','draining','filling','tank_cycle'], 'overflow': {'sensor_labels': ['Overflow sensor 1', 'Overflow sensor 2'], 'filling': [True, True], 'draining': [True, True], 'tank_cycle': [True, True]}},
    'pump': {'continuous_mode': False, 'photo_cycle_enabled': False, 'photo_cycle_start_hour': 6, 'photo_cycle_end_hour': 24, 'safety_gpio_pin': None, 'pump_runtime_limit_seconds': 300, 'pump_cooldown_seconds': 60},
    'ph': {'enabled': True, 'sensor_type': 'arduino', 'serial_port': None, 'i2c_address': None, 'optimal_min': 6.5, 'optimal_max': 7.5, 'critical_min': 5.5, 'critical_max': 8.5, 'alert_enabled': True, 'auto_adjustment_enabled': True, 'monitoring_interval_seconds': 60, 'history_size': 1440},
    'schedules': {'lighting': {'enabled': False, 'start_minute': 360, 'end_minute': 1320}, 'watering': {'pump': 'filling', 'enabled': False, 'start_minute': 360, 'end_minute': 1320, 'every_minutes': 60, 'run_seconds': 30}},
    'network': {'dashboard_port': 8443, 'command_port': 9443, 'command_token_configured': False},
    'version': '0.2.0', 'device_time': '2026-09-22T10:30:00-04:00',
}

class Device:
    def __init__(self):
        self.settings = copy.deepcopy(SETTINGS)
        self.outputs = {'outputs': [{'channel': i, 'on': False, 'remaining_seconds': None, 'cooldown_seconds': 0, 'maximum_seconds': None} for i in range(1, 5)], 'error': None, 'pump_block_reason': None, 'last_event': 'Ready for manual timed runs', 'stop_pending': False, 'automation_paused': False, 'next_watering_in_seconds': None}
        self.history = [{'timestamp': NOW - (119-i)*60, 'ph_value': 6.85 + .15*math.sin(i/15), 'raw_value': 6.85, 'temperature': 24.5, 'alert_level': 'Normal'} for i in range(120)]
        self.requests = []
        self.ph_error = False
        self.stats_error = False
        self.cameras = []
        self.camera_error = False
        self.authenticated = True

    def output_status(self):
        for output in self.outputs['outputs']:
            role = self.settings['setup']['relay_roles'][output['channel']-1]
            output['role'] = role
            output['maximum_seconds'] = None
            output['blocked_reason'] = 'Assign a function in Setup settings' if role == 'unassigned' else self.outputs['pump_block_reason'] if role in ['filling','draining','tank_cycle'] else None
            output['schedule_controlled'] = role == 'lights_air' and self.settings['schedules']['lighting']['enabled'] and not self.outputs['automation_paused']
        return self.outputs

    def ph(self):
        status = {'current_ph': self.history[-1]['ph_value'] if self.history else None, 'average_ph': 6.85 if self.history else None, 'trend': 'Stable', 'alert_level': 'Normal', 'last_calibration': 0, 'calibration_valid': False, 'adjustment_suggestion': '', 'last_reading': NOW if self.history else None, 'reading_count': len(self.history), 'enabled': True}
        return {'status': status, 'history': self.history, 'calibration': {'point_4': None, 'point_7': None, 'point_10': None, 'slope': 1.0, 'offset': 0.0, 'last_calibration': 0}, 'error': None, 'config': self.settings['ph'], 'raw_value': 6.85, 'server_time': NOW}

    def sensor_history(self, hours):
        series = []
        for key, label, unit, base in [('co2', 'CO₂', 'ppm', 820), ('tvoc', 'TVOC', 'ppb', 18), ('pm25', 'PM2.5', 'µg/m³', 4.2), ('pm10', 'PM10', 'µg/m³', 6.1), ('temp', 'Temperature', '°C', 24.5), ('hum', 'Humidity', '%', 62)]:
            points = [{'timestamp': NOW-(119-i)*60, 'value': base + math.sin(i/15)} for i in range(120) if NOW-(119-i)*60 >= NOW-hours*3600]
            values = [point['value'] for point in points]
            series.append({'key': key, 'label': label, 'unit': unit, 'available': True, 'latest': points[-1], 'count': len(points), 'minimum': min(values), 'maximum': max(values), 'average': sum(values)/len(values), 'points': points})
        return {'server_time': NOW, 'range_hours': hours, 'sample_interval_seconds': 60, 'retention_hours': 168, 'error': None, 'series': series}

    def route(self, route):
        request = route.request
        path = urlparse(request.url).path
        def reply(data, status=200):
            route.fulfill(status=status, content_type='application/json', body=json.dumps(data))
        if path.startswith('/api/'):
            if not self.authenticated:
                reply({'error': 'Sign in'}, 401); return
            if request.method == 'POST':
                data = request.post_data_json
                self.requests.append((path, data))
                assert request.headers.get('x-aog-csrf') == 'browser-test-token', request.headers
                if path == '/api/settings':

                    for key, value in data.items(): self.settings[key].update(value)
                    reply(self.settings); return
                if path == '/api/account/password':
                    if data['current_password'] != 'OldPassword123!':
                        reply({'error': 'Current password is incorrect'}, 403); return
                    self.authenticated = False; reply({'message': 'Password changed'}); return
                if path in ['/api/outputs/stop-all', '/api/schedules/pause']:
                    self.outputs['automation_paused'] = True
                    for output in self.outputs['outputs']: output['on'] = False
                    self.outputs['last_event'] = 'All outputs stopped'; reply(self.output_status()); return
                if path == '/api/schedules/resume':
                    self.outputs['automation_paused'] = False; reply(self.output_status()); return
                if path == '/api/outputs/start':
                    output = self.outputs['outputs'][data['channel']-1]
                    output['on'] = True; output['remaining_seconds'] = data.get('seconds'); reply(self.output_status()); return
                if path == '/api/outputs/stop':
                    self.outputs['outputs'][data['channel']-1]['on'] = False
                    self.outputs['automation_paused'] = True; reply(self.output_status()); return
                reply({'error': 'Unexpected action'}, 404); return
            if path == '/api/session': reply({'username': 'admin', 'csrf_token': 'browser-test-token'})
            elif path == '/api/settings': reply(self.settings)
            elif path == '/api/outputs': reply(self.output_status())
            elif path == '/api/cameras': reply({'error': 'Camera status temporarily unavailable'}, 503) if self.camera_error else reply(self.cameras)
            elif path == '/api/ph': reply({'error': 'Sensor history temporarily unavailable'}, 503) if self.ph_error else reply(self.ph())
            elif path == '/api/history': reply(self.sensor_history(int(parse_qs(urlparse(request.url).query).get('hours', ['24'])[0])))
            elif path == '/api/stats': reply({'error': 'Readings temporarily unavailable'}, 503) if self.stats_error else reply({'temp':'24.50C', 'hum':'62.00%', 'co2':'820.00ppm', 'pm25':'4.2', 'pm10':'6.1', 'tvoc':'18ppb', 't1_ovf':'NONE', 't2_ovf':'NONE', 'overflow_error':False, 'ph':'6.85', 'ph_status':None, 'device_time':self.settings['device_time']})
            else: reply({'error': 'Unexpected API'}, 404)
            return
        file = ROOT / (path.lstrip('/') or 'index.html')
        if file.is_file(): route.fulfill(status=200, body=file.read_bytes(), content_type=mimetypes.guess_type(file.name)[0] or 'application/octet-stream')
        else: route.fulfill(status=404, body='Not found')

def open_page(browser, device, width=1440, height=1080, path='index.html'):
    context = browser.new_context(viewport={'width':width, 'height':height})
    context.route('https://aog.test/**', device.route)
    page = context.new_page(); errors=[]
    failed_requests = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.on('requestfailed', lambda request: failed_requests.append({'url': request.url, 'error': request.failure}))
    page.goto(f'https://aog.test/{path}')
    try:
        # A cold Pi can need the first automatic retry after a request timeout.
        expect(page.locator('#connection')).to_contain_text('Connected', timeout=30000)
        expect(page.locator('#ph-trend')).to_contain_text('Stable')
    except Exception:
        print('Startup diagnostics:', json.dumps({
            'url': page.url, 'page_errors': errors, 'failed_requests': failed_requests,
            'connection': page.locator('#connection').text_content(),
            'setup': page.locator('#global-message').text_content(),
            'outputs': page.locator('#outputs-error').text_content(),
        }), flush=True)
        page.screenshot(path='/tmp/aog-ui-startup-failure.png', full_page=True)
        raise
    return context, page, errors

def navigate(page, destination):
    toggle = page.locator('#navigation-toggle')
    if toggle.is_visible() and toggle.get_attribute('aria-expanded') != 'true':
        toggle.click()
    previous = page.locator('[data-page][aria-current=page]').get_attribute('data-page')
    page.click(f'[data-page={destination}]')
    expect(page.locator(f'#page-{destination}')).to_be_visible()
    expect(page.locator(f'[data-page={destination}]')).to_have_attribute('aria-current', 'page')
    if destination != previous:
        expect(page.locator('#page-title')).to_be_focused()
    if toggle.is_visible():
        expect(toggle).to_have_attribute('aria-expanded', 'false')

def assert_fits(page):
    dimensions = page.evaluate('({viewport: innerWidth, content: document.documentElement.scrollWidth})')
    assert dimensions['content'] <= dimensions['viewport'], f'Horizontal overflow on {page.url}: {dimensions}'
    escaped_fields = page.evaluate('''() => [...document.querySelectorAll('.page:not([hidden]) .panel input, .page:not([hidden]) .panel select, .page:not([hidden]) .panel textarea')].filter(input => {
        const bounds = input.getBoundingClientRect(), panel = input.closest('.panel').getBoundingClientRect();
        return bounds.width && (bounds.left < panel.left || bounds.right > panel.right);
    }).map(input => input.id)''')
    assert not escaped_fields, f'Form fields escape their panels on {page.url}: {escaped_fields}'

def check_reports(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    exports = []
    pending = []
    response_mode = {'value': 'success'}
    bodies = {
        'csv': ('text/csv', 'timestamp,co2\n2026-09-24T12:00:00Z,820\n'),
        'json': ('application/json', '{"samples":[{"co2":820}],"units":{"co2":"ppm"}}'),
        'html': ('text/html', '<!doctype html><html lang="en"><title>Simulated air report</title><h1>Recorded readings</h1><p>CO₂: 820 ppm</p></html>'),
    }

    def export_route(route):
        query = parse_qs(urlparse(route.request.url).query)
        exports.append(query)
        if response_mode['value'] == 'pending':
            pending.append(route)
            return
        if response_mode['value'] == 'error':
            route.fulfill(status=503, content_type='application/json', body='{"error":"Recorded data temporarily unavailable"}')
            return
        if response_mode['value'] == 'wrong-type':
            route.fulfill(status=200, content_type='text/html', body='<html><h1>Sign in</h1></html>')
            return
        kind = query.get('format', ['html'])[0]
        content_type, body = bodies[kind]
        route.fulfill(status=200, content_type=content_type, body=body,
                      headers={'Content-Disposition': f'attachment; filename="recorded-readings.{kind}"'})

    context.route('https://aog.test/api/air-quality/**', export_route)
    navigate(page, 'history')
    page.click('[data-history-range="6"]')
    page.click('#history-to-reports')
    expect(page.locator('#page-reports')).to_be_visible()
    expect(page.locator('#report-range')).to_have_value('6')
    expect(page.locator('#report-selection-summary')).to_contain_text('Last 6 hours')
    page.uncheck('#report-current')
    page.select_option('#report-range', 'current')
    expect(page.locator('#report-current')).to_be_checked()
    expect(page.locator('#report-current')).to_be_disabled()
    page.select_option('#report-range', '24')
    expect(page.locator('#report-current')).not_to_be_checked()
    expect(page.locator('#report-current')).to_be_enabled()
    expect(page.locator('#report-selection-summary')).to_contain_text('historical readings only')

    page.select_option('#report-range', 'custom')
    page.fill('#report-start', '')
    page.click('#export-csv')
    expect(page.locator('#export-result')).to_have_class('form-result error')
    expect(page.locator('#report-start')).to_be_focused()
    expect(page.locator('#report-start')).to_have_attribute('aria-invalid', 'true')
    assert not exports, 'Invalid report dates must not make export requests'
    custom = page.evaluate('''() => {
        const local = date => new Date(date.getTime() - date.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
        return {start: local(new Date(Date.now() - 7200000)), end: local(new Date(Date.now() - 60000))};
    }''')
    page.fill('#report-start', custom['start'])
    page.fill('#report-end', custom['start'])
    page.click('#report-html')
    expect(page.locator('#report-end')).to_be_focused()
    expect(page.locator('#report-end')).to_have_attribute('aria-invalid', 'true')
    assert not exports, 'An empty custom period must not download a report'
    page.fill('#report-end', custom['end'])
    assert page.locator('#report-end').get_attribute('aria-invalid') is None
    page.fill('#report-title', '  Kitchen air & light  ')
    page.fill('#report-notes', '  A change in ventilation.  ')

    response_mode['value'] = 'pending'
    page.click('#export-csv')
    for selector in ['#export-csv', '#export-json', '#report-html', '#report-preview']:
        expect(page.locator(selector)).to_be_disabled()
    expect(page.locator('#export-csv')).to_have_attribute('aria-busy', 'true')
    assert len(pending) == 1
    query = exports[-1]
    assert query['current'] == ['0'] and query['title'] == ['Kitchen air & light']
    assert query['notes'] == ['A change in ventilation.']
    assert int(query['start'][0]) < int(query['end'][0])
    with page.expect_download() as downloaded:
        pending.pop().fulfill(status=200, content_type='text/csv', body=bodies['csv'][1],
                              headers={'Content-Disposition': 'attachment; filename="recorded-readings.csv"'})
    download = downloaded.value
    assert download.suggested_filename == 'recorded-readings.csv'
    assert Path(download.path()).read_text() == bodies['csv'][1]
    expect(page.locator('#export-result')).to_contain_text('CSV download is ready')
    for selector in ['#export-csv', '#export-json', '#report-html', '#report-preview']:
        expect(page.locator(selector)).to_be_enabled()
    assert page.locator('#export-csv').get_attribute('aria-busy') is None

    response_mode['value'] = 'error'
    page.click('#report-html')
    expect(page.locator('#report-result')).to_contain_text('Recorded data temporarily unavailable')
    expect(page.locator('#report-html')).to_be_enabled()
    expect(page.locator('#report-notes')).to_have_value('  A change in ventilation.  ')
    with page.expect_popup() as opened:
        page.click('#report-preview')
    failed_preview = opened.value
    expect(page.locator('#report-result')).to_contain_text('Recorded data temporarily unavailable')
    assert failed_preview.is_closed(), 'A failed preview must not leave a blank report tab'
    response_mode['value'] = 'wrong-type'
    page.click('#export-json')
    expect(page.locator('#export-result')).to_have_class('form-result error')
    expect(page.locator('#export-json')).to_be_enabled()

    response_mode['value'] = 'success'
    with page.expect_download() as downloaded:
        page.click('#export-json')
    assert json.loads(Path(downloaded.value.path()).read_text())['samples'][0]['co2'] == 820
    with page.expect_download() as downloaded:
        page.click('#report-html')
    assert 'Recorded readings' in Path(downloaded.value.path()).read_text()
    with page.expect_popup() as opened:
        page.click('#report-preview')
    preview = opened.value
    expect(preview.locator('h1')).to_have_text('Recorded readings')
    assert preview.evaluate('window.opener === null'), 'Report previews must not retain an opener'
    preview.close()
    page.screenshot(path='/tmp/aog-ui-reports-validated.png', full_page=True)
    previous_exports = len(exports)
    page.evaluate('window.open = () => null')
    page.click('#report-preview')
    expect(page.locator('#report-result')).to_contain_text('Download report')
    assert len(exports) == previous_exports, 'Blocked previews should offer a download without fetching a hidden report'
    assert not errors, errors
    assert not device.requests, 'Report workflows must not change the device'
    context.close()

def check_cameras(browser):
    device = Device()
    device.cameras = [
        {'id': camera_id, 'label': label, 'detected': camera_id == 'ribbon',
         'stream_url': f'/api/cameras/{camera_id}/stream', 'live': camera_id == 'ribbon',
         'running': camera_id == 'ribbon', 'timestamp_ms': NOW * 1000, 'error': None,
         'viewers': 1, 'width': 640, 'height': 480, 'fps': 10}
        for camera_id, label in [('ribbon', 'Raspberry Pi camera'), ('usb', 'USB webcam')]
    ]
    context, page, errors = open_page(browser, device)
    stream_mode = {'fail': False}
    streams = []
    # A tiny image stands in for an authenticated camera stream; no camera is opened.
    frame = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=')

    def stream_route(route):
        streams.append(route.request.url)
        if stream_mode['fail']:
            route.abort('failed')
        else:
            route.fulfill(status=200, content_type='image/png', body=frame)

    context.route('https://aog.test/api/cameras/*/stream?*', stream_route)
    navigate(page, 'cameras')
    expect(page.locator('#camera-status-ribbon')).to_have_text('Live')
    expect(page.locator('#camera-viewport-ribbon img')).to_have_count(1)
    page.wait_for_function("document.querySelector('#camera-viewport-ribbon img')?.naturalWidth === 1")
    expect(page.locator('#camera-status-usb')).to_have_text('Not detected')
    expect(page.locator('#camera-detail-usb')).to_contain_text('USB')
    expect(page.locator('#camera-fullscreen-usb')).to_be_disabled()
    pause = page.locator('#camera-toggle-ribbon')
    expect(pause).to_have_attribute('aria-pressed', 'false')
    pause.click()
    expect(pause).to_have_attribute('aria-pressed', 'true')
    expect(page.locator('#camera-status-ribbon')).to_have_text('Paused')
    expect(page.locator('#camera-viewport-ribbon img')).to_have_count(0)
    expect(page.locator('#camera-fullscreen-ribbon')).to_be_disabled()
    pause.click()
    expect(page.locator('#camera-status-ribbon')).to_have_text('Live')
    assert len(streams) >= 2, 'Resuming a paused view should reconnect its stream'
    pause.click()
    stream_mode['fail'] = True
    pause.click()
    expect(page.locator('#camera-status-ribbon')).to_have_text('Reconnecting')
    expect(page.locator('#camera-placeholder-ribbon')).to_be_visible()
    expect(page.locator('#camera-fullscreen-ribbon')).to_be_disabled()
    stream_mode['fail'] = False
    pause.click(); pause.click()
    expect(page.locator('#camera-status-ribbon')).to_have_text('Live')

    device.camera_error = True
    page.click('#refresh-button')
    expect(page.locator('#cameras-error')).to_contain_text('Camera status temporarily unavailable')
    expect(page.locator('#camera-status-ribbon')).to_have_text('Offline')
    expect(page.locator('#camera-viewport-ribbon img')).to_have_count(0)
    expect(page.locator('#camera-fullscreen-ribbon')).to_be_disabled()
    device.camera_error = False
    page.click('#refresh-button')
    expect(page.locator('#camera-status-ribbon')).to_have_text('Live')
    expect(page.locator('#cameras-error')).not_to_be_visible()
    navigate(page, 'overview')
    expect(page.locator('.camera-stream')).to_have_count(0)
    assert not errors, errors
    assert not device.requests, 'Viewing cameras must not change device outputs'
    context.close()

def check_chart_inspection(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    for destination, chart_id, tooltip_id, response_url in [
        ('overview', 'ph-chart-svg', 'chart-tooltip', '**/api/ph'),
        ('history', 'history-chart-co2', 'history-tooltip-co2', '**/api/history?*'),
    ]:
        navigate(page, destination)
        chart = page.locator(f'#{chart_id}')
        tooltip = page.locator(f'#{tooltip_id}')
        marker = chart.locator('.chart-inspection')
        chart.focus()
        expect(tooltip).to_be_visible()
        expect(marker).to_have_attribute('visibility', 'visible')
        chart.press('Home')
        first = tooltip.text_content()
        first_x = marker.locator('circle').get_attribute('cx')
        chart.press('End')
        assert tooltip.text_content() != first
        assert marker.locator('circle').get_attribute('cx') != first_x
        chart.press('Home')
        page.screenshot(path=f'/tmp/aog-ui-{destination}-inspection.png', full_page=True)
        with page.expect_response(response_url):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(tooltip).to_have_text(first)
        expect(tooltip).to_be_visible()
        page.set_viewport_size({'width': 390, 'height': 844})
        expect(tooltip).to_have_text(first)
        expect(marker).to_have_attribute('visibility', 'visible')
        chart.dispatch_event('pointerleave')
        expect(tooltip).to_be_visible()
        chart.press('Escape')
        expect(tooltip).not_to_be_visible()
        expect(marker).to_have_attribute('visibility', 'hidden')
        chart.press('ArrowRight')
        expect(tooltip).to_be_visible()
        chart.press('Tab')
        expect(tooltip).to_be_visible()
        page.locator('#refresh-button').focus()
        expect(tooltip).not_to_be_visible()
        expect(marker).to_have_attribute('visibility', 'hidden')
        assert_fits(page)
        page.set_viewport_size({'width': 1440, 'height': 1080})
    assert not errors, errors
    context.close()

def check_sensor_navigation(browser):
    device = Device()
    context, page, errors = open_page(browser, device, path='index.html#history/temp')
    expect(page.locator('.history-card:visible')).to_have_count(1)
    expect(page.locator('#history-card-temp')).to_be_visible()
    expect(page.locator('[data-history-sensor=temp]')).to_have_attribute('aria-current', 'location')
    expect(page.locator('#history-selection')).to_have_text('Temperature · Last 24 hours')
    expect(page.locator('#page-title')).to_be_focused()
    chart = page.locator('#history-chart-temp')
    chart.focus(); chart.press('Home')
    expect(page.locator('#history-tooltip-temp')).to_be_visible()
    page.click('[data-history-sensor=hum]')
    expect(page.locator('#history-card-hum')).to_be_visible()
    expect(page.locator('#history-tooltip-temp')).not_to_be_visible()
    page.go_back()
    expect(page.locator('#history-card-temp')).to_be_visible()
    page.go_forward()
    expect(page.locator('#history-card-hum')).to_be_visible()
    with page.expect_response('**/api/history?hours=6'):
        page.click('[data-history-range="6"]')
    page.click('[data-history-sensor=temp]')
    expect(page.locator('#history-selection')).to_have_text('Temperature · Last 6 hours')
    expect(page.locator('[data-history-range="6"]')).to_have_attribute('aria-pressed', 'true')
    expect(page.locator('#history-chart-temp path')).to_have_count(1)
    page.screenshot(path='/tmp/aog-ui-sensor-focused-desktop.png', full_page=True)
    for width in [390, 320]:
        page.set_viewport_size({'width': width, 'height': 844})
        assert_fits(page)
        page.click('[data-history-sensor=""]')
        expect(page.locator('.history-card:visible')).to_have_count(6)
        page.click('[data-history-sensor=pm25]')
        expect(page.locator('.history-card:visible')).to_have_count(1)
        expect(page.locator('#history-selection')).to_have_text('PM2.5 · Last 6 hours')
        assert_fits(page)
        if width == 390:
            page.screenshot(path='/tmp/aog-ui-sensor-focused-mobile.png', full_page=True)
        navigate(page, 'overview')
        page.click('.metric-history[href="#history/temp"]')
        expect(page.locator('#history-card-temp')).to_be_visible()
        expect(page.locator('#page-title')).to_be_focused()
        navigate(page, 'overview')
        page.click('.metric-history[href="#overview/ph"]')
        expect(page.locator('#ph-history-title')).to_be_focused()
        bounds = page.locator('#ph-history-title').bounding_box()
        menu = page.locator('.sidebar').bounding_box()
        assert bounds['y'] >= menu['y'] + menu['height'], 'Menu obscures pH history heading'
        page.click('.air-reading[href="#history/tvoc"]')
        expect(page.locator('#history-card-tvoc')).to_be_visible()
    page.click('#history-to-reports')
    expect(page.locator('#report-range')).to_have_value('6')
    page.evaluate("location.hash = '#history/temp'")
    expect(page.locator('#history-chart-temp path')).to_have_count(1)

    mode = {'value': 'failed'}
    history_requests = []
    def history_route(route):
        history_requests.append(route.request.url)
        if mode['value'] == 'failed':
            route.fulfill(status=503, content_type='application/json', body='{"error":"History storage unavailable"}')
            return
        reply = device.sensor_history(6)
        reply['error'] = 'Could not read saved history'
        if mode['value'] == 'empty':
            for series in reply['series']:
                series.update(points=[], count=0, minimum=None, maximum=None, average=None, latest=None)
        route.fulfill(status=200, content_type='application/json', body=json.dumps(reply))
    context.route('https://aog.test/api/history?*', history_route)
    # Selecting the active range must preserve data without an unnecessary fetch.
    page.click('[data-history-range="6"]')
    expect(page.locator('#history-chart-temp path')).to_have_count(1)
    assert not history_requests
    page.click('#refresh-button')
    expect(page.locator('#history-error')).to_contain_text('Showing the last loaded history')
    expect(page.locator('#history-status-temp')).to_have_text('Last loaded')
    expect(page.locator('#history-chart-temp path')).to_have_count(1)
    mode['value'] = 'warning'
    page.click('#refresh-button')
    expect(page.locator('#history-status-temp')).to_have_text('History warning')
    mode['value'] = 'empty'
    page.click('#refresh-button')
    expect(page.locator('#history-empty-temp')).to_have_text('Could not read saved history')
    expect(page.locator('#history-status-temp')).to_have_text('Unavailable')
    expect(page.locator('#history-chart-temp')).not_to_be_visible()
    page.evaluate("location.hash = '#history/invalid-sensor'")
    expect(page.locator('.history-card:visible')).to_have_count(6)
    assert not errors, errors
    assert not device.requests, 'Reading sensor history must not write device settings'
    context.close()

def check_settings_navigation(browser):
    device = Device()
    context, page, errors = open_page(browser, device, 390, 844, 'index.html#settings/ph')
    expect(page.locator('#page-settings')).to_be_visible()
    expect(page.locator('[data-page=settings]')).to_have_attribute('aria-current', 'page')
    expect(page.locator('#ph-settings-form h2')).to_be_focused()
    expect(page.locator('[data-settings-section=ph]')).to_have_attribute('aria-current', 'location')
    page.fill('#ph-min', '8')
    expect(page.locator('[data-settings-section=ph] .section-edit-status')).to_contain_text('Unsaved changes')
    page.click('#ph-settings-form [type=submit]')
    expect(page.locator('#ph-min')).to_be_focused()
    expect(page.locator('#ph-min')).to_have_attribute('aria-invalid', 'true')
    expect(page.locator('#ph-max')).to_have_attribute('aria-invalid', 'true')
    assert {'ph-settings-result', 'ph-limits-help'} <= set(page.locator('#ph-min').get_attribute('aria-describedby').split())
    assert not device.requests, 'Invalid pH limits must not reach the device'
    page.fill('#ph-min', '6.5')
    assert page.locator('#ph-min').get_attribute('aria-invalid') is None
    expect(page.locator('[data-settings-section=ph] .section-edit-status')).to_have_text('')
    page.click('[data-settings-section=relays]')
    expect(page.locator('#setup-form h2')).to_be_focused()
    expect(page).to_have_url('https://aog.test/index.html#settings/relays')
    page.select_option('#relay-role-2', 'lights_air')
    page.click('#setup-form [type=submit]')
    expect(page.locator('#relay-role-1')).to_be_focused()
    expect(page.locator('#relay-role-2')).to_have_attribute('aria-invalid', 'true')
    assert not device.requests, 'Duplicate relay functions must not reach the device'
    page.click('[data-discard-form=setup-form]')
    expect(page.locator('#relay-role-2')).to_have_value('draining')
    assert page.locator('#relay-role-2').get_attribute('aria-invalid') is None
    page.fill('#setup-name-input', '   ')
    page.click('#setup-form [type=submit]')
    expect(page.locator('#setup-name-input')).to_be_focused()
    expect(page.locator('#setup-name-input')).to_have_attribute('aria-invalid', 'true')
    assert not device.requests, 'Whitespace-only names must not reach the device'
    page.click('[data-discard-form=setup-form]')

    pending_settings = []
    def hold_settings(route):
        if route.request.method == 'POST': pending_settings.append(route)
        else: route.fallback()
    context.route('https://aog.test/api/settings', hold_settings)
    page.fill('#setup-name-input', 'A retained setup draft')
    page.click('#setup-form [type=submit]')
    page.click('[data-settings-section=ph]')
    expect(page.locator('#ph-settings-form h2')).to_be_focused()
    assert len(pending_settings) == 1
    pending_settings.pop().fulfill(status=503, content_type='application/json', body='{"error":"Settings temporarily unavailable"}')
    expect(page.locator('#setup-result')).to_contain_text('Settings temporarily unavailable')
    expect(page.locator('#ph-settings-form h2')).to_be_focused()
    expect(page.locator('#setup-name-input')).to_have_value('A retained setup draft')
    page.click('[data-settings-section=relays]')
    page.click('#setup-form [type=submit]')
    page.locator('#overflow-label-1').focus()
    assert len(pending_settings) == 1
    pending_settings.pop().fulfill(status=503, content_type='application/json', body='{"error":"Settings still unavailable"}')
    expect(page.locator('#setup-result')).to_contain_text('Settings still unavailable')
    expect(page.locator('#overflow-label-1')).to_be_focused()
    page.click('[data-discard-form=setup-form]')
    context.unroute('https://aog.test/api/settings', hold_settings)

    page.click('[data-settings-section=calibration]')
    expect(page.locator('#calibration-form h2')).to_be_focused()
    expect(page.locator('#navigation-toggle')).to_be_in_viewport()
    bounds = page.locator('#calibration-form h2').bounding_box()
    sidebar = page.locator('.sidebar').bounding_box()
    assert bounds['y'] >= sidebar['y'] + sidebar['height'], 'Sticky menu hides the selected settings heading'
    page.go_back()
    expect(page.locator('#setup-form h2')).to_be_focused()
    page.go_forward()
    expect(page.locator('#calibration-form h2')).to_be_focused()
    page.screenshot(path='/tmp/aog-ui-settings-sections-mobile.png')
    page.locator('#calibration-form [type=submit]').scroll_into_view_if_needed()
    expect(page.locator('#navigation-toggle')).to_be_in_viewport()
    page.click('#navigation-toggle')
    expect(page.locator('#primary-navigation')).to_be_visible()
    page.click('[data-page=history]')
    expect(page.locator('#page-history')).to_be_visible()
    expect(page.locator('#page-title')).to_be_focused()
    page.set_viewport_size({'width': 740, 'height': 320})
    page.click('#navigation-toggle')
    page.locator('[data-page=account]').scroll_into_view_if_needed()
    expect(page.locator('[data-page=account]')).to_be_in_viewport()
    page.click('[data-page=account]')
    expect(page.locator('#page-account')).to_be_visible()
    expect(page.locator('#navigation-toggle')).to_be_in_viewport()
    assert_fits(page)
    assert not errors, errors
    context.close()

def check_login(browser):
    device = Device()
    context = browser.new_context(viewport={'width': 320, 'height': 740})
    context.route('https://aog.test/**', device.route)
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.goto('https://aog.test/login.html?expired=1')
    expect(page.locator('#login-notice')).to_contain_text('session expired')
    assert_fits(page)
    page.fill('#input_password', 'ExamplePassword123!')
    page.check('#show-password')
    expect(page.locator('#input_password')).to_have_attribute('type', 'text')
    page.uncheck('#show-password')
    expect(page.locator('#input_password')).to_have_attribute('type', 'password')
    assert page.locator('#input_password').evaluate('el => parseFloat(getComputedStyle(el).fontSize) >= 16'), 'Mobile inputs should not trigger focus zoom'

    pending = []
    context.route('https://aog.test/authenticate', lambda route: pending.append(route))
    page.click('#login-form [type=submit]')
    expect(page.locator('#login-form')).to_have_attribute('aria-busy', 'true')
    expect(page.locator('#login-form [type=submit]')).to_be_disabled()
    page.evaluate("document.querySelector('#login-form').requestSubmit()")
    assert len(pending) == 1, 'Duplicate sign-in request while submitting'
    pending.pop().fulfill(status=401, content_type='application/json', body='{"error":"Unauthorized"}')
    expect(page.locator('#login-error')).to_contain_text('Username or password is incorrect')
    expect(page.locator('#login-error')).to_be_focused()
    expect(page.locator('#input_password')).to_have_value('ExamplePassword123!')
    expect(page.locator('#input_password')).to_have_attribute('aria-invalid', 'true')
    expect(page.locator('#login-form [type=submit]')).to_be_enabled()
    page.fill('#input_password', 'CorrectedPassword123!')
    assert page.locator('#input_password').get_attribute('aria-invalid') is None
    page.click('#login-form [type=submit]')
    expect(page.locator('#login-form')).to_have_attribute('aria-busy', 'true')
    assert len(pending) == 1
    pending.pop().abort('failed')
    expect(page.locator('#login-error')).to_contain_text('Could not reach your device')
    expect(page.locator('#login-error')).to_be_focused()
    expect(page.locator('#login-form [type=submit]')).to_be_enabled()
    assert_fits(page)
    page.screenshot(path='/tmp/aog-ui-mobile-login.png', full_page=True)
    page.click('#login-form [type=submit]')
    expect(page.locator('#login-form')).to_have_attribute('aria-busy', 'true')
    assert len(pending) == 1
    pending.pop().fulfill(status=200, content_type='application/json', body='{}')
    expect(page).to_have_url('https://aog.test/index.html')
    expect(page.locator('#connection')).to_contain_text('Connected')
    assert not errors, errors
    context.close()

def run():
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox','--disable-dev-shm-usage'])
        print('Checking dashboard, settings saves, controls, and account flows…', flush=True)
        device=Device(); context,page,errors=open_page(browser,device)
        expect(page.locator('#ph-chart-svg path')).to_have_count(1)
        expect(page.locator('#ph-calibration-badge')).to_have_text('Not calibrated')
        assert not errors, errors
        assert page.locator('.sidebar').evaluate('sidebar => [...sidebar.querySelectorAll("nav a")].every(link => link.getBoundingClientRect().right <= sidebar.getBoundingClientRect().right)'), 'Navigation escapes the desktop sidebar'
        main_top = page.locator('#main').bounding_box()['y']
        page.screenshot(path='/tmp/aog-ui-desktop.png',full_page=True)
        chart = page.locator('#ph-chart-svg')
        chart.focus(); chart.press('Home')
        expect(page.locator('#chart-tooltip')).to_be_visible()
        first_sample = page.locator('#chart-tooltip').text_content()
        chart.press('End')
        assert page.locator('#chart-tooltip').text_content() != first_sample
        chart.press('Escape')
        expect(page.locator('#chart-tooltip')).not_to_be_visible()
        chart.click(position={'x': 100, 'y': 100})
        expect(page.locator('#chart-tooltip')).to_contain_text('pH')
        chart.press('Tab')
        expect(page.locator('#chart-tooltip')).to_be_visible()
        page.locator('#refresh-button').focus()
        expect(page.locator('#chart-tooltip')).not_to_be_visible()
        navigate(page, 'schedules')
        assert abs(page.locator('#main').bounding_box()['y'] - main_top) < 1, 'Page navigation shifts the desktop header vertically'
        page.locator('.skip-link').focus(); page.keyboard.press('Enter')
        expect(page).to_have_url('https://aog.test/index.html#schedules')
        expect(page.locator('#main')).to_be_focused()
        expect(page.locator('#page-schedules')).to_be_visible()
        page.check('#lighting-enabled'); page.check('#watering-enabled')
        page.fill('#lighting-start','22:00'); page.fill('#lighting-end','06:00')
        page.fill('#watering-interval','15'); page.fill('#watering-duration','30')
        page.click('#schedule-form [type=submit]')
        expect(page.locator('#schedule-result')).to_contain_text('Saved')
        assert device.settings['schedules']['lighting']['start_minute']==1320
        assert device.settings['schedules']['watering']['every_minutes']==15
        page.screenshot(path='/tmp/aog-ui-schedules.png',full_page=True)
        page.fill('#watering-interval','1'); page.click('#schedule-form [type=submit]')
        expect(page.locator('#schedule-result')).to_contain_text('Saved')
        expect(page.locator('#schedule-form [type=submit]')).to_be_enabled()
        page.click('#stop-all'); expect(page.locator('#paused-banner')).to_be_visible()
        navigate(page, 'outputs'); page.click('[data-output="1"]')
        expect(page.locator('#output-state-1')).to_have_text('On')
        assert ('/api/outputs/start', {'channel':1}) in device.requests
        page.click('[data-output="1"]'); expect(page.locator('#output-state-1')).to_have_text('Off')
        navigate(page, 'settings')
        form_state = page.locator('[data-form-state="setup-form"]')
        discard = page.locator('[data-discard-form="setup-form"]')
        expect(form_state).to_have_text('All changes saved')
        page.fill('#setup-name-input', 'Unsaved setup')
        expect(form_state).to_have_text('Unsaved changes')
        expect(discard).to_be_visible()
        page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_enabled()
        expect(page.locator('#setup-name-input')).to_have_value('Unsaved setup')
        navigate(page, 'overview'); navigate(page, 'settings')
        expect(page.locator('#setup-name-input')).to_have_value('Unsaved setup')
        posts_before_discard = len(device.requests)
        discard.click()
        expect(page.locator('#setup-name-input')).to_have_value('AOG Grow System')
        expect(form_state).to_have_text('All changes saved')
        expect(discard).not_to_be_visible()
        assert len(device.requests) == posts_before_discard, 'Discard should not save settings'
        page.fill('#setup-name-input', 'Reverted setup')
        page.fill('#setup-name-input', 'AOG Grow System')
        expect(form_state).to_have_text('All changes saved')
        page.fill('#setup-name-input','Kitchen grow')
        page.click('#setup-form [type=submit]'); expect(page.locator('#setup-result')).to_contain_text('Saved')
        expect(page.locator('#setup-name')).to_have_text('Kitchen grow')
        expect(form_state).to_have_text('All changes saved')
        pending_settings = []
        def hold_settings(route):
            if route.request.method == 'POST': pending_settings.append(route)
            else: route.fallback()
        context.route('https://aog.test/api/settings', hold_settings)
        page.fill('#setup-name-input', 'Saved snapshot')
        page.click('#setup-form [type=submit]')
        expect(form_state).to_have_text('Saving changes…')
        expect(discard).to_be_disabled()
        page.fill('#setup-name-input', 'Newer unsaved edit')
        assert len(pending_settings) == 1
        device.route(pending_settings.pop())
        expect(page.locator('#setup-result')).to_contain_text('Earlier changes saved')
        expect(page.locator('#setup-name-input')).to_have_value('Newer unsaved edit')
        expect(form_state).to_have_text('Unsaved changes')
        page.screenshot(path='/tmp/aog-ui-settings-unsaved.png', full_page=True)
        discard.click()
        expect(page.locator('#setup-name-input')).to_have_value('Saved snapshot')
        expect(form_state).to_have_text('All changes saved')
        context.unroute('https://aog.test/api/settings', hold_settings)
        device.settings['setup']['setup_name'] = 'Changed on another browser'
        page.click('#refresh-button')
        expect(page.locator('#setup-name-input')).to_have_value('Changed on another browser')
        expect(form_state).to_have_text('All changes saved')
        navigate(page, 'overview')
        device.stats_error = True
        page.click('#refresh-button')
        expect(page.locator('#connection')).to_contain_text('Offline')
        expect(page.locator('#temperature-value')).to_have_text('—')
        expect(page.locator('#humidity-value')).to_have_text('—')
        expect(page.locator('#co2-value')).to_have_text('—')
        expect(page.locator('#tank1-status')).to_have_text('Unavailable')
        expect(page.locator('#updated-at')).to_contain_text('unavailable')
        device.stats_error = False
        page.click('#refresh-button')
        expect(page.locator('#connection')).to_contain_text('Connected')
        expect(page.locator('#temperature-value')).to_contain_text('24.5')
        device.history=[]; page.click('#refresh-button')
        expect(page.locator('#ph-chart-message')).to_contain_text('No pH history yet')
        expect(page.locator('#ph-chart-svg')).not_to_be_visible()
        expect(page.locator('#ph-trend')).to_have_text('Waiting for readings')
        device.ph_error=True; page.click('#refresh-button')
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        expect(page.locator('#ph-chart-svg')).not_to_be_visible()
        navigate(page, 'settings')
        expect(page.locator('#calibration-live-raw')).to_have_text('Unavailable')
        expect(page.locator('#calibration-raw')).to_have_value('')
        navigate(page, 'overview')
        page.set_viewport_size({'width':1400,'height':1080})
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        page.click('[data-range="6"]')
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        device.ph_error=False
        navigate(page, 'account')
        page.fill('#current-password','WrongPassword123!'); page.fill('#new-password','NewPassword456!'); page.fill('#confirm-password','NewPassword456!')
        page.click('#password-form [type=submit]')
        expect(page.locator('#password-result')).to_contain_text('Current password is incorrect')
        page.fill('#current-password','OldPassword123!'); page.click('#password-form [type=submit]')
        expect(page).to_have_url('https://aog.test/login.html?changed=1')
        expect(page.locator('#login-notice')).to_contain_text('Password changed')
        assert not errors,errors
        context.close()
        for width in [390, 320]:
            device=Device(); context,page,errors=open_page(browser,device,width,844)
            assert_fits(page)
            toggle = page.locator('#navigation-toggle')
            expect(toggle).to_be_visible()
            expect(toggle).to_have_attribute('aria-expanded', 'false')
            expect(page.locator('#primary-navigation')).not_to_be_visible()
            page.screenshot(path=f'/tmp/aog-ui-mobile-{width}.png', full_page=True)
            toggle.click()
            expect(page.locator('#primary-navigation')).to_be_visible()
            assert_fits(page)
            page.screenshot(path=f'/tmp/aog-ui-mobile-menu-{width}.png', full_page=True)
            page.keyboard.press('Escape')
            expect(toggle).to_have_attribute('aria-expanded', 'false')
            expect(toggle).to_be_focused()
            navigate(page, 'overview')
            expect(page.locator('#page-title')).to_be_focused()
            for destination in ['history', 'cameras', 'reports', 'schedules', 'outputs', 'settings', 'account', 'overview']:
                navigate(page, destination)
                assert_fits(page)
                expect(page.locator('#stop-all')).to_be_in_viewport()
                if destination == 'reports':
                    page.select_option('#report-range', 'custom')
                    expect(page.locator('#report-start')).to_be_visible()
                    assert_fits(page)
                if destination == 'schedules':
                    expect(page.locator('#lighting-start')).to_be_visible()
                    page.screenshot(path=f'/tmp/aog-ui-mobile-schedules-{width}.png', full_page=True)
            assert not errors,errors
            context.close()
        from web_ui_workflow_test import check_output_recovery, check_schedule_previews
        from web_ui_drafts_test import check_draft_summary, check_initial_settings_load
        from web_ui_session_test import check_session_return
        from web_ui_calibration_test import check_calibration
        from web_ui_attention_test import check_attention_summary, check_refresh_and_device_clock
        from web_ui_output_actions_test import check_output_actions
        from web_ui_account_test import check_account_requests, check_login_requests, check_login_bookmarks
        from web_ui_chart_steps_test import check_chart_steps
        from web_ui_report_options_test import check_report_options
        from web_ui_settings_conflicts_test import check_settings_conflicts
        for check in [check_login, check_settings_navigation, check_chart_inspection,
                      check_sensor_navigation, check_reports, check_cameras,
                      check_output_recovery, check_schedule_previews,
                      check_draft_summary, check_initial_settings_load,
                      check_session_return, check_calibration, check_attention_summary,
                      check_refresh_and_device_clock, check_output_actions,
                      check_account_requests, check_login_requests, check_login_bookmarks,
                      check_chart_steps, check_report_options, check_settings_conflicts]:
            print(f'Checking {check.__name__.removeprefix("check_").replace("_", " ")}…', flush=True)
            check(browser)
        browser.close()
        print('PASS: chart inspection and polling, single-sensor navigation, schedule previews and output recovery, settings deep links and validation, unsaved edits and save races, report downloads and errors, camera interruption recovery, offline recovery, password/login flows, mobile navigation, desktop and 320/390px layouts, and no uncaught JavaScript errors')

if __name__=='__main__': run()
