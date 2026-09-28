# Social assets

| File | What it is |
|---|---|
| `guestkit-share-card.html` / `.png` | 1200×630 light card: README hero, `docs/README.md` / `docs/INDEX.md` hero, and the website `og:image` (`website/docusaurus.config.ts` serves this directory as static files) |
| `guestkit-share-card-dark.html` / `.png` | The same card in the dark palette, used by the README for dark mode (`<picture>` + `prefers-color-scheme`) |
| `render.py` | Renders both cards to PNG |

## Rebuild

```bash
pip install playwright        # once; uses your installed Google Chrome, no browser download
python3 docs/social/render.py
```

The cards use system fonts only (Helvetica Neue, Menlo), so no network access is needed.

## Palette

Apple-style blue and white. Light: `#ffffff` / `#f5f5f7` background with a faint blue wash, ink `#1d1d1f`,
secondary `#6e6e73`, hairline `#d2d2d7`, blue `#0071e3` → `#2997ff`. Dark: `#000` / `#0b0b0f`, text `#f5f5f7`,
panels `#1d1d1f`, blue `#0a84ff` / `#64b5ff`. Orange `#ff6a2a` appears exactly once per card, as the dot on the
QCOW2 node. The dark card is the light card with only the `:root` colour block changed.

## GitHub social preview

GitHub's repository "Social preview" image cannot be set through the API. After changing the card, upload
`guestkit-share-card.png` by hand under **Settings → General → Social preview**.
