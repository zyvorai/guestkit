#!/usr/bin/env python3
"""Capture the docs screenshots from the bundled OSS web UI's offline demo.

Serves deploy/ui/ on a local port, drives it with Google Chrome via Playwright
(`pip install playwright`), and writes docs/img/ui-*.png. The data shown is the
static demo-doctor.json / demo-inspect.json that ships with the UI, not a live
deployment.
"""
import functools
import http.server
import threading
from pathlib import Path

from playwright.sync_api import sync_playwright

root = Path(__file__).resolve().parent.parent
ui = root / "deploy" / "ui"
out = root / "docs" / "img"

class QuietHandler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


handler = functools.partial(QuietHandler, directory=str(ui))
server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
threading.Thread(target=server.serve_forever, daemon=True).start()
url = f"http://127.0.0.1:{server.server_address[1]}/index.html"


def shot(page, name):
    path = out / name
    page.screenshot(path=str(path))
    print(f"wrote {path} ({path.stat().st_size // 1024} KB)")


def scroll_to_workspace(page):
    # Leave the sticky Zyvor nav visible above the cards.
    page.evaluate("window.scrollTo(0, document.querySelector('#workspace').offsetTop - 84)")
    page.wait_for_timeout(300)


with sync_playwright() as p:
    browser = p.chromium.launch(channel="chrome")
    page = browser.new_page(viewport={"width": 1280, "height": 960}, device_scale_factor=2)
    page.goto(url)
    page.wait_for_timeout(800)
    shot(page, "ui-02-landing.png")

    page.click("text=Try offline demo")
    page.wait_for_timeout(800)
    scroll_to_workspace(page)
    shot(page, "ui-00-doctor-demo.png")

    page.click(".ptab[data-panel=summary]")
    page.wait_for_timeout(400)
    scroll_to_workspace(page)
    shot(page, "ui-01-inspect-demo.png")
    browser.close()
server.shutdown()
