"""Browser appearance, sensor freshness and contrast; simulated device only."""
import json
import os
from pathlib import Path
from urllib.parse import urlparse
from playwright.sync_api import expect, sync_playwright
from web_ui_hardware_test import Hardware
from web_ui_test import open_page, navigate, assert_fits

PAGES = ['overview','connections','history','cameras','reports','schedules','outputs','settings','account']
ART = Path(os.environ.get('AOG_UI_ARTIFACTS','/tmp/aog-appearance'))

class AppearanceDevice(Hardware):
    def route(self, route):
        if urlparse(route.request.url).path == '/api/stats' and self.stats_error:
            route.fulfill(status=503, content_type='application/json', body='{"error":"Readings unavailable"}')
            return
        super().route(route)


def audit(page, label, screenshot=True):
    assert_fits(page)
    if os.environ.get('AOG_AXE_PATH'):
        if not page.evaluate('typeof window.axe !== "undefined"'):
            page.add_script_tag(path=os.environ['AOG_AXE_PATH'])
        violations = page.evaluate("async () => (await axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
        if violations:
            ART.mkdir(parents=True,exist_ok=True)
            page.screenshot(path=str(ART / (label+'-failure.png')),full_page=True)
        assert not violations, (label,violations)
    if screenshot:
        ART.mkdir(parents=True,exist_ok=True)
        page.evaluate('window.scrollTo(0,0)')
        page.screenshot(path=str(ART / (label+'.png')),full_page=True)


def check_pages(browser,width):
    device=AppearanceDevice()
    device.co2_settings={'analog_model':'dfrobot_sen0219_analog','primary_source':'analog','reference_mv':5000}
    context,page,errors=open_page(browser,device,width=width,height=900)
    try:
        expect(page.locator('html')).to_have_attribute('data-appearance','system')
        page.emulate_media(color_scheme='dark')
        expect(page.locator('html')).to_have_attribute('data-theme','dark')
        for route in PAGES:
            navigate(page,route)
            audit(page,f'{route}-dark-{width}')
            print(f'Checked {route} in dark at {width}px',flush=True)
        page.select_option('#dashboard-appearance','light')
        expect(page.locator('html')).to_have_attribute('data-theme','light')
        audit(page,f'account-light-{width}')
        for route in PAGES:
            navigate(page,route)
            assert_fits(page)
        navigate(page,'overview')
        expect(page.locator('#pm25-value')).to_have_text('4.2 µg/m³')
        expect(page.locator('#tvoc-value')).to_have_text('18 ppb')
        expect(page.locator('#tvoc-value small')).to_have_text('ppb')
        device.stats_error=True
        page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_enabled()
        for pin in ['d6','d8']:
            expect(page.locator(f'#water-{pin}-value')).to_have_text('—')
            expect(page.locator(f'#water-{pin}-value').locator('..')).to_have_attribute('data-available','false')
        expect(page.locator('#tvoc-value')).to_have_text('Unavailable')
        audit(page,f'overview-offline-light-{width}')
        page.emulate_media(color_scheme='light')
        navigate(page,'account');page.select_option('#dashboard-appearance','dark')
        navigate(page,'overview')
        audit(page,f'overview-offline-dark-{width}')
        device.stats_error=False
        page.click('#refresh-button')
        expect(page.locator('#water-d6-value')).to_have_text('21.5 °C')
        expect(page.locator('#water-d6-value').locator('..')).to_have_attribute('data-available','true')
        expect(page.locator('#tvoc-value')).to_have_text('18 ppb')
        page.goto('https://aog.test/login.html')
        expect(page.locator('#login-appearance')).to_have_value('dark')
        expect(page.locator('html')).to_have_attribute('data-theme','dark')
        audit(page,f'login-dark-{width}')
        page.fill('#input_password','ExamplePassword123!')
        page.select_option('#login-appearance','light')
        expect(page.locator('#input_password')).to_have_value('ExamplePassword123!')
        audit(page,f'login-light-{width}')
        assert not device.requests, 'Appearance and layout checks must not write to the device'
        assert not errors, errors
        print(f'PASS: themes, readable units, unavailable values and recovery at {width}px',flush=True)
    finally:
        context.close()


def check_preferences(browser):
    print('Checking appearance preferences…',flush=True)
    device=AppearanceDevice()
    context,page,errors=open_page(browser,device,path='index.html#account')
    try:
        page.fill('#new-password','KeepMyDraft123!')
        page.locator('#new-password').blur()
        assert page.evaluate('localStorage.length') == 0, 'Typing in a form must not save an appearance preference'
        page.select_option('#dashboard-appearance','dark')
        expect(page.locator('#new-password')).to_have_value('KeepMyDraft123!')
        page.emulate_media(color_scheme='light')
        expect(page.locator('html')).to_have_attribute('data-theme','dark')
        print('Checking cross-tab appearance sync…',flush=True)
        other=context.new_page();other.goto('https://aog.test/login.html')
        expect(other.locator('#login-appearance')).to_have_value('dark')
        other.select_option('#login-appearance','light')
        expect(page.locator('#dashboard-appearance')).to_have_value('light')
        expect(page.locator('html')).to_have_attribute('data-theme','light')
        expect(page.locator('#new-password')).to_have_value('KeepMyDraft123!')
        other.close()
        page.select_option('#dashboard-appearance','system')
        page.emulate_media(color_scheme='dark')
        expect(page.locator('html')).to_have_attribute('data-theme','dark')
        page.emulate_media(color_scheme='light')
        expect(page.locator('html')).to_have_attribute('data-theme','light')
        print('Checking retained drafts…',flush=True)
        navigate(page,'settings');page.fill('#setup-name-input','A retained grow name')
        navigate(page,'account');page.select_option('#dashboard-appearance','dark')
        navigate(page,'settings')
        expect(page.locator('#setup-name-input')).to_have_value('A retained grow name')
        page.click('[data-discard-form=setup-form]')
        page.reload()
        expect(page.locator('html')).to_have_attribute('data-theme','dark')
        navigate(page,'account')
        expect(page.locator('#dashboard-appearance')).to_have_value('dark')
        assert page.evaluate('JSON.stringify(Object.entries(localStorage))') == '[["aog.appearance","dark"]]'
        # A failed storage write must not break the visible preference or forms.
        blocked=context.new_page()
        blocked.add_init_script("Object.defineProperty(Storage.prototype, 'setItem', {value(){throw new DOMException('Unavailable','SecurityError')}});Object.defineProperty(Storage.prototype, 'getItem', {value(){throw new DOMException('Unavailable','SecurityError')}})")
        blocked.goto('https://aog.test/login.html')
        blocked.select_option('#login-appearance','dark')
        expect(blocked.locator('html')).to_have_attribute('data-theme','dark')
        blocked.fill('#input_password','CanStillType123!')
        expect(blocked.locator('#input_password')).to_be_editable()
        blocked.close()
        assert not device.requests
        assert not errors,errors
        print('PASS: system changes, persistence, cross-tab sync, draft preservation and unavailable storage',flush=True)
    finally:
        context.close()


def check_expanded_states(browser):
    device=AppearanceDevice()
    context,page,errors=open_page(browser,device)
    try:
        page.emulate_media(color_scheme='dark')
        navigate(page,'settings');page.click('[data-settings-view=all]')
        page.fill('#setup-name-input','My draft')
        page.fill('#ph-min','12');page.click('#ph-settings-form [type=submit]')
        audit(page,'settings-expanded-dark')
        navigate(page,'connections')
        # Native disclosures expose firmware, water and CO₂ controls for contrast checks.
        page.locator('.connection-tool').evaluate_all('items=>items.forEach(item=>item.open=true)')
        page.click('#inspection-tab-data')
        audit(page,'connections-expanded-dark')
        navigate(page,'history');page.click('[data-history-view=summary]')
        audit(page,'history-comparison-dark')
        page.locator('#history-card-temp .history-open-chart').click()
        page.locator('#history-chart-temp').press('Home')
        selected=page.locator('#history-reading-temp').inner_text()
        page.emulate_media(color_scheme='light')
        expect(page.locator('#history-reading-temp')).to_have_text(selected)
        expect(page.locator('#history-tooltip-temp')).to_be_visible()
        page.emulate_media(color_scheme='dark')
        expect(page.locator('#history-reading-temp')).to_have_text(selected)
        audit(page,'history-inspection-dark')
        navigate(page,'schedules')
        page.check('#watering-enabled')
        page.fill('#watering-start','22:00');page.fill('#watering-end','06:00')
        expect(page.locator('#watering-preview-text')).to_contain_text('crossing midnight')
        audit(page,'schedules-draft-dark')
        night=page.locator('#watering-preview').evaluate('el=>getComputedStyle(el).backgroundImage')
        page.emulate_media(color_scheme='light')
        day=page.locator('#watering-preview').evaluate('el=>getComputedStyle(el).backgroundImage')
        assert night != day, 'The schedule preview must follow appearance changes without editing times'
        expect(page.locator('#watering-start')).to_have_value('22:00')
        page.emulate_media(color_scheme='dark')
        device.outputs['outputs'][0]['on']=True
        navigate(page,'outputs');page.click('#refresh-button')
        expect(page.locator('#output-live-summary')).to_have_text('1 of 4 outputs running')
        audit(page,'outputs-running-dark')
        device.outputs['pump_block_reason']='Check the overflow switch before running a pump'
        page.click('#refresh-button')
        audit(page,'outputs-blocked-dark')
        navigate(page,'account')
        page.fill('#new-password','ExamplePassword123!')
        page.fill('#confirm-password','DifferentPassword123!')
        expect(page.locator('#password-match')).to_have_attribute('data-match','false')
        audit(page,'password-mismatch-dark')
        page.fill('#confirm-password','ExamplePassword123!')
        expect(page.locator('#password-match')).to_have_attribute('data-match','true')
        audit(page,'password-match-dark')
        assert not device.requests
        assert not errors,errors
        print('PASS: expanded diagnostics, draft/error states and chart inspection across themes',flush=True)
    finally:
        context.close()


if __name__=='__main__':
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
        try:
            check_preferences(browser)
            for width in [1440,390,320]:check_pages(browser,width)
            check_expanded_states(browser)
        finally:browser.close()
