"""Unconfirmed analog pH remains diagnostics; only explicit wiring confirmation enables it."""
import json,os,time
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright,expect
from web_ui_hardware_test import Hardware
from web_ui_test import navigate
from web_ui_appearance_test import audit

class PhDevice(Hardware):
    def __init__(self):
        super().__init__()
        self.ph_input={'connected':False,'confirmed_at':0}
        self.recorded=False
    def ph(self):
        data=super().ph()
        data['history']=data['history'][-1:] if self.recorded else []
        data['status'].update(reading_count=len(data['history']),calibration_points=0)
        if not self.ph_input['connected']:
            data['raw_value']=None
            data['status'].update(current_ph=None,last_reading=None,adjustment_suggestion='',average_ph=None)
            data['error']='pH input is unverified. Connect the Gravity board and confirm the physical connection in Connections.'
        data['history_warning']='Unverified pH records are excluded from this chart and its averages. The original records remain stored for diagnosis.'
        data['excluded_unverified_records']=2
        return data
    def route(self,route):
        if urlparse(route.request.url).path=='/api/ph-input':
            assert route.request.method=='POST'
            assert route.request.headers.get('x-aog-csrf')=='browser-test-token'
            body=route.request.post_data_json
            self.requests.append(('/api/ph-input',body))
            self.ph_input={'connected':body['connected'],'confirmed_at':int(time.time())}
            self.recorded|=body['connected']
            route.fulfill(status=200,content_type='application/json',body=json.dumps(self.ph_input));return
        super().route(route)

def check(browser,width):
    device=PhDevice()
    context=browser.new_context(viewport={'width':width,'height':900})
    context.route('https://aog.test/**',device.route)
    page=context.new_page();errors=[]
    page.on('pageerror',lambda error:errors.append(str(error)))
    try:
        page.goto('https://aog.test/index.html')
        expect(page.locator('#connection')).to_contain_text('Connected')
        expect(page.locator('#ph-value')).to_have_text('—')
        expect(page.locator('#ph-detail')).to_contain_text('unverified')
        expect(page.locator('#ph-history-warning')).to_be_visible()
        for theme in ['light','dark']:
            page.emulate_media(color_scheme=theme)
            audit(page,f'ph-unverified-{theme}-{width}')
        navigate(page,'connections')
        page.click('[data-board=DUAL_OVF_SENSOR]')
        page.click('[data-trace=ph]')
        expect(page.locator('#trace-value')).to_have_text('313 ADC counts')
        expect(page.locator('#trace-status')).to_have_text('Connection not confirmed')
        page.click('#trace-ph-setup')
        expect(page.locator('#ph-tools')).to_have_attribute('open','')
        expect(page.locator('#ph-tools')).to_contain_text('Arduino A0')
        expect(page.locator('#ph-tools')).to_contain_text('Arduino 5V')
        expect(page.locator('#ph-tools')).to_contain_text('Arduino GND')
        expect(page.locator('#ph-input-enable')).to_be_disabled()
        for theme in ['light','dark']:
            page.emulate_media(color_scheme=theme)
            audit(page,f'ph-wiring-{theme}-{width}')
        assert not device.requests,'Reading diagnostics must not confirm a physical connection'
        page.check('#ph-input-confirm')
        page.click('#ph-input-enable')
        expect(page.locator('#ph-input-result')).to_contain_text('Previous calibration was archived')
        expect(page.locator('#ph-input-disable')).to_be_visible()
        assert device.requests==[('/api/ph-input',{'connected':True})]
        navigate(page,'overview');page.click('#refresh-button')
        expect(page.locator('#ph-value')).not_to_have_text('—')
        expect(page.locator('#ph-history-warning')).to_be_visible()
        navigate(page,'connections')
        page.locator('#ph-tools').evaluate('el=>el.open=true')
        page.click('#ph-input-disable')
        expect(page.locator('#ph-input-enable')).to_be_visible()
        expect(page.locator('#ph-input-enable')).to_be_disabled()
        navigate(page,'overview');page.click('#refresh-button')
        expect(page.locator('#ph-value')).to_have_text('—')
        assert device.requests==[('/api/ph-input',{'connected':True}),('/api/ph-input',{'connected':False})]
        assert not errors,errors
        print(f'PASS: pH exclusion, raw voltage, wiring, explicit enable/disable and accessibility at {width}px',flush=True)
    finally:context.close()

if __name__=='__main__':
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
        try:
            for width in [1440,390,320]:check(browser,width)
        finally:browser.close()
