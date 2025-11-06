# 1. Installation

imgforge is a Rust application that wraps Axum, Tokio, and libvips into a standalone image proxy. You can run it entirely inside Docker or install the native toolchain for bespoke builds.

## Container-first setup (recommended)

Docker is the fastest way to evaluate imgforge and mirrors the production deployment model.

1. **Install Docker** – Version 24 or newer is recommended. Podman works as an alternative as long as it supports multi-stage builds.
2. **Pull the image** – Use the published container from GitHub Container Registry:
   ```bash
   docker pull ghcr.io/imgforger/imgforge:latest
   ```
   If you need a custom image (for example to bundle watermarks or presets), see [Deployment](10.2_deployment_manual.md) for building a derivative image.
3. **Generate secrets** – Generate random secrets for HMAC signing:
   ```bash
   openssl rand -hex 32
   ```
4. **Start a container** – Provide HMAC secrets via environment variables or an env file:
   ```bash
   docker run \
     --rm \
     -p 3000:3000 \
     -e IMGFORGE_KEY=<generated_key> \
     -e IMGFORGE_SALT=<generated_salt> \
     -e IMGFORGE_ALLOW_UNSIGNED=true \
     ghcr.io/imgforger/imgforge:latest --help
   ```
5. **Persist cache data (optional)** – Mount a volume when using disk or hybrid caching:
   ```bash
   docker run -d \
     -p 3000:3000 \
     -v imgforge-cache:/var/cache/imgforge \
     -e IMGFORGE_KEY=<generated_key> \
     -e IMGFORGE_SALT=<generated_salt> \
     -e IMGFORGE_CACHE_MODE=hybrid \
     -e IMGFORGE_CACHE_MEMORY_CAPACITY=1000
     -e IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge \
     -e IMGFORGE_CACHE_DISK_CAPACITY=1000 \
     ghcr.io/imgforger/imgforge:latest
   ```

Continue to [Quick Start](2_quick_start.md) to run real transformations inside the container.

## Native installation (for custom builds)

If you need to extend imgforge or integrate it directly on a host, install the Rust toolchain and libvips locally.

### Supported platforms

imgforge targets Linux and macOS. It also runs inside containers built from Debian- or Alpine-based images as long as libvips is available. Windows development is possible through WSL2.

### Prerequisites

| Requirement      | Minimum | Notes                                                                                                       |
|------------------|---------|-------------------------------------------------------------------------------------------------------------|
| Rust toolchain   | 1.90    | Install via [rustup](https://rustup.rs/). Ensure `cargo`, `rustc`, and `rustfmt` are on your `PATH`.        |
| libvips          | 8.12+   | Provides the core image processing primitives. Both development headers and runtime libraries are required. |
| pkg-config       | —       | Required for cargo to discover libvips. Usually bundled on Linux; install explicitly on macOS.              |
| OpenSSL          | 1.1+    | Used by reqwest and HMAC signing utilities. Provided by default on most systems.                            |
| Optional: Docker | 24+     | Useful for running parity tests against the container image.                                                |

#### Installing prerequisites

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

### Fetching the source

```bash
git clone https://github.com/imgforger/imgforge.git
cd imgforge
```

If you are working from a fork, replace the URL accordingly. The repository uses Git submodules only for documentation assets, so a normal clone is sufficient.

### Toolchain configuration

Set the project’s preferred toolchain (optional but recommended):

```bash
rustup override set stable
```

Check that libvips is discoverable:

```bash
pkg-config --modversion vips
```

If the command fails, ensure the libvips development package is installed and `PKG_CONFIG_PATH` includes its `.pc` file directory.

### Building from source

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

### Verifying runtime dependencies

Before running the server, confirm that libvips can load dynamic modules:

```bash
ldd target/release/imgforge | grep vips
```

If libvips is marked as “not found,” add its library directory to `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS) or install the runtime package (e.g. `libvips`).

### Next steps

Whether you chose Docker or a native build, proceed to [Quick Start](2_quickstart.md) to configure secrets, start the server, and perform your first image transformation.
