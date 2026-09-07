#!/usr/bin/env python3
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# https://zyvor.dev · info@zyvor.dev

"""Branded user documentation: premium PDFs + offline welcome page."""
from __future__ import annotations

import argparse
import base64
import html
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

try:
    from fpdf import FPDF
    from fpdf.enums import XPos, YPos
except ImportError as exc:
    print(f"ERROR: fpdf2 required: {exc}", file=sys.stderr)
    sys.exit(1)

# Zyvor brand palette
ORANGE = (249, 115, 22)
ORANGE_LIGHT = (251, 146, 60)
SLATE_950 = (15, 23, 42)
SLATE_800 = (30, 41, 59)
SLATE_600 = (71, 85, 105)
SLATE_400 = (148, 163, 184)
WHITE = (255, 255, 255)
OFF_WHITE = (248, 250, 252)

DOC_FILES = (
    "START_HERE.txt",
    "HELP.txt",
    "README.txt",
    "QUICKSTART.txt",
    "ZYVOR_INSTALL.txt",
    "PREREQUISITES.txt",
    "CLUSTER_SETUP.txt",
    "HOST_SETUP.txt",
)

WELCOME_STEPS = (
    ("Extract", "tar xzf PRODUCT-*-linux-amd64.tar.gz"),
    ("Enter folder", "cd PRODUCT-*-linux-amd64"),
    ("Install", "./install-everything.sh"),
)


def _safe(text: str) -> str:
    return text.encode("latin-1", "replace").decode("latin-1")


def _is_rule(line: str) -> bool:
    s = line.strip()
    return len(s) >= 3 and len(set(s)) == 1 and s[0] in "=-_*"


def _heading_level(line: str, nxt: str | None) -> int:
    s = line.strip()
    if not s:
        return 0
    if nxt and _is_rule(nxt):
        return 1 if set(nxt.strip()) == {"="} else 2
    if s.isupper() and len(s) < 72 and not s.startswith("  "):
        return 2
    if s.endswith(":") and len(s) < 64 and not s.startswith(" "):
        return 3
    return 0


class BrandedPDF(FPDF):
    product: str = "Zyvor"
    doc_title: str = ""
    logo_path: Path | None = None

    def header(self) -> None:
        self.set_fill_color(*SLATE_950)
        self.rect(0, 0, self.w, 28, style="F")
        self.set_fill_color(*ORANGE)
        self.rect(0, 28, self.w, 1.2, style="F")

        if self.logo_path and self.logo_path.is_file():
            self.image(str(self.logo_path), x=self.l_margin, y=5, h=18)

        self.set_xy(self.l_margin + 44, 8)
        self.set_font("Helvetica", "B", 13)
        self.set_text_color(*ORANGE)
        self.cell(0, 6, _safe(self.product), new_x=XPos.LMARGIN, new_y=YPos.NEXT)

        if self.doc_title:
            self.set_x(self.l_margin + 44)
            self.set_font("Helvetica", "", 9)
            self.set_text_color(*SLATE_400)
            self.cell(0, 5, _safe(self.doc_title), new_x=XPos.LMARGIN, new_y=YPos.NEXT)

        self.set_y(34)

    def footer(self) -> None:
        self.set_y(-14)
        self.set_fill_color(*SLATE_950)
        self.rect(0, self.h - 14, self.w, 14, style="F")
        self.set_y(-11)
        self.set_font("Helvetica", "I", 8)
        self.set_text_color(*SLATE_400)
        self.cell(
            0,
            6,
            _safe(f"Zyvor · zyvor.dev · HyperSDK · © 2026 · page {self.page_no()}"),
            align="C",
        )


