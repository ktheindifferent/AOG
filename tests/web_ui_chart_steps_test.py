"""Precise chart stepping on mobile with simulated pH and history samples."""
import os

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, NOW, open_page, navigate, assert_fits


class DenseChartDevice(Device):
    def __init__(self):
        super().__init__()
        self.history = [{'timestamp': NOW - (1439 - index) * 5, 'ph_value': 7.0, 'raw_value': 7.0, 'temperature': 25.0, 'alert_level': 'Normal'} for index in range(1440)]
        self.empty_sensors = False
        self.settings['ph']['monitoring_interval_seconds'] = 5

    def sensor_history(self, hours):
        response = super().sensor_history(hours)
        if self.empty_sensors:
            for series in response['series']:
                series.update(points=[], latest=None, count=0, minimum=None, maximum=None, average=None, available=False)
        return response


def check_chart_steps(browser):
    device = DenseChartDevice()
    context, page, errors = open_page(browser, device, 390, 844)
    try:
        chart = page.locator('#ph-chart-svg')
        group = page.locator('#ph-chart-svg').locator('xpath=ancestor::div[contains(@class,"chart-interaction")]')
        previous = page.get_by_role('button', name='Previous pH reading', exact=True)
        next_reading = page.get_by_role('button', name='Next pH reading', exact=True)
        reading = page.locator('#ph-chart-reading')
        tooltip = page.locator('#chart-tooltip')
        announcement = group.locator('.chart-announcement')
        expect(reading).to_contain_text('Reading 1440 of 1440')
        expect(next_reading).to_have_attribute('aria-disabled', 'true')

        # No tap/focus on the plot is needed to begin precise stepping.
        previous.click()
        expect(reading).to_contain_text('Reading 1439 of 1440')
        expect(tooltip).to_be_visible()
        first = reading.text_content()
        expected_time = page.evaluate("stamp => new Date(stamp * 1000).toLocaleString([], {month:'short',day:'numeric',hour:'2-digit',minute:'2-digit',second:'2-digit'})", device.history[-2]['timestamp'])
        expect(reading).to_contain_text(expected_time)
        next_reading.click()
        expect(reading).to_contain_text('Reading 1440 of 1440')
        assert reading.text_content() != first
        previous.click()
        selected = reading.text_content()
        with page.expect_response('**/api/ph'):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(reading).to_have_text(selected)
        expect(tooltip).to_be_visible()
        expect(previous).to_be_focused()

        chart.focus()
        chart.press('Home')
        expect(reading).to_contain_text('Reading 1 of 1440')
        expect(previous).to_have_attribute('aria-disabled', 'true')
        previous.click(force=True)
        expect(reading).to_contain_text('Reading 1 of 1440')
        next_reading.focus()
        next_reading.press('Enter')
        expect(reading).to_contain_text('Reading 2 of 1440')
        expect(tooltip).to_be_visible()
        expect(announcement).to_have_text(reading.text_content())
        next_reading.press('Escape')
        expect(tooltip).not_to_be_visible()
        next_reading.press('Enter')
        expect(reading).to_contain_text('Reading 3 of 1440')

        # Hover changes the visible sample without repeatedly announcing it.
        spoken = announcement.text_content()
        bounds = chart.bounding_box()
        chart.dispatch_event('pointermove', {'clientX': bounds['x'] + bounds['width'] - 15, 'clientY': bounds['y'] + 80})
        expect(announcement).to_have_text(spoken)
        expect(reading).to_have_attribute('aria-live', 'off')
        page.locator('#refresh-button').focus()
        expect(tooltip).not_to_be_visible()
        assert_fits(page)

        navigate(page, 'history')
        page.click('[data-history-sensor="co2"]')
        history_reading = page.locator('#history-reading-co2')
        history_previous = page.get_by_role('button', name='Previous CO₂ reading', exact=True)
        history_next = page.get_by_role('button', name='Next CO₂ reading', exact=True)
        expect(history_reading).to_contain_text('Reading 120 of 120')
        history_previous.click()
        expect(history_reading).to_contain_text('Reading 119 of 120')
        history_next.focus()
        expect(page.locator('#history-tooltip-co2')).to_be_visible()
        history_next.press('Enter')
        expect(history_reading).to_contain_text('Reading 120 of 120')
        expect(history_next).to_have_attribute('aria-disabled', 'true')
        history_next.click(force=True)
        expect(history_reading).to_contain_text('Reading 120 of 120')
        selected = history_reading.text_content()
        with page.expect_response('**/api/history?*'):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(history_reading).to_have_text(selected)
        expect(history_next).to_be_focused()
        expect(page.locator('#history-tooltip-co2')).to_be_visible()
        assert_fits(page)

        device.history = []
        device.empty_sensors = True
        page.click('#refresh-button')
        expect(page.locator('#refresh-button')).to_be_enabled()
        expect(history_reading).to_have_text('No readings to inspect.')
        expect(history_previous).to_be_disabled()
        expect(history_next).to_be_disabled()
        expect(page.locator('#history-chart-co2')).not_to_be_visible()
        navigate(page, 'overview')
        expect(reading).to_have_text('No readings to inspect.')
        expect(previous).to_be_disabled()
        expect(next_reading).to_be_disabled()
        expect(chart).not_to_be_visible()
        assert not device.requests, 'Chart inspection must never write to device APIs'
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_chart_steps(browser)
        finally:
            browser.close()
    print('PASS: precise mobile chart steps, second-resolution pH, keyboard focus, polling, quiet hover, and empty states')


if __name__ == '__main__':
    run()
