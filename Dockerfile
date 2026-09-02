# ============================================================
# Dockerfile — QUANTA Blockchain Node
# ============================================================
# V3.1.5-alpha Release for testnet testing — Sync deadlock fix
# Image : xd637/quanta-node:v3.2.1-alpha
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

FROM rust:bookworm AS builder

# Version metadata
LABEL version="3.2.2-alpha"
LABEL org.quanta.network.protocol="59"
LABEL org.opencontainers.image.title="Quanta Node" \
      org.opencontainers.image.description="QuantaChain V3 node — PQC BFT, Falcon-512, X25519MLKEM768 TLS transport, Sync deadlock fix. v3.2.2-alpha." \
      org.opencontainers.image.version="v3.2.2-alpha" \
      org.opencontainers.image.vendor="QuantaChain" \
      org.opencontainers.image.source="https://hub.docker.com/r/xd637/quanta-node" \
      org.opencontainers.image.licenses="Apache-2.0"

# Install build dependencies
# cmake: required by aws-lc-rs (AWS LibCrypto) for ML-KEM / X25519MLKEM768 PQC key exchange
# Added v3.1.0-alpha (2026-08-20) for PQC TLS transport
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy all source files
COPY Cargo.toml Cargo.lock ./
COPY src ./src

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
COPY --chown=quanta:quanta quanta.toml /home/quanta/quanta.toml
COPY --chown=quanta:quanta genesis.json /home/quanta/genesis.json

# Create data directories and set permissions
RUN mkdir -p /home/quanta/quanta_data \
    /home/quanta/logs && \
    chown -R quanta:quanta /home/quanta

USER quanta

# Expose ports
# API: 3000, P2P: 8333, RPC: 7782, Metrics: 9090
EXPOSE 3000 8333 7782 9090

# Define volumes for data persistence
VOLUME ["/home/quanta/quanta_data", "/home/quanta/logs"]

# Health check (dynamic port based on config)
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${API_PORT:-3000}/health || exit 1

# Default command
CMD ["/usr/local/bin/quanta", "start", "-c", "/home/quanta/quanta.toml"]
