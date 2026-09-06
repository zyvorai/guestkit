// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

#!/usr/bin/env node
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const USER_DOCS = resolve(ROOT, 'docs/user')
const LINK = /\[[^\]]*\]\(([^)]+)\)/g

function walk(dir) {
  const out = []
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry)
    if (statSync(full).isDirectory()) out.push(...walk(full))
    else if (entry.endsWith('.md')) out.push(full)
  }
  return out
}

let broken = 0
let checked = 0
for (const file of walk(USER_DOCS)) {
  for (const m of readFileSync(file, 'utf8').matchAll(LINK)) {
    const target = m[1].trim()
    if (/^(https?:|mailto:|#)/.test(target)) continue
    const [pathPart] = target.split('#')
    if (!pathPart) continue
    checked++
    if (!existsSync(resolve(dirname(file), pathPart))) {
      broken++
      console.error(`BROKEN ${file.replace(`${ROOT}/`, '')} -> ${target}`)
    }
  }
}
console.log(`${checked} relative links checked, ${broken} broken`)
process.exit(broken ? 1 : 0)
