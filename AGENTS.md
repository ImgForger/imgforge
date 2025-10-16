# Repository Guidelines

## Project Structure & Module Organization
imgforge is a Rust binary crate; `src/main.rs` boots the HTTP server and `src/lib.rs` wires shared state. Request routing lives in `src/server.rs`, HTTP handlers in `src/handlers.rs`, and middleware in `src/middleware.rs`. Image operations are grouped under `src/processing/`, with cache logic in `src/caching/` and configuration helpers in `src/config.rs`. Integration coverage sits in `tests/`, while Docker assets support container runs.

## Architecture Overview
An Axum router backed by `AppState` coordinates configuration, caches, and metrics. Middleware handles logging, rate enforcement, and signature checks before dispatching to handlers. `src/fetch.rs` pulls source assets, `src/processing/` applies transforms, and `src/caching/` stores reusable outputs. `src/monitoring.rs` exports Prometheus metrics; extend it when adding endpoints.

## Build, Test, and Development Commands
- `cargo check` — fast compile-time validation before pushing changes.
- `cargo fmt` and `cargo fmt -- --check` — format Rust sources and verify formatting in CI.
- `cargo clippy --all-targets -- -D warnings` — run lints; treat warnings as build failures.
- `IMGFORGE_KEY=... IMGFORGE_SALT=... cargo run` — launch the server locally on port 3000.
- `cargo test --all -- --test-threads=1` — execute the full test suite with deterministic threading.

## Coding Style & Naming Conventions
Use four-space indentation and rely on `rustfmt` for layout. Keep modules and functions in `snake_case`, public types in `PascalCase`, and constants/envars in `UPPER_SNAKE_CASE` (e.g., `IMGFORGE_MAX_SRC_FILE_SIZE`). Keep helpers near their domain module and favor explicit names (e.g., `AppStateBuilder`) over abbreviations.

## Testing Guidelines
Unit tests may live alongside modules under `#[cfg(test)]`, but integration coverage belongs in `tests/handlers_integration_tests.rs` and `_extended.rs`. Reuse the wiremock fixtures there and keep new tests self-contained. Target one case with `cargo test --test handlers_integration_tests test_status_handler_success`, and expand the extended suite when touching processing, caching, or security.

## Commit & Pull Request Guidelines
Follow the existing short, imperative commit style (`Remove unnecessary pointers`, `Fix table`). Each pull request should summarize changes, list the validation commands you ran (`cargo fmt`, `cargo clippy`, `cargo test --all -- --test-threads=1`), and link related issues. Share screenshots or sample requests only when response formats change, and call out any configuration updates.

## Security & Configuration Tips
Never commit real `IMGFORGE_KEY` or `IMGFORGE_SALT` values; load them via your shell or a `.env` excluded from git. Document any change to `IMGFORGE_ALLOW_UNSIGNED` because it alters security posture. When testing remote fetch logic, point wiremock or staging assets to `http://localhost` sources to avoid leaking traffic.
