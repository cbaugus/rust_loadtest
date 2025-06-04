FROM rust:bullseye AS builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .


#RUN apt-get update && apt-get install -y pkg-config libssl-dev ca-certificates && \
#    cargo build --release

# --- Stage 2: Create the final, smaller runtime image ---
# Use a minimal base image for the final runtime
FROM debian:bullseye
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
	libssl1.1  \
    pkg-config \
    libssl-dev \
	openssl \
	libc6 \
	ca-certificates \
	&& apt-get autoremove -y


# Set the working directory
WORKDIR /usr/local/bin

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/local/cargo/bin/rust_loadtest /usr/local/bin/rust_loadtest

# Expose the Prometheus metrics port
EXPOSE 9090

# Command to run the application when the container starts
CMD ["/usr/local/bin/rust_loadtest"]

