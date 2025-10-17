# 1. Installation

imgforge is a Rust application that wraps Axum, Tokio, and libvips into a standalone image proxy. This document prepares your workstation or build environment to compile and run the service.

## Supported platforms

imgforge targets Linux and macOS. It also runs inside containers built from Debian- or Alpine-based images as long as libvips is available. Windows development is possible through WSL2.

## Prerequisites

| Requirement      | Minimum | Notes                                                                                                       |
|------------------|---------|-------------------------------------------------------------------------------------------------------------|
| Rust toolchain   | 1.90    | Install via [rustup](https://rustup.rs/). Ensure `cargo`, `rustc`, and `rustfmt` are on your `PATH`.        |
| libvips          | 8.12+   | Provides the core image processing primitives. Both development headers and runtime libraries are required. |
| pkg-config       | —       | Required for cargo to discover libvips. Usually bundled on Linux; install explicitly on macOS.              |
| OpenSSL          | 1.1+    | Used by reqwest and HMAC signing utilities. Provided by default on most systems.                            |
| Optional: Docker | 24+     | For container builds using the provided multi-stage Dockerfile.                                             |

### Installing prerequisites

**Debian / Ubuntu**

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libvips-dev libvips openssl ca-certificates
```

**Fedora / RHEL**

```bash
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config vips-devel openssl-devel
```

**macOS (Homebrew)**

```bash
brew install vips pkg-config openssl@3
```

> **Tip:** After installing rustup, run `rustup default stable` and `rustup component add rustfmt clippy` to match the repository tooling.

## Fetching the source

```bash
git clone https://github.com/imgforger/imgforge.git
cd imgforge
```

If you are working from a fork, replace the URL accordingly. The repository uses Git submodules only for documentation assets, so a normal clone is sufficient.

## Toolchain configuration

Set the project’s preferred toolchain (optional but recommended):

```bash
rustup override set stable
```

Check that libvips is discoverable:

```bash
pkg-config --modversion vips
```

If the command fails, ensure the libvips development package is installed and `PKG_CONFIG_PATH` includes its `.pc` file directory.

## Building from source

Compile the debug binary:

```bash
cargo build
```

The compilation downloads crates specified in `Cargo.lock`. On the first build this can take a few minutes. Subsequent builds are incremental.

Compile the optimized binary:

```bash
cargo build --release
```

The executable will be placed in `target/release/imgforge`.

## Verifying runtime dependencies

Before running the server, confirm that libvips can load dynamic modules:

```bash
ldd target/release/imgforge | grep vips
```

If libvips is marked as “not found,” add its library directory to `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS) or install the runtime package (e.g. `libvips`).

## Docker-based installation

The repository ships with a multi-stage Dockerfile that installs libvips and compiles the binary in an isolated builder image. Build the container locally:

```bash
docker build -t imgforge:latest .
```

Run the container (you will configure secrets in the quick start guide):

```bash
docker run --rm -it imgforge:latest --help
```

For production-grade usage, see [10_deployment.md](10_deployment.md).

## Next steps

Continue to [2_quick_start.md](2_quick_start.md) to configure environment variables, start the server, and perform your first image transformation.
