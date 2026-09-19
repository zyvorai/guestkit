# GuestKit docs site

Built with [Docusaurus](https://docusaurus.io/). Serves the live docs at https://zyvorai.github.io/guestkit/.

Points directly at the repo's existing `docs/` folder (`docusaurus.config.ts`'s `docs.path: '../docs'`) rather than a hand-curated copy — every doc becomes a page automatically, sidebar auto-generated from the folder structure.

## Local development

```bash
npm install
npm start
```

## Build

```bash
npm run build
npm run serve   # preview the production build locally
```

## Images

Screenshots and the social card are **not** duplicated into `website/static/` — `docusaurus.config.ts`'s `staticDirectories` serves `../docs/social` and `../docs/img` in place, so the root README, the docs and this site all reference the same physical files. Add new screenshots to `docs/img/`, not here.

- `docs/social/guestkit-share-card.html` is the source of the 1200×630 share card (also the site's `og:image`); regenerate the PNG with `python3 docs/social/render.py`.
- `docs/img/ui-*.png` come from the bundled web console's offline demo; regenerate with `python3 scripts/capture-ui-demo.py`. Both scripts need `pip install playwright` and Google Chrome.

## Deployment

Deployment is automatic: `.github/workflows/pages.yml` builds and publishes this site to GitHub Pages on every push to `main` that touches `website/`, `docs/`, or the workflow file itself.
