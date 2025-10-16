# Contributing

We welcome bug reports, feature ideas, and pull requests! This guide outlines how to set up a development environment, follow project conventions, and validate your changes.

## Development environment

1. **Install Rust** via [rustup](https://rustup.rs/) (Rust 1.90 or newer).
2. **Install libvips** and build prerequisites:
   - Debian/Ubuntu: `sudo apt-get install libvips-dev libvips pkg-config build-essential`
   - macOS (Homebrew): `brew install vips`
3. Clone the repository and install Git hooks if you use any local automation.

Project layout (selected files):

```
src/
  main.rs          # Entrypoint
  server.rs        # Axum router and runtime wiring
  handlers.rs      # HTTP handlers and common flow
  processing/      # Option parsing, transforms, save pipeline
  caching/         # Cache adapters backed by foyer
  url.rs           # Path parsing and signature validation
```

## Coding standards

- Format Rust code with `cargo fmt` (configured via `.rustfmt.toml`).
- Address clippy lints (`cargo clippy --all-targets --all-features`).
- Keep functions well-scoped and prefer idiomatic async patterns.
- Write documentation in Markdown under `doc/` and keep README succinct.
- Favor descriptive logging using the existing `tracing` spans.

## Testing

Run the full test suite before submitting a PR:

```bash
cargo test
cargo test --all -- --test-threads=1    # integration tests expect serialized execution
```

Integration tests under `tests/` hit live HTTP endpoints (e.g., httpbin.org). Ensure you have internet access or adjust the fixtures accordingly.

## Benchmarking and profiling

imgforge does not ship dedicated benchmarks yet. When evaluating performance:

- Use `cargo build --release` and exercise endpoints with tools such as [`wrk`](https://github.com/wg/wrk) or [`vegeta`](https://github.com/tsenart/vegeta).
- Inspect Prometheus histograms (`image_processing_duration_seconds`, `http_requests_duration_seconds`) to track regressions.

## Branching and pull requests

1. Fork the repository and branch off `main` (for docs and contributions we use feature branches such as `docs/add-docs-suite-imgproxy-rust`).
2. Keep commits focused, with descriptive messages referencing tickets when relevant.
3. Update or add tests when behavior changes.
4. Update documentation in `doc/` when adding or modifying configuration or processing options.
5. Open a pull request and fill out the template, describing testing performed and any observability implications.

## Reporting issues

When filing an issue, please include:

- imgforge version (`cargo pkgid` or git commit SHA).
- Operating system and libvips version.
- Minimal, signed request URLs (or `unsafe` URLs for tests) that reproduce the problem.
- Relevant logs and metrics snapshots.

Thank you for investing in imgforge! Your feedback and contributions make the project more robust for everyone.
