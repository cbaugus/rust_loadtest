# Multi-stage build for rust-loadtest
# Stage 1: Build
FROM rustlang/rust:nightly-slim AS builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and build script
COPY Cargo.toml Cargo.lock ./
COPY build.rs ./

# Copy proto definitions (required by build.rs to generate gRPC stubs)
COPY proto ./proto

# Copy source code
COPY src ./src
COPY tests ./tests
COPY examples ./examples

# Build release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder (Cargo uses underscore)
COPY --from=builder /app/target/release/rust_loadtest /usr/local/bin/rust-loadtest

# Copy example configs and data
COPY examples/configs /app/configs
COPY examples/data /app/data
COPY docs /app/docs

# Set working directory
WORKDIR /app

# Default command shows help
CMD ["rust-loadtest", "--help"]
