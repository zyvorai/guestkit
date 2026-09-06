# 02 — Offline repair worker

**Goal:** Stand up a Linux jump box or `guestkit-worker` that can doctor, plan, rescue, and apply day-0 packs **without booting** the guest.

---

## 1. Host requirements

| Need | Notes |
|------|--------|
| Linux | Primary support surface |
| `qemu-img`, `losetup`, `qemu-nbd` | Install via distro packages |
| Root or equivalent for NBD/loop | Mount/repair often needs privileges |
| Disk / I/O | Local SSD for working copies; don’t repair the only golden on NFS without backup |
| Optional | `guestkit` from crates.io / package; or GHCR worker image |

```bash
cargo install guestkit          # guestkit + guestctl
guestkit --help
guestctl tui --help
```

---

## 2. GHCR stack (eval)

```bash
docker compose -f deploy/docker-compose.ghcr.yml pull
docker compose -f deploy/docker-compose.ghcr.yml up -d
open http://localhost:8088
```

Images: `zyvor-ui`, `zyvor-api`, `guestkit-worker` under `ghcr.io/zyvorai`.

**Eval only** — unauthenticated by default. Do not expose beyond localhost. Production: `deploy/docker-compose.prod.example.yml` / Helm `deploy/helm/zyvor`. Change default console login before any network exposure ([DEPLOY-REMOTE](../guides/DEPLOY-REMOTE.md)).

---

## 3. Standard repair loop

```bash
DISK=./work/vm.qcow2
TARGET=proxmox

# 1) Copy to working volume (never mutate the only backup blindly)
cp -a "$GOLDEN" "$DISK"

# 2) Score + plan
guestkit doctor "$DISK" --target "$TARGET" --explain | tee doctor.txt
guestkit migrate-plan "$DISK" --target "$TARGET" --export plan.yaml

# 3) Day-0 packs (examples)
guestkit plan generate "$DISK" -p linux-ssh --user ubuntu --key-file ~/.ssh/id_ed25519.pub -o ssh.yaml
guestkit plan generate "$DISK" -p windows-rdp -o rdp.yaml
guestkit plan generate "$DISK" -p windows-domain-leave --workgroup WORKGROUP -o leave.yaml

# 4) Rescue shortcuts
guestkit rescue "$DISK" -o fix-grub --force
guestkit rescue "$DISK" -o enable-ssh
# Windows password — vault the secret; rotate after cutover
guestkit rescue "$DISK" -o reset-password --user Administrator --password "$TEMP_PW"

# 5) Apply with backup/rollback semantics
guestkit plan apply plan.yaml --vm "$DISK" --yes

# 6) Re-score + Passport
guestkit doctor "$DISK" --target "$TARGET" --explain
guestkit passport emit "$DISK" --target "$TARGET" -o passport.json --bundle
guestkit passport verify passport.json --fail-below 80
```

Use **guestctl TUI** for human triage: `guestctl tui "$DISK"`.

---

## 4. Worker profiles / perf

For fleet jobs, set parallelism:

```bash
export GUESTKIT_FLEET_JOBS=8
```

Keep convert and repair on separate pools if both hammer disk. Prefer local scratch; rsync results back to object store.

---

## 5. Safety rails

- Always snapshot/copy before `plan apply` / rescue mutate.  
- Temp Windows passwords: CI secret → rotate → never leave in logs.  
- BitLocker: offline hard-block — decrypt/export unlock first (triage runbook).  
- AES SAM needs `registry-write`/hivex-capable build; else RunOnce fallback ([Windows blog](https://zyvor.dev/blog/guestkit-windows-aes-sam-runonce)).  
