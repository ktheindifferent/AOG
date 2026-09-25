"""Focused setup, history comparison, report outlines and credential feedback.
All requests are intercepted; these checks do not operate physical hardware.
"""
import os
from pathlib import Path
from playwright.sync_api import expect, sync_playwright
from web_ui_hardware_test import Hardware
from web_ui_history_loading_test import HeldHistoryDevice
from web_ui_test import open_page, navigate, assert_fits

ART = Path(os.environ.get('AOG_UI_ARTIFACTS', '/tmp/aog-ui-refinement'))


def check_refinement(browser, width):
    device = Hardware()
    device.co2_settings = {'analog_model': 'dfrobot_sen0219_analog', 'primary_source': 'analog', 'reference_mv': 5000}
    context, page, errors = open_page(browser, device, width=width, height=900)
    try:
        if os.environ.get('AOG_AXE_PATH'):
            page.add_script_tag(path=os.environ['AOG_AXE_PATH'])

        def review(name):
            assert_fits(page)
            if os.environ.get('AOG_AXE_PATH'):
                violations = page.evaluate("async () => (await axe.run(document, {runOnly:{type:'tag',values:['wcag2a','wcag2aa','wcag21aa']}})).violations.map(v=>({id:v.id,nodes:v.nodes.map(n=>({target:n.target,summary:n.failureSummary}))}))")
                assert not violations, (name, width, violations)
            ART.mkdir(parents=True, exist_ok=True)
            page.screenshot(path=str(ART / f'{name}-{width}.png'), full_page=True)

        navigate(page, 'settings')
        expect(page.locator('.settings-grid > form:visible')).to_have_count(1)
        expect(page.locator('#setup-form')).to_be_visible()
        expect(page.locator('[data-settings-section=relays]')).to_have_attribute('aria-current', 'location')
        review('settings-relays')
        page.fill('#setup-name-input', 'Unsubmitted grow name')
        page.click('[data-settings-section=ph]')
        expect(page.locator('#settings-ph-title')).to_be_focused()
        expect(page.locator('#setup-form')).not_to_be_visible()
        page.fill('#ph-min', '6.7')
        page.go_back()
        expect(page.locator('#setup-form')).to_be_visible()
        expect(page.locator('#setup-name-input')).to_have_value('Unsubmitted grow name')
        page.go_forward()
        expect(page.locator('#ph-min')).to_have_value('6.7')
        page.click('[data-settings-view=all]')
        expect(page.locator('.settings-grid > form:visible')).to_have_count(5)
        page.click('[data-settings-view=focused]')
        expect(page.locator('.settings-grid > form:visible')).to_have_count(1)
        expect(page.locator('#ph-settings-form')).to_be_visible()
        page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_enabled()
        expect(page.locator('#ph-min')).to_have_value('6.7')
        expect(page.locator('#setup-name-input')).to_have_value('Unsubmitted grow name')
        page.click('[data-discard-form=ph-settings-form]')
        page.click('[data-settings-section=relays]')
        page.click('[data-discard-form=setup-form]')
        for section in ['overflow', 'safety', 'ph', 'calibration']:
            page.click(f'[data-settings-section={section}]')
            expect(page.locator('.settings-grid > form:visible')).to_have_count(1)
            review('settings-' + section)

        navigate(page, 'history')
        expect(page.locator('#history-cards')).to_have_attribute('aria-busy', 'false')
        page.click('[data-history-view=summary]')
        expect(page.locator('#history-cards')).to_have_attribute('data-view', 'summary')
        expect(page.locator('.history-card:visible')).to_have_count(8)
        expect(page.locator('#history-cards .chart-interaction:visible')).to_have_count(0)
        expect(page.locator('.history-open-chart:visible')).to_have_count(8)
        latest = page.locator('#history-value-temp').inner_text()
        review('history-compare')
        page.click('#history-card-temp .history-open-chart')
        expect(page.locator('#history-chart-temp')).to_be_visible()
        expect(page.locator('#history-value-temp')).to_have_text(latest)
        expect(page.locator('#history-view-controls')).not_to_be_visible()
        page.locator('#history-chart-temp').press('Home')
        expect(page.locator('#history-tooltip-temp')).to_be_visible()
        page.go_back()
        expect(page.locator('#history-cards')).to_have_attribute('data-view', 'summary')
        expect(page.locator('#history-title-temp')).to_have_text('Temperature')
        page.click('[data-history-view=charts]')
        expect(page.locator('#history-chart-temp')).to_be_visible()
        expect(page.locator('#history-title-temp')).to_have_text('Temperature over time')

        navigate(page, 'reports')
        page.fill('#report-title', '<b>Grow room</b>')
        page.fill('#report-notes', 'Day 4\nWater probes checked.')
        expect(page.locator('#report-outline-title')).to_have_text('<b>Grow room</b>')
        expect(page.locator('#report-outline-title b')).to_have_count(0)
        expect(page.locator('#report-outline-notes')).to_have_text('Day 4\nWater probes checked.')
        expect(page.locator('#report-notes-count')).to_have_text('27 / 2,000')
        page.uncheck('#report-current')
        expect(page.locator('#report-outline-snapshot')).to_have_text('Historical readings only')
        page.select_option('#report-range', 'current')
        expect(page.locator('#report-outline-contents')).not_to_contain_text('charts')
        expect(page.locator('#report-outline-period')).to_have_text('Current readings only')
        page.select_option('#report-range', '6')
        expect(page.locator('#report-outline-period')).to_contain_text('Last 6 hours')
        expect(page.locator('#report-outline-snapshot')).to_have_text('Historical readings only')
        review('report-outline')
        page.fill('#report-title', '')
        expect(page.locator('#report-outline-title')).to_have_text('Environment & water report')

        navigate(page, 'account')
        expect(page.locator('[data-password-rule][data-met=true]')).to_have_count(0)
        page.fill('#new-password', 'Äbcdefghijkl1!')
        expect(page.locator('[data-password-rule][data-met=true]')).to_have_count(4)
        page.fill('#confirm-password', 'different')
        expect(page.locator('#password-match')).to_have_text('Passwords do not match yet.')
        page.fill('#confirm-password', 'Äbcdefghijkl1!')
        expect(page.locator('#password-match')).to_have_text('Passwords match.')
        review('account-feedback')
        page.fill('#new-password', 'Ab123!😀😀😀')
        expect(page.locator('[data-password-rule=length]')).to_have_attribute('data-met', 'false')
        page.fill('#new-password', 'Äbcdefghijkl!')
        expect(page.locator('[data-password-rule=number]')).to_have_attribute('data-met', 'false')
        page.fill('#confirm-password', '')
        expect(page.locator('#password-match')).not_to_be_visible()
        page.click('#back-to-top')
        expect(page.locator('#page-title')).to_be_focused()
        assert page.evaluate('window.scrollY') == 0
        assert not device.requests, 'View changes and previews must not submit settings or commands'
        assert not errors, errors
        print(f'PASS: refined views, drafts, report outline and password feedback at {width}px', flush=True)
    finally:
        context.close()


