"""Camera controls and local-image recovery against simulated status and images."""
import base64
import json
import os
from urllib.parse import urlparse

from playwright.sync_api import expect, sync_playwright

from web_ui_test import Device, NOW, open_page, navigate, assert_fits


def check_camera_controls(browser):
    device = Device()
    device.cameras = [
        {'id': camera_id, 'label': label, 'detected': camera_id == 'ribbon',
         'stream_url': f'/api/cameras/{camera_id}/stream', 'live': camera_id == 'ribbon',
         'running': camera_id == 'ribbon', 'timestamp_ms': NOW * 1000, 'error': None,
         'viewers': 1, 'width': 640, 'height': 480, 'fps': 10}
        for camera_id, label in [('ribbon', 'Raspberry Pi camera'), ('usb', 'USB webcam')]
    ]
    context, page, errors = open_page(browser, device, 390, 844)
    frame = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+ip1sAAAAASUVORK5CYII=')
    mode = {'status': 'normal', 'stream': 'hold'}
    pending_status, pending_images, streams, status_reads = [], [], [], []

    def status_route(route):
        assert route.request.method == 'GET'
        status_reads.append(route.request.url)
        if mode['status'] == 'hold':
            pending_status.append(route)
        elif mode['status'] == 'malformed':
            route.fulfill(status=200, content_type='application/json', body=json.dumps(device.cameras[:1]))
        else:
            device.route(route)

    def stream_route(route):
        assert route.request.method == 'GET'
        streams.append(urlparse(route.request.url).path)
        if mode['stream'] == 'hold':
            pending_images.append(route)
        elif mode['stream'] == 'fail':
            route.abort('failed')
        else:
            route.fulfill(status=200, content_type='image/png', body=frame)

    context.route('https://aog.test/api/cameras', status_route)
    context.route('https://aog.test/api/cameras/*/stream?*', stream_route)
    try:
        navigate(page, 'cameras')
        ribbon_status = page.locator('#camera-status-ribbon')
        ribbon_retry = page.locator('#camera-retry-ribbon')
        ribbon_toggle = page.locator('#camera-toggle-ribbon')
        usb_status = page.locator('#camera-status-usb')
        usb_retry = page.locator('#camera-retry-usb')
        usb_toggle = page.locator('#camera-toggle-usb')

        # Another viewer can keep the shared encoder live while our first
        # image is still pending. This view must not claim to be Live yet.
        expect(ribbon_status).to_have_text('Starting')
        expect(page.locator('#camera-detail-ribbon')).to_contain_text('first frame')
        expect(page.locator('#camera-placeholder-ribbon')).to_be_visible()
        expect(page.locator('#camera-fullscreen-ribbon')).to_be_disabled()
        page.wait_for_function("document.querySelector('#camera-viewport-ribbon img')?.naturalWidth === 0")
        assert len(pending_images) == 1
        pending_images.pop().fulfill(status=200, content_type='image/png', body=frame)
        expect(ribbon_status).to_have_text('Live')
        expect(page.locator('#camera-placeholder-ribbon')).not_to_be_visible()
        expect(page.locator('#camera-fullscreen-ribbon')).to_be_enabled()

        # Recovery is a single explicit action, without a Pause/Resume cycle.
        mode['stream'] = 'fail'
        ribbon_retry.click()
        expect(ribbon_status).to_have_text('Reconnecting')
        expect(page.locator('#camera-fullscreen-ribbon')).to_be_disabled()
        mode['stream'] = 'normal'
        ribbon_retry.focus()
        expect(ribbon_retry).to_have_accessible_name('Retry view: Raspberry Pi camera')
        ribbon_retry.press('Enter')
        expect(ribbon_status).to_have_text('Live')

        # An incomplete status response is an error, not endless Connecting.
        mode['status'] = 'malformed'
        ribbon_retry.click()
        expect(ribbon_status).to_have_text('Offline')
        expect(page.locator('#cameras-error')).to_have_text('Camera status is unavailable.')
        expect(page.locator('#camera-viewport-ribbon img')).to_have_count(0)
        mode['status'] = 'normal'
        ribbon_retry.click()
        expect(ribbon_status).to_have_text('Live')
        expect(page.locator('#cameras-error')).not_to_be_visible()

        # A newly attached camera can be checked without reconnecting the
        # other healthy view. Buttons remain distinct in a screen reader.
        expect(usb_status).to_have_text('Not detected')
        expect(usb_retry).to_have_accessible_name('Check again: USB webcam')
        ribbon_streams = streams.count('/api/cameras/ribbon/stream')
        device.cameras[1].update(detected=True, live=True, running=True)
        usb_retry.click()
        expect(usb_status).to_have_text('Live')
        assert streams.count('/api/cameras/ribbon/stream') == ribbon_streams
        expect(usb_retry).to_have_accessible_name('Retry view: USB webcam')
        assert_fits(page)
        page.set_viewport_size({'width': 320, 'height': 844})
        assert_fits(page)

        # Share status reads when focus/polling overlaps a manual check.
        # A later Pause wins over the earlier request's eventual response.
        mode['status'] = 'hold'
        with page.expect_request('https://aog.test/api/cameras'):
            usb_retry.click()
        expect(usb_status).to_have_text('Checking')
        expect(usb_retry).to_be_disabled()
        expect(usb_toggle).to_be_enabled()
        assert len(pending_status) == 1
        reads = len(status_reads)
        page.evaluate("window.dispatchEvent(new Event('focus'))")
        page.wait_for_timeout(150)
        assert len(status_reads) == reads, 'Focus must share the pending camera status request'
        usb_toggle.click()
        expect(usb_status).to_have_text('Paused')
        stream_count = len(streams)
        mode['status'] = 'normal'
        pending_status.pop().fulfill(status=200, content_type='application/json', body=json.dumps(device.cameras))
        expect(usb_retry).to_have_text('Retry view')
        expect(usb_status).to_have_text('Paused')
        expect(usb_retry).to_be_disabled()
        expect(page.locator('#camera-viewport-usb img')).to_have_count(0)
        assert len(streams) == stream_count, 'Completing a check must not resume a newly paused view'
        usb_toggle.press('Enter')
        expect(usb_status).to_have_text('Live')

        # A held first frame also times out when another client keeps the
        # encoder live. Advance only Date.now; real fetch timeouts are intact.
        ribbon_toggle.click()
        mode['stream'] = 'hold'
        ribbon_toggle.click()
        expect(ribbon_status).to_have_text('Starting')
        page.wait_for_function("document.querySelector('#camera-viewport-ribbon img')?.naturalWidth === 0")
        assert len(pending_images) == 1
        page.evaluate("window.cameraTestNow = Date.now; window.cameraTestOffset = 21000; Date.now = () => window.cameraTestNow() + window.cameraTestOffset")
        with page.expect_response('https://aog.test/api/cameras'):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(ribbon_status).to_have_text('Reconnecting')
        expect(page.locator('#camera-viewport-ribbon img')).to_have_count(0)
        pending_images.pop().abort('failed')
        mode['stream'] = 'normal'
        page.evaluate('window.cameraTestOffset += 3100')
        with page.expect_response('https://aog.test/api/cameras'):
            page.evaluate("window.dispatchEvent(new Event('focus'))")
        expect(ribbon_status).to_have_text('Live')
        page.evaluate('Date.now = window.cameraTestNow')

        ribbon_toggle.click()
        expect(ribbon_status).to_have_text('Paused')
        expect(ribbon_retry).to_be_disabled()
        navigate(page, 'overview')
        expect(page.locator('.camera-stream')).to_have_count(0)
        navigate(page, 'cameras')
        expect(ribbon_status).to_have_text('Paused')
        expect(page.locator('#camera-viewport-ribbon img')).to_have_count(0)
        expect(usb_status).to_have_text('Live')
        assert not device.requests, 'Camera recovery must never send device writes'
        assert not errors, errors
    finally:
        context.close()


def run():
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(executable_path=os.environ.get('CHROMIUM', '/usr/bin/chromium'), headless=True, args=['--no-sandbox', '--disable-dev-shm-usage'])
        try:
            check_camera_controls(browser)
        finally:
            browser.close()
    print('PASS: camera first-frame status, retries, unavailable cameras, paused request recovery, keyboard and mobile controls')


if __name__ == '__main__':
    run()
