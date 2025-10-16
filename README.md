# imgforge

imgforge is a fast, secure image proxy and transformation server written in Rust. Built on top of Axum, Tokio, and libvips, it delivers imgproxy-compatible URL semantics with an async-first architecture and optional, pluggable caching backends.

## Highlights

- On-the-fly resizing, cropping, format conversion, watermarking, and other libvips-backed transforms.
- Signed URL enforcement with optional bearer authentication and rate limiting.
- Memory, disk, and hybrid caches powered by [Foyer](https://docs.rs/foyer) for efficient content reuse.
- Prometheus metrics, structured tracing, and health endpoints suitable for production observability.

## Documentation

The detailed documentation suite lives under [`doc/`](doc/):

- [`doc/getting-started.md`](doc/getting-started.md) – Install prerequisites, build from source, and run your first request.
- [`doc/configuration.md`](doc/configuration.md) – Environment variables, cache selection, and production profiles.
- [`doc/usage.md`](doc/usage.md) – Endpoint overview, URL structure, and signing workflow.
- [`doc/processing-options.md`](doc/processing-options.md) – Comprehensive reference for every transformation option.
- [`doc/deployment.md`](doc/deployment.md) – Docker, systemd, observability, and security guidance.
- [`doc/contributing.md`](doc/contributing.md) – Development workflow and contribution guidelines.

If you are upgrading from imgproxy, most existing URL builders will continue to work. Consult the processing options reference for the few imgforge-specific differences.

## Community

Issues and pull requests are welcome. Please review the contributing guide before submitting significant changes.
