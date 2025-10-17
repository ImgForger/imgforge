# 2. Quick Start

This walkthrough launches imgforge locally, configures the required secrets, and exercises the public endpoints. It assumes you have completed the steps in [1_installation.md](1_installation.md).

## Minimal configuration

imgforge signs every request with an HMAC computed from `IMGFORGE_KEY` and `IMGFORGE_SALT`. Generate development-only values with OpenSSL:

```bash
export IMGFORGE_KEY=$(openssl rand -hex 32)
export IMGFORGE_SALT=$(openssl rand -hex 32)
```

For early experiments you may also allow unsigned URLs:

```bash
export IMGFORGE_ALLOW_UNSIGNED=true
```

> **Production reminder:** Leave `IMGFORGE_ALLOW_UNSIGNED` unset (or `false`) outside of local development. See [4_url_structure.md](4_url_structure.md) for signing guidance.

## Starting the server

### Via Cargo

```bash
cargo run
```

By default the server listens on `http://0.0.0.0:3000`. Adjust the bind address with `IMGFORGE_BIND`.

### Via Docker

```bash
docker run \
  --rm \
  -p 3000:3000 \
  -e IMGFORGE_KEY \
  -e IMGFORGE_SALT \
  -e IMGFORGE_ALLOW_UNSIGNED=true \
  imgforge:latest
```

Use `--env-file` to load additional configuration. When deploying, replace the development values with secrets from your vault.

## Issuing a transformation request

With the server running, craft a development URL that resizes and converts an image to WebP:

```bash
curl "http://localhost:3000/unsafe/resize:fill:600:400/plain/https://images.unsplash.com/photo-1529626455594-4ff0802cfb7e@webp" \
  --output portrait.webp
```

- `unsafe` bypasses signature validation (allowed because `IMGFORGE_ALLOW_UNSIGNED=true`).
- `resize:fill:600:400` resizes and crops the source to match the target aspect ratio.
- `@webp` triggers format conversion.

Open `portrait.webp` in your image viewer to confirm the result.

## Inspecting available endpoints

| Endpoint          | Description                                                                                                             |
|-------------------|-------------------------------------------------------------------------------------------------------------------------|
| `GET /status`     | Returns `{ "status": "ok" }` and an `X-Request-ID` header. Integrate this into liveness/readiness probes.               |
| `GET /info/{...}` | Validates the URL signature, downloads the source image, and responds with JSON metadata (`width`, `height`, `format`). |
| `GET /{...}`      | Full processing endpoint. The path encodes processing options and the source URL.                                       |
| `GET /metrics`    | Exposes Prometheus metrics (request latency, processing duration, cache statistics, status code counters).              |

If `IMGFORGE_SECRET` is set, include `Authorization: Bearer <token>` on `/info` and image requests.

## Reviewing logs and metrics

Logs are emitted via the `tracing` subscriber. Set `IMGFORGE_LOG_LEVEL=imgforge=debug` to see detailed request flow. For metrics:

```bash
curl http://localhost:3000/metrics | head
```

Look for buckets such as `image_processing_duration_seconds` and counters like `processed_images_total`.

## Next steps

- Understand configuration knobs in [3_configuration.md](3_configuration.md).
- Learn how URLs are structured and signed in [4_url_structure.md](4_url_structure.md).
- Explore the full list of processing directives in [5_processing_options.md](5_processing_options.md).
- Review the internal request lifecycle in [6_processing_pipeline.md](6_processing_pipeline.md).
