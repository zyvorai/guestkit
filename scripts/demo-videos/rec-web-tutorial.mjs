// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

// Records a detailed tutorial walkthrough of the GuestKit web dashboard (zyvor-ui).
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
  recordVideo: { dir: `${outDir}/raw/web-tutorial`, size: { width: 1920, height: 1080 } },
});
const page = await context.newPage();
const t0 = Date.now();
const mark = (l) => console.log(`${((Date.now() - t0) / 1000).toFixed(2)}s ${l}`);

await page.goto(base, { waitUntil: "load", timeout: 20000 });
await page.waitForTimeout(3200);
mark("landing");
await page.locator('text="Skip"').first().click().catch(() => {});
await page.waitForTimeout(1500);

// Local Upload intake — explain source formats + analyze-without-booting
await page.waitForTimeout(2800);
mark("local-upload-intake");

// Server Vault sources tabs
for (const tab of ["Server Vault", "URL", "S3", "NFS", "OVA / Cluster", "Local Upload"]) {
  const el = page.locator(`text="${tab}"`).first();
  if (await el.count()) {
    await el.click().catch(() => {});
    await page.waitForTimeout(900);
  }
}
mark("source-tabs");

// Browse the live KubeVirt cluster
await page.locator('text="Browse cluster"').first().click();
await page.waitForTimeout(3500);
mark("browse-cluster");

// Select the real running VM
await page.locator('text="forge/bug-hunt-vm-kv"').first().click();
await page.waitForTimeout(3200);
mark("vm-selected");

// Fingerprint (Inspect)
await page.locator("#dockInspect").click();
await page.waitForTimeout(4200);
mark("inspect");

// Boot Doctor
await page.locator("#dockDoctor").click();
await page.waitForTimeout(4200);
mark("doctor");

// Tour the rest of the bottom dock
for (const id of ["dockRepair", "dockPlan", "dockLaunch", "dockLogsBtn", "dockYamlBtn"]) {
  const el = page.locator(`#${id}`);
  if (await el.count()) {
    await el.click().catch(() => {});
    await page.waitForTimeout(1800);
  }
}
mark("dock-tour");

await page.waitForTimeout(1500);
mark("end");
await context.close();
await browser.close();
console.log("DONE");
