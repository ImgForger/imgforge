# Usage

This guide covers the runtime interface for imgforge: available endpoints, URL composition, request signing, and operational concerns such as caching and security.

## Running the server

Start imgforge using Cargo or the prebuilt binary:

```bash
IMGFORGE_KEY=<hex-key> \
IMGFORGE_SALT=<hex-salt> \
./target/release/imgforge
```

Override configuration with the environment variables described in [doc/configuration.md](configuration.md). Logs are emitted via [`tracing`](https://docs.rs/tracing) and respect `IMGFORGE_LOG_LEVEL`.

## HTTP endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/status` | Health check returning `{ "status": "ok" }` and a request identifier in `X-Request-ID`. |
| `GET` | `/info/{signature/...}` | Fetches the source image, validates the signature, and returns metadata (width, height, format). |
| `GET` | `/{signature/...}` | Main image processing endpoint. Applies transformations described by the path. |
| `GET` | `/metrics` | Prometheus scrape endpoint exposing request, cache, and image-processing metrics. |

All endpoints honor `Authorization: Bearer <token>` when `IMGFORGE_SECRET` is set.

## URL anatomy

imgforge closely mirrors imgproxy’s path format:

```
http://<host>/<signature>/<processing_options>/plain/<source_url>@<extension>
http://<host>/<signature>/<processing_options>/<base64_source>.<extension>
```

- **Signature** – An HMAC-SHA256 signature over the path (see below). Use `unsafe` only when `IMGFORGE_ALLOW_UNSIGNED=true`.
- **Processing options** – Slash-separated directives such as `resize:fill:800:600/quality:85/blur:2`. See [doc/processing-options.md](processing-options.md).
- **Source URL** – Either:
  - `plain/<percent-encoded-url>` when the source is URL-encoded; append `@format` to request an output format.
  - `<base64url-no-pad-encoded-url>` optionally followed by `.<format>`.
- **Extension / format** – Optional output format (e.g. `webp`, `avif`, `jpeg`). If omitted, the original format is kept.

### Generating signatures

Signatures are computed over the portion of the path after the first slash. imgforge decodes `IMGFORGE_KEY` and `IMGFORGE_SALT` from hex strings and feeds both into HMAC-SHA256; the resulting digest is base64 URL-safe encoded without padding.

```python
import base64
import hmac
import hashlib

key = bytes.fromhex("<hex-key>")
salt = bytes.fromhex("<hex-salt>")
path = "/resize:fill:800:600/plain/https://example.com/cat.jpg@webp"

mac = hmac.new(key, salt + path.encode("utf-8"), hashlib.sha256)
signature = base64.urlsafe_b64encode(mac.digest()).rstrip(b"=").decode()

print(f"{signature}{path}")
# -> <signature>/resize:fill:800:600/plain/https://example.com/cat.jpg@webp
```

Embed the generated signature in the request URL: `http://localhost:3000/<signature>/resize:…`.

### Using unsigned URLs

When `IMGFORGE_ALLOW_UNSIGNED=true`, the server accepts paths starting with `unsafe/…`. This is useful for development but should remain disabled in production.

```bash
curl "http://localhost:3000/unsafe/resize:fit:600:0/plain/https://example.com/dog.jpg" --output dog.jpg
```

### Source URL encoding tips

| Mode | When to use | Example |
| --- | --- | --- |
| `plain` | Simple ASCII URLs. Remember to percent-encode query strings. | `/plain/https://example.com/image.jpg@webp` |
| Base64 | Binary-heavy or already signed source URLs. Encode without padding using URL-safe alphabet. | `/aHR0cHM6Ly9leGFtcGxlLmNvbS9pbWFnZS5qcGc.webp` |

## Common operations

- **Resize & format conversion**:

  ```bash
  curl "http://localhost:3000/${SIG}/resize:fill:800:600/quality:85/plain/https://example.com/cat.jpg@avif" --output cat.avif
  ```

- **Crop and watermark**:

  ```bash
  curl "http://localhost:3000/${SIG}/crop:100:100:500:500/watermark:0.3:south_east/watermark_url:${WM_URL}/plain/${SRC}" --output cropped.png
  ```
  where `${WM_URL}` is the Base64 URL-safe encoded watermark image URL.

- **Context-aware DPR scaling**: specify `dpr:2` to scale width/height in high-DPI contexts.

See [doc/processing-options.md](processing-options.md) for the full catalog.

## Caching semantics

If caching is enabled (memory/disk/hybrid), imgforge stores rendered binaries keyed by the full request path. Any change to processing options or output format generates a new cache key. Cache hits and misses are exported via Prometheus metrics (`cache_hits_total`, `cache_misses_total`) labeled by backend type.

- Memory cache suits high-throughput, ephemeral deployments.
- Disk and hybrid caches require persistent volumes.
- Use the `cache_buster:<token>` option to invalidate specific keys on demand.

## Security considerations

- Keep `IMGFORGE_ALLOW_UNSIGNED=false` in production.
- Rotate `IMGFORGE_KEY`/`IMGFORGE_SALT` periodically and treat them as secrets.
- Enable the global rate limiter (`IMGFORGE_RATE_LIMIT_PER_MINUTE`) when operating on the public internet.
- Require `Authorization: Bearer` via `IMGFORGE_SECRET` when routing through shared infrastructure.
- Limit source image origins at the reverse proxy or networking layer when possible.

## Observability

- `/metrics` exposes counters and histograms for request duration, processing time, cache performance, and source fetch outcomes. Scrape it with Prometheus or any compatible collector.
- `/status` is a fast readiness probe.
- Logs include a per-request identifier; forward them to your logging stack for correlation.

For deployment patterns and production advice, continue to [doc/deployment.md](deployment.md).
