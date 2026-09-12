# Open source vs Enterprise (Zyvor)

**GuestKit (this repo)** is the full Apache-2.0 **offline disk engine** — free for personal, lab, and commercial production use (no license key).  
**GuestKit Enterprise** is Zyvor’s commercial **migration control plane** — every Command Center screen below — calling the **same** `guestkit doctor` evidence. Not a forked binary. Not a feature hostage. Enterprise support, SLAs, and the control plane are licensed separately.

Product: [zyvor.dev/guestkit](https://zyvor.dev/guestkit?utm_source=github&utm_medium=guestkit) · [Book a demo](https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo) · [sales@zyvor.dev](mailto:sales@zyvor.dev)

**30-day Enterprise trial (binary):** [v1.0.0-enterprise-trial](https://github.com/zyvorai/guestkit/releases/tag/v1.0.0-enterprise-trial) · [install guide](enterprise-trial-install.md)

---

## Full capability matrix

### Positioning

| Capability | Open source (this repo) | GuestKit Enterprise |
| --- | --- | --- |
| What you get | Offline disk engine | Migration **operating system** on that engine |
| Who it is for | Engineers, CI, labs, small fleets | Platform / SRE / migration leads · 50–10,000+ VMs |
| Success metric | Disk score & Passport JSON | Estate readiness % · wave completion · audit |
| Support | GitHub Issues | **SLA** · workshops · hypervisor exit programs |
| Platform pipeline | Pair with [hyper2kvm](https://github.com/hypersdk/hyper2kvm) | HyperSDK → hyper2kvm → GuestKit → **Zeus OS** → PacketWolf |

### Offline engine (shared)

| Capability | Open source | Enterprise |
| --- | --- | --- |
| `doctor` / `migrate-plan` / repair / harden | ✅ CLI · TUI · Python · zyvor-ui | ✅ Same engine via workers / adapter |
| Disk formats · boot score 0–100 | ✅ | ✅ Surfaced in Command Center & Image Vault |
| Passport emit (CLI / `zyvorai/guestkit@v1`) | ✅ | ✅ Keep using + in-product Authority |
| Fleet / policy-as-code | ✅ | ✅ + control-plane waves & Policies UI |
| In-guest agent (Linux + Windows) | ✅ | ✅ Online agent doctor / repair in Vault |

### Command Center console (Enterprise)

| Capability | Open source | Enterprise |
| --- | --- | --- |
| **Command Center** (KPIs, readiness, velocity, run assessment) | — | ✅ |
| **Portfolio** (risk-ranked workloads, remediate) | Spreadsheets | ✅ |
| **Assurance** (blocker workflow) | CLI repair | ✅ |
| **Migration Factory** (waves, owners, risk, windows) | `fleet wave-plan` | ✅ |
| **Passport Authority** (issue, certify, score gates, JSON download) | CLI emit | ✅ |
| **Dependencies** map | Fleet helpers | ✅ |
| **Policies** catalog | Policy files | ✅ |
| **Compliance** posture | — | ✅ |
| **Reports** export (estate JSON + CSV) | CLI exports | ✅ |
| **Sites & Workers** fabric view | Self-host DIY | ✅ |
| **KubeVirt** cluster inventory | Free Cluster tab | ✅ |
| **Integrations** catalog | DIY | ✅ |
| **Migration Copilot** | Optional CLI AI | ✅ In-console |
| **Administration** (identity, roles, license) | — | ✅ |
| Command palette / search | — | ✅ |
| Mobile console (iOS / Android) | — | ✅ Expo |
| Light / dark theme | zyvor-ui themes | ✅ |

### Image Vault (disk dock)

| Capability | Open source (`zyvor-ui`) | Enterprise Image Vault |
| --- | --- | --- |
| Vault screen under SSO / RBAC | Lab / self-secured | ✅ |
| Inspect inventory tabs (OS/packages/services/users/network/…) | ✅ | ✅ + evidence pane |
| Doctor · migration-plan · passport · provision YAML | ✅ | ✅ |
| Repair plan preview + gated apply | ✅ | ✅ |
| Profiles / Issues | ✅ `POST …/profile` | ✅ |
| Read-only Files browse (`guestkit.explore`) | ✅ | ✅ |
| Attach / register `disk_path` (sources) | ✅ | ✅ |
| Batch doctor | ✅ | ✅ Multi-select + API |
| Launch / provision YAML (KubeVirt) | ✅ | ✅ |
| Online agent doctor / repair | ✅ | ✅ |
| Evidence JSON + plan/YAML panes | ✅ | ✅ |

### Identity · audit · ops

| Capability | Open source | Enterprise |
| --- | --- | --- |
| OIDC / SSO (Keycloak reference) | Configure yourself | ✅ Productized |
| RBAC role gates | DIY | ✅ |
| Audit stream (assessments, Passports, disk actions) | Limited / DIY | ✅ |
| Estate assessment job (one-click) | CLI / fleet batch | ✅ |
| Control-plane API (bootstrap, waves, vault, reports) | OSS API for free UI | ✅ Enterprise API |
| k3s / user packaging / hardening docs | GHCR · Helm | ✅ |
| Air-gapped / disconnected packs | Build yourself | ✅ |

---

## Why teams upgrade

1. Spreadsheets stop working for waves and Passports  
2. Security blocks DIY dashboards — need SSO + audit  
3. Executives need one readiness number and exportable reports  
4. Cutover risk is commercial — SLA and workshops  
5. Engineers keep doctor / vault — Enterprise wraps them in program ops  

---

## When to stay on open source

CI / golden-image gates · lab evaluation · small fleets owned by one engineer.

## When to contact Zyvor

Hypervisor exit with program governance · regulated SSO/audit · multi-site cutovers · contractual accountability.

**→ [Book a demo](https://zyvor.dev/contact?utm_source=github&utm_medium=guestkit&intent=demo)** · **[Pricing](https://zyvor.dev/pricing?utm_source=github&utm_medium=guestkit)** · **[sales@zyvor.dev](mailto:sales@zyvor.dev)**

See also: [zyvor-enterprise.md](zyvor-enterprise.md) · live table on [zyvor.dev/guestkit#enterprise](https://zyvor.dev/guestkit#enterprise)
