# Export Formats

## Purpose

Export Formats — Export surface.

## When to use it

- Operate **Export Formats** when your job matches this surface
- Prefer dry-run / doctor before mutating repairs on disks
- Shut down the guest before write operations

## How to get there

- Doc id: `export-formats`
- Nav: **Export → Export Formats**
- Primary interface: `--output` vs `--export` on inspect; plan export; passport JSON

## Operate from CLI / TUI (UX)

1. `--output` vs `--export` on inspect; plan export; passport JSON.
2. Data: `inspect -o json|yaml|csv`.
3. Docs: `--export html|markdown|pdf --export-output PATH`.
4. Plans: `plan export plan.yaml -f bash -o fix.sh`.
5. Assurance: `migrate-plan --export`, `passport emit -o`.
6. TUI view export / web Passport download.
7. **Empty / fail:** Missing `--export-output`; PDF may need Chrome/wkhtmltopdf.
8. **Success:** File opens (HTML) or validates (JSON/YAML).

Host needs Linux + `qemu-img` / losetup / qemu-nbd; mount/repair often need root. GuestKit does not invent disk contents.

## Related pages

- [Inspect](../inspection/inspect.md)
- [Profiles](../profiles/profiles.md)
- [Fix Plans](../fix-plans/fix-plans.md)
- [Migration Assurance](../assurance/migration-assurance.md)
- [Getting Started](../../getting-started.md)
- [Page index](../../PAGE_INDEX.md)
