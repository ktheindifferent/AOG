"""Browser regression checks with simulated APIs; never switches device outputs.
Run with a Python environment containing playwright and Chromium installed.
"""
import copy
import json
import math
import mimetypes
import os
import time
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright, expect

# Allow the same request timeout as the UI plus rendering time on a Pi.
expect.set_options(timeout=15000)

ROOT = Path(__file__).resolve().parents[1] / 'www'
NOW = int(time.time())
SETTINGS = {
    'setup': {'setup_name': 'AOG Grow System', 'relay_labels': ['Lights & air', 'Drain pump', 'Fill pump', 'Relay 4'], 'relay_roles': ['lights','draining','filling','unassigned']},
    'pump': {'continuous_mode': False, 'photo_cycle_enabled': False, 'photo_cycle_start_hour': 6, 'photo_cycle_end_hour': 24, 'safety_gpio_pin': None, 'pump_runtime_limit_seconds': 300, 'pump_cooldown_seconds': 60},
    'ph': {'enabled': True, 'sensor_type': 'arduino', 'serial_port': None, 'i2c_address': None, 'optimal_min': 6.5, 'optimal_max': 7.5, 'critical_min': 5.5, 'critical_max': 8.5, 'alert_enabled': True, 'auto_adjustment_enabled': True, 'monitoring_interval_seconds': 60, 'history_size': 1440},
    'schedules': {'lighting': {'enabled': False, 'start_minute': 360, 'end_minute': 1320}, 'watering': {'enabled': False, 'start_minute': 360, 'end_minute': 1320, 'every_minutes': 60, 'run_seconds': 30}},
    'network': {'dashboard_port': 8443, 'command_port': 9443, 'command_token_configured': False},
    'version': '0.2.0', 'device_time': '2026-09-22T10:30:00-04:00',
}

class Device:
    def __init__(self):
        self.settings = copy.deepcopy(SETTINGS)
        self.outputs = {'outputs': [{'channel': i, 'on': False, 'remaining_seconds': None, 'cooldown_seconds': 0, 'maximum_seconds': 86400 if i == 1 else 300} for i in range(1, 5)], 'error': None, 'pump_block_reason': None, 'last_event': 'Ready for manual timed runs', 'stop_pending': False, 'automation_paused': False, 'next_watering_in_seconds': None}
        self.history = [{'timestamp': NOW - (119-i)*60, 'ph_value': 6.85 + .15*math.sin(i/15), 'raw_value': 6.85, 'temperature': 24.5, 'alert_level': 'Normal'} for i in range(120)]
        self.requests = []
        self.ph_error = False
        self.authenticated = True

    def output_status(self):
        for output in self.outputs['outputs']:
            role = self.settings['setup']['relay_roles'][output['channel']-1]
            output['role'] = role
            output['maximum_seconds'] = 86400 if role in ['lights','air'] else 0 if role == 'unassigned' else min(300 if role == 'filling' else 600, self.settings['pump']['pump_runtime_limit_seconds'])
            output['blocked_reason'] = 'Assign a function in Setup settings' if role == 'unassigned' else self.outputs['pump_block_reason'] if role in ['filling','draining'] else None
            output['schedule_controlled'] = role == 'lights' and self.settings['schedules']['lighting']['enabled'] and not self.outputs['automation_paused']
        return self.outputs

    def ph(self):
        status = {'current_ph': self.history[-1]['ph_value'] if self.history else None, 'average_ph': 6.85 if self.history else None, 'trend': 'Stable', 'alert_level': 'Normal', 'last_calibration': 0, 'calibration_valid': False, 'adjustment_suggestion': '', 'last_reading': NOW if self.history else None, 'reading_count': len(self.history), 'enabled': True}
        return {'status': status, 'history': self.history, 'calibration': {'point_4': None, 'point_7': None, 'point_10': None, 'slope': 1.0, 'offset': 0.0, 'last_calibration': 0}, 'error': None, 'config': self.settings['ph'], 'raw_value': 6.85, 'server_time': NOW}

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
                    if 'schedules' in data and data['schedules']['watering']['every_minutes'] == 1:
                        reply({'error': 'Watering interval must include cooldown'}, 400); return
                    self.settings.update(data); reply(self.settings); return
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
                    output['on'] = True; output['remaining_seconds'] = data['seconds']; reply(self.output_status()); return
                if path == '/api/outputs/stop':
                    self.outputs['outputs'][data['channel']-1]['on'] = False
                    self.outputs['automation_paused'] = True; reply(self.output_status()); return
                reply({'error': 'Unexpected action'}, 404); return
            if path == '/api/session': reply({'username': 'admin', 'csrf_token': 'browser-test-token'})
            elif path == '/api/settings': reply(self.settings)
            elif path == '/api/outputs': reply(self.output_status())
            elif path == '/api/cameras': reply([])
            elif path == '/api/ph': reply({'error': 'Sensor history temporarily unavailable'}, 503) if self.ph_error else reply(self.ph())
            elif path == '/api/stats': reply({'temp':'24.50C', 'hum':'62.00%', 'co2':'820.00ppm', 'pm25':'4.2', 'pm10':'6.1', 'tvoc':'18ppb', 't1_ovf':'NONE', 't2_ovf':'NONE', 'overflow_error':False, 'ph':'6.85', 'ph_status':None, 'device_time':self.settings['device_time']})
            else: reply({'error': 'Unexpected API'}, 404)
            return
        file = ROOT / (path.lstrip('/') or 'index.html')
        if file.is_file(): route.fulfill(status=200, body=file.read_bytes(), content_type=mimetypes.guess_type(file.name)[0] or 'application/octet-stream')
        else: route.fulfill(status=404, body='Not found')

