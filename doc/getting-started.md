# Getting Started

Welcome to **imgforge** – a Rust-based, libvips-powered image proxy inspired by [`imgproxy`](https://github.com/imgproxy/imgproxy). This guide walks you through setting up a development environment, building the server, and issuing your first transformation request.

## Prerequisites

- **Rust**: version 1.90 or newer (the Dockerfile pins to `rust:1.90`). Install via [rustup](https://rustup.rs/) if you do not have it yet.
- **System libraries**: libvips and pkg-config headers are required at build and runtime.
  - Debian/Ubuntu: `sudo apt-get install libvips-dev libvips pkg-config`  
  - macOS (Homebrew): `brew install vips`
- **Optional**: Docker 24+ for container builds, `docker compose` for orchestration.

## Cloning the repository

```bash
git clone https://github.com/your-org/imgforge.git
cd imgforge
```

## Running from source

1. **Create secrets**: imgforge requires an HMAC signing key and salt, both provided as hex-encoded byte strings.

   ```bash
   openssl rand -hex 32  # Repeat twice for key and salt
   ```

2. **Configure environment variables** (export them directly, or use a `.env` file as shown below):

   ```bash
   export IMGFORGE_KEY="<hex-key>"
   export IMGFORGE_SALT="<hex-salt>"
   export IMGFORGE_ALLOW_UNSIGNED=true             # Optional, useful for quick tests
   ```

3. **Build and run** the server:

   ```bash
   cargo run
   ```

   The server listens on `http://0.0.0.0:3000` by default. Adjust the bind address via `IMGFORGE_BIND`.

### Using a `.env` file

imgforge reads configuration from the process environment. To avoid exporting variables manually, place them in a `.env` file and run the server through a helper such as [`dotenvx`](https://github.com/dotenvx/dotenvx) or by sourcing the file yourself.

```env
# .env (example)
IMGFORGE_KEY=9f1d2c2b5af0a9d9b4850f917fe55117808a2b7f82cb05ad7f2a3082d689f942
IMGFORGE_SALT=d9bc6d5c9d8d4f8a19991bc185e90c42f5ce23246c9318680c9f8b3fa76bcdf1
IMGFORGE_ALLOW_UNSIGNED=true
IMGFORGE_LOG_LEVEL=info
```

Run with:

```bash
dotenvx run -- cargo run
# or
source .env && cargo run
```

## Docker workflow

The repository ships with a multi-stage Dockerfile that installs libvips and compiles the release binary.

```bash
# Build the image
docker build -t imgforge:latest .

# Run the container (bind port 3000 and pass required secrets)
docker run \
  -p 3000:3000 \
  -e IMGFORGE_KEY=<hex-key> \
  -e IMGFORGE_SALT=<hex-salt> \
  imgforge:latest
```

Alternatively, adapt `docker-compose.yml` to your environment and run `docker compose up`.

## Quick start request

With the server running locally and `IMGFORGE_ALLOW_UNSIGNED=true`, you can issue an unsigned (development-only) request that resizes an external image and converts it to WebP:

```bash
curl "http://localhost:3000/unsafe/resize:fill:400:300/plain/https://images.unsplash.com/photo-1529626455594-4ff0802cfb7e@webp" \
  --output portrait.webp
```

Key elements of the URL:

- `unsafe` disables signature verification (requires the server to allow unsigned URLs).
- `resize:fill:400:300` instructs imgforge to crop and fill to 400×300.
- `plain/<source>@webp` supplies the original image as a plain URL and requests WebP as the output format.

For production, keep `IMGFORGE_ALLOW_UNSIGNED` disabled and sign URLs as described in [doc/usage.md](usage.md).

## Next steps

- Explore configuration options in [doc/configuration.md](configuration.md).
- Learn how to sign URLs and compose advanced requests in [doc/usage.md](usage.md) and [doc/processing-options.md](processing-options.md).
- Ready to deploy? See [doc/deployment.md](deployment.md).
