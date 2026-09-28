#!/usr/bin/env python3
"""Render the GuestKit share cards to PNG (1200x630).

    python3 docs/social/render.py

Writes guestkit-share-card.png (light; also the website og:image, served straight
from this directory) and guestkit-share-card-dark.png (README dark mode).

Needs `pip install playwright` and Google Chrome installed (uses channel="chrome",
so no Playwright browser download). The cards use system fonts only (Helvetica Neue,
Menlo), so no network access is needed.
"""
from pathlib import Path

from playwright.sync_api import sync_playwright

here = Path(__file__).resolve().parent
cards = [
    ("guestkit-share-card.html", "guestkit-share-card.png"),
    ("guestkit-share-card-dark.html", "guestkit-share-card-dark.png"),
]

with sync_playwright() as p:
    browser = p.chromium.launch(channel="chrome")
    page = browser.new_page(viewport={"width": 1200, "height": 630}, device_scale_factor=1)
    for html, png in cards:
        out = here / png
        page.goto((here / html).as_uri(), wait_until="load")
        page.evaluate("document.fonts.ready")
        page.screenshot(path=str(out), clip={"x": 0, "y": 0, "width": 1200, "height": 630})
        print(f"wrote {out} ({out.stat().st_size // 1024} KB)")
    browser.close()
