# Publishing GuestKit

## PyPI (`zyvor-guestkit`)

The Python distribution is **`zyvor-guestkit`**, owned by the [zyvor](https://pypi.org/user/zyvor/) account. Do not publish to `hypersdk-guestkit`. The import name stays `guestkit`.

`PYPI_API_TOKEN` must be an API token from that account (https://pypi.org/manage/account/token/). The first upload creates the project; the old `hypersdk` token cannot.

```bash
pip install zyvor-guestkit
```

CI workflow: **Build and Publish Python Wheels** — requires GitHub secret `PYPI_API_TOKEN`.

Manual publish after building wheels locally:

```bash
maturin build --release --features python-bindings --out dist
twine upload dist/*
```

## crates.io (`guestkit`)

The Rust crate name stays **`guestkit`**. That crate is owned by the crates.io user `ssahani`. The newest version there is **0.3.2**. `cargo install guestkit` does not install v1.2.4.

`CARGO_TOKEN` is a token for the `zyvorai` account. That account can publish only after `ssahani` adds it and the invite is accepted:

```bash
cargo owner --add zyvorai guestkit
```

`guestkit-agent-protocol` 0.1.0 is already published by `zyvorai`. The main crate depends on it. Until an owner publishes `guestkit` 1.2.4, install the [GitHub Release](https://github.com/zyvorai/guestkit/releases/tag/v1.2.4) binaries instead of crates.io.

Workflow: **Release** → job `publish-crate`. Token: https://crates.io/settings/tokens

## GitHub release assets

Tag push `v*` builds Linux x86_64 binaries and creates a GitHub Release from `docs/development/CHANGELOG.md`.
