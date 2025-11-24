# Dockerfile for imgforge

# Builder stage
FROM rust:1.90 AS builder

RUN apt-get update && apt-get install -y libvips-dev pkg-config

WORKDIR /usr/src/imgforge

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    build-essential libssl3 \
    pkg-config \
    meson \
    ninja-build \
    libdav1d-dev \
    libsvtav1-dev \
    libde265-dev \
    libx265-dev \
    libheif1 \
    libheif-dev \
    libaom3 \
    libaom-dev \
    libwebp-dev \
    libpng-dev \
    libjpeg-dev \
    libtiff-dev \
    libgif-dev \
    libexpat1-dev \
    libglib2.0-dev \
    wget libvips-dev libvips-doc libvips-tools \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Build libvips 8.17.3
RUN wget https://github.com/libvips/libvips/archive/refs/tags/v8.17.3.tar.gz && \
    mkdir /vips && \
    tar xvzf v8.17.3.tar.gz -C /vips --strip-components 1 && \
    cd /vips && \
    meson setup build --buildtype=release --prefix=/usr/local -Dopenexr=disabled -Ddebug=false && \
    ninja -C build && \
    ninja -C build install && \
    rm -rf /vips v8.17.3.tar.gz
RUN ldconfig

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/imgforge/target/release/imgforge .

# Expose the port the application runs on
EXPOSE 3000

# Set the entrypoint
CMD ["./imgforge"]
