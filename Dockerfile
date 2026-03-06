# Quotey - Multi-stage Docker Build

# Stage 1: Build
FROM rust:1.75-slim as builder

WORKDIR /app

# Install dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests first (for better caching)
COPY Cargo.toml Cargo.lock ./
COPY crates/*/Cargo.toml ./crates/

# Copy source code
COPY . .

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy binaries from builder
COPY --from=builder /app/target/release/quotey-server /usr/local/bin/
COPY --from=builder /app/target/release/quotey /usr/local/bin/

# Copy config and templates
COPY --from=builder /app/config/ /app/config/
COPY --from=builder /app/templates/ /app/templates/
COPY --from=builder /app/migrations/ /app/migrations/

# Create data directory
RUN mkdir -p /data

# Environment
ENV QUOTEY_CONFIG=/app/config/quotey.toml
ENV RUST_LOG=info

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose health check port
EXPOSE 8080

# Run the server
CMD ["quotey-server"]