def _render_body_line(pdf: BrandedPDF, line: str, level: int) -> None:
    pdf.set_x(pdf.l_margin)
    text = line.rstrip()
    if not text.strip():
        pdf.ln(2.5)
        return

    stripped = text.strip()
    indent = len(text) - len(text.lstrip(" "))
    bullet = stripped.startswith(("-", "•", "*", "[ ]", "[x]", "[X]")) or re.match(r"^\d+\.", stripped)

    if level == 1:
        pdf.ln(3)
        pdf.set_font("Helvetica", "B", 15)
        pdf.set_text_color(*SLATE_950)
        pdf.multi_cell(pdf.epw, 7, _safe(stripped))
        pdf.ln(1)
        return

    if level == 2:
        pdf.ln(2)
        pdf.set_font("Helvetica", "B", 12)
        pdf.set_text_color(*ORANGE)
        pdf.multi_cell(pdf.epw, 6, _safe(stripped))
        pdf.ln(0.5)
        return

    if level == 3:
        pdf.set_font("Helvetica", "B", 10)
        pdf.set_text_color(*SLATE_800)
        pdf.multi_cell(pdf.epw, 5.5, _safe(stripped))
        return

    x_off = min(indent * 0.6, 18)
    pdf.set_x(pdf.l_margin + x_off)
    w = pdf.epw - x_off

    if bullet:
        pdf.set_font("Helvetica", "", 9.5)
        pdf.set_text_color(*SLATE_800)
        pdf.multi_cell(w, 4.8, _safe(stripped))
        return

    if stripped.startswith("./") or stripped.startswith("export ") or "tar xzf" in stripped:
        pdf.set_font("Courier", "", 8.5)
        pdf.set_fill_color(*OFF_WHITE)
        pdf.set_text_color(30, 64, 110)
        pdf.multi_cell(w, 5, _safe("  " + stripped), fill=True)
        pdf.ln(0.5)
        return

    pdf.set_font("Helvetica", "", 9.5)
    pdf.set_text_color(40, 48, 60)
    pdf.multi_cell(w, 4.8, _safe(stripped))


def txt_to_pdf(txt_path: Path, pdf_path: Path, logo_path: Path, product: str) -> None:
    title = txt_path.stem.replace("_", " ")
    body_lines = txt_path.read_text(encoding="utf-8", errors="replace").splitlines()

    pdf = BrandedPDF(format="Letter", unit="mm")
    pdf.product = product
    pdf.doc_title = title
    pdf.logo_path = logo_path
    pdf.set_auto_page_break(auto=True, margin=20)
    pdf.set_margins(18, 36, 18)
    pdf.add_page()

    i = 0
    while i < len(body_lines):
        line = body_lines[i]
        nxt = body_lines[i + 1] if i + 1 < len(body_lines) else None

        if nxt and _is_rule(nxt):
            level = _heading_level(line, nxt)
            _render_body_line(pdf, line, level)
            i += 2
            continue

        level = _heading_level(line, None)
        if level:
            _render_body_line(pdf, line, level)
        else:
            _render_body_line(pdf, line, 0)
        i += 1

    pdf_path.parent.mkdir(parents=True, exist_ok=True)
    pdf.output(str(pdf_path))


def welcome_pdf(pdf_path: Path, logo_path: Path, product: str, version: str) -> None:
    pdf = BrandedPDF(format="Letter", unit="mm")
    pdf.product = product
    pdf.doc_title = "Welcome"
    pdf.logo_path = logo_path
    pdf.set_auto_page_break(auto=False)
    pdf.add_page()

    pdf.set_fill_color(*SLATE_950)
    pdf.rect(0, 0, pdf.w, pdf.h, style="F")
    pdf.set_fill_color(*ORANGE)
    pdf.rect(0, 0, pdf.w, 2.5, style="F")

    if logo_path.is_file():
        pdf.image(str(logo_path), x=(pdf.w - 70) / 2, y=42, w=70)

    pdf.set_y(118)
    pdf.set_font("Helvetica", "B", 28)
    pdf.set_text_color(*WHITE)
    pdf.cell(0, 12, _safe(product), align="C", new_x=XPos.LMARGIN, new_y=YPos.NEXT)

    pdf.set_font("Helvetica", "", 14)
    pdf.set_text_color(*ORANGE_LIGHT)
    pdf.cell(0, 8, "Client installation bundle", align="C", new_x=XPos.LMARGIN, new_y=YPos.NEXT)

    pdf.ln(6)
    pdf.set_font("Helvetica", "", 10)
    pdf.set_text_color(*SLATE_400)
    pdf.cell(0, 6, _safe(f"Version {version} · linux-amd64 · ready to install"), align="C", new_x=XPos.LMARGIN, new_y=YPos.NEXT)

    pdf.ln(14)
    box_w = 150
    box_x = (pdf.w - box_w) / 2
    pdf.set_fill_color(*SLATE_800)
    pdf.set_draw_color(*ORANGE)
    pdf.set_line_width(0.4)
    pdf.rect(box_x, pdf.get_y(), box_w, 58, style="DF")

    y0 = pdf.get_y() + 8
    for n, (label, cmd) in enumerate(WELCOME_STEPS, 1):
        pdf.set_xy(box_x + 10, y0 + (n - 1) * 17)
        pdf.set_font("Helvetica", "B", 10)
        pdf.set_text_color(*ORANGE)
        pdf.cell(12, 6, str(n) + ".", new_x=XPos.RIGHT, new_y=YPos.TOP)
        pdf.set_font("Helvetica", "B", 10)
        pdf.set_text_color(*WHITE)
        pdf.cell(28, 6, _safe(label), new_x=XPos.RIGHT, new_y=YPos.TOP)
        pdf.set_font("Courier", "", 8)
        pdf.set_text_color(*SLATE_400)
        cmd_show = cmd.replace("PRODUCT", product.lower())
        pdf.cell(0, 6, _safe(cmd_show))

    pdf.set_y(pdf.h - 36)
    pdf.set_font("Helvetica", "", 9)
    pdf.set_text_color(*SLATE_400)
    pdf.cell(0, 5, "Open docs/welcome.html in your browser  ·  or docs/pdf/WELCOME.pdf", align="C", new_x=XPos.LMARGIN, new_y=YPos.NEXT)
    pdf.cell(0, 5, "zyvor.dev · HyperSDK · © 2026", align="C")

    pdf_path.parent.mkdir(parents=True, exist_ok=True)
    pdf.output(str(pdf_path))


