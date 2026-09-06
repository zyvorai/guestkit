// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

// Render title/caption PNG cards for the GuestKit CLI/TUI demo video.
// Requires `playwright` — run from this directory (node_modules symlinked
// to a sibling project's node_modules, see README.md).
import { chromium } from "playwright";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const outDir = join(__dirname, "png");

function titleHtml(kicker, line1, line2) {
  return `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;padding:0;width:1920px;height:1080px;background:#0d0b12;
    background-image:radial-gradient(ellipse 60% 50% at 20% 0%, rgba(249,115,22,0.20), transparent 55%),
                      radial-gradient(ellipse 50% 45% at 85% 100%, rgba(234,88,12,0.12), transparent 55%);
    font-family:-apple-system,'SF Pro Text',Segoe UI,sans-serif;display:flex;align-items:center;justify-content:center;}
  .wrap{text-align:center;padding:0 200px;}
  .kicker{font-family:ui-monospace,'SF Mono',monospace;color:#fb923c;letter-spacing:0.3em;font-size:24px;font-weight:700;margin-bottom:33px;text-transform:uppercase;}
  h1{color:#f4f7fc;font-size:68px;font-weight:800;letter-spacing:-0.02em;margin:0 0 27px;line-height:1.15;}
  p{color:#93a4bd;font-size:32px;font-weight:400;margin:0;line-height:1.5;max-width:1280px;}
  .rule{width:96px;height:5px;background:linear-gradient(90deg,#f97316,#ea580c);margin:39px auto 0;border-radius:5px;}
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
  .bar{width:1770px;background:rgba(8,6,12,0.88);border:1px solid rgba(249,115,22,0.25);border-radius:21px;
    padding:27px 45px;display:flex;align-items:center;gap:21px;box-shadow:0 18px 45px rgba(0,0,0,0.4);}
  .dot{width:14px;height:14px;border-radius:50%;background:#fb923c;flex:none;box-shadow:0 0 15px #fb923c;}
  .text{color:#f1f5f9;font-size:32px;font-weight:500;line-height:1.4;}
  </style></head><body>
  <div class="bar"><div class="dot"></div><div class="text">${text}</div></div>
  </body></html>`;
}

const cards = [
  { file: "title-main", kicker: "GUESTKIT", line1: "Offline VM Intelligence, From the Terminal", line2: "Real inspection, boot scoring, and migration planning — against a real Ubuntu cloud image." },
  { file: "title-inspect", kicker: "1", line1: "Inspect — Full OS Fingerprint, Offline" },
  { file: "title-doctor", kicker: "2", line1: "Doctor — Will It Survive First Boot?" },
  { file: "title-migrate", kicker: "3", line1: "Migrate-Plan — What KVM Needs to Boot It" },
  { file: "title-tui", kicker: "4", line1: "The Carbon TUI" },
  { file: "outro", kicker: "", line1: "One Rust binary. No legacy-appliance-tooling appliance. No boot required.", line2: "zyvor.dev/guestkit" },
];

const captions = [
  { file: "cap-inspect", text: "Full OS fingerprint offline — distro, kernel, packages, services, users, SSL certs — no boot required." },
  { file: "cap-doctor", text: "A real 42% boot-assurance score with a ranked root-cause chain — the missing initramfs, explained." },
  { file: "cap-migrate", text: "Migration score, required driver injections, and exact config changes — before you touch the target." },
  { file: "cap-tui", text: "A k9s-style, Carbon-themed dashboard — health, risk profile, and live trend sparklines, keyboard-driven." },
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
