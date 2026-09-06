// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

// Records a live tour of the GuestKit web dashboard (zyvor-ui) for the demo video.
// Env vars:
//   GK_WEB_URL   Base URL of the running zyvor-ui dashboard (default: http://212.8.248.187:30081/)
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const base = process.env.GK_WEB_URL || "http://212.8.248.187:30081/";
const outDir = __dirname;

const browser = await chromium.launch({ channel: "chrome" });
const context = await browser.newContext({
  viewport: { width: 1920, height: 1080 },
  recordVideo: { dir: `${outDir}/raw/web-tour`, size: { width: 1920, height: 1080 } },
});
const page = await context.newPage();
const t0 = Date.now();
const mark = (l) => console.log(`${((Date.now() - t0) / 1000).toFixed(2)}s ${l}`);

await page.goto(base, { waitUntil: "load", timeout: 20000 });
await page.waitForTimeout(2500);
mark("landing");
await page.locator('text="Skip"').first().click().catch(() => {});
await page.waitForTimeout(1200);

// Browse the live KubeVirt cluster
await page.locator('text="Browse cluster"').first().click();
await page.waitForTimeout(3200);
mark("browse-cluster");

// Select a real running VM
const vmCard = page.locator('.vault-card, [class*="card"]').filter({ hasText: "bug-hunt-vm-kv" }).first();
await page.locator('text="forge/bug-hunt-vm-kv"').first().click();
await page.waitForTimeout(2800);
mark("vm-selected");

// Inspect via the bottom dock (safe now — real result or a sensible message)
await page.locator("#dockInspect").click();
await page.waitForTimeout(3500);
mark("inspect-clicked");

// Doctor
await page.locator("#dockDoctor").click();
await page.waitForTimeout(3500);
mark("doctor-clicked");

// Local upload intake panel
await page.locator('text="Local Upload"').first().click().catch(() => {});
await page.waitForTimeout(1800);
mark("local-upload-tab");

await page.waitForTimeout(1500);
mark("end");
await context.close();
await browser.close();
console.log("DONE");
