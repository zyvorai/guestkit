// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

// Render title/caption PNG cards for the GuestKit *web dashboard* demo videos
// (tour + tutorial) — carbon/teal theme matching zyvor-ui's own aesthetic.
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outDir = join(__dirname, "png");

function titleHtml(kicker, line1, line2) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;padding:0;width:1920px;height:1080px;background:#04100c;
    background-image:radial-gradient(ellipse 60% 50% at 20% 0%, rgba(45,212,191,0.20), transparent 55%),
                      radial-gradient(ellipse 50% 45% at 85% 100%, rgba(16,185,129,0.12), transparent 55%);
    font-family:-apple-system,'SF Pro Text',Segoe UI,sans-serif;display:flex;align-items:center;justify-content:center;}
  .wrap{text-align:center;padding:0 200px;}
  .kicker{font-family:ui-monospace,'SF Mono',monospace;color:#2dd4bf;letter-spacing:0.3em;font-size:24px;font-weight:700;margin-bottom:33px;text-transform:uppercase;}
  h1{color:#f4f7fc;font-size:68px;font-weight:800;letter-spacing:-0.02em;margin:0 0 27px;line-height:1.15;}
  p{color:#93a4bd;font-size:32px;font-weight:400;margin:0;line-height:1.5;max-width:1280px;}
  .rule{width:96px;height:5px;background:linear-gradient(90deg,#2dd4bf,#10b981);margin:39px auto 0;border-radius:5px;}
  </style></head><body>
  <div class="wrap">
    ${kicker ? `<div class="kicker">${kicker}</div>` : ""}
    <h1>${line1}</h1>
    ${line2 ? `<p>${line2}</p>` : ""}
    <div class="rule"></div>
  </div>
  </body></html>`;
}

function captionHtml(text) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;padding:0;width:1920px;height:220px;background:transparent;
    font-family:-apple-system,'SF Pro Text',Segoe UI,sans-serif;display:flex;align-items:center;justify-content:center;}
  .bar{width:1770px;background:rgba(4,16,12,0.88);border:1px solid rgba(45,212,191,0.25);border-radius:21px;
    padding:27px 45px;display:flex;align-items:center;gap:21px;box-shadow:0 18px 45px rgba(0,0,0,0.4);}
  .dot{width:14px;height:14px;border-radius:50%;background:#2dd4bf;flex:none;box-shadow:0 0 15px #2dd4bf;}
  .text{color:#f1f5f9;font-size:32px;font-weight:500;line-height:1.4;}
  </style></head><body>
  <div class="bar"><div class="dot"></div><div class="text">${text}</div></div>
  </body></html>`;
}

const cards = [
  { file: "tour-title-main", kicker: "GUESTKIT WEB", line1: "Server Image Vault — Live", line2: "Browse a real KubeVirt cluster and fingerprint VMs from the browser." },
  { file: "tour-title-cluster", kicker: "", line1: "Browse the Live Cluster" },
  { file: "tour-title-actions", kicker: "", line1: "Fingerprint, Doctor — One Click" },
  { file: "tour-outro", kicker: "", line1: "Upload a disk, browse a cluster, or import from S3/NFS/URL.", line2: "zyvor.dev/guestkit" },

  { file: "tut-title-main", kicker: "GUESTKIT WEB TUTORIAL", line1: "The Server Image Vault, End to End", line2: "Sources, live cluster browsing, and one-click intelligence — all from the browser." },
  { file: "tut-title-intake", kicker: "1", line1: "Bring a Disk — Any Source" },
  { file: "tut-title-sources", kicker: "2", line1: "Local, Vault, URL, S3, NFS, OVA" },
  { file: "tut-title-cluster", kicker: "3", line1: "Browse a Live KubeVirt Cluster" },
  { file: "tut-title-select", kicker: "4", line1: "Select a Real Running VM" },
  { file: "tut-title-inspect", kicker: "5", line1: "Fingerprint From the Browser" },
  { file: "tut-title-doctor", kicker: "6", line1: "Boot Doctor, One Click" },
  { file: "tut-outro", kicker: "", line1: "The same GuestKit engine — CLI, TUI, and now a full web control plane.", line2: "zyvor.dev/guestkit" },
];

const captions = [
  { file: "cap-landing", text: "Drag a disk, or browse VMs already running in the cluster — GuestKit fingerprints before you ever boot." },
  { file: "cap-cluster", text: "A real KubeVirt VM, live from the cluster — namespace, status, node, and IP." },
  { file: "cap-selected", text: "Select any VM to see its live guest-agent status and boot readiness." },
  { file: "cap-inspect", text: "Fingerprint runs the same GuestKit engine as the CLI — right from the dashboard." },
  { file: "cap-doctor", text: "Boot Doctor scores this VM's bootability without ever touching the CLI." },
  { file: "cap-upload", text: "Local upload, Server Vault, URL, S3, NFS, OVA, or a Cluster PVC — six ways in." },
];

const browser = await chromium.launch({ channel: "chrome" });

for (const t of cards) {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  await page.setContent(titleHtml(t.kicker, t.line1, t.line2));
  await page.screenshot({ path: `${outDir}/${t.file}.png` });
  await page.close();
  console.log("title:", t.file);
}

for (const c of captions) {
  const page = await browser.newPage({ viewport: { width: 1920, height: 220 } });
  await page.setContent(captionHtml(c.text));
  await page.screenshot({ path: `${outDir}/${c.file}.png`, omitBackground: true });
  await page.close();
  console.log("caption:", c.file);
}

await browser.close();
