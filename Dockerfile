# Dockerfile for QUANTA Blockchain Node
FROM rust:1.75-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
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

# Copy binary from builder
COPY --from=builder /app/target/release/quanta /usr/local/bin/quanta
COPY --chown=quanta:quanta quanta.toml /home/quanta/quanta.toml

# Create data directory
RUN mkdir -p /home/quanta/data && \
    chown -R quanta:quanta /home/quanta

USER quanta

# Expose API and P2P ports
EXPOSE 3000 7000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Default command
CMD ["quanta", "start", "-c", "/home/quanta/quanta.toml"]
