# Build stage
FROM rust:1.92.0-slim-trixie AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build for release
# Disable debug symbols for Docker builds (override Cargo.toml profile setting)
ENV CARGO_PROFILE_RELEASE_DEBUG=0
RUN cargo build
RUN cargo build --release --locked

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder stage
COPY --from=builder /app/target/release/imgr-serve /usr/local/bin/imgr-serve

# Create directory for persistent storage
ENV PERSISTENT_STORAGE_DIR=/app/data
RUN mkdir -p /app/data

# Expose port
EXPOSE 3021

# Run the application
CMD ["imgr-serve"]

