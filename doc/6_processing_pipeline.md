# 6. Processing Pipeline

Understanding imgforge’s internal workflow helps you reason about performance, error handling, and where to add observability. Each request flows through the following stages:

## 1. Routing & middleware

1. **Ingress** – Axum receives the HTTP request and attaches tracing spans (`TraceLayer`).
2. **Rate limiting** – If `IMGFORGE_RATE_LIMIT_PER_MINUTE` is set, the `rate_limit_middleware` checks the global token bucket and returns `429` when capacity is exhausted.
3. **Status code metrics** – Outgoing responses increment the `status_codes_total` Prometheus counter.

## 2. URL parsing & authentication

1. **Path parsing** – `imgforge::url::parse_path` splits the signature, processing options, and source URL. Invalid paths yield `400 Bad Request`.
2. **Signature validation** – Unless the signature is `unsafe`, `validate_signature` recomputes the HMAC using `IMGFORGE_KEY` and `IMGFORGE_SALT`. Failure yields `403 Forbidden`.
3. **Bearer token** – When `IMGFORGE_SECRET` is configured, the handler checks the `Authorization: Bearer` header before continuing.

## 3. Cache lookup

With caching enabled, imgforge hashes the full request path and checks the configured backend:

- **Memory cache**: O(1) lookup backed by Foyer’s in-memory store.
- **Disk / Hybrid cache**: Asynchronous lookup using Foyer’s block engine.
- Cache hits short-circuit the pipeline, returning cached bytes immediately. Metrics: `cache_hits_total`, `cache_misses_total`.

## 4. Source acquisition

1. **Permit acquisition** – Unless the `raw` option is set, the handler acquires a semaphore permit (up to `IMGFORGE_WORKERS` concurrent jobs).
2. **Download** – `fetch::fetch_image` uses `reqwest` to download the source within `IMGFORGE_DOWNLOAD_TIMEOUT` seconds.
3. **Validation** – The handler enforces:
   - File size (`IMGFORGE_MAX_SRC_FILE_SIZE` or per-request override).
   - MIME type allowlist (`IMGFORGE_ALLOWED_MIME_TYPES`).
   - Resolution limit via libvips metadata (`IMGFORGE_MAX_SRC_RESOLUTION`).
4. **Watermark assets** – If the request specifies `watermark_url` or the server sets `IMGFORGE_WATERMARK_PATH`, the watermark image is fetched or read from disk.

Failures at this stage return `400 Bad Request` with descriptive messages (see [8_error_troubleshooting.md](8_error_troubleshooting.md)).

## 5. Option parsing

`processing::options::parse_all_options` consumes the directives and builds a `ParsedOptions` struct. Invalid arguments (e.g., non-numeric width) produce `400 Bad Request` responses.

## 6. Image transformation

Inside `processing::process_image`:

1. **DPR scaling** adjusts configured dimensions and padding.
2. **Image load** – libvips loads the source buffer into memory (`VipsImage::new_from_buffer`).
3. **EXIF correction** – Auto-rotation runs by default.
4. **Transform chain** – Cropping, resizing, extending, padding, rotation, blur/sharpen/pixelate, zoom, minimum dimension checks, and watermarking execute in a deterministic order.
5. **Background flattening** – JPEG outputs combine with the requested background color.
6. **Encode** – The image is saved with the chosen format and quality via `processing::save`.

Processing duration is recorded in the `image_processing_duration_seconds` histogram and increments `processed_images_total`.

## 7. Response & caching

1. **Cache populate** – On success, the rendered bytes are inserted into the cache backend (if configured). Errors during caching are logged but do not affect the response.
2. **Response composition** – The handler returns `200 OK` with the processed bytes and appropriate `Content-Type`. Additional headers include `X-Request-ID` (for debugging) and any inherited headers from source validation.

## 8. Metrics & logging

- Fetch durations feed the `source_image_fetch_duration_seconds` histogram and the `source_images_fetched_total` counter (success/error labels).
- Request spans include method, URI, and a random request ID, making it easy to correlate logs and traces.
- `/metrics` aggregates all counters and histograms for scraping.

## Error pathways

- **Signature / auth** – Returns `403 Forbidden`.
- **Invalid options** – `400 Bad Request` with a plain-text reason string.
- **Timeouts** – `504` for processing timeout, `408` if the upstream proxy times out first, or `400` when the download timeout triggers.
- **Unhandled errors** – Logged at `error` level and surfaced as `500 Internal Server Error`.

Understanding each stage helps you tune performance ([9_performance.md](9_performance.md)), design monitoring dashboards, and debug issues quickly.
