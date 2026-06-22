# syntax=docker/dockerfile:1
#
# MoxUI — multi-stage Docker build
#
# Stage 1: Builder
#   Uses the official Rust image with all build deps.
#   Compiles the binary with release profile and strips debug symbols.
#
# Stage 2: Runtime
#   Ultra-slim debian:bookworm-slim with only the binary, assets,
#   and CA certificates (for Proxmox TLS connections).
#   Runs as non-root user `moxui` (UID 10001).

# ── Builder ──────────────────────────────────────────────────────────
FROM rust:1.78-slim-bookworm AS builder

# Install build deps (mainly for linking)
RUN apt-get update -qq && apt-get install -y -qq \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy src/main.rs so `cargo build` can fetch + compile deps
RUN mkdir src tests benches ui && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/lib.rs && \
    touch tests/fixtures/test_jwt_priv.pem \
          tests/fixtures/test_jwt_pub.pem

# Build dependencies (this layer is cached unless Cargo.toml/lock change)
RUN cargo build --release --locked 2>&1 && \
    # Remove the dummy artifacts so the real build doesn't get confused
    rm -rf src/main.rs src/lib.rs target/release/.fingerprint/moxui-* \
           target/release/deps/moxui-* 2>/dev/null; true

# Copy the real source and assets
COPY src/ src/
COPY tests/ tests/
COPY benches/ benches/
COPY ui/ ui/
COPY Makefile ./

# Re-build with real source (only the moxui crate recompiles)
RUN cargo build --release --locked

# Strip the binary to save space
RUN strip target/release/moxui

# ── Runtime ─────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Install CA certificates (needed for Proxmox TLS verification)
RUN apt-get update -qq && apt-get install -y -qq \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --gid 10001 moxui && \
    useradd --uid 10001 --gid moxui --create-home --shell /usr/sbin/nologin moxui

# Create data directories
RUN mkdir -p /etc/moxui /var/lib/moxui/data /var/lib/moxui/logs && \
    chown -R moxui:moxui /etc/moxui /var/lib/moxui

WORKDIR /var/lib/moxui

# Copy the binary and default config
COPY --from=builder /build/target/release/moxui /usr/local/bin/moxui
COPY config.example.yaml /etc/moxui/config.yaml

# Default environment (override at runtime)
ENV MOXUI_CONFIG=/etc/moxui/config.yaml
ENV MOXUI_SERVER__BIND=0.0.0.0:8080
ENV MOXUI_DATABASE__PATH=/var/lib/moxui/data/moxui.db
ENV MOXUI_LOG_LEVEL=info

USER moxui:moxui

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/livez || exit 1

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/moxui"]