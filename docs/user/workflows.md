# Common workflows

End-to-end GuestKit jobs. Prefer CLI; optional TUI / web where noted.

## First-hour assurance

1. Install CLI (`cargo install guestkit` or release binary); confirm `qemu-img`
2. [Doctor](pages/assurance/doctor.md) — `guestkit doctor IMAGE --target kvm --explain`
3. [Migration Plan](pages/assurance/migrate-plan.md) — export a FixPlan YAML
4. Optional: [TUI](pages/interfaces/tui.md) — `guestctl tui IMAGE`

## Inspect without booting

1. [Inspect](pages/inspection/inspect.md)
2. [Filesystems](pages/inspection/filesystems.md) → mount in [Interactive Mode](pages/onboarding/interactive-mode.md) if needed
3. [Packages](pages/inspection/packages.md) / [Network](pages/inspection/network.md) / [Guest Files](pages/guest-files/files.md)

## Repair then re-score

1. Doctor blockers
2. [Repair](pages/fix-plans/repair.md) dry-run → apply (guest shut down)
3. Or [Fix Plans](pages/fix-plans/fix-plans.md) — preview → apply
4. Re-run Doctor / passport verify

## Fleet wave

1. [Fleet](pages/fleet/fleet.md) — `fleet analyze` / `wave-plan`
2. [Forensic Diff](pages/forensics/forensic-diff.md) or `fleet watch` for drift
3. [Policy Gate](pages/assurance/policy.md) before cutover

## Related

- [Getting Started](getting-started.md)
- [Page-by-page guides](pages/README.md)
- [Admin basics](admin-basics.md)
