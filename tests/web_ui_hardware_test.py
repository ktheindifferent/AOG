"""Exercise wiring/status semantics, stale data, probe binding, and update actions."""
import copy, json, os, time
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright, expect
from web_ui_test import Device, open_page, navigate

class Hardware(Device):
    def __init__(self):
        super().__init__()
        self.devices = [
            {'device_id':'SENSORKIT_MK1','port':'/dev/ttyUSB1','observed_at':int(time.time()),'error':None,'fields':{'FIRMWARE_VERSION':'2.0.0','PROTOCOL_VERSION':'2','TEMP':'24.50C','HUM':'62%','DHT_STATUS':'ok','CO2':'820ppm','TVOC':'18ppb','CCS811_STATUS':'ok','CO2_ADC':'428','WATER_D6_STATUS':'not_detected','WATER_D8_STATUS':'not_detected','BUFFER_PENDING':'7','BUFFER_CAPACITY':'24','BUFFER_DROPPED':'0','UPTIME_S':'3900'}},
            {'device_id':'DUAL_OVF_SENSOR','port':'/dev/ttyUSB0','observed_at':int(time.time()),'error':None,'fields':{'FIRMWARE_VERSION':'2.0.0','PROTOCOL_VERSION':'2','T1_OVF':'NONE','T2_OVF':'OVERFLOW','PH':'6.85','PH_ADC':'313','PH_MV':'1528.3','PH_STATUS':'analog_unverified','WATER_D6_STATUS':'ok','WATER_D6':'21.50C','WATER_D6_ROM':'28010203040506AA','WATER_D8_STATUS':'crc_error','WATER_D8_ROM':'28010203040507BB','BUFFER_PENDING':'0','BUFFER_CAPACITY':'24','BUFFER_DROPPED':'2','UPTIME_S':'1200'}}]
        self.probes=[{'label':'Water · D6','source':None},{'label':'Water · D8','source':None}]
        self.firmware={'package':{'version':'2.0.0','protocol':2,'artifacts':[{'device':d,'board':'nano-old','fqbn':'arduino:avr:nano:cpu=atmega328old','file':f'{d}.hex','sha256':'a'*64} for d in ['SENSORKIT_MK1','DUAL_OVF_SENSOR']]},'updater_available':True,'active':False,'job':{'stage':'','message':''}}
        self.hardware_error=False
        self.offline=set()
    def route(self, route):
        path=urlparse(route.request.url).path
        def reply(data,status=200): route.fulfill(status=status,content_type='application/json',body=json.dumps(data))
        if path=='/api/connections':
            if self.hardware_error: reply({'error':'USB diagnostics unavailable'},503);return
            for device in self.devices:
                if device['device_id'] not in self.offline: device['observed_at']=int(time.time())
            reply({'server_time':int(time.time()),'devices':self.devices,'water_probes':self.probes,'water_readings':[[21.5,int(time.time())],None],'retention_days':7});return
        if path=='/api/firmware':reply(self.firmware);return
        if path in ['/api/firmware/update','/api/water-probes']:
            assert route.request.method=='POST'
            assert route.request.headers.get('x-aog-csrf')=='browser-test-token'
            body=route.request.post_data_json;self.requests.append((path,body))
            if path=='/api/water-probes':self.probes=body;reply({'saved':True})
            else:
                self.firmware['active']=True;self.firmware['job']={'stage':'draining','message':'Saving the Arduino’s buffered readings before reset'}
                reply({'started':True},202)
            return
        super().route(route)

def run(browser,width):
    d=Hardware();context,page,errors=open_page(browser,d,width=width)
    navigate(page,'connections')
    expect(page.locator('#board-port')).to_have_text('/dev/ttyUSB1')
    expect(page.locator('#board-buffer')).to_have_text('7 / 24 readings')
    expect(page.locator('#sensor-connections')).to_contain_text('Analog · unverified')
    page.click('[data-board=DUAL_OVF_SENSOR]')
    expect(page.locator('#sensor-connections')).to_contain_text('crc error')
    expect(page.locator('#sensor-connections')).to_contain_text('DS18B20 family detected')
    expect(page.locator('#sensor-connections')).to_contain_text('Circuit open')
    expect(page.locator('#pin-d2-label')).to_have_text('Overflow switch 1')
    expect(page.locator('#board-dropped')).to_have_text('2 since board reset')
    page.fill('#water-label-0','Display tank')
    page.select_option('#water-source-0','DUAL_OVF_SENSOR')
    page.click('#hardware-refresh')
    expect(page.locator('#water-label-0')).to_have_value('Display tank')
    page.click('#water-save')
    expect(page.locator('#water-settings-result')).to_contain_text('Probe settings saved')
    assert d.probes[0]=={'label':'Display tank','source':'DUAL_OVF_SENSOR'}
    d.devices[1]['observed_at']=int(time.time())-100
    d.offline.add('DUAL_OVF_SENSOR')
    page.click('#hardware-refresh')
    expect(page.locator('#board-status')).to_have_text('Offline')
    expect(page.locator('#sensor-connections')).not_to_contain_text('21.50C')
    page.select_option('#firmware-board','nano-old')
    expect(page.locator('#firmware-install')).to_be_disabled()
    d.devices[1]['observed_at']=int(time.time());d.devices[0]['observed_at']=int(time.time())
    d.offline.clear()
    page.click('#hardware-refresh')
    expect(page.locator('#firmware-install')).to_be_enabled()
    page.click('#firmware-install')
    expect(page.locator('#firmware-job')).to_contain_text('Saving the Arduino')
    assert ('/api/firmware/update',{'device':'DUAL_OVF_SENSOR','board':'nano-old'}) in d.requests
    expect(page.locator('#firmware-install')).to_be_disabled()
    d.hardware_error=True
    page.click('#hardware-refresh')
    expect(page.locator('#hardware-error')).to_be_visible()
    expect(page.locator('#board-status')).not_to_have_text('USB reporting')
    expect(page.locator('#water-live-0')).to_have_text('No unambiguous reading')
    d.hardware_error=False;d.devices[1]['observed_at']=int(time.time());d.devices[0]['observed_at']=int(time.time());d.firmware['active']=False;d.firmware['job']={'stage':'complete','message':'Firmware verified. Schedules remain paused.'}
    page.click('#hardware-refresh')
    expect(page.locator('#board-status')).to_have_text('USB reporting')
    # No unbounded page overflow on narrow screens; only the labelled diagram scrolls.
    assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')
    output=Path(os.environ.get('AOG_UI_ARTIFACTS','/tmp/aog-ui-hardware'));output.mkdir(parents=True,exist_ok=True)
    page.screenshot(path=str(output/f'connections-{width}.png'),full_page=True)
    navigate(page,'history');page.click('[data-history-sensor=water_temp_d6]')
    expect(page.locator('#history-card-water_temp_d6')).to_be_visible()
    expect(page.locator('#history-value-water_temp_d6')).not_to_have_text('—')
    assert not errors,errors
    context.close()

if __name__=='__main__':
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
        for width in [1440,390,320]:run(browser,width)
        browser.close()
    print('Hardware UI tests passed at 1440, 390, and 320 pixels')