def legal_doc_links_html(stage: Path) -> str:
  """HTML links for LICENSE / Zyvor legal files present in the bundle root."""
  links: list[str] = []
  items = [
    ("LICENSE", "LICENSE — software license"),
    ("LICENSE.txt", "LICENSE.txt — software license"),
    ("LEGAL-INDEX.txt", "Legal file index"),
  ]
  icons = {
    "LICENSE": "⚖️",
    "LICENSE.txt": "⚖️",
    "LEGAL-INDEX.txt": "📋",
  }
  for name, label in items:
    if (stage / name).is_file():
      links.append(
        f'        <a class="doc-link" href="../{html.escape(name)}">'
        f'<span class="icon">{icons.get(name, "⚖️")}</span>{html.escape(label)}</a>'
      )
  legal_readme = stage / "docs" / "legal" / "README.md"
  if legal_readme.is_file():
    links.append(
      '        <a class="doc-link" href="../docs/legal/README.md">'
      '<span class="icon">🏢</span>Company legal reference</a>'
    )
  if not links:
    return (
      '        <p style="font-size:.85rem;color:var(--slate-500)">'
      "Read LICENSE in the bundle root before install.</p>"
    )
  return "\n".join(links)


def welcome_html(
    html_path: Path,
    logo_path: Path,
    product: str,
    version: str,
    pdf_names: list[str],
    stage: Path,
) -> None:
    logo_b64 = ""
    if logo_path.is_file():
        logo_b64 = base64.b64encode(logo_path.read_bytes()).decode("ascii")

    esc = html.escape
    pdf_links = "\n".join(
        f'        <a class="doc-link" href="pdf/{esc(n)}"><span class="icon">📄</span>{esc(n.replace(".pdf", "").replace("_", " "))}</a>'
        for n in pdf_names
    )
    legal_links = legal_doc_links_html(stage)

    content = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>{esc(product)} — Zyvor client bundle</title>
  <style>
    :root {{
      --orange: #f97316;
      --orange-light: #fb923c;
      --slate-950: #0f172a;
      --slate-900: #1e293b;
      --slate-700: #334155;
      --slate-500: #64748b;
      --slate-300: #cbd5e1;
    }}
    * {{ box-sizing: border-box; margin: 0; padding: 0; }}
    body {{
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      background: linear-gradient(160deg, var(--slate-950) 0%, #1a1033 45%, var(--slate-900) 100%);
      color: var(--slate-300);
      min-height: 100vh;
      line-height: 1.6;
    }}
    .accent-bar {{ height: 4px; background: linear-gradient(90deg, #f0583a, var(--orange), #c084fc); }}
    .wrap {{ max-width: 920px; margin: 0 auto; padding: 2.5rem 1.5rem 4rem; }}
    header {{ text-align: center; margin-bottom: 2.5rem; }}
    .logo {{ height: 72px; margin-bottom: 1.25rem; filter: drop-shadow(0 8px 24px rgba(249,115,22,.25)); }}
    h1 {{
      font-size: 2.25rem; font-weight: 700; color: #fff;
      letter-spacing: -0.02em; margin-bottom: .35rem;
    }}
    .tagline {{ color: var(--orange-light); font-size: 1.1rem; font-weight: 500; }}
    .meta {{ color: var(--slate-500); font-size: .9rem; margin-top: .5rem; }}
    .hero-card {{
      background: rgba(30,41,59,.85); border: 1px solid rgba(249,115,22,.35);
      border-radius: 16px; padding: 1.75rem 2rem; margin-bottom: 2rem;
      box-shadow: 0 20px 50px rgba(0,0,0,.35);
    }}
    .hero-card h2 {{ color: #fff; font-size: 1.15rem; margin-bottom: 1rem; }}
    .steps {{ list-style: none; counter-reset: step; }}
    .steps li {{
      counter-increment: step; display: flex; align-items: flex-start; gap: 1rem;
      padding: .85rem 0; border-bottom: 1px solid rgba(255,255,255,.06);
    }}
    .steps li:last-child {{ border-bottom: none; }}
    .steps li::before {{
      content: counter(step); flex-shrink: 0;
      width: 2rem; height: 2rem; line-height: 2rem; text-align: center;
      background: var(--orange); color: #fff; font-weight: 700; border-radius: 50%;
      font-size: .85rem;
    }}
    .step-body strong {{ display: block; color: #fff; margin-bottom: .25rem; }}
  code {{
      display: block; background: var(--slate-950); color: #7dd3fc;
      padding: .65rem 1rem; border-radius: 8px; font-size: .85rem;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      border-left: 3px solid var(--orange);
    }}
    .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 1.25rem; }}
    .panel {{
      background: rgba(15,23,42,.75); border: 1px solid rgba(255,255,255,.08);
      border-radius: 14px; padding: 1.35rem 1.5rem;
    }}
    .panel h3 {{ color: var(--orange); font-size: .95rem; text-transform: uppercase; letter-spacing: .06em; margin-bottom: .85rem; }}
    .doc-link {{
      display: flex; align-items: center; gap: .6rem;
      color: var(--slate-300); text-decoration: none; padding: .45rem 0;
      border-bottom: 1px solid rgba(255,255,255,.05); font-size: .92rem;
      transition: color .15s;
    }}
    .doc-link:hover {{ color: var(--orange-light); }}
    .doc-link .icon {{ opacity: .7; }}
    .install-btn {{
      display: inline-block; margin-top: 1rem; padding: .75rem 1.5rem;
      background: linear-gradient(135deg, #f0583a, var(--orange));
      color: #fff !important; text-decoration: none; font-weight: 600;
      border-radius: 10px; box-shadow: 0 4px 20px rgba(249,115,22,.35);
    }}
    footer {{ text-align: center; margin-top: 3rem; color: var(--slate-500); font-size: .85rem; }}
    footer a {{ color: var(--orange); text-decoration: none; }}
  </style>
</head>
<body>
  <motion-div class="accent-bar"></motion-div>
  <motion-div class="wrap">
    <header>
      {"<img class='logo' src='data:image/png;base64," + logo_b64 + "' alt='Zyvor'/>" if logo_b64 else "<div class='tagline' style='font-size:2rem;margin-bottom:1rem'>Zyvor</motion-div>"}
      <h1>{esc(product)}</h1>
      <p class="tagline">Client installation bundle</p>
      <p class="meta">Version {esc(version)} · linux-amd64 · no compile required on this machine</p>
    </header>

    <section class="hero-card">
      <h2>Get started in three steps</h2>
      <ol class="steps">
        <li><div class="step-body"><strong>Extract the archive</strong><code>tar xzf {esc(product.lower())}-*-linux-amd64.tar.gz</code></div></li>
        <li><motion-div class="step-body"><strong>Enter this folder</strong><code>cd {esc(product.lower())}-*-linux-amd64</code></motion-div></li>
        <li><div class="step-body"><strong>Run the installer</strong><code>./install-everything.sh</code></div></li>
      </ol>
      <p style="margin-top:1rem;font-size:.9rem;color:var(--slate-500)">
        Always run install scripts from inside the extracted folder — not from $HOME.
      </p>
    </section>

    <div class="grid">
      <div class="panel">
        <h3>Printable guides (PDF)</h3>
{pdf_links}
        <a class="doc-link" href="pdf/WELCOME.pdf"><span class="icon">✨</span>Welcome overview</a>
      </div>
      <div class="panel">
        <h3>Text guides (bundle root)</h3>
        <a class="doc-link" href="../START_HERE.txt"><span class="icon">🚀</span>START_HERE.txt</a>
        <a class="doc-link" href="../HELP.txt"><span class="icon">📖</span>HELP.txt — all scripts</a>
        <a class="doc-link" href="../QUICKSTART.txt"><span class="icon">⚡</span>QUICKSTART.txt</a>
        <a class="doc-link" href="../README.txt"><span class="icon">📦</span>README.txt</a>
        <p style="margin-top:1rem;font-size:.85rem;color:var(--slate-500)">
          After install: run <code style="display:inline;padding:.2rem .45rem">./test-package.sh</code>
        </p>
      </motion-div>
      <div class="panel">
        <h3>License &amp; legal</h3>
{legal_links}
        <p style="margin-top:1rem;font-size:.85rem;color:var(--slate-500)">
          Read LICENSE before install; <code style="display:inline;padding:.2rem .45rem">./install.sh</code> prompts acceptance.
        </p>
      </div>
    </div>

    <footer>
      <p><a href="https://zyvor.dev" target="_blank" rel="noopener">zyvor.dev</a> · HyperSDK · © 2026</p>
      <p style="margin-top:.35rem">Packaged for enterprise deployment — support your vendor for updates.</p>
    </footer>
  </div>
</body>
</html>
"""
    # fix accidental motion-div typos from template - use div
    content = content.replace("motion-div", "motion-div").replace("<motion-div", "<div").replace("</motion-div>", "</div>")
    html_path.write_text(content, encoding="utf-8")


def write_open_first(stage: Path, product: str) -> None:
    lines = [
        "",
        "  ╔══════════════════════════════════════════════════════════════╗",
        "  ║                                                              ║",
        f"  ║   ✦  Welcome to {product} — Zyvor client bundle",
        "  ║                                                              ║",
        "  ╚══════════════════════════════════════════════════════════════╝",
        "",
        "  OPEN FIRST (best experience)",
        "    • Browser:  docs/welcome.html",
        "    • PDF:      docs/pdf/WELCOME.pdf",
        "",
        "  QUICK INSTALL",
        "    cd into this folder, then:",
        "    ./install-everything.sh",
        "",
        "  LICENSE (read before install)",
        "    cat LICENSE",
        "    cat LEGAL-INDEX.txt",
        "",
        "  MORE HELP",
        "    cat START_HERE.txt",
        "    ls docs/pdf/",
        "",
        "  zyvor.dev · HyperSDK · © 2026",
        "",
    ]
    # Pad product line in box - keep simple
    (stage / "OPEN_FIRST.txt").write_text("\n".join(lines), encoding="utf-8")


def write_index(out_dir: Path, names: list[str], product: str) -> None:
    lines = [
        f"{product} — documentation index",
        "═" * 44,
        "",
        "  ✦  Start here",
        "     docs/welcome.html      interactive guide (offline, in your browser)",
        "     docs/pdf/WELCOME.pdf   one-page overview with install steps",
        "",
        "  📄  Printable PDFs (docs/pdf/)",
    ]
    for name in sorted(names):
        label = name.replace(".pdf", "").replace("_", " ")
        lines.append(f"     {name:<22} {label}")
    lines.extend(
        [
            "",
            "  📁  Text originals (bundle root)",
            "     START_HERE.txt · HELP.txt · QUICKSTART.txt · README.txt",
            "",
            "  ⚖️  License & legal (bundle root)",
            "     LICENSE · LEGAL-INDEX.txt",
            "",
            "  🎨  Branding: docs/zyvor-logo.png",
            "",
            "  zyvor.dev · HyperSDK · © 2026",
            "",
        ]
    )
    (out_dir / "PDF_INDEX.txt").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate branded user PDFs + welcome page")
    parser.add_argument("stage", type=Path)
    parser.add_argument("product")
    parser.add_argument("logo", type=Path)
    parser.add_argument("--version", default="")
    args = parser.parse_args()

    stage: Path = args.stage
    product: str = args.product
    version = args.version.strip() or "latest"
    docs_pdf = stage / "docs" / "pdf"
    docs_root = stage / "docs"
    docs_root.mkdir(parents=True, exist_ok=True)

    if args.logo.is_file():
        (docs_root / "zyvor-logo.png").write_bytes(args.logo.read_bytes())

    welcome_pdf(docs_pdf / "WELCOME.pdf", args.logo, product, version)
    print("  pdf: docs/pdf/WELCOME.pdf")

    made: list[str] = ["WELCOME.pdf"]
    for name in DOC_FILES:
        txt = stage / name
        if not txt.is_file():
            continue
        pdf_name = f"{txt.stem}.pdf"
        txt_to_pdf(txt, docs_pdf / pdf_name, args.logo, product)
        made.append(pdf_name)
        print(f"  pdf: docs/pdf/{pdf_name}")

    if len(made) < 2:
        print("ERROR: no .txt docs found to convert", file=sys.stderr)
        return 1

    welcome_html(docs_root / "welcome.html", args.logo, product, version, made, stage)
    print("  html: docs/welcome.html")

    write_open_first(stage, product)
    print("  txt: OPEN_FIRST.txt")

    write_index(docs_root, made, product)
    print(f"  index: docs/PDF_INDEX.txt ({len(made)} PDFs)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
