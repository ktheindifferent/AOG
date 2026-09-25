"""Navigation, honest summaries and responsive page tools; simulated device only."""
import json
import os
from pathlib import Path
from playwright.sync_api import expect, sync_playwright
from web_ui_hardware_test import Hardware
from web_ui_test import open_page, navigate, assert_fits

PAGES = ['overview','connections','history','cameras','reports','schedules','outputs','settings','account']


def check_workspace(browser, width):
    device=Hardware()
    device.co2_settings={'analog_model':'dfrobot_sen0219_analog','primary_source':'analog','reference_mv':5000}
    device.cameras=[{'id':name,'label':label,'detected':False,'live':False,'error':None} for name,label in [('ribbon','Raspberry Pi camera'),('usb','USB webcam')]]
    context,page,errors=open_page(browser,device,width=width,height=900)
    try:
        expect(page.locator('#co2-value')).to_have_text('1,191 ppm')
        expect(page.locator('#water-d6-value').locator('..')).to_contain_text('21.5 °C')
        assert page.locator('#water-d6-value').locator('..').inner_text().count('°C')==1
        navigate(page,'schedules')
        expect(page.locator('#watering-enabled-label')).to_have_text('Disabled')
        page.check('#watering-enabled')
        page.fill('#watering-start','22:00');page.fill('#watering-end','06:00')
        expect(page.locator('#watering-enabled-label')).to_have_text('Enabled')
        expect(page.locator('#watering-preview-text')).to_contain_text('crossing midnight')
        expect(page.locator('#schedule-preview-state')).to_contain_text('unsaved')
        assert 'linear-gradient' in page.locator('#watering-preview').evaluate('(el)=>el.style.background')
        page.fill('#watering-end','22:00')
        expect(page.locator('#watering-preview-text')).to_contain_text('all day')
        assert 'linear-gradient' not in page.locator('#watering-preview').evaluate('(el)=>el.style.background')
        if width<=780:
            page.click('[data-quick-page=history]')
            expect(page.locator('#page-history')).to_be_visible()
            expect(page.locator('[data-quick-page=history]')).to_have_attribute('aria-current','page')
            page.click('[data-history-sensor=temp]')
            expect(page.locator('[data-quick-page=history]')).to_have_attribute('aria-current','page')
            page.go_back()
            expect(page).to_have_url('https://aog.test/index.html#history')
            page.locator('[data-quick-page=history]').focus()
            page.set_viewport_size({'width':1440,'height':900})
            expect(page.locator('[data-page=history]')).to_be_focused()
            page.set_viewport_size({'width':width,'height':900})
            expect(page.locator('#navigation-toggle')).to_be_focused()
        navigate(page,'schedules')
        expect(page.locator('#watering-start')).to_have_value('22:00')
        expect(page.locator('#watering-end')).to_have_value('22:00')
        page.click('[data-discard-form=schedule-form]')
        expect(page.locator('#watering-enabled-label')).to_have_text('Disabled')
        navigate(page,'outputs')
        expect(page.locator('#output-live-summary')).to_have_text('0 of 4 outputs running')
        expect(page.locator('#output-automation-summary')).to_contain_text('No schedules enabled')
        device.outputs['outputs'][0]['on']=True
        page.click('#refresh-button')
        expect(page.locator('#output-live-summary')).to_have_text('1 of 4 outputs running')
        device.outputs['error']='Relay communication unavailable'
        page.click('#refresh-button')
        expect(page.locator('#output-live-summary')).to_have_text('Output state unavailable')
        expect(page.locator('#output-automation-summary')).not_to_contain_text('No schedules enabled')
        device.outputs['error']=None
        page.click('#refresh-button')
        navigate(page,'cameras')
        page.click('#camera-toggle-ribbon')
        expect(page.locator('#camera-status-ribbon')).to_have_text('Paused')
        if width>780:
            page.click('[data-camera-layout=stacked]')
            expect(page.locator('[data-camera-layout=stacked]')).to_have_attribute('aria-pressed','true')
            cards=page.locator('.camera-card')
            assert cards.nth(1).bounding_box()['y']>cards.nth(0).bounding_box()['y']
            page.click('[data-camera-layout=grid]')
            assert abs(cards.nth(1).bounding_box()['y']-cards.nth(0).bounding_box()['y'])<2
            expect(page.locator('#camera-status-ribbon')).to_have_text('Paused')
        navigate(page,'reports')
        assert page.locator('.export-card').bounding_box()['y']<page.locator('.report-preview-card').bounding_box()['y']
        assert not device.requests,'Navigation and previews must not change device settings or outputs'
        if os.environ.get('AOG_AXE_PATH'):
            page.add_script_tag(path=os.environ['AOG_AXE_PATH'])
        for destination in PAGES:
            navigate(page,destination)
            assert_fits(page)
            if width<=780:
                expect(page.locator('.mobile-quick-nav')).to_be_in_viewport()
                page.evaluate('window.scrollTo(0,document.documentElement.scrollHeight)')
                footer=page.locator('.page-footer').bounding_box();dock=page.locator('.mobile-quick-nav').bounding_box()
                assert footer['y']+footer['height']<=dock['y'],(destination,footer,dock)
                page.evaluate('window.scrollTo(0,0)')
            if os.environ.get('AOG_AXE_PATH'):
                violations=page.evaluate("async ()=>(await axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
                assert not violations,(destination,width,violations)
            print(f'Checked {destination} at {width}px',flush=True)
        page.goto('https://aog.test/login.html')
        assert_fits(page)
        if os.environ.get('AOG_AXE_PATH'):
            page.add_script_tag(path=os.environ['AOG_AXE_PATH'])
            violations=page.evaluate("async ()=>(await axe.run(document,{runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
            assert not violations,('login',width,violations)
        assert not errors,errors
    finally:
        context.close()


def run():
    with sync_playwright() as p:
        browser=p.chromium.launch(executable_path=os.environ.get('CHROMIUM','/usr/bin/chromium'),headless=True,args=['--no-sandbox','--disable-dev-shm-usage'])
        try:
            for width in [1440,390,320]:check_workspace(browser,width)
        finally:browser.close()
    print('PASS: all pages, quick navigation, focus, retained drafts, schedule windows, output summaries, camera layout, export order, units, and accessibility',flush=True)

if __name__=='__main__':run()
