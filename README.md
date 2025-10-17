# imgforge

imgforge is a fast, secure image proxy and transformation server written in Rust. Built on top of Axum, Tokio, and libvips, it delivers imgproxy-compatible URL semantics with an async-first architecture and optional, pluggable caching backends.

## Highlights

- On-the-fly resizing, cropping, format conversion, watermarking, and other libvips-backed transforms.
- Signed URL enforcement with optional bearer authentication and rate limiting.
- Memory, disk, and hybrid caches powered by [Foyer](https://docs.rs/foyer) for efficient content reuse.
- Prometheus metrics, structured tracing, and health endpoints suitable for production observability.

## Documentation

A numbered documentation suite lives under [`doc/`](doc/). Start with `1_installation.md` and follow the sequence to move from local setup through production deployment, advanced tuning, and contribution guidelines.

If you are upgrading from imgproxy, most existing URL builders will continue to work. Consult the processing and URL references in the docs for the few imgforge-specific differences.

## Community

Issues and pull requests are welcome. Please review the contributing guide before submitting significant changes.
