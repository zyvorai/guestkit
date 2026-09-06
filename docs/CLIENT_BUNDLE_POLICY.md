# Client bundle policy — binaries only, no source tree

User tarballs from `scripts/package-binary-remote.sh` must **never** require a git clone or compile on the install host. Build happens on **your** remote pack host; the user gets **artifacts + install scripts only**.

## Bundle types

| Type | Products | What ships | User runs |
|------|----------|------------|---------------|
| **A — Native binary** | VMRogue, v9s, machina, guestkit, hypersdk, packetwolf, ragnarok, Aether, IronWolf | Single executable(s), optional `web/dist` or `frontend/dist`, env example | `./install.sh` → `./binary` or systemd via `install-full.sh` (machina) |
| **B — Container extract** | VMRogue, v9s | Binary + UI from OCI image build (still type A at install time) | Same as A |
| **C — Python venv bundle** | **hyper2kvm**, **forge** | `venv/` with `pip install` already done, wrapper scripts in `bin/`, static UI | `./install.sh`; run `./bin/*` (uses bundled `venv/`) |
| **D — Go multi-binary** | hypersdk | `bin/hypervisord`, `hyperctl`, … + `dashboard/` | `./bin/hypervisord` |
| **E — K8s cluster add-on** | VMRogue, v9s only | `cluster/` YAML + `install-cluster.sh` (not app source) | Cluster admin scripts; app still type A/B |

## What must NOT be in user tarballs

- Full git tree, `Cargo.toml` / `Makefile` (except optional small `contrib/` snippets machina ships for mkosi defs)
- `target/`, `node_modules/`, `.git/`
- Installers that call `cargo build`, `npm run build`, or `git clone` on the user host

## Python products (hyper2kvm, forge) — distribute differently

These are **not** shipped as one static ELF like Rust/Go tools.

1. **Remote build** creates `.pkg-venv` on the pack host (`pip install .` or `requirements.txt`).
2. **Tarball** contains the whole **`venv/`** directory (relocated paths; user path is fixed at extract dir).
3. **`bin/hyper2kvm`** (and similar) are **wrappers** that exec `venv/bin/python -m …`.
4. User needs **Python 3.10+ system libs** (libvirt, openssl) via `install-client-deps.sh`; they do **not** need to run `pip install` again unless recreating the venv.

hyper2kvm additionally bundles **Go `h2kweb`** as a native binary; dashboard is static files under `web/dashboard/`.

forge bundles **minimal** `services/api-gateway/` (entry module) + full **venv** + `ui/dist/`.

## Per-product checklist

| Product | Type | Sources in tarball? | Notes |
|---------|------|---------------------|--------|
| VMRogue | B→A | No | `vmrogue` + optional `virtctl`; cluster scripts only |
| v9s | B→A | No | `v9s-web` + `ui/dist` |
| machina | A | No | `machina-daemon`, TUI, `web/dist`; `install-full.sh` = bundle-aware host install |
| guestkit | A | No | `guestkit` binary only |
| hypersdk | D | No | `bin/*` + `dashboard/` |
| hyper2kvm | C + Go | No app source | **`venv/` required**; not a single static binary |
| packetwolf | A | No | `packetwolf-api` + `ui/` |
| ragnarok | A | No | `ragnarok` + `frontend/dist` |
| Aether | A | No | `aether` (embedded UI) |
| IronWolf | A | No | `ironwolf-web` + `dashboard/dist` |
| forge | C | No full tree | **`venv/`** + thin API tree + `ui/dist` |

## `install.sh` vs `install-full.sh` (machina only)

| Script | Purpose |
|--------|---------|
| `install.sh` | Lightweight: deps check, config template, verify bundled binaries |
| `install-full.sh` | Production host: OS deps (mkosi, packer, …), systemd, `/usr/local/bin`, TLS — **uses bundle, no compile** |

Do not copy the repo `install.sh` into tarballs without **bundle mode** (machina commit `721537e+`).

## Remote pack script naming

Phases say **“Sync to build host”** (rsync for remote *build*, not user source). User-facing docs must say **“extract tarball”**, not “clone repo”.

## Adding a new product

1. Pick type **A**, **C**, or **D** above.
2. Implement `scripts/lib/package-install.sh` to only reference paths **inside the tarball**.
3. In `package-binary-remote.sh` REMOTE_PACK: copy artifacts only; for Python use **venv + wrappers**.
4. Never `cp` the repo’s full `install.sh` unless it detects bundle layout (like machina).

See also: `docs/PACKAGE_BINARY_REMOTE.md` per repo.
