// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

#!/usr/bin/env node
import { mkdirSync, readFileSync, writeFileSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const PAGES = resolve(ROOT, 'docs/customer/pages')
const { routes } = JSON.parse(readFileSync(resolve(ROOT, 'scripts/customer-docs/routes.json'), 'utf8'))
const purposes = JSON.parse(readFileSync(resolve(ROOT, 'scripts/customer-docs/page-purposes.json'), 'utf8'))
const PRODUCT = process.env.CUSTOMER_DOCS_PRODUCT || 'Product'

function catDir(category) {
  return category.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '') || 'other'
}
function slug(path) {
  return path.replace(/^\//, '').replace(/\//g, '-').replace(/:/g, '') || 'home'
}

function guideTemplate({ title, path, category, purpose }) {
  return `# ${title}

## Purpose

${purpose}

## When to use it

- Open this surface when the job matches the purpose above
- Start from the product home / dashboard if you are unsure where to begin
- Confirm auth and that required backends/operators are reachable if data looks empty

## How to get there

- Route / id: \`${path}\`
- Nav: **${category} → ${title}** (sidebar, command palette, or desktop nav)

## What you can do

1. Open \`${path}\` and wait for live data from ${PRODUCT}.
2. Use filters and search when the page provides them.
3. Drill into a row or card for detail, then jump to related surfaces.
4. For mutating actions: review impact, role gates, and confirmation dialogs first.

If the page stays empty, check service health, auth configuration, and that dependencies for this domain are installed.

## Related pages

- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
`
}

let written = 0, skipped = 0
for (const r of routes) {
  const file = join(PAGES, catDir(r.category), `${slug(r.path)}.md`)
  mkdirSync(dirname(file), { recursive: true })
  if (existsSync(file)) { skipped++; continue }
  writeFileSync(file, guideTemplate({ title: r.label, path: r.path, category: r.category, purpose: purposes[r.path] || `${r.label} page.` }))
  written++
}
console.log(`Wrote ${written} guides (skipped existing ${skipped})`)
