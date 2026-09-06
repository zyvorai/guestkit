# GuestKit Zyvor Orange / White GA UX

This layer replaces the legacy visual treatment without changing GuestKit API contracts or JavaScript element IDs.

## Design

- Zyvor orange: `#F36B21`
- White full-bleed application canvas
- Near-black primary text
- Existing `img/zyvor-logo.png` for GuestKit branding
- Black activity console with semantic log colors
- No blue product accent

## Integration

The GA integration is direct HTML loading, not Nginx `sub_filter` rewriting.

`index.html` and `login.html` load `zyvor-ux.css` last in `<head>` and `zyvor-ux.js` with `defer` before `</body>`.

The existing `#targetSelect`, `#recentDisksToggle`, `#cinemaModeBtn`, authentication elements, VM upload dropzone, and all backend/API hooks remain intact.

## Terminal

The console supports:

- `Cmd/Ctrl + backtick` toggle
- drag / resize and persistent geometry
- `.log`, `.txt`, `.json`, `.jsonl`, `.ndjson`, `.out`
- search and severity filtering
- follow, copy, export, clear
- browser console/error mirroring
- a 5,000-entry cap
- 4 MiB per imported file, reading the tail for larger files
- batch rendering to avoid quadratic DOM updates

Log drops are intercepted only when the dropped data is recognized as a text/log/JSON file. VM disks continue through GuestKit's existing uploader.

## Verification

```bash
node --check deploy/ui/zyvor-ux.js
node deploy/ui/tests/zyvor-ux.smoke.mjs
```
