"""Credential request and bookmarked sign-in checks using intercepted APIs only."""
import os
from urllib.parse import parse_qs, urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, open_page, navigate, assert_fits


def check_account_requests(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    pending = []
    submissions = []
    password = 'ÉlanStrongPassword123!'
    controls = ['current-password', 'new-password', 'confirm-password', 'show-passwords']

    def hold_password_change(route):
        assert route.request.method == 'POST'
        assert route.request.headers.get('x-aog-csrf') == 'browser-test-token'
        submissions.append(route.request.post_data_json)
        pending.append(route)

    context.route('https://aog.test/api/account/password', hold_password_change)

    def submit():
        with page.expect_request('https://aog.test/api/account/password'):
            page.click('#password-form [type=submit]')
        expect(page.locator('#password-form')).to_have_attribute('aria-busy', 'true')
        for control in controls:
            expect(page.locator(f'#{control}')).to_be_disabled()
        expect(page.locator('#password-form [type=submit]')).to_be_disabled()
        count = len(submissions)
        page.evaluate("document.querySelector('#password-form').requestSubmit()")
        assert len(submissions) == count and len(pending) == 1, 'Duplicate password change'
        assert submissions[-1] == {
            'current_password': page.input_value('#current-password'),
            'new_password': password, 'confirm_password': password,
        }

    try:
        navigate(page, 'account')
        page.fill('#current-password', 'WrongPassword123!')
        page.fill('#new-password', password)
        page.fill('#confirm-password', password)
        page.check('#show-passwords')
        submit()
        pending.pop().fulfill(status=403, content_type='application/json',
                              body='{"error":"Current password is incorrect"}')
        expect(page.locator('#password-result')).to_contain_text('Current password is incorrect')
        expect(page.locator('#password-result')).to_be_focused()
        for control in controls:
            expect(page.locator(f'#{control}')).to_be_enabled()
        expect(page.locator('#new-password')).to_have_value(password)
        expect(page.locator('#confirm-password')).to_have_value(password)
        expect(page.locator('#show-passwords')).to_be_checked()

        # A failed response must not interrupt someone who moved to another
        # control, and must not enable controls that were already disabled.
        page.locator('#show-passwords').evaluate('field => { field.disabled = true; }')
        submit()
        page.locator('#refresh-button').focus()
        pending.pop().fulfill(status=503, content_type='application/json',
                              body='{"error":"Password update temporarily unavailable"}')
        expect(page.locator('#password-result')).to_contain_text('temporarily unavailable')
        expect(page.locator('#refresh-button')).to_be_focused()
        expect(page.locator('#show-passwords')).to_be_disabled()
        for control in controls[:-1]:
            expect(page.locator(f'#{control}')).to_be_enabled()

        page.locator('#show-passwords').evaluate('field => { field.disabled = false; }')
        submit()
        navigate(page, 'overview')
        pending.pop().fulfill(status=403, content_type='application/json',
                              body='{"error":"Current password is incorrect"}')
        expect(page.locator('#password-result')).to_contain_text('Current password is incorrect')
        expect(page.locator('#page-title')).to_be_focused()
        expect(page.locator('#page-overview')).to_be_visible()

        navigate(page, 'account')
        page.fill('#current-password', 'OldPassword123!')
        submit()
        device.authenticated = False
        pending.pop().fulfill(status=200, content_type='application/json',
                              body='{"message":"Password changed"}')
        expect(page).to_have_url('https://aog.test/login.html?changed=1')
        expect(page.locator('#login-notice')).to_have_text('Password changed. Sign in with your new password.')
        expect(page.locator('#input_password')).to_have_value('')
        assert len(submissions) == 4
        assert not device.requests, 'Account checks must not write settings or operate equipment'
        assert not errors, errors
    finally:
        context.close()


def check_login_requests(browser):
    device = Device()
    context = browser.new_context(viewport={'width': 390, 'height': 844})
    context.route('https://aog.test/**', device.route)
    pending = []
    context.route('https://aog.test/authenticate', lambda route: pending.append(route))
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    password = 'SubmittedPassword123!'
    controls = ['input_username', 'input_password', 'show-password']

    def submit():
        with page.expect_request('https://aog.test/authenticate'):
            page.click('#login-form [type=submit]')
        expect(page.locator('#login-form')).to_have_attribute('aria-busy', 'true')
        for control in controls:
            expect(page.locator(f'#{control}')).to_be_disabled()
        expect(page.locator('#login-form [type=submit]')).to_be_disabled()
        page.evaluate("document.querySelector('#login-form').requestSubmit()")
        assert len(pending) == 1, 'Duplicate sign-in request'
        assert parse_qs(pending[0].request.post_data or '') == {
            'input_username': ['admin'], 'input_password': [password],
        }, 'Disabling fields must not remove credentials from the submitted body'

    try:
        page.goto('https://aog.test/login.html#history/temp')
        page.fill('#input_password', password)
        page.check('#show-password')
        submit()
        pending.pop().fulfill(status=401, content_type='application/json',
                              body='{"error":"Unauthorized"}')
        expect(page.locator('#login-error')).to_contain_text('Username or password is incorrect')
        expect(page.locator('#login-error')).to_be_focused()
        for control in controls:
            expect(page.locator(f'#{control}')).to_be_enabled()
        expect(page.locator('#input_password')).to_have_value(password)
        expect(page.locator('#input_password')).to_have_attribute('aria-invalid', 'true')
        expect(page.locator('#show-password')).to_be_checked()

        # Supply a focusable element outside the form to represent moving on
        # while waiting; the response should announce the error without focus theft.
        page.evaluate("""() => {
            const other = document.createElement('button');
            other.id = 'outside-login';
            other.textContent = 'Another task';
            document.querySelector('.login-footer').append(other);
            document.querySelector('#show-password').disabled = true;
        }""")
        password = 'CorrectedPassword123!'
        page.fill('#input_password', password)
        assert page.locator('#input_password').get_attribute('aria-invalid') is None
        submit()
        page.locator('#outside-login').focus()
        pending.pop().abort('failed')
        expect(page.locator('#login-error')).to_contain_text('Could not reach your device')
        expect(page.locator('#outside-login')).to_be_focused()
        expect(page.locator('#input_username')).to_be_enabled()
        expect(page.locator('#input_password')).to_be_enabled()
        expect(page.locator('#show-password')).to_be_disabled()
        expect(page.locator('#login-form [type=submit]')).to_be_enabled()
        expect(page.locator('#login-form')).to_have_attribute('aria-busy', 'false')
        expect(page.locator('#input_password')).to_have_value(password)
        assert_fits(page)
        assert not errors, errors
    finally:
        context.close()


def check_login_bookmarks(browser):
    device = Device()
    context = browser.new_context()
    context.route('https://aog.test/**', device.route)
    unexpected_requests = []

    def block_external_requests(route):
        if urlparse(route.request.url).hostname != 'aog.test':
            unexpected_requests.append(route.request.url)
            route.abort()
        else:
            route.fallback()

    # A successful fetch receives HTML after the real backend's redirect.
    # Mock its final response, since mocked 302s bypass Playwright routing.
    context.route('https://aog.test/authenticate', lambda route: route.fulfill(
        status=200, content_type='text/html', body='<!doctype html><title>AOG</title>'))
    context.route('**/*', block_external_requests)
    page = context.new_page()
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    cases = [
        ('#history/temp', 'history/temp', 'Temperature history'),
        ('#settings/calibration', 'settings/calibration', 'Probe calibration'),
        ('#overview/ph', 'overview/ph', 'pH history'),
        ('#https://elsewhere.invalid/path', None, None),
        ('#history/invalid', None, None),
        ('#__proto__', None, None),
        ('?return=account#history/temp', 'account', 'Your account'),
        ('?return=invalid#history/temp', None, None),
        ('?changed=1&return=account#history/temp', None, 'Password changed.'),
    ]
    try:
        for suffix, view, notice in cases:
            page.goto(f'https://aog.test/login.html{suffix}')
            if notice:
                expect(page.locator('#login-notice')).to_contain_text(notice)
            else:
                expect(page.locator('#login-notice')).to_be_hidden()
            page.fill('#input_password', 'NotARealPassword123!')
            page.click('#login-form [type=submit]')
            expect(page).to_have_url(f'https://aog.test/index.html{("#" + view) if view else ""}')
            expect(page.locator('#connection')).to_contain_text('Connected')
            expect(page.locator(f'#page-{view.split("/")[0] if view else "overview"}')).to_be_visible()
            if view == 'settings/calibration':
                expect(page.locator('#calibration-form h2')).to_be_focused()
            elif view == 'history/temp':
                expect(page.locator('#history-card-temp')).to_be_visible()
                expect(page.locator('#history-card-co2')).not_to_be_visible()
            elif view == 'overview/ph':
                expect(page.locator('#ph-history-title')).to_be_focused()
        assert not unexpected_requests, unexpected_requests
        assert not device.requests, 'Bookmark sign-in must not write settings or operate equipment'
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
            headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_account_requests(browser)
            check_login_requests(browser)
            check_login_bookmarks(browser)
        finally:
            browser.close()
    print('PASS: frozen credentials, request failure focus, and safe bookmarked sign-in returns')


if __name__ == '__main__':
    run()
