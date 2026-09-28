# Why teams switch

> Part of the [GuestKit README](../README.md). Back to the [documentation map](documentation-map.md).

# Why teams switch

| Before GuestKit | With GuestKit |
|-----------------|---------------|
| “Will it boot?” answered at power-on | Offline **doctor** score + root-cause chain |
| guestkit scripts and tribal knowledge | Structured plans, JSON/YAML, CI gates |
| Surprises on cutover weekend | Hypervisor-aware **migrate-plan** + day-0 packs |
| No audit trail MTV / virt-v2v can skip | Signed **Cutover Passport** |
| Fleet drift invisible until outage | `fleet analyze` / `watch`, forensic diff, policy-as-code |
| Migration order guessed by hand | `fleet wave-plan` — dependency-aware waves |
| Deep inspect needs a running guest | Carbon **TUI** + in-guest agent over QGA |
| Assured first boot still means hand-built QEMU argv | **`guestkit-qemu`** plans/runs from the same evidence gate |
| Live guest ops still mean `virsh qemu-agent-command` | **`guestkit qga`** / `agent-call` speak the QGA socket directly |
