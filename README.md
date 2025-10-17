# imgforge

imgforge is a fast, secure image proxy and transformation server written in Rust. Built on top of Axum, Tokio, and libvips, it delivers imgproxy-compatible URL semantics with an async-first architecture and optional, pluggable caching backends.

## Highlights

- On-the-fly resizing, cropping, format conversion, watermarking, and other libvips-backed transforms.
- Signed URL enforcement with optional bearer authentication and rate limiting.
- Memory, disk, and hybrid caches powered by [Foyer](https://foyer-rs.github.io/foyer/) for efficient content reuse.
- Prometheus metrics, structured tracing, and health endpoints suitable for production observability.

## Documentation

Documentation is available under [`doc/`](doc/).

If you are upgrading from imgproxy, most existing URL builders will continue to work. Consult the processing and URL references in the docs for the few imgforge-specific differences.

## Community

Issues and pull requests are welcome. Please review the contributing guide before submitting significant changes.
