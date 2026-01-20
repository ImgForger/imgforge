# Dockerfile for imgforge

# Builder stage
FROM ubuntu:24.04 AS builder

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y ca-certificates curl libvips-dev pkg-config build-essential
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /usr/src/imgforge

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y ca-certificates curl libssl3 libvips-tools && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/imgforge/target/release/imgforge .

# Expose the port the application runs on
EXPOSE 3000

# Set the entrypoint
CMD ["./imgforge"]
