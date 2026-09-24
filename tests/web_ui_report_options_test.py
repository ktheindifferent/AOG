"""Report request snapshots while next-export options remain editable; APIs are mocked."""
import html
import json
import os
from pathlib import Path
from urllib.parse import parse_qs, urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, open_page, navigate, assert_fits


def check_report_options(browser):
    device = Device()
    context, page, errors = open_page(browser, device)
    pending = []
    submissions = []
    downloads = []
    actions = ['report-preview', 'report-html', 'export-csv', 'export-json']

    def hold_export(route):
        assert route.request.method == 'GET'
        query = parse_qs(urlparse(route.request.url).query, keep_blank_values=True)
        submissions.append(query)
        pending.append((route, query))

    context.route('https://aog.test/api/air-quality/**', hold_export)
    page.on('download', lambda download: downloads.append(download))

    def submit(kind, preview=False):
        button = 'report-preview' if preview else 'report-html' if kind == 'html' else f'export-{kind}'
        with page.expect_request(lambda request: '/api/air-quality/' in request.url):
            page.click(f'#{button}')
        for action in actions:
            expect(page.locator(f'#{action}')).to_be_disabled()
        for field in ['report-range', 'report-title', 'report-notes']:
            expect(page.locator(f'#{field}')).to_be_enabled()
        assert len(pending) == 1
        return submissions[-1]

    def finish(preview=False):
        route, query = pending.pop()
        kind = query['format'][0]
        if kind == 'html':
            content_type = 'text/html'
            body = f'<!doctype html><html lang="en"><title>Report</title><h1>{html.escape(query["title"][0])}</h1><p>{html.escape(query["notes"][0])}</p></html>'
        elif kind == 'csv':
            content_type = 'text/csv'
            body = f'hours,current\n{query["hours"][0]},{query["current"][0]}\n'
        else:
            content_type = 'application/json'
            body = json.dumps({'submitted': query})
        headers = {'Content-Disposition': f'attachment; filename="submitted-selection.{kind}"'}
        if preview:
            route.fulfill(status=200, content_type=content_type, body=body)
            return
        with page.expect_download() as completed:
            route.fulfill(status=200, content_type=content_type, body=body, headers=headers)
        download = completed.value
        assert download.suggested_filename == f'submitted-selection.{kind}'
        assert Path(download.path()).read_text() == body

    def ready():
        for action in actions:
            expect(page.locator(f'#{action}')).to_be_enabled()

    try:
        navigate(page, 'reports')
        page.fill('#report-title', 'First report')
        page.fill('#report-notes', 'First observations')
        query = submit('csv')
        assert query['hours'] == ['24'] and query['current'] == ['1']
        expect(page.locator('#export-result')).to_contain_text('Selected: Last 24 hours')
        page.select_option('#report-range', '168')
        page.uncheck('#report-current')
        page.fill('#report-title', 'Second report')
        page.fill('#report-notes', 'Next observations')
        expect(page.locator('#report-selection-summary')).to_contain_text('Last 7 days · historical readings only')
        finish()
        expect(page.locator('#export-result')).to_contain_text('CSV download is ready')
        expect(page.locator('#export-result')).to_contain_text('Prepared for: Last 24 hours · includes current readings')
        expect(page.locator('#export-result')).to_contain_text('options changed during preparation')
        expect(page.locator('#export-result')).not_to_contain_text('Last 7 days')
        expect(page.locator('#report-notes')).to_be_focused()
        expect(page.locator('#report-title')).to_have_value('Second report')
        expect(page.locator('#report-notes')).to_have_value('Next observations')
        ready()

        # HTML captures its title and notes as well as the data selection.
        query = submit('html')
        assert query['title'] == ['Second report'] and query['notes'] == ['Next observations']
        page.fill('#report-title', 'Third report')
        page.fill('#report-notes', 'Newer notes for another report')
        finish()
        expect(page.locator('#report-result')).to_contain_text('Prepared for: “Second report” · Last 7 days')
        expect(page.locator('#report-result')).to_contain_text('options changed during preparation')
        expect(page.locator('#report-result')).not_to_contain_text('Third report')
        expect(page.locator('#report-notes')).to_have_value('Newer notes for another report')
        ready()

        # The history shortcut changes options too, even while an export waits.
        query = submit('json')
        assert query['hours'] == ['168']
        navigate(page, 'history')
        page.click('[data-history-range="6"]')
        page.click('#history-to-reports')
        expect(page.locator('#page-reports')).to_be_visible()
        expect(page.locator('#report-range')).to_have_value('6')
        finish()
        expect(page.locator('#export-result')).to_contain_text('Prepared for: Last 7 days')
        expect(page.locator('#export-result')).to_contain_text('options changed during preparation')
        expect(page.locator('#report-selection-summary')).to_contain_text('Last 6 hours')
        ready()

        # Compare effective options, so changing and reverting does not warn.
        long_title = 'r' * 100
        page.fill('#report-title', long_title)
        submit('html')
        page.fill('#report-title', 'Temporary title')
        page.fill('#report-title', long_title)
        finish()
        expect(page.locator('#report-result')).to_contain_text(f'Prepared for: “{long_title}” · Last 6 hours')
        expect(page.locator('#report-result')).not_to_contain_text('options changed')
        page.set_viewport_size({'width': 320, 'height': 844})
        assert_fits(page)
        page.set_viewport_size({'width': 1440, 'height': 1080})
        ready()

        count = len(downloads)
        submit('json')
        pending.pop()[0].fulfill(status=503, content_type='application/json',
                                body='{"error":"Recorded data temporarily unavailable"}')
        expect(page.locator('#export-result')).to_have_class('form-result error')
        expect(page.locator('#export-result')).to_contain_text('Recorded data temporarily unavailable')
        expect(page.locator('#export-result')).to_contain_text('Requested for: Last 6 hours')
        expect(page.locator('#export-result')).not_to_contain_text('options changed')
        assert len(downloads) == count
        ready()

        # New incomplete custom dates must not be validated or focused by a
        # response to the earlier valid selection, including on request failure.
        submit('csv')
        page.select_option('#report-range', 'custom')
        page.fill('#report-start', '')
        page.fill('#report-end', '')
        page.fill('#report-notes', 'Keep this unfinished next export')
        pending.pop()[0].abort('failed')
        expect(page.locator('#export-result')).to_contain_text('Could not reach the device')
        expect(page.locator('#export-result')).to_contain_text('Requested for: Last 6 hours')
        expect(page.locator('#export-result')).to_contain_text('preserved for your next attempt')
        expect(page.locator('#report-notes')).to_be_focused()
        expect(page.locator('#report-start')).to_have_value('')
        expect(page.locator('#report-end')).to_have_value('')
        assert page.locator('#report-start').get_attribute('aria-invalid') is None
        assert page.locator('#report-end').get_attribute('aria-invalid') is None
        assert len(downloads) == count
        ready()

        page.select_option('#report-range', '24')
        page.fill('#report-title', 'Preview selection')
        page.fill('#report-notes', 'Preview notes')
        with page.expect_popup() as opened:
            submit('html', preview=True)
        preview = opened.value
        page.fill('#report-notes', 'New preview notes')
        finish(preview=True)
        expect(preview.locator('h1')).to_have_text('Preview selection')
        expect(preview.locator('p')).to_have_text('Preview notes')
        assert preview.evaluate('window.opener === null')
        expect(page.locator('#report-result')).to_contain_text('Report opened in a new tab')
        expect(page.locator('#report-result')).to_contain_text('options changed during preparation')
        expect(page.locator('#report-notes')).to_have_value('New preview notes')
        preview.close()
        ready()

        # Raw files never include report-only metadata, so editing notes alone
        # must not imply that a second raw export would include those notes.
        submit('json')
        page.fill('#report-notes', 'Notes apply to HTML reports only')
        finish()
        expect(page.locator('#export-result')).to_contain_text('JSON download is ready')
        expect(page.locator('#export-result')).not_to_contain_text('options changed')
        ready()
        assert len(submissions) == 8
        assert not device.requests, 'Report workflows must not change settings or equipment'
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'),
            headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_report_options(browser)
        finally:
            browser.close()
    print('PASS: submitted report selections, editable next exports, history shortcuts, and failure recovery')


if __name__ == '__main__':
    run()
