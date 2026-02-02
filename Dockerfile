# Dockerfile for QUANTA Blockchain Node
FROM rust:latest as builder

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
FROM debian:sid-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 quanta

WORKDIR /home/quanta

# Copy binary from builder
# Expose ports
# API: 3000, P2P: 8333, RPC: 7782, Metrics: 9090
EXPOSE 3000 8333 7782 9090

# Health check (dynamic port based on config)
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${API_PORT:-3000}/health || exit 1

# Default command
CMD ["quanta", "start", "-c", "/home/quanta/quanta.toml"]
