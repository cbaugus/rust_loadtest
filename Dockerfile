FROM rust:bullseye AS builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

# --- Stage 2: Create the final, smaller runtime image ---
# Use a minimal base image for the final runtime
FROM debian:bullseye-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       libssl1.1 \
       ca-certificates \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/local/cargo/bin/rust_loadtest /usr/local/bin/rust_loadtest

# Expose the Prometheus metrics port
EXPOSE 9090

# Command to run the application when the container starts
CMD ["/usr/local/bin/rust_loadtest"]

