# guestkit User Stories

**Product:** Offline VM intelligence and migration assurance

Cross-reference: [Documentation index](README.md) · [Main README](../README.md) · [Industry use cases](INDUSTRY_USE_CASES.md)

## Personas

| Persona | Name | Focus |
|---------|------|-------|
| Migration Engineer | Alex | Pre-flight VM inspection before cutover |
| SRE | Morgan | Fleet drift analysis and forensic diff |
| Platform Architect | Jordan | Boot probability scoring and fix plans |
| Security / Forensics Analyst | Riley | Offline evidence collection, incident-time drift comparison |
| MSP Migration Consultant | Casey | Batch assessment across user VM exports, signed deliverables |

---

### Story 1 — Score boot probability offline

**As Alex** (Migration Engineer), I want to inspect a qcow2/vmdk without powering it on and get a boot probability score with root-cause blockers, **so that** I can tell a user *before* cutover weekend whether a VM will actually come up on the target hypervisor — not find out live, during the maintenance window.

| Criterion | Notes |
|-----------|-------|
| Core capability | `guestkit doctor`, `--explain`, `--target`, blockers list |

---

### Story 2 — Export migration fix plan

**As Alex** (Migration Engineer), I want to generate a hypervisor-aware fix plan as reviewable YAML before cutover, **so that** the plan can go through change control and be applied unattended during the maintenance window instead of me hand-fixing each blocker live.

| Criterion | Notes |
|-----------|-------|
| Core capability | `guestkit migrate-plan`, `--export`, `plan apply` (dry-run + backup) |

---

### Story 3 — Explore disk in TUI

**As Morgan** (SRE), I want to browse partitions, files, and assurance views in the Carbon TUI, **so that** I can triage *why* a specific VM scored low without writing one-off scripts against raw disk offsets.

| Criterion | Notes |
|-----------|-------|
| Core capability | `guestctl tui`, Assurance tab |

---

### Story 4 — Fleet analyze for drift

**As Morgan** (SRE), I want to compare fleet images for configuration drift against a golden baseline, **so that** I catch snowflake VMs and order migration waves by risk before scheduling cutover slots for the whole fleet.

| Criterion | Notes |
|-----------|-------|
| Core capability | `fleet analyze`, `forensic-diff` |

---

### Story 5 — CI gate on doctor score

**As Alex** (Migration Engineer), I want the pipeline to fail if boot probability is below a threshold, **so that** a bad image can never reach the convert step without a human explicitly overriding the gate.

| Criterion | Notes |
|-----------|-------|
| Core capability | `guestkit doctor --fail-below <score>` with `-o json`; `passport verify --fail-below` |

---

### Story 6 — Offline evidence at incident time

**As Riley** (Security / Forensics Analyst), I want to pull structured offline evidence from a stopped or snapshotted VM and diff it against a known-good baseline, **so that** I can establish what changed without booting a possibly-compromised guest and risking tipping off an attacker or destroying volatile evidence.

| Criterion | Notes |
|-----------|-------|
| Core capability | `forensic-diff`, offline evidence snapshot, no guest execution required |

---

### Story 7 — Batch assessment deliverable for a user engagement

**As Casey** (MSP Migration Consultant), I want to run one command across a user's exported VM inventory and get a signed, per-VM readiness report (score, blockers, target recommendation, fix plan), **so that** the assessment phase of an engagement produces a defensible deliverable instead of a pile of ad-hoc notes.

| Criterion | Notes |
|-----------|-------|
| Core capability | `fleet analyze`, `passport emit --sign-key`, batch `doctor` across a directory |

---

## Validation

Map each story to smoke tests, CI jobs, or manual lab steps before marking production-ready.
