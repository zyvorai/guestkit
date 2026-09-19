#!/usr/bin/env python3
"""Render guestkit-share-card.html to guestkit-share-card.png (1200x630).

Needs `pip install playwright` and Google Chrome installed (uses channel="chrome",
so no Playwright browser download). Fonts load from Google Fonts, so run online.
"""
from pathlib import Path

from playwright.sync_api import sync_playwright

here = Path(__file__).resolve().parent
src = here / "guestkit-share-card.html"
out = here / "guestkit-share-card.png"

with sync_playwright() as p:
    browser = p.chromium.launch(channel="chrome")
    page = browser.new_page(viewport={"width": 1200, "height": 630}, device_scale_factor=1)
    page.goto(src.as_uri(), wait_until="networkidle")
    page.evaluate("document.fonts.ready")
    page.screenshot(path=str(out), clip={"x": 0, "y": 0, "width": 1200, "height": 630})
    browser.close()

print(f"wrote {out} ({out.stat().st_size // 1024} KB)")
