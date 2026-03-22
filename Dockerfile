# ============================================================
# Dockerfile — QUANTA Blockchain Node
# ============================================================
# Alpha v2 — Release for testnet testing
#
# Image : xd637/quanta-node:alpha-v2
# Repo  : https://hub.docker.com/r/xd637/quanta-node
#
# Quick start (single node):
#   docker compose -f docker-compose.single.yml up -d
#
# Ports:
#   3000 — REST API
#   8333 — P2P network
#   7782 — RPC
#   9090 — Prometheus metrics
# ============================================================

FROM rust:latest AS builder

# Image metadata (OCI standard)
LABEL org.opencontainers.image.title="quanta-node" \
      org.opencontainers.image.description="QuantaChain blockchain node — post-quantum secure, PoW/PoS hybrid. Alpha v2 release." \
      org.opencontainers.image.version="alpha-v2" \
      org.opencontainers.image.vendor="QuantaChain" \
      org.opencontainers.image.source="https://hub.docker.com/r/xd637/quanta-node" \
      org.opencontainers.image.licenses="Apache-2.0"

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 quanta

WORKDIR /home/quanta

# Copy all binaries from builder
COPY --from=builder /app/target/release/quanta        /usr/local/bin/quanta
COPY --from=builder /app/target/release/quanta-wallet /usr/local/bin/quanta-wallet
COPY --from=builder /app/target/release/quanta-miner  /usr/local/bin/quanta-miner
COPY --chown=quanta:quanta quanta.toml /home/quanta/quanta.toml

# Create data directories and set permissions
RUN mkdir -p /home/quanta/quanta_data \
    /home/quanta/logs && \
    chown -R quanta:quanta /home/quanta

USER quanta

# Expose ports
# API: 3000, P2P: 8333, RPC: 7782, Metrics: 9090
EXPOSE 3000 8333 7782 9090

# Health check (dynamic port based on config)
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${API_PORT:-3000}/health || exit 1

# Default command
CMD ["/usr/local/bin/quanta", "start", "-c", "/home/quanta/quanta.toml"]
