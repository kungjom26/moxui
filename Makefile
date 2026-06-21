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
