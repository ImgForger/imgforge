# imgforge

imgforge is a fast, secure image proxy and transformation server written in Rust. Built with Rust and libvips, it delivers imgproxy-compatible URL semantics with an async-first architecture and optional, pluggable caching backends.

## Why choose imgforge

- **Production-ready from day one** – Health checks, structured logging, and Prometheus metrics make imgforge easy to drop into modern platforms.
- **Container-native** – Ship the provided multi-stage Docker image anywhere, or extend it with your own watermark assets and presets.
- **High-fidelity transforms** – Resize, crop, format-convert, blur, sharpen, watermark, and more—powered by libvips for incredible performance.
- **Defense in depth** – Signed URLs, bearer tokens, per-request safeguards, and global rate limiting protect your origins from abuse.

## Feature highlights

- On-the-fly resizing, cropping, format conversion, watermarking, and other libvips-backed transforms.
- Signed URL enforcement with optional bearer authentication and rate limiting.
- Memory, disk, and hybrid caches powered by [Foyer](https://foyer-rs.github.io/foyer/) for efficient content reuse.
- Prometheus metrics, structured tracing, and health endpoints suitable for production observability.

## Get started in minutes

```bash
docker pull ghcr.io/imgforger/imgforge:latest
docker run --rm -p 3000:3000 \
  -e IMGFORGE_KEY=$(openssl rand -hex 32) \
  -e IMGFORGE_SALT=$(openssl rand -hex 32) \
  ghcr.io/imgforger/imgforge:latest
```

Then follow the [Quick Start guide](doc/2_quick_start.md) to sign URLs and try your first transformation. Prefer bare-metal builds or CI integrations? See [Installation](doc/1_installation.md) for native toolchain instructions.

## Documentation

- [Introduction](doc/introduction.md)
- [Quick Start](doc/2_quick_start.md)
- [Installation](doc/1_installation.md)
- [URL structure and signing](doc/4_url_structure.md)
- [Processing options reference](doc/5_processing_options.md)
- [Request lifecycle overview](doc/6_request_lifecycle.md)
- [Image processing pipeline deep dive](doc/12_image_processing_pipeline.md)
- [Prometheus monitoring playbooks](doc/11_prometheus_monitoring.md)

Browse the full documentation set under [`doc/`](doc/).

## Community

Issues and pull requests are welcome. Please review the [contributing guide](CONTRIBUTING.md) before submitting significant changes. If you are upgrading from imgproxy, most existing URL builders will continue to work—consult the processing and URL references for the few imgforge-specific differences.
