# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
CARGO ?= cargo
BINARY = guestkit

WIN_TARGET ?= x86_64-pc-windows-gnu

.PHONY: all build release install uninstall clean check test fmt clippy selftest \
	test-features test-features-remote \
	agent-linux agent-windows linux-bundle windows-bundle windows-check \
	deploy deploy-remote deploy-remote-quick deploy-remote-preflight deploy-remote-verify deploy-remote-uninstall deploy-remote-fleet

all: build

build:
	$(CARGO) build

release:
	$(CARGO) build --release

install: release
	install -Dm755 target/release/$(BINARY) $(DESTDIR)$(BINDIR)/$(BINARY)

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/$(BINARY)

clean:
	$(CARGO) clean

check: test clippy

test:
	$(CARGO) test

fmt:
	$(CARGO) fmt

clippy:
	$(CARGO) clippy -- -D warnings

selftest:
	bash scripts/selftest.sh

## --- In-guest agent (Linux + Windows) ---

agent-linux: ## Build the Linux in-guest agent (guestkitd/guestkitctl/guestkitd-exec)
	$(CARGO) build --release -p zyvor-guest-agent

agent-windows: ## Cross-compile the Windows agent (needs mingw-w64 + rustup win-gnu target)
	rustup target add $(WIN_TARGET) 2>/dev/null || true
	$(CARGO) build --release -p zyvor-guest-agent --target $(WIN_TARGET)

windows-check: ## Type-check the Windows agent without linking (fast CI gate)
	rustup target add $(WIN_TARGET) 2>/dev/null || true
	$(CARGO) check -p zyvor-guest-agent --target $(WIN_TARGET)

linux-bundle: ## Build Linux agent + self-contained tarball (static musl if available)
	bash scripts/build-linux-bundle.sh

windows-bundle: ## Build Windows agent + MSI + downloadable ISO (needs wixl + genisoimage)
	bash scripts/build-windows-bundle.sh

test-features: ## Compile+clippy matrix of every feature (run locally; needs libhivex/libsystemd)
	bash scripts/test-feature-matrix.sh

test-features-remote: ## Feature matrix on a remote box: make test-features-remote H=ip U=user [SETUP=1]
	@test -n "$(H)" || { echo "  ❌ H (host) is required"; exit 1; }
	bash scripts/test-feature-matrix-remote.sh $(H) $(or $(U),root) $(if $(SETUP),--setup) --key $(ARGS)

deploy: deploy-remote ## Alias for deploy-remote

deploy-remote: ## Full remote deploy: make deploy-remote H=ip U=root
	@test -n "$(H)" || { echo "  ❌ H (host) is required"; exit 1; }
	bash scripts/deploy-remote.sh $(H) $(or $(U),root) $(PASS) --key $(ARGS)

deploy-remote-quick: ## Quick remote deploy: make deploy-remote-quick H=ip U=root
	@test -n "$(H)" || { echo "  ❌ H is required"; exit 1; }
	bash scripts/deploy-remote.sh $(H) $(or $(U),root) $(PASS) --key --quick $(ARGS)

deploy-remote-preflight: ## SSH preflight only: make deploy-remote-preflight H=ip
	@test -n "$(H)" || { echo "  ❌ H is required"; exit 1; }
	bash scripts/deploy-remote.sh $(H) $(or $(U),root) --key --preflight-only

deploy-remote-verify: ## Remote selftest only: make deploy-remote-verify H=ip
	@test -n "$(H)" || { echo "  ❌ H is required"; exit 1; }
	bash scripts/deploy-remote.sh $(H) $(or $(U),root) --key --verify-only

deploy-remote-uninstall: ## Remove guestkit from host: make deploy-remote-uninstall H=ip
	@test -n "$(H)" || { echo "  ❌ H is required"; exit 1; }
	bash scripts/deploy-remote.sh $(H) $(or $(U),root) --key --uninstall

deploy-remote-fleet: ## Fleet deploy: make deploy-remote-fleet FILE=hosts.txt
	@test -n "$(FILE)" || { echo "  ❌ FILE is required"; exit 1; }
	bash scripts/deploy-remote.sh --fleet $(FILE)
