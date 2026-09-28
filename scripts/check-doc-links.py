#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
"""Check relative links and #anchors in the top-level Markdown docs.

Covers README.md, FEATURES.md, CHANGELOG.md and every Markdown file under docs/. For each
relative Markdown link or HTML src/href/srcset it checks that the target file
(or directory) exists, and that a #fragment resolves to a GitHub-slugified
heading or an explicit <a id="..."> / name="..." in the target Markdown file.

Usage: scripts/check-doc-links.py [file ...]   (default: the set above)
Exit status is non-zero when any link is broken.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

LINK_RE = re.compile(r"!?\[[^\]\n]*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
HTML_RE = re.compile(r"""(?:src|href|srcset)\s*=\s*["']([^"']+)["']""", re.I)
ID_RE = re.compile(r"""<a\s+[^>]*?(?:id|name)\s*=\s*["']([^"']+)["']""", re.I)
FENCE_RE = re.compile(r"^\s*(```|~~~)")


def strip_fences(text):
    out, fenced = [], False
    for line in text.splitlines():
        if FENCE_RE.match(line):
            fenced = not fenced
            out.append("")
            continue
        out.append("" if fenced else line)
    return out


def slugify(heading):
    s = re.sub(r"<[^>]+>", "", heading)
    s = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", s)
    s = s.replace("`", "").replace("*", "")
    s = s.strip().lower()
    s = re.sub(r"[^\w\- ]", "", s, flags=re.UNICODE)
    return s.replace(" ", "-")


_anchor_cache = {}


def anchors(path):
    if path in _anchor_cache:
        return _anchor_cache[path]
    found, seen = set(), {}
    for line in strip_fences(path.read_text(encoding="utf-8", errors="replace")):
        m = re.match(r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$", line)
        if m:
            base = slugify(m.group(1))
            n = seen.get(base, 0)
            seen[base] = n + 1
            found.add(base if n == 0 else f"{base}-{n}")
        for i in ID_RE.findall(line):
            found.add(i)
    _anchor_cache[path] = found
    return found


def targets(path):
    for lineno, line in enumerate(strip_fences(path.read_text(encoding="utf-8", errors="replace")), 1):
        for m in LINK_RE.finditer(line):
            yield lineno, m.group(1)
        for m in HTML_RE.finditer(line):
            for part in re.split(r"[\s,]+", m.group(1)):
                if part:
                    yield lineno, part


def check(path):
    bad = []
    for lineno, target in targets(path):
        if re.match(r"^(?:[a-z][a-z0-9+.-]*:|//)", target, re.I):
            continue
        ref, _, frag = target.partition("#")
        ref = ref.split("?")[0]
        dest = path if not ref else (path.parent / ref).resolve()
        if ref and not dest.exists():
            bad.append((lineno, target, "missing file"))
            continue
        if frag and dest.is_file() and dest.suffix.lower() == ".md":
            if frag not in anchors(dest):
                bad.append((lineno, target, "missing anchor"))
    return bad


def main(argv):
    if argv:
        files = [Path(a).resolve() for a in argv]
    else:
        files = [ROOT / "README.md", ROOT / "FEATURES.md", ROOT / "CHANGELOG.md"]
        files += sorted((ROOT / "docs").rglob("*.md"))
    total = 0
    for f in files:
        if not f.exists():
            continue
        for lineno, target, why in check(f):
            print(f"{f.relative_to(ROOT)}:{lineno}: {why}: {target}")
            total += 1
    print(f"{total} broken link(s) in {len(files)} file(s)")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
