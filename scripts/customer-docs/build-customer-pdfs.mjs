// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

#!/usr/bin/env node
/**
 * Shared PDF builder — set CUSTOMER_DOCS_PRODUCT and optional CUSTOMER_DOCS_SLUG.
 */
import { execFileSync } from 'node:child_process'
import { existsSync, mkdirSync, readFileSync, writeFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
try {
  const envText = readFileSync(resolve(ROOT, 'scripts/customer-docs/product.env'), 'utf8')
  for (const line of envText.split('\n')) {
    const m = line.match(/^([A-Z0-9_]+)=(.*)$/)
    if (m) process.env[m[1]] = m[2].replace(/^['"]|['"]$/g, '')
  }
} catch {}
const CUSTOMER = resolve(ROOT, 'docs/customer')
const PDF_DIR = resolve(CUSTOMER, 'pdf')
const PRODUCT = process.env.CUSTOMER_DOCS_PRODUCT || 'Product'
const SLUG = (process.env.CUSTOMER_DOCS_SLUG || PRODUCT).toLowerCase().replace(/\s+/g, '-')
const PDF_PREFIX = process.env.CUSTOMER_DOCS_PDF_PREFIX || PRODUCT.replace(/\s+/g, '-')

const MARKED_CANDIDATES = [
  resolve(ROOT, '.docs-tools/node_modules/.bin/marked'),
  resolve(ROOT, 'web/dashboard/node_modules/.bin/marked'),
  resolve(ROOT, 'web/dashboard-react/node_modules/.bin/marked'),
  resolve(ROOT, 'web-ui/node_modules/.bin/marked'),
  resolve(ROOT, 'ui/node_modules/.bin/marked'),
  resolve(ROOT, 'desktop/node_modules/.bin/marked'),
  resolve(ROOT, 'crates/atlas-gateway/ui/node_modules/.bin/marked'),
  resolve(ROOT, '../ironwolf/.docs-tools/node_modules/.bin/marked'),
  resolve(ROOT, '../forge/web-ui/node_modules/.bin/marked'),
]
const MARKED = MARKED_CANDIDATES.find((p) => existsSync(p))

const THEMES = {
  HyperCluster: { accent: '#34d399', grad: '#052e16,#14532d,#0d1b2a', brandHtml: 'Hyper<span>Cluster</span>' },
  Hyper2KVM: { accent: '#22d3ee', grad: '#083344,#155e75,#0d1b2a', brandHtml: 'Hyper<span>2KVM</span>' },
  GuestKit: { accent: '#fbbf24', grad: '#1c1917,#78350f,#0d1b2a', brandHtml: 'Guest<span>Kit</span>' },
  Ragnarok: { accent: '#f43f5e', grad: '#1a0a0f,#881337,#0d1b2a', brandHtml: 'Ragna<span>rok</span>' },
  Aether: { accent: '#818cf8', grad: '#0f0a1a,#312e81,#0d1b2a', brandHtml: 'Ae<span>ther</span>' },
  'Zyvor Fabric': { accent: '#2dd4bf', grad: '#042f2e,#115e59,#0d1b2a', brandHtml: 'Zyvor <span>Fabric</span>' },
  Hermes: { accent: '#fb7185', grad: '#1c0a12,#9f1239,#0d1b2a', brandHtml: 'Her<span>mes</span>' },
  HyperSDK: { accent: '#60a5fa', grad: '#0a0a1a,#1e3a5f,#0d1b2a', brandHtml: 'Hyper<span>SDK</span>' },
  ZySign: { accent: '#a3e635', grad: '#14532d,#365314,#0d1b2a', brandHtml: 'Zy<span>Sign</span>' },
}
const theme = THEMES[PRODUCT] || { accent: '#60a5fa', grad: '#0a0a1a,#1e3a5f,#0d1b2a', brandHtml: PRODUCT }
const ACCENT = theme.accent
const GRAD = theme.grad

const CHROME_CANDIDATES = [
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/Applications/Chromium.app/Contents/MacOS/Chromium',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
]

function fail(msg) {
  console.error(`ERROR: ${msg}`)
  process.exit(1)
}

function findChrome() {
  for (const c of CHROME_CANDIDATES) if (existsSync(c)) return c
  return null
}

function mdToHtmlBody(md, tmpBase) {
  const tmpMd = `${tmpBase}.tmp.md`
  const tmpHtml = `${tmpBase}.tmp.html`
  writeFileSync(tmpMd, md)
  execFileSync(MARKED, ['--gfm', '-i', tmpMd, '-o', tmpHtml])
  const html = readFileSync(tmpHtml, 'utf8')
  rmSync(tmpMd)
  rmSync(tmpHtml)
  return html
}

function demoteHeadings(md) {
  let inFence = false
  return md
    .split('\n')
    .map((line) => {
      if (/^\s*```/.test(line)) inFence = !inFence
      if (!inFence && /^#{1,5}\s/.test(line)) return `#${line}`
      return line
    })
    .join('\n')
}

function stripFirstH1(md) {
  return md.replace(/^#\s+[^\n]+\n+/, '')
}

function wrapHtml(title, bodyHtml) {
  const today = new Date().toISOString().slice(0, 10)
  return `<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"/><title>${title} — ${PRODUCT}</title>
<style>
@page{size:A4;margin:15mm 16mm}*{margin:0;padding:0;box-sizing:border-box}
html{-webkit-print-color-adjust:exact;print-color-adjust:exact}
body{font-family:'Segoe UI',system-ui,sans-serif;color:#1f2937;line-height:1.58;font-size:10.5pt}
.cover{height:262mm;margin:-15mm -16mm 0;background:linear-gradient(135deg,${GRAD});color:#fff;display:flex;flex-direction:column;justify-content:center;align-items:center;text-align:center;page-break-after:always;position:relative}
.cover .kicker{letter-spacing:5px;text-transform:uppercase;opacity:.72;margin-bottom:1.4em}
.cover h1{font-size:3.2em;font-weight:800;letter-spacing:-2px}
.cover h1 span{color:${ACCENT}}
.cover .sub{font-size:1.3em;font-weight:300;opacity:.93;margin:1em 0;max-width:24em}
.cover .badge{display:inline-block;background:rgba(96,165,250,.2);border:1px solid ${ACCENT};padding:8px 22px;border-radius:20px;margin-top:2em}
.cover .foot{position:absolute;bottom:34px;font-size:.82em;opacity:.6}
h2{page-break-before:always;margin:0 0 .35em;font-size:1.55em;font-weight:750;color:#12203a;border-bottom:2px solid #e5edfb;padding-bottom:.28em}
h2:first-of-type{page-break-before:avoid}
h3{margin:1.1em 0 .4em;font-size:1.12em;font-weight:700}
p,li{margin:.4em 0}ul,ol{margin:.5em 0 .9em 1.4em}
a{color:#2563eb;text-decoration:none}
code{background:#eef2f7;padding:1px 6px;border-radius:4px;font-size:.86em;font-family:ui-monospace,Menlo,monospace}
pre{background:#0f172a;color:#e2e8f0;padding:12px 15px;border-radius:9px;overflow-x:auto;margin:.8em 0}
table{width:100%;border-collapse:collapse;margin:.8em 0 1.1em;font-size:9.5pt}
th,td{border:1px solid #e5e7eb;padding:6px 8px;text-align:left;vertical-align:top}th{background:#f1f5f9}
hr{border:none;border-top:1px solid #e5e7eb;margin:1.2em 0}
</style></head><body>
<section class="cover"><div class="kicker">ZyvorAI Labs · Customer Documentation</div>
<h1>${theme.brandHtml}</h1>
<div class="sub">${title}</div>
<div class="badge">${today}</div>
<div class="foot">zyvor.dev · Confidential — for licensed customers</div></section>
<main>${bodyHtml}</main></body></html>`
}

function printPdf(chrome, htmlPath, pdfPath) {
  execFileSync(chrome, ['--headless', '--disable-gpu', '--no-pdf-header-footer', `--print-to-pdf=${pdfPath}`, htmlPath], {
    stdio: 'inherit',
  })
}

function collectPageGuides() {
  const pages = join(CUSTOMER, 'pages')
  const files = []
  if (!existsSync(pages)) return files
  for (const dir of readdirSync(pages).sort()) {
    const full = join(pages, dir)
    if (!statSync(full).isDirectory()) continue
    for (const f of readdirSync(full).sort()) if (f.endsWith('.md')) files.push(join(full, f))
  }
  return files
}

if (!MARKED) fail('marked not found — npm install marked in .docs-tools/ or reuse ../ironwolf/.docs-tools')
const chrome = findChrome()
if (!chrome) fail('Chrome/Chromium required')

mkdirSync(PDF_DIR, { recursive: true })
for (const script of ['generate-guides.mjs', 'generate-guide-index.mjs', 'generate-page-index.mjs']) {
  execFileSync(process.execPath, [resolve(ROOT, 'scripts/customer-docs', script)], {
    stdio: 'inherit',
    env: { ...process.env, CUSTOMER_DOCS_PRODUCT: PRODUCT, CUSTOMER_DOCS_SLUG: SLUG },
  })
}

const books = [
  { id: `${PDF_PREFIX}-Customer-README`, title: 'Customer Documentation Overview', sources: [join(CUSTOMER, 'README.md')] },
  {
    id: `${PDF_PREFIX}-Getting-Started`,
    title: 'Getting Started',
    sources: [join(CUSTOMER, 'getting-started.md'), join(CUSTOMER, 'using-the-dashboard.md'), join(CUSTOMER, 'workflows.md')],
  },
  { id: `${PDF_PREFIX}-Admin-Basics`, title: 'Admin Basics', sources: [join(CUSTOMER, 'admin-basics.md')] },
  {
    id: `${PDF_PREFIX}-Page-by-Page`,
    title: 'Page-by-Page Product Manual',
    demote: true,
    sources: [join(CUSTOMER, 'pages/README.md'), ...collectPageGuides(), join(CUSTOMER, 'PAGE_INDEX.md')],
  },
]

const indexLines = [`# ${PRODUCT} customer PDFs`, '', `Generated: ${new Date().toISOString().slice(0, 10)}`, '', 'Rebuild: `node scripts/customer-docs/build-customer-pdfs.mjs`', '']
for (const book of books) {
  const parts = book.sources.filter((p) => existsSync(p)).map((p, i) => {
    const raw = readFileSync(p, 'utf8')
    if (book.demote) return demoteHeadings(raw)
    return i === 0 ? raw : stripFirstH1(raw)
  })
  const combined = parts.join('\n\n---\n\n')
  const bodyHtml = mdToHtmlBody(combined, join(PDF_DIR, book.id)).replace(/^\s*<h1[^>]*>.*?<\/h1>\s*/is, '')
  const htmlPath = join(PDF_DIR, `${book.id}.html`)
  const pdfPath = join(PDF_DIR, `${book.id}.pdf`)
  writeFileSync(htmlPath, wrapHtml(book.title, bodyHtml))
  console.log(`Printing ${book.id}.pdf …`)
  printPdf(chrome, `file://${htmlPath}`, pdfPath)
  indexLines.push(`- \`${book.id}.pdf\` — ${book.title}`)
}
writeFileSync(join(PDF_DIR, 'PDF_INDEX.md'), indexLines.join('\n') + '\n')
console.log(`Done → ${PDF_DIR} (slug=${SLUG})`)
