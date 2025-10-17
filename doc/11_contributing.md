# 11. Contributing

We welcome bug reports, feature ideas, and pull requests! This guide outlines the development workflow, coding standards, and expectations for contributors.

## Development environment

1. Install Rust 1.90 or newer using [rustup](https://rustup.rs/).
2. Install libvips and build prerequisites:
   - **Debian / Ubuntu**: `sudo apt-get install libvips-dev libvips pkg-config build-essential`
   - **macOS (Homebrew)**: `brew install vips pkg-config`
3. Clone the repository and install any project-specific Git hooks you rely on.

Project layout (selected files):

```
src/
  main.rs          # Entrypoint
  server.rs        # Axum router and runtime wiring
  handlers.rs      # HTTP handlers and common flow
  processing/      # Option parsing, transforms, save pipeline
  caching/         # Cache adapters backed by Foyer
  url.rs           # Path parsing and signature validation
```

Refer to [1_installation.md](1_installation.md) for a comprehensive setup checklist and [6_processing_pipeline.md](6_processing_pipeline.md) for architectural context.

## Coding standards

- Run `cargo fmt` to ensure code adheres to `.rustfmt.toml`.
- Address `cargo clippy --all-targets --all-features` warnings before submitting.
- Favor idiomatic async Rust; keep functions short and well-scoped.
- Document new functionality and update numbered docs under `doc/` when behavior changes.
- Use structured logging via `tracing` rather than `println!`.

## Testing

Run the full test suite locally:

```bash
cargo test
cargo test --all -- --test-threads=1    # integration tests expect serialized execution
```

Integration tests under `tests/` contact external services (e.g., httpbin.org). Ensure outbound network access is available or adapt fixtures to local mocks.

## Benchmarking and profiling

- Build in release mode for representative numbers: `cargo build --release`.
- Exercise endpoints with [`wrk`](https://github.com/wg/wrk) or [`vegeta`](https://github.com/tsenart/vegeta).
- Use `perf`, `cargo flamegraph`, or sampling profilers to locate hotspots.

## Branching & pull requests

1. Branch from `main` using a descriptive name (e.g., `feat/resize-presets` or `docs/cache-guidelines`).
2. Keep commits focused and reference relevant issues or tickets in commit messages.
3. Include tests for new behavior and update documentation where applicable (see [7_caching.md](7_caching.md), [8_error_troubleshooting.md](8_error_troubleshooting.md), etc.).
4. Fill out the pull request template with testing evidence and operational considerations.
5. Expect review feedback; incorporate changes and re-run tests before requesting re-review.

## Reporting issues

Please provide the following details to help triage quickly:

- imgforge version (`cargo pkgid` or git commit SHA).
- Operating system and libvips version (`vips --version`).
- Full request path (redact signatures if necessary) and the resulting status code.
- Relevant log excerpts (including `X-Request-ID` values) and metrics snapshots.
- Configuration values tied to the issue (timeouts, cache mode, security flags).

Review [8_error_troubleshooting.md](8_error_troubleshooting.md) before filing; many common pitfalls and solutions are documented there.

Thank you for investing in imgforge! Your contributions make the project faster, safer, and easier to operate.
