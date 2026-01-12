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
COPY --from=builder /app/target/release/quanta /usr/local/bin/quanta
COPY --chown=quanta:quanta quanta.toml /home/quanta/quanta.toml
COPY entrypoint.sh /usr/local/bin/entrypoint.sh
COPY testnet_entrypoint.sh /usr/local/bin/testnet_entrypoint.sh

# Fix line endings for scripts (Windows -> Unix)
RUN sed -i 's/\r$//' /usr/local/bin/entrypoint.sh && \
    sed -i 's/\r$//' /usr/local/bin/testnet_entrypoint.sh

# Create data directories and set permissions
RUN mkdir -p /home/quanta/quanta_data_node1 \
    /home/quanta/quanta_data_node2 \
    /home/quanta/quanta_data_node3 \
    /home/quanta/quanta_data_testnet \
    /home/quanta/logs && \
    chown -R quanta:quanta /home/quanta && \
    chmod +x /usr/local/bin/entrypoint.sh && \
    chmod +x /usr/local/bin/testnet_entrypoint.sh

USER quanta

# Expose ports for multi-node setup
# API ports (3000-3002), P2P ports (8333-8335), RPC ports (7782-7784), Metrics (9090-9092)
EXPOSE 3000 3001 3002 8333 8334 8335 7782 7783 7784 9090 9091 9092

# Health check (dynamic port based on config)
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:${API_PORT:-3000}/health || exit 1

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

# Default command
CMD ["quanta", "start", "-c", "/home/quanta/quanta.toml"]
