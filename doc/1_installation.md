# 1. Installation

Run imgforge from the published container, or build it natively if you need to modify it.

## Container setup (recommended)

The container is also how imgforge is meant to run in production, so what you test locally matches what you deploy.

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
     ghcr.io/imgforger/imgforge:latest
   ```
5. **Persist cache data (optional)** – Mount a volume when using disk or hybrid caching:
   ```bash
   docker run -d \
     -p 3000:3000 \
     -v imgforge-cache:/var/cache/imgforge \
     -e IMGFORGE_KEY=<generated_key> \
     -e IMGFORGE_SALT=<generated_salt> \
     -e IMGFORGE_CACHE_TYPE=hybrid \
     -e IMGFORGE_CACHE_MEMORY_CAPACITY=1000 \
     -e IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge \
     -e IMGFORGE_CACHE_DISK_CAPACITY=1000 \
     ghcr.io/imgforger/imgforge:latest
   ```

Continue to [Quick Start](2_quickstart.md) to run real transformations inside the container.

## Native installation (for custom builds)

If you need to extend imgforge or integrate it directly on a host, install the Rust toolchain and libvips locally.

### Supported platforms

imgforge targets Linux and macOS. It also runs inside containers built from Debian- or Alpine-based images as long as libvips is available. Windows development is possible through WSL2.

### Prerequisites

| Requirement      | Minimum | Notes                                                                                                       |
| ---------------- | ------- | ----------------------------------------------------------------------------------------------------------- |
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

### Toolchain configuration

Set the project's preferred toolchain (optional but recommended):

```bash
rustup override set stable
```

Check that libvips is discoverable:

```bash
pkg-config --modversion vips
```

If the command fails, ensure the libvips development package is installed and `PKG_CONFIG_PATH` includes its `.pc` file directory.

### Building from source

```bash
cargo build            # debug
cargo build --release  # optimized, lands in target/release/imgforge
```

The first build downloads every crate in `Cargo.lock` and takes a few minutes; later builds are incremental.

### Verifying runtime dependencies

Before running the server, confirm that libvips can load dynamic modules:

```bash
ldd target/release/imgforge | grep vips
```

If libvips is marked as “not found,” add its library directory to `LD_LIBRARY_PATH` (Linux) or `DYLD_LIBRARY_PATH` (macOS) or install the runtime package (e.g. `libvips`).

## Use imgforge as a library

imgforge ships as both an HTTP server and a Rust crate, so you can embed the processing engine without starting the Axum server:

```rust
use imgforge::{config::Config, Imgforge};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Provide your HMAC key and salt (hex encoded, truncated for brevity)
    let mut config = Config::with_hex_keys("deadbeef", "cafebabe")?;
    config.allow_unsigned = true;

    // Optional: configure caches programmatically with CacheConfig
    let imgforge = Imgforge::new(config, None).await?;

    let result = imgforge
        .process_path("unsafe/resize:fit:300:300/plain/https://example.com/image.jpg")
        .await?;

    std::fs::write("resized.jpg", result.bytes)?;
    Ok(())
}
```

The same API exposes metadata through `image_info` and accepts signed paths exactly as the HTTP interface does.

### Next steps

[Quick Start](2_quickstart.md) – configure secrets and run your first transformation.
