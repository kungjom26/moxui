# MoxUI Makefile
# Enforces the same gates as CI locally — run `make lint` before every push.

.PHONY: help
help: ## Show this help
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: fmt
fmt: ## Format code (cargo fmt)
	cargo fmt

.PHONY: fmt-check
fmt-check: ## Check formatting (cargo fmt --check)
	cargo fmt --check

.PHONY: clippy
clippy: ## Run clippy with -D warnings (same as CI)
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: test
test: ## Run tests (cargo test --all-features)
	cargo test --all-features

.PHONY: audit
audit: ## Run cargo audit (security advisories)
	cargo audit

.PHONY: deny
deny: ## Run cargo deny (license + ban + advisory)
	cargo deny check

.PHONY: build
build: ## Debug build
	cargo build

.PHONY: build-release
build-release: ## Release build (LTO + strip + abort-on-panic)
	cargo build --release

.PHONY: lint
lint: fmt-check clippy ## Local CI check: fmt + clippy (run before push)
	@echo ""
	@echo "✓ fmt + clippy pass — safe to push"

.PHONY: check-all
check-all: fmt-check clippy test audit ## Full local check: fmt + clippy + test + audit
	@echo ""
	@echo "✓ All CI gates pass locally"

.PHONY: clean
clean: ## Clean build artifacts
	cargo clean

.PHONY: run
run: ## Run the server (debug)
	RUST_LOG=info cargo run

.PHONY: run-release
run-release: build-release ## Run the server (release)
	RUST_LOG=info ./target/release/moxui

# -----------------------------------------------------------------------------
# Packaging
# -----------------------------------------------------------------------------

VERSION ?= $(shell grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)
BINARY := target/release/moxui
PREFIX ?= /usr/local

.PHONY: install
install: build-release ## Install the binary + systemd unit to PREFIX (default /usr/local)
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 0755 $(BINARY) $(DESTDIR)$(PREFIX)/bin/moxui
	install -d $(DESTDIR)$(PREFIX)/lib/systemd/system
	install -m 0644 contrib/moxui.service $(DESTDIR)$(PREFIX)/lib/systemd/system/moxui.service
	install -d $(DESTDIR)$(PREFIX)/share/doc/moxui
	install -m 0644 README.md $(DESTDIR)$(PREFIX)/share/doc/moxui/README.md
	install -d $(DESTDIR)/etc/moxui
	install -m 0640 contrib/moxui.yaml.example $(DESTDIR)/etc/moxui/config.yaml.example
	@echo ""
	@echo "✓ moxui $(VERSION) installed to $(PREFIX)"
	@echo "  Next: edit /etc/moxui/config.yaml, then:"
	@echo "    systemctl daemon-reload"
	@echo "    systemctl enable --now moxui.service"

.PHONY: uninstall
uninstall: ## Remove installed files (does NOT remove state)
	rm -f $(DESTDIR)$(PREFIX)/bin/moxui
	rm -f $(DESTDIR)$(PREFIX)/lib/systemd/system/moxui.service
	rm -rf $(DESTDIR)$(PREFIX)/share/doc/moxui
	@echo "✓ moxui removed (state under /var/lib/moxui preserved)"

.PHONY: package-deb
package-deb: build-release ## Build a .deb package (requires dpkg-deb + fakeroot)
	@which dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb not found"; exit 1; }
	@tmp=$$(mktemp -d) && \
	  install -d $$tmp/DEBIAN $$tmp/usr/bin $$tmp/usr/lib/systemd/system $$tmp/etc/moxui $$tmp/usr/share/doc/moxui && \
	  install -m 0755 $(BINARY) $$tmp/usr/bin/moxui && \
	  install -m 0644 contrib/moxui.service $$tmp/usr/lib/systemd/system/moxui.service && \
	  install -m 0644 contrib/moxui.yaml.example $$tmp/etc/moxui/config.yaml.example && \
	  install -m 0644 README.md $$tmp/usr/share/doc/moxui/README.md && \
	  echo "Package: moxui"                         >  $$tmp/DEBIAN/control && \
	  echo "Version: $(VERSION)"                    >> $$tmp/DEBIAN/control && \
	  echo "Section: admin"                         >> $$tmp/DEBIAN/control && \
	  echo "Priority: optional"                     >> $$tmp/DEBIAN/control && \
	  echo "Architecture: $$(dpkg --print-architecture)" >> $$tmp/DEBIAN/control && \
	  echo "Maintainer: kungjom26 <kungjom26@gmail.com>" >> $$tmp/DEBIAN/control && \
	  echo "Description: Modern Rust-based web UI for Proxmox VE" >> $$tmp/DEBIAN/control && \
	  echo " moxui provides a secure HTTPS-only management UI"     >> $$tmp/DEBIAN/control && \
	  echo " for one or more Proxmox VE clusters."                  >> $$tmp/DEBIAN/control && \
	  install -d $$tmp/DEBIAN && \
	  echo "/etc/moxui/config.yaml.example" > $$tmp/DEBIAN/conffiles && \
	  echo '#!/bin/sh'                              >  $$tmp/DEBIAN/postinst && \
	  echo 'set -e'                                 >> $$tmp/DEBIAN/postinst && \
	  echo 'getent group moxui >/dev/null || groupadd --system moxui' >> $$tmp/DEBIAN/postinst && \
	  echo 'getent passwd moxui >/dev/null || useradd --system --gid moxui --home /var/lib/moxui --shell /usr/sbin/nologin moxui' >> $$tmp/DEBIAN/postinst && \
	  echo 'mkdir -p /var/lib/moxui /var/log/moxui /etc/moxui/tls'    >> $$tmp/DEBIAN/postinst && \
	  echo 'chown moxui:moxui /var/lib/moxui /var/log/moxui'          >> $$tmp/DEBIAN/postinst && \
	  echo 'chmod 0750 /var/lib/moxui /var/log/moxui'                  >> $$tmp/DEBIAN/postinst && \
	  echo 'chmod 0750 /etc/moxui'                                     >> $$tmp/DEBIAN/postinst && \
	  echo 'systemctl daemon-reload || true'                           >> $$tmp/DEBIAN/postinst && \
	  echo 'echo "moxui installed. Edit /etc/moxui/config.yaml, then run: systemctl enable --now moxui.service"' >> $$tmp/DEBIAN/postinst && \
	  chmod 0755 $$tmp/DEBIAN/postinst && \
	  fakeroot dpkg-deb --build $$tmp moxui_$(VERSION)_$$(dpkg --print-architecture).deb && \
	  rm -rf $$tmp
	@echo ""
	@echo "✓ Built: moxui_$(VERSION)_$$(dpkg --print-architecture).deb"

.PHONY: package
package: package-deb ## Alias for package-deb
