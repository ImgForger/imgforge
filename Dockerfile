# Dockerfile for imgforge

# Builder stage
FROM rust:1.90 as builder

WORKDIR /usr/src/imgforge

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/imgforge/target/release/imgforge .

# Expose the port the application runs on
EXPOSE 3000

# Set the entrypoint
CMD ["./imgforge"]
