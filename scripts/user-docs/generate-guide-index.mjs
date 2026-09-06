// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

#!/usr/bin/env node
import { readFileSync, readdirSync, writeFileSync, existsSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const PAGES = resolve(ROOT, 'docs/user/pages')
const OUT = join(PAGES, 'README.md')

function titleOf(file) {
  const first = readFileSync(file, 'utf8').split('\n').find((l) => l.startsWith('# '))
  return first ? first.slice(2).trim() : null
}

function summaryOf(file) {
  const lines = readFileSync(file, 'utf8').split('\n')
  const i = lines.findIndex((l) => l.trim() === '## Purpose')
  if (i === -1) return ''
  for (let j = i + 1; j < lines.length; j++) {
    const t = lines[j].trim()
    if (t && !t.startsWith('#')) return t.replace(/\s+/g, ' ').replace(/\|/g, '\\|').replace(/\*\*/g, '')
  }
  return ''
}

const lines = [
  '# Page-by-page guides',
  '',
  'Each guide follows: Purpose → When to use it → How to get there → What you can do → Related pages.',
  '',
  'Every route is also listed in the [complete page index](../PAGE_INDEX.md).',
  '',
]

let total = 0
if (existsSync(PAGES)) {
  for (const dir of readdirSync(PAGES).sort()) {
    const fullDir = join(PAGES, dir)
    try {
      const guides = readdirSync(fullDir).filter((f) => f.endsWith('.md') && f !== 'README.md').sort()
      if (!guides.length) continue
      const heading = dir.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
      lines.push(`## ${heading}`, '', '| Page | What it covers |', '|------|----------------|')
      for (const f of guides) {
        const full = join(fullDir, f)
        lines.push(`| [${titleOf(full) ?? f}](${dir}/${f}) | ${summaryOf(full)} |`)
        total++
      }
      lines.push('')
    } catch {
      /* skip */
    }
  }
}

lines.push('---', '', `${total} guides. Regenerate: \`node scripts/user-docs/generate-guide-index.mjs\`.`, '')
writeFileSync(OUT, lines.join('\n'))
console.log(`Wrote ${OUT} (${total} guides)`)