def open_page(browser, device, width=1440, height=1080):
    context = browser.new_context(viewport={'width':width, 'height':height})
    context.route('https://aog.test/**', device.route)
    page = context.new_page(); errors=[]
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.goto('https://aog.test/index.html')
    expect(page.locator('#connection')).to_contain_text('Connected')
    expect(page.locator('#ph-trend')).to_contain_text('Stable')
    return context, page, errors

def run():
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox','--disable-dev-shm-usage'])
        device=Device(); context,page,errors=open_page(browser,device)
        expect(page.locator('#ph-chart-svg polyline')).to_have_count(1)
        expect(page.locator('#ph-calibration-badge')).to_have_text('Not calibrated')
        assert not errors, errors
        page.screenshot(path='/tmp/aog-ui-desktop.png',full_page=True)
        page.click('[data-page=schedules]')
        page.check('#lighting-enabled'); page.check('#watering-enabled')
        page.fill('#lighting-start','22:00'); page.fill('#lighting-end','06:00')
        page.fill('#watering-interval','15'); page.fill('#watering-duration','30')
        page.click('#schedule-form [type=submit]')
        expect(page.locator('#schedule-result')).to_contain_text('Saved')
        assert device.settings['schedules']['lighting']['start_minute']==1320
        assert device.settings['schedules']['watering']['every_minutes']==15
        page.screenshot(path='/tmp/aog-ui-schedules.png',full_page=True)
        page.fill('#watering-interval','1'); page.click('#schedule-form [type=submit]')
        expect(page.locator('#schedule-result')).to_contain_text('cooldown')
        expect(page.locator('#schedule-form [type=submit]')).to_be_enabled()
        page.click('#stop-all'); expect(page.locator('#paused-banner')).to_be_visible()
        page.click('[data-page=outputs]'); page.click('[data-start="1"]')
        expect(page.locator('#run-dialog')).to_be_visible()
        page.fill('#run-seconds','10'); page.click('#run-form [type=submit]')
        expect(page.locator('#run-dialog')).not_to_be_visible()
        expect(page.locator('#output-state-1')).to_have_text('On')
        assert ('/api/outputs/start', {'channel':1, 'seconds':10}) in device.requests
        page.click('[data-stop="1"]'); expect(page.locator('#output-state-1')).to_have_text('Off')
        page.click('[data-page=settings]'); page.fill('#setup-name-input','Kitchen grow')
        page.click('#setup-form [type=submit]'); expect(page.locator('#setup-result')).to_contain_text('Saved')
        expect(page.locator('#setup-name')).to_have_text('Kitchen grow')
        page.click('[data-page=overview]')
        device.history=[]; page.click('#refresh-button')
        expect(page.locator('#ph-chart-message')).to_contain_text('No pH history yet')
        expect(page.locator('#ph-chart-svg')).not_to_be_visible()
        expect(page.locator('#ph-trend')).to_have_text('Waiting for readings')
        device.ph_error=True; page.click('#refresh-button')
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        expect(page.locator('#ph-chart-svg')).not_to_be_visible()
        page.set_viewport_size({'width':1400,'height':1080})
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        page.click('[data-range="6"]')
        expect(page.locator('#ph-chart-message')).to_contain_text('temporarily unavailable')
        device.ph_error=False
        page.click('[data-page=account]')
        page.fill('#current-password','WrongPassword123!'); page.fill('#new-password','NewPassword456!'); page.fill('#confirm-password','NewPassword456!')
        page.click('#password-form [type=submit]')
        expect(page.locator('#password-result')).to_contain_text('Current password is incorrect')
        page.fill('#current-password','OldPassword123!'); page.click('#password-form [type=submit]')
        expect(page).to_have_url('https://aog.test/login.html?changed=1')
        expect(page.locator('#login-notice')).to_contain_text('Password changed')
        assert not errors,errors
        context.close()
        device=Device(); context,page,errors=open_page(browser,device,390,844)
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile page overflows horizontally'
        page.screenshot(path='/tmp/aog-ui-mobile.png',full_page=True)
        page.click('[data-page=schedules]'); expect(page.locator('#lighting-start')).to_be_visible()
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Mobile schedules overflow horizontally'
        page.screenshot(path='/tmp/aog-ui-mobile-schedules.png',full_page=True)
        page.click('[data-page=account]'); expect(page.locator('#new-password')).to_be_visible()
        assert not errors,errors
        context.close();browser.close()
        print('PASS: pH with no calibration, empty/error states, schedule save and validation, output controls, setup save, password flow, desktop/mobile layout, and no uncaught JavaScript errors')

if __name__=='__main__': run()
