```asciiart
╔═════════════════════════════════════════════════════════════════════╗
║                                                                     ║
║  ██╗███╗   ███╗ ██████╗ ███████╗ ██████╗ ██████╗  ██████╗ ███████╗  ║
║  ██║████╗ ████║██╔════╝ ██╔════╝██╔═══██╗██╔══██╗██╔════╝ ██╔════╝  ║
║  ██║██╔████╔██║██║  ███╗█████╗  ██║   ██║██████╔╝██║  ███╗█████╗    ║
║  ██║██║╚██╔╝██║██║   ██║██╔══╝  ██║   ██║██╔══██╗██║   ██║██╔══╝    ║
║  ██║██║ ╚═╝ ██║╚██████╔╝██║     ╚██████╔╝██║  ██║╚██████╔╝███████╗  ║
║  ╚═╝╚═╝     ╚═╝ ╚═════╝ ╚═╝      ╚═════╝ ╚═╝  ╚═╝ ╚═════╝ ╚══════╝  ║
║                                                                     ║
║              Fast, Secure Image Transformation Server               ║
║                                                                     ║
╚═════════════════════════════════════════════════════════════════════╝
```

[![crates.io](https://img.shields.io/crates/v/imgforge.svg)](https://crates.io/crates/imgforge)
[![Build](https://github.com/ImgForger/imgforge/actions/workflows/build.yml/badge.svg)](https://github.com/ImgForger/imgforge/actions/workflows/build.yml)
[![Release](https://github.com/ImgForger/imgforge/actions/workflows/release.yml/badge.svg)](https://github.com/ImgForger/imgforge/actions/workflows/release.yml)
[![dependency status](https://deps.rs/repo/github/ImgForger/imgforge/status.svg)](https://deps.rs/repo/github/ImgForger/imgforge)

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

### One-line deployment (Recommended)

Deploy imgforge on any Linux machine with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh | bash
```

The interactive script will:
- Install Docker (if needed)
- Let you choose a caching strategy (Memory, Disk, Hybrid, or None)
- Optionally enable Prometheus + Grafana monitoring with pre-built dashboards
- Generate secure keys automatically
- Start imgforge on port 3000

See the [deployment guide](deployment/README.md) for more options.

### Manual Docker setup

Generate development-only values with OpenSSL:

```bash
openssl rand -hex 32
```

```bash
docker pull ghcr.io/imgforger/imgforge:latest
docker run --rm -p 3000:3000 \
  -e IMGFORGE_KEY=<generated_key> \
  -e IMGFORGE_SALT=<generated_salt> \
  ghcr.io/imgforger/imgforge:latest
```

Then follow the [Quick Start guide](doc/2_quick_start.md) to sign URLs and try your first transformation. Prefer bare-metal builds or CI integrations? See [Installation](doc/1_installation.md) for native toolchain instructions.

## Documentation

- [Introduction](doc/introduction.md)
- [Installation](doc/1_installation.md)
- [Quick Start](doc/2_quick_start.md)
- [URL structure and signing](doc/4_url_structure.md)
- [Processing options reference](doc/5_processing_options.md)
- [Request lifecycle overview](doc/6_request_lifecycle.md)
- [Image processing pipeline deep dive](doc/12_image_processing_pipeline.md)
- [Prometheus monitoring playbooks](doc/11_prometheus_monitoring.md)
- [K6 load testing suite](loadtest/README.md)

Browse the full documentation set under [`doc/`](doc/).

## Community

Issues and pull requests are welcome. Please review the [contributing guide](CONTRIBUTING.md) before submitting significant changes. If you are upgrading from imgproxy, most existing URL builders will continue to work—consult the processing and URL references for the few imgforge-specific differences.
