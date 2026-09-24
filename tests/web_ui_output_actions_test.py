"""Output command ordering checks using intercepted, simulated device writes."""
import os

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, open_page, navigate, assert_fits


def check_output_actions(browser):
    # A Stop while Resume is pending must be sent immediately and reasserted.
    device = Device()
    device.outputs['automation_paused'] = True
    context, page, errors = open_page(browser, device)
    try:
        navigate(page, 'outputs')
        held = []
        context.route('**/api/schedules/resume', lambda route: held.append(route))
        with page.expect_request('**/api/schedules/resume'):
            page.click('#paused-banner .resume-schedules')
        expect(page.locator('#stop-all')).to_be_enabled()
        page.click('#stop-all')
        expect(page.locator('#output-action-feedback')).to_contain_text('Waiting for earlier')
        assert len(held) == 1
        assert any(path == '/api/outputs/stop-all' for path, _ in device.requests)
        expect(page.locator('[data-output="3"]')).to_be_disabled()
        expect(page.locator('#paused-banner .resume-schedules')).to_be_disabled()
        device.route(held.pop())
        expect(page.locator('#stop-all')).to_have_text('■ Stop all')
        expect(page.locator('#stop-all')).to_be_enabled()
        assert [path for path, _ in device.requests].count('/api/outputs/stop-all') == 2
        assert device.outputs['automation_paused'] is True
        assert not errors, errors
    finally:
        context.close()

    for failed_start in [False, True]:
        device = Device()
        context, page, errors = open_page(browser, device, 390, 844)
        try:
            navigate(page, 'outputs')
            held = []
            context.route('**/api/outputs/start', lambda route: held.append(route))
            relay = page.locator('[data-output="3"]')
            with page.expect_request('**/api/outputs/start'):
                relay.click()
            expect(relay).to_have_text('Stop')
            expect(relay).to_be_enabled()
            page.click('#stop-all')
            expect(page.locator('#output-action-feedback')).to_contain_text('Waiting for earlier')
            expect(relay).to_be_disabled()
            assert len(held) == 1
            if failed_start:
                held.pop().abort('failed')
            else:
                # The device receives this earlier Start AFTER the first Stop.
                device.route(held.pop())
            expect(page.locator('#stop-all')).to_have_text('■ Stop all')
            expect(page.locator('#stop-all')).to_be_enabled()
            assert [path for path, _ in device.requests].count('/api/outputs/stop-all') == 2
            assert all(not output['on'] for output in device.outputs['outputs'])
            if failed_start:
                expect(page.locator('#output-action-feedback')).to_contain_text('could not be confirmed')
                expect(page.locator('#output-action-feedback')).to_contain_text('may still reach the device')
            else:
                expect(page.locator('#output-action-feedback')).not_to_be_visible()
            assert_fits(page)
            assert not errors, errors
        finally:
            context.close()

    # A per-relay Stop can also supersede its pending Start.
    device = Device()
    context, page, errors = open_page(browser, device)
    try:
        navigate(page, 'outputs')
        held = []
        context.route('**/api/outputs/start', lambda route: held.append(route))
        relay = page.locator('[data-output="3"]')
        with page.expect_request('**/api/outputs/start'):
            relay.click()
        expect(relay).to_have_text('Stop')
        relay.click()
        expect(page.locator('#output-action-feedback')).to_contain_text('Waiting for earlier')
        device.route(held.pop())
        expect(relay).to_have_text('Start')
        expect(relay).to_be_enabled()
        assert [path for path, _ in device.requests].count('/api/outputs/stop') == 2
        assert device.outputs['outputs'][2]['on'] is False
        assert not errors, errors
    finally:
        context.close()

    # Paused automation can still have manual outputs that need stopping.
    device = Device()
    device.outputs['automation_paused'] = True
    device.outputs['outputs'][2]['on'] = True
    context, page, errors = open_page(browser, device)
    try:
        navigate(page, 'schedules')
        pause = page.locator('#pause-schedules')
        expect(pause).to_be_enabled()
        context.route('**/api/schedules/pause', lambda route: route.fulfill(status=409, content_type='application/json', body='{"error":"Relay board did not confirm Stop"}'))
        pause.click()
        expect(page.locator('#output-action-feedback')).to_contain_text('stop request could not be confirmed')
        expect(page.locator('#global-message')).to_contain_text('did not confirm Stop')
        expect(pause).to_be_enabled()
        expect(page.locator('#stop-all')).to_be_enabled()
        context.unroute('**/api/schedules/pause')
        pause.click()
        expect(page.locator('#output-state-3')).to_have_text('Off')
        expect(pause).to_be_disabled()
        assert all(not output['on'] for output in device.outputs['outputs'])
        assert not errors, errors
    finally:
        context.close()

    # A malformed success body must not poison cached status or bypass readback.
    device = Device()
    device.outputs['outputs'][2]['on'] = True
    context, page, errors = open_page(browser, device)
    try:
        navigate(page, 'outputs')
        context.route('**/api/outputs/stop-all', lambda route: route.fulfill(status=200, content_type='application/json', body='{}'))
        with page.expect_response('**/api/outputs'):
            page.click('#stop-all')
        expect(page.locator('#global-message')).to_contain_text('incomplete relay status')
        expect(page.locator('#output-action-feedback')).to_contain_text('could not be confirmed')
        expect(page.locator('#output-state-3')).to_have_text('On')
        expect(page.locator('[data-output="3"]')).to_have_text('Stop')
        expect(page.locator('[data-output="3"]')).to_be_enabled()
        expect(page.locator('#stop-all')).to_be_enabled()
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_output_actions(browser)
        finally:
            browser.close()
    print('PASS: Stop during pending Resume/Start, stop reassertion, uncertain commands, per-relay Stop, and paused manual output controls')


if __name__ == '__main__':
    run()
