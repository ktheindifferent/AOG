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
            {'device_id':'SENSORKIT_MK1','port':'/dev/ttyUSB1','observed_at':int(time.time()),'error':None,'fields':{'FIRMWARE_VERSION':'2.0.0','PROTOCOL_VERSION':'2','TEMP':'24.50C','HUM':'62%','DHT_STATUS':'ok','CO2':'1200ppm','TVOC':'18ppb','CCS811_STATUS':'ok','CO2_ADC':'160','WATER_D6_STATUS':'not_detected','WATER_D8_STATUS':'not_detected','BUFFER_PENDING':'7','BUFFER_CAPACITY':'24','BUFFER_DROPPED':'0','UPTIME_S':'3900'}},
            {'device_id':'DUAL_OVF_SENSOR','port':'/dev/ttyUSB0','observed_at':int(time.time()),'error':None,'fields':{'FIRMWARE_VERSION':'2.0.0','PROTOCOL_VERSION':'2','T1_OVF':'NONE','T2_OVF':'OVERFLOW','PH':'6.85','PH_ADC':'313','PH_MV':'1528.3','PH_STATUS':'analog_unverified','WATER_D6_STATUS':'ok','WATER_D6':'21.50C','WATER_D6_ROM':'28010203040506AA','WATER_D8_STATUS':'crc_error','WATER_D8_ROM':'28010203040507BB','BUFFER_PENDING':'0','BUFFER_CAPACITY':'24','BUFFER_DROPPED':'2','UPTIME_S':'1200'}}]
        self.probes=[{'label':'Water · D6','source':None},{'label':'Water · D8','source':None}]
        self.firmware={'package':{'version':'2.0.0','protocol':2,'artifacts':[{'device':d,'board':'nano-old','fqbn':'arduino:avr:nano:cpu=atmega328old','file':f'{d}.hex','sha256':'a'*64} for d in ['SENSORKIT_MK1','DUAL_OVF_SENSOR']]},'updater_available':True,'active':False,'job':{'stage':'','message':''}}
        self.co2_settings={'analog_model':'unidentified','primary_source':'ccs811','reference_mv':5000}
        self.hardware_error=False
        self.firmware_error=False
        self.firmware_hold=False
        self.held_firmware=[]
        self.offline=set()
    def sensor_history(self, hours):
        data=super().sensor_history(hours)
        data['series'][0]['source']='dfrobot_sen0219_analog' if self.co2_settings['primary_source']=='analog' else 'ccs811'
        return data
    def route(self, route):
        path=urlparse(route.request.url).path
        def reply(data,status=200): route.fulfill(status=status,content_type='application/json',body=json.dumps(data))
        if path=='/api/stats':
            fields=self.devices[0]['fields'];analog=self.co2_settings['primary_source']=='analog'
            mv=int(fields.get('CO2_ADC','0'))*self.co2_settings['reference_mv']/1024
            co2=f'{(mv-400)*3.125}ppm' if analog and 400<=mv<=2000 else 'N/A' if analog else fields.get('CO2','N/A')
            reply({'water_temp_d6':'21.50C','water_temp_d8':'22.10C','temp':'24.50C','hum':'62%','co2':co2,'co2_source':'dfrobot_sen0219_analog' if analog else 'ccs811','pm25':'4.2','pm10':'6.1','tvoc':'18ppb','t1_ovf':'NONE','t2_ovf':'NONE','overflow_error':False,'ph':'6.85','ph_status':None,'device_time':self.settings['device_time']});return
        if path=='/api/connections':
            if self.hardware_error: reply({'error':'USB diagnostics unavailable'},503);return
            for device in self.devices:
                if device['device_id'] not in self.offline: device['observed_at']=int(time.time())
            kit=self.devices[0];f=kit['fields'];fresh=0<=time.time()-kit['observed_at']<=5;v2=f.get('PROTOCOL_VERSION')=='2'
            raw=f.get('CO2_ADC');adc=int(raw) if fresh and raw and raw.isdigit() and 0<=int(raw)<=1023 else None
            ppm=float(f['CO2'].removesuffix('ppm')) if fresh and f.get('CO2') else None
            mv=adc*self.co2_settings['reference_mv']/1024 if adc is not None else None
            infrared=self.co2_settings['analog_model']=='dfrobot_sen0219_analog'
            converted=(mv-400)*3.125 if infrared and mv is not None and 400<=mv<=2000 else None
            primary=self.co2_settings['primary_source']=='analog'
            diagnostic={'fresh':fresh,'source':'dfrobot_sen0219_analog' if primary else 'ccs811' if v2 else 'legacy_mixed','digital_eco2_ppm':ppm if v2 and f.get('CCS811_STATUS')=='ok' else None,'reported_ppm':ppm,'analog_adc':adc,'analog_co2_ppm':converted,'primary_ppm':converted if primary else ppm,'analog_millivolts':mv,'analog_status':'unavailable' if adc is None else 'near_ground' if adc<=1 else 'near_supply' if adc>=1022 else 'converted' if converted is not None else 'below_range' if infrared and mv<400 else 'above_range' if infrared else 'unidentified','assumed_reference_mv':self.co2_settings['reference_mv'],'compensation':'live' if f.get('DHT_STATUS')=='ok' else 'unavailable','startup_warmup':int(f.get('UPTIME_S','9999'))<1200}
            reply({'server_time':int(time.time()),'devices':self.devices,'co2_diagnostics':diagnostic,'co2_settings':self.co2_settings,'water_probes':self.probes,'water_readings':[[21.5,int(time.time())],None],'retention_days':7});return
        if path=='/api/firmware':
            if self.firmware_hold:self.held_firmware.append(route);return
            reply({'error':'Package unavailable'} if self.firmware_error else self.firmware,503 if self.firmware_error else 200);return
        if path in ['/api/firmware/update','/api/water-probes','/api/co2-settings']:
            assert route.request.method=='POST'
            assert route.request.headers.get('x-aog-csrf')=='browser-test-token'
            body=route.request.post_data_json;self.requests.append((path,body))
            if path=='/api/co2-settings':self.co2_settings=body;reply(body)
            elif path=='/api/water-probes':self.probes=body;reply({'saved':True})
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
    expect(page.locator('#sensor-connections')).to_contain_text('Unverified analog')
    expect(page.locator('[data-sensor=ccs] .sensor-row-reading')).to_have_text('1,200 ppm eCO₂')
    expect(page.locator('#gas-digital-value')).to_have_text('1,200')
    expect(page.locator('#gas-analog-value')).to_have_text('160')
    expect(page.locator('#gas-analog-voltage')).to_contain_text('0.781 V')
    expect(page.locator('#gas-source-rule')).to_contain_text('never averaged')
    expect(page.locator('[data-sensor=analog]')).not_to_contain_text('160ppm')
    for tool in ['co2-tools','water-tools','firmware-tools','board-details']:
        assert not page.locator('#'+tool).evaluate('(el) => el.open')
    # Checks across both boards navigate directly to the relevant connection.
    page.click('[data-check="DUAL_OVF_SENSOR:overflow2"]')
    expect(page.locator('#trace-title')).to_have_text('Overflow switch 2')
    expect(page.locator('#diagnosis-title')).to_contain_text('open circuit')
    expect(page.locator('#wire-paths')).to_be_visible()
    page.click('[data-check="DUAL_OVF_SENSOR:water8"]')
    expect(page.locator('#trace-status')).to_have_text('Data check failed')
    expect(page.locator('#diagnosis-detail')).to_contain_text('integrity check')
    page.locator('#inspection-tab-wiring').focus()
    page.keyboard.press('ArrowRight')
    expect(page.locator('#inspection-tab-data')).to_be_focused()
    expect(page.locator('#inspection-data')).to_be_visible()
    expect(page.locator('#inspection-wiring')).to_be_hidden()
    expect(page.locator('[data-field=WATER_D8_STATUS] dd')).to_have_text('crc_error')
    with page.expect_download() as download:
        page.click('#trace-download')
    saved=json.loads(Path(download.value.path()).read_text())
    assert saved['device']=='DUAL_OVF_SENSOR' and saved['sensor']=='water8'
    assert saved['fields']['WATER_D8_STATUS']=='crc_error' and saved['display_value'] is None
    assert saved['live'] and saved['reported_at']
    page.locator('#inspection-tab-data').focus();page.keyboard.press('Home')
    expect(page.locator('#inspection-wiring')).to_be_visible()
    page.click('[data-board=SENSORKIT_MK1]')
    page.locator('#hardware-refresh').focus()
    page.evaluate('window.AogHardware.refresh()')
    expect(page.locator('#hardware-refresh')).to_be_focused()
    expect(page.locator('#hardware-refresh')).to_have_text('↻ Refresh connections')
    # Polling updates existing controls instead of replacing the row under a click or keyboard focus.
    page.locator('[data-trace=analog]').focus()
    page.evaluate("window.originalSensorButton=document.querySelector('[data-trace=analog]')")
    page.evaluate('window.AogHardware.refresh()')
    expect(page.locator('[data-trace=analog]')).to_be_focused()
    assert page.evaluate("window.originalSensorButton===document.querySelector('[data-trace=analog]')")
    page.click('[data-trace=analog]')
    expect(page.locator('#wiring-inspector')).to_be_focused()
    expect(page.locator('#trace-title')).to_have_text('Analog gas input')
    expect(page.locator('#wire-paths')).to_contain_text('Analog OUT')
    expect(page.locator('#trace-value')).to_have_text('160 counts')
    if width<=780:
        assert page.evaluate("document.querySelector('[data-sensor=analog]').nextElementSibling.id==='wiring-inspector'")
        assert page.locator('#wiring-inspector').bounding_box()['y']>=90
    page.click('#inspection-tab-data')
    expect(page.locator('[data-field=CO2_ADC] dd')).to_have_text('160')
    expect(page.locator('#trace-ppm-label')).to_have_text('Profile needed')
    page.click('#trace-back')
    expect(page.locator('[data-trace=analog]')).to_be_focused()
    if width<=780:
        expect(page.locator('#wiring-inspector')).to_be_hidden()
        expect(page.locator('[data-trace=analog]')).to_have_attribute('aria-expanded','false')
        page.click('[data-trace=analog]')
        expect(page.locator('#wiring-inspector')).to_be_visible()
    page.locator('[data-pin-sensor=analog]').focus()
    page.evaluate('window.AogHardware.refresh()')
    expect(page.locator('[data-pin-sensor=analog]')).to_be_focused()
    d.devices[0]['fields'].update(CO2='400ppm',DHT_STATUS='read_error',CO2_ADC='0')
    page.click('#hardware-refresh')
    expect(page.locator('#gas-digital-detail')).to_contain_text('reporting floor')
    expect(page.locator('#gas-digital-detail')).to_contain_text('DHT error')
    expect(page.locator('#gas-analog-value')).to_have_text('0')
    expect(page.locator('#gas-analog-status')).to_have_text('Check signal voltage')
    expect(page.locator('#map-health-kit')).to_contain_text('2 sensors need attention')
    page.click('[data-sensor-filter=attention]')
    expect(page.locator('[data-sensor=ccs]')).to_be_hidden()
    expect(page.locator('[data-sensor=water6]')).to_be_hidden()
    expect(page.locator('[data-sensor=analog]')).to_be_visible()
    expect(page.locator('[data-sensor=dht] .sensor-row-reading')).to_have_text('—')
    expect(page.locator('#sensor-attention-count')).to_have_text('2')
    page.locator('[data-pin-sensor=analog]').focus()
    d.devices[0]['fields'].update(CO2='1200ppm',DHT_STATUS='ok',CO2_ADC='160')
    page.evaluate('window.AogHardware.refresh()')
    expect(page.locator('#sensor-empty')).to_be_visible()
    expect(page.locator('#wiring-inspector')).to_be_hidden()
    expect(page.locator('[data-sensor-filter=attention]')).to_be_focused()
    page.click('[data-sensor-filter=all]')
    expect(page.locator('[data-sensor=water6]')).to_be_visible()
    expect(page.locator('#wiring-inspector')).to_be_visible()
    page.click('#trace-setup')
    assert page.locator('#co2-tools').evaluate('(el) => el.open')
    expect(page.locator('#co2-tools > summary')).to_be_focused()
    page.click('#co2-settings-details summary')
    page.select_option('#co2-model','dfrobot_sen0219_analog')
    page.select_option('#co2-primary-source','analog')
    page.fill('#co2-reference','4.900')
    page.click('#hardware-refresh')
    expect(page.locator('#co2-reference')).to_have_value('4.900')
    page.click('#co2-save')
    expect(page.locator('#co2-settings-result')).to_contain_text('CO₂ settings saved')
    expect(page.locator('#gas-analog-primary')).to_have_text('1,143')
    expect(page.locator('#gas-digital-value')).to_have_text('1,200')
    page.fill('#co2-reference','5.000');page.click('#co2-save')
    expect(page.locator('#gas-analog-primary')).to_have_text('1,191')
    expect(page.locator('#trace-adc')).to_have_text('160')
    expect(page.locator('#trace-volts')).to_have_text('0.781')
    expect(page.locator('#trace-ppm')).to_have_text('1,191')
    expect(page.locator('#gas-analog-unit')).to_have_text('ppm CO₂')
    expect(page.locator('#gas-source-rule')).to_contain_text('DFRobot infrared CO₂')
    expect(page.locator('[data-sensor=ccs] a')).to_have_count(0)
    expect(page.locator('[data-sensor=analog] a')).to_have_count(1)
    assert d.co2_settings=={'analog_model':'dfrobot_sen0219_analog','primary_source':'analog','reference_mv':5000}
    d.devices[0]['fields']['CO2_ADC']='0';page.click('#hardware-refresh')
    expect(page.locator('#gas-analog-primary')).to_have_text('—')
    expect(page.locator('#gas-digital-value')).to_have_text('1,200')
    d.devices[0]['fields']['CO2_ADC']='160';page.click('#hardware-refresh')
    page.click('#co2-settings-details summary')
    navigate(page,'overview')
    expect(page.locator('#co2-value')).to_have_text('1,191')
    expect(page.locator('#co2-source')).to_contain_text('DFRobot')
    navigate(page,'connections')
    page.click('[data-board=DUAL_OVF_SENSOR]')
    expect(page.locator('#sensor-connections')).to_contain_text('Data check failed')
    expect(page.locator('#sensor-connections')).to_contain_text('Circuit open')
    expect(page.locator('#pin-d2-label')).to_have_text('Overflow switch 1')
    page.click('[data-trace=water6]')
    expect(page.locator('#trace-note')).to_contain_text('DS18B20 family detected')
    expect(page.locator('#wire-paths')).to_contain_text('DQ / data')
    expect(page.locator('[data-field=WATER_D6_ROM] dd')).to_have_text('28010203040506AA')
    expect(page.locator('#trace-value')).to_have_text('21.5 °C')
    page.click('#trace-setup')
    assert page.locator('#water-tools').evaluate('(el) => el.open')
    page.click('[data-trace=overflow2]')
    expect(page.locator('#wire-paths')).to_contain_text('D4')
    expect(page.locator('#trace-help')).to_contain_text('cable continuity')
    expect(page.locator('#gas-digital-value')).to_have_text('1,200')
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
    expect(page.locator('[data-sensor=water6] .sensor-row-reading')).to_have_text('—')
    page.click('#firmware-jump')
    assert page.locator('#firmware-tools').evaluate('(el) => el.open')
    expect(page.locator('#firmware-tools > summary')).to_be_focused()
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
    # An independent firmware API failure must not turn healthy USB links offline.
    d.firmware_error=True
    page.click('#hardware-refresh')
    expect(page.locator('#board-status')).to_have_text('USB reporting')
    expect(page.locator('#firmware-install')).to_be_disabled()
    expect(page.locator('#firmware-tool')).to_contain_text('could not be refreshed')
    d.firmware_error=False
    if width==1440:
        # A delayed independent API must not keep old sensor values looking live.
        page.click('[data-board=SENSORKIT_MK1]')
        page.evaluate('window.AogHardware.refresh()')
        d.firmware_hold=True
        page.evaluate('void window.AogHardware.refresh(true)')
        expect(page.locator('#hardware-refresh')).to_be_disabled()
        expect(page.locator('#trace-value')).to_have_text('—',timeout=9000)
        expect(page.locator('#hardware-error')).to_be_hidden()
        expect(page.locator('#trace-freshness')).to_contain_text('Last report')
        expect(page.locator('#raw-sensor-frame')).to_contain_text('160')
        d.offline={'SENSORKIT_MK1','DUAL_OVF_SENSOR'}
        d.firmware_hold=False
        for held in d.held_firmware:
            held.fulfill(status=200,content_type='application/json',body=json.dumps(d.firmware))
        d.held_firmware.clear()
        expect(page.locator('#hardware-refresh')).to_be_enabled()
        expect(page.locator('#board-status')).to_have_text('Offline')
        d.offline.clear();page.click('#hardware-refresh')
        expect(page.locator('#trace-value')).to_have_text('1,191 ppm')
    d.devices[0]['observed_at']=int(time.time())-100;d.offline.add('SENSORKIT_MK1')
    page.click('#hardware-refresh')
    expect(page.locator('#gas-digital-value')).to_have_text('—')
    expect(page.locator('#gas-analog-value')).to_have_text('—')
    expect(page.locator('#gas-analog-status')).to_have_class('badge neutral')
    d.offline.clear();d.devices[0]['fields']['PROTOCOL_VERSION']='1';d.co2_settings['primary_source']='ccs811'
    page.click('#hardware-refresh')
    expect(page.locator('#gas-digital-value')).to_have_text('—')
    expect(page.locator('#gas-source-rule')).to_contain_text('Legacy firmware supplies a mixed')
    d.devices[0]['fields']['PROTOCOL_VERSION']='2';d.co2_settings['primary_source']='analog'
    page.click('[data-board=SENSORKIT_MK1]');page.click('#hardware-refresh')
    expect(page.locator('#trace-title')).to_have_text('DFRobot infrared CO₂')
    # Compact disclosures and mobile inline placement survive repeated board changes and viewport changes.
    for tool in ['co2-tools','water-tools','firmware-tools']:
        page.locator('#'+tool+' > summary').click()
    page.click('[data-trace=analog]')
    page.click('#inspection-tab-wiring')
    original_width=width
    page.set_viewport_size({'width':900 if width<=780 else 390,'height':1080})
    page.locator('[data-pin-sensor=analog]').focus()
    page.set_viewport_size({'width':original_width,'height':1080})
    expect(page.locator('[data-pin-sensor=analog]')).to_be_focused()
    # The complete topology and wiring guide fit narrow screens without panning.
    assert page.evaluate('document.documentElement.scrollWidth <= innerWidth + 1')
    output=Path(os.environ.get('AOG_UI_ARTIFACTS','/tmp/aog-ui-hardware'));output.mkdir(parents=True,exist_ok=True)
    page.evaluate('window.scrollTo(0,0)')
    page.screenshot(path=str(output/f'connections-{width}.png'),full_page=width>600,animations="disabled",timeout=60000)
    if width<=600:
        page.click('#co2-tools > summary')
        page.locator('.co2-diagnostics').scroll_into_view_if_needed()
        page.screenshot(path=str(output/f'co2-{width}.png'),animations="disabled",timeout=60000)
        page.click('#co2-tools > summary')
        page.locator('#wiring-inspector').scroll_into_view_if_needed()
        page.screenshot(path=str(output/f'wiring-{width}.png'),animations="disabled",timeout=60000)
    page.click('#inspection-tab-data')
    page.locator('#inspection-data').scroll_into_view_if_needed()
    page.screenshot(path=str(output/f'raw-data-{width}.png'),animations="disabled",timeout=60000)
    if os.environ.get('AOG_AXE_PATH'):
        # Audit the forms and maintenance controls as well as the main connection inspector.
        page.evaluate("document.querySelectorAll('#page-connections details').forEach(el=>el.open=true)")
        page.add_script_tag(path=os.environ['AOG_AXE_PATH'])
        issues=page.evaluate("async () => (await axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
        assert not issues,issues
        page.click('#inspection-tab-wiring')
        issues=page.evaluate("async () => (await axe.run(document.getElementById('inspection-wiring'),{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
        assert not issues,issues
        print(f'Accessibility passed at {width}px',flush=True)
    navigate(page,'history');page.click('[data-history-sensor=water_temp_d6]')
    expect(page.locator('#history-card-water_temp_d6')).to_be_visible()
    expect(page.locator('#history-value-water_temp_d6')).not_to_have_text('—')
    page.click('[data-history-sensor=co2]')
    expect(page.locator('#history-detail-co2')).to_contain_text('Average is over saved samples')
    assert not errors,errors
    context.close()
    print(f'Connection and CO₂ workflows passed at {width}px',flush=True)

if __name__=='__main__':
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
        for width in [1440,390,320]:run(browser,width)
        browser.close()
    print('Hardware UI tests passed at 1440, 390, and 320 pixels')
