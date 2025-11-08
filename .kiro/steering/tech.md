# Technology Stack

## Core Technologies

- **Language**: Rust (Edition 2021)
- **Web Framework**: Axum 0.8.6 with async/await
- **Image Processing**: libvips 1.7.1 for high-performance transformations
- **HTTP Client**: reqwest 0.12.24 with JSON support
- **Async Runtime**: Tokio 1.48.0 with full features
- **Caching**: Foyer 0.20.0 for memory/disk/hybrid caching
- **Monitoring**: Prometheus metrics via axum-prometheus
- **Logging**: tracing + tracing-subscriber with structured logging
- **Rate Limiting**: governor 0.10.1
- **Cryptography**: HMAC-SHA256 for URL signing

## Build System

Uses standard Cargo toolchain with these key commands:

### Development Commands
```bash
# Build the project
cargo build
make build

# Run tests (single-threaded for integration tests)
cargo test --all -- --test-threads=1
make test

# Format code (max_width = 120)
cargo fmt
make format

# Lint with clippy (warnings as errors)
cargo clippy -- -D warnings
make lint

# Clean build artifacts
cargo clean
make clean
```

### Dependencies Management
```bash
# Update dependencies
cargo update
make update

# Upgrade dependencies (requires cargo-upgrade)
cargo-upgrade upgrade
make upgrade
```

## Configuration

- Environment-based configuration via `src/config.rs`
- Key environment variables: `IMGFORGE_KEY`, `IMGFORGE_SALT`, `IMGFORGE_BIND`
- Support for presets via `IMGFORGE_PRESETS` environment variable
- Optional Prometheus metrics endpoint configuration

## Docker

Multi-stage Dockerfile:
- Builder: rust:1.90 with libvips-dev
- Runtime: debian:bookworm-slim with libvips-tools
- Exposes port 3000 by default

## Testing

- Integration tests in `tests/` directory
- Unit tests embedded in modules
- Test utilities for handlers and presets
- Single-threaded test execution for proper isolation