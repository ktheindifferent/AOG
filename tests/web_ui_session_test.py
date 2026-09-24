"""Session return-route checks with simulated sign-in and intercepted APIs only."""
import os
from urllib.parse import parse_qs, urlencode, urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, open_page, assert_fits


def check_session_return(browser):
    device = Device()
    context, page, errors = open_page(browser, device, width=390, height=844)
    authentication = {'fail': False}
    submissions = []
    unexpected_requests = []
    test_password = 'NotARealPassword123!'
    draft_name = 'Unsubmitted setup draft'

    def authenticate(route):
        assert route.request.method == 'POST'
        submissions.append(parse_qs(route.request.post_data or ''))
        if authentication['fail']:
            route.fulfill(status=401, content_type='application/json',
                          body='{"error":"Invalid username or password"}')
            return
        device.authenticated = True
        # Fetch receives the dashboard's successful HTML response after the
        # backend's 302. Fulfill that final response here: Chromium bypasses
        # Playwright's route interception when following a mocked redirect.
        route.fulfill(status=200, content_type='text/html', body='<!doctype html><title>AOG</title>')

    def block_external_requests(route):
        if urlparse(route.request.url).hostname != 'aog.test':
            unexpected_requests.append(route.request.url)
            route.abort()
        else:
            route.fallback()

    context.route('https://aog.test/authenticate', authenticate)
    context.route('**/*', block_external_requests)

    def sign_in(destination):
        page.fill('#input_password', test_password)
        page.click('#login-form [type=submit]')
        expect(page).to_have_url(f'https://aog.test{destination}')
        expect(page.locator('#connection')).to_contain_text('Connected')

    try:
        for view, label in [
            ('settings/calibration', 'Probe calibration'),
            ('history/temp', 'Temperature history'),
            ('overview/ph', 'pH history'),
        ]:
            page.goto(f'https://aog.test/index.html#{view}')
            expect(page.locator('#connection')).to_contain_text('Connected')
            if view == 'settings/calibration':
                page.fill('#setup-name-input', draft_name)
                expect(page.locator('[data-form-state="setup-form"]')).to_have_text('Unsaved changes')
            device.authenticated = False
            # Let the existing status polls observe the 401. Clicking a control
            # here races the automatic redirect and may target a detached page.
            expected_query = {'expired': '1', 'return': view}
            if view == 'settings/calibration':
                expected_query['unsaved'] = '1'
            login_url = 'https://aog.test/login.html?' + urlencode(expected_query)
            expect(page).to_have_url(login_url)
            expect(page.locator('#login-notice')).to_contain_text(f'return to {label}')
            expect(page.locator('#input_password')).to_have_value('')
            assert parse_qs(urlparse(page.url).query) == {key: [value] for key, value in expected_query.items()}
            if view == 'settings/calibration':
                expect(page.locator('#login-notice')).to_contain_text('Your unsaved edits were not saved. Enter them again after signing in.')
            else:
                expect(page.locator('#login-notice')).not_to_contain_text('unsaved edits')
            assert_fits(page)

            if view == 'settings/calibration':
                authentication['fail'] = True
                page.fill('#input_password', test_password)
                page.click('#login-form [type=submit]')
                expect(page.locator('#login-error')).to_contain_text('Username or password is incorrect')
                expect(page).to_have_url(login_url)
                expect(page.locator('#login-notice')).to_contain_text('Probe calibration')
                stored_values = page.evaluate('JSON.stringify([Object.entries(localStorage), Object.entries(sessionStorage)])')
                assert test_password not in stored_values and draft_name not in stored_values
                authentication['fail'] = False

            sign_in(f'/index.html#{view}')
            if view == 'settings/calibration':
                expect(page.locator('#calibration-form h2')).to_be_focused()
                expect(page.locator('#setup-name-input')).to_have_value(device.settings['setup']['setup_name'])
            elif view == 'history/temp':
                expect(page.locator('#history-card-temp')).to_be_visible()
                expect(page.locator('#history-card-co2')).not_to_be_visible()
            else:
                expect(page.locator('#ph-history-title')).to_be_focused()

        # The parameter is a view identifier, never a redirect URL. Reject
        # external URLs, script schemes, extra segments, and prototype keys.
        for invalid in [
            'https://elsewhere.invalid/path', '//elsewhere.invalid/path',
            'javascript:alert(1)', 'settings/calibration/extra',
            'history/invalid', 'constructor', '__proto__',
        ]:
            page.goto('https://aog.test/login.html?' + urlencode({'expired': '1', 'return': invalid}))
            expect(page.locator('#login-notice')).to_have_text('Your session expired. Sign in again to continue.')
            sign_in('/index.html')
            expect(page.locator('#page-overview')).to_be_visible()

        # Password changes deliberately retain their existing confirmation and
        # overview destination even if a return parameter is supplied.
        page.goto('https://aog.test/login.html?' + urlencode({'changed': '1', 'return': 'history/temp', 'unsaved': '1'}))
        expect(page.locator('#login-notice')).to_have_text('Password changed. Sign in with your new password.')
        sign_in('/index.html')
        assert not unexpected_requests, unexpected_requests
        assert not device.requests, 'Session navigation must not save drafts or operate equipment'
        assert submissions and all(set(fields) == {'input_username', 'input_password'} for fields in submissions)
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
            headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_session_return(browser)
        finally:
            browser.close()
    print('PASS: session return routes, rejected redirect URLs, failed sign-in recovery, and unchanged password confirmation')


if __name__ == '__main__':
    run()