def check_summary_loading(browser):
    device = HeldHistoryDevice()
    context, page, errors = open_page(browser, device, path='index.html#history')
    try:
        page.click('[data-history-view=summary]')
        expect(page.locator('#history-value-temp')).to_have_text('—')
        first = device.take(24)
        with page.expect_request('**/api/history?hours=6'):
            page.click('[data-history-range="6"]')
        six = device.take(6)
        device.reply(first, value=99)
        expect(page.locator('#history-value-temp')).to_have_text('—')
        device.reply(six, value=26)
        expect(page.locator('#history-value-temp')).to_have_text('26')
        expect(page.locator('#history-status-temp')).to_have_text('Recording')
        with page.expect_request('**/api/history?hours=6'):
            page.click('#history-refresh')
        device.reply(device.take(6), payload={'error': 'History unavailable'}, status=503)
        expect(page.locator('#history-status-temp')).to_have_text('Last loaded')
        expect(page.locator('#history-value-temp')).to_have_text('26')
        page.click('[data-history-view=charts]')
        expect(page.locator('#history-chart-temp')).to_be_visible()
        expect(page.locator('#history-status-temp')).to_have_text('Last loaded')
        assert not device.requests
        assert not errors, errors
        print('PASS: comparison loading, range races and stale history feedback', flush=True)
    finally:
        context.close()


if __name__ == '__main__':
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            for width in [1440, 390, 320]:
                check_refinement(browser, width)
            check_summary_loading(browser)
        finally:
            browser.close()
