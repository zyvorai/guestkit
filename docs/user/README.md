# GuestKit — User Documentation

Offline VM intelligence — inspect disks without booting, score boot readiness, and produce hypervisor-aware fix plans.

| You want to… | Open |
|--------------|------|
| Install and log in | [Getting Started](getting-started.md) |
| Learn the shell | [Using the Dashboard](using-the-dashboard.md) |
| Follow a page, step by step | [Page-by-page guides](pages/README.md) |
| Look up any screen | [Complete page index](PAGE_INDEX.md) |
| Deploy, auth, ports | [Admin basics](admin-basics.md) |
| Multi-page jobs | [Common workflows](workflows.md) |
| Capability map | [Feature Guide](../guestkit-user-feature-guide.md) |

## Printable PDFs

```bash
node scripts/user-docs/build-user-pdfs.mjs
```

Output lands in [`pdf/`](pdf/):

| PDF | Contents |
|-----|----------|
| `GuestKit-User-README.pdf` | This overview |
| `GuestKit-Getting-Started.pdf` | Access, basics, workflows |
| `GuestKit-Page-by-Page.pdf` | Complete page manual |
| `GuestKit-Admin-Basics.pdf` | Deploy, auth, ports |

## Product at a glance

Offline VM intelligence — inspect disks without booting, score boot readiness, and produce hypervisor-aware fix plans.

## Support surfaces (quick map)

| Need | Surface |
|------|--------|
| Inspect | `guestkit inspect` |
| Doctor | `guestkit doctor` |
| Fix plans | fix-plans / repair |
| TUI | `guestctl` |

---

*ZyvorAI Labs · [zyvor.dev](https://zyvor.dev) · GuestKit*
