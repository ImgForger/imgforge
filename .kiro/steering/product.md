# Product Overview

imgforge is a fast, secure image proxy and transformation server written in Rust. It provides imgproxy-compatible URL semantics with high-performance image processing capabilities.

## Core Features

- **On-the-fly image transformations**: Resize, crop, format conversion, watermarking, blur, sharpen
- **Named presets**: Consistent transformation governance across requests
- **Security**: Signed URLs, bearer authentication, rate limiting, per-request safeguards
- **Caching**: Memory, disk, and hybrid caches powered by Foyer for efficient content reuse
- **Production-ready**: Health checks, structured logging, Prometheus metrics
- **Container-native**: Multi-stage Docker image with optional monitoring stack

## Architecture

Built with async-first architecture using:
- libvips for high-fidelity image processing
- Axum web framework for HTTP handling
- Foyer for pluggable caching backends
- Prometheus for observability

## Target Use Cases

- Image CDN and proxy services
- On-demand image transformation APIs
- High-performance image processing pipelines
- Container-based deployments with monitoring