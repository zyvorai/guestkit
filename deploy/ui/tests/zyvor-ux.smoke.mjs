import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const ui = path.resolve(here, '..');
const css = fs.readFileSync(path.join(ui, 'zyvor-ux.css'), 'utf8');
const js = fs.readFileSync(path.join(ui, 'zyvor-ux.js'), 'utf8');
const docker = fs.readFileSync(path.join(ui, 'Dockerfile'), 'utf8');
const nginx = fs.readFileSync(path.join(ui, 'nginx.conf'), 'utf8');

assert.match(css, /--zyvor-orange:\s*#f36b21/i, 'canonical Zyvor orange missing');
assert.match(css, /--zyvor-bg:\s*#ffffff/i, 'white canvas missing');
assert.doesNotMatch(css, /#0071e3|#2997ff|#0a84ff/i, 'blue product accent found');
assert.doesNotMatch(css, /#targetSelect[\s\S]{0,120}display:\s*none/i, 'migration target must remain accessible');
assert.doesNotMatch(css, /#recentDisksToggle[\s\S]{0,120}display:\s*none/i, 'Recent must remain accessible');
assert.match(css, /\.zyvor-terminal\s*\{[\s\S]{0,900}background:\s*rgba\(8,\s*8,\s*10,/i, 'black terminal shell missing');

assert.match(js, /enhanceHeaderControls\(\)/, 'header functional controls are not enhanced');
assert.match(js, /Target · KubeVirt/, 'migration target label missing');
assert.match(js, /transferHasFiles/, 'cross-browser DataTransfer compatibility missing');
assert.match(js, /types\.contains/, 'DOMStringList fallback missing');
assert.match(js, /const batch = \[\]/, 'batched log import missing');
assert.match(js, /if \(batch\.length\) addEntries\(batch\)/, 'single batch render missing');
assert.match(js, /stopImmediatePropagation/, 'log drops must stop legacy VM-drop handlers');
assert.match(js, /log\|txt\|json\|jsonl\|ndjson\|out/, 'log extension allowlist missing');
assert.match(js, /img\/zyvor-logo\.png/, 'Zyvor logo enforcement missing');

assert.match(docker, /zyvor-ux\.css/, 'Dockerfile does not package CSS');
assert.match(docker, /zyvor-ux\.js/, 'Dockerfile does not package JS');
assert.doesNotMatch(nginx, /sub_filter/, 'Nginx runtime HTML injection must not be used');

console.log('GuestKit Zyvor orange/white GA smoke tests passed.');
