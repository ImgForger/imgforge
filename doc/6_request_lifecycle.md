# 6. Request Lifecycle

Understanding imgforge’s internal workflow helps you reason about performance, error handling, and observability. Each request flows through the following stages:


## Request flow diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          Request Lifecycle                                  │
└─────────────────────────────────────────────────────────────────────────────┘

    ┌──────────────────────┐
    │  Incoming Request    │  → HTTP GET /<sig>/<options>/<source>
    └──────────┬───────────┘
               │
               ▼
    ┌──────────────────────┐
    │ Routing & Middleware │  → Attach tracing spans
    │                      │    Rate limiting check (429 if depleted)
    └──────────┬───────────┘
               │
               ▼
    ┌──────────────────────┐
    │ URL Parsing &        │  → Split: signature | options | source
    │ Authentication       │    HMAC validation (403 if invalid)
    │                      │    Bearer token check (401 if required)
    └──────────┬───────────┘
               │
               ▼
    ┌──────────────────────┐         ┌─────────────┐
    │   Cache Lookup       │────────▶│ Cache Hit?  │─┐
    │ (Memory/Disk/Hybrid) │         └─────────────┘ │
    └──────────────────────┘                         │
               │                                     │ YES
               │ NO                                  │
               ▼                                     │
    ┌──────────────────────┐                        │
    │ Source Acquisition   │  → Acquire semaphore   │
    │                      │    Download image      │
    │                      │    Validate size/MIME  │
    │                      │    Check resolution    │
    │                      │    Fetch watermark     │
    └──────────┬───────────┘                        │
               │                                     │
               ▼                                     │
    ┌──────────────────────┐                        │
    │  Option Parsing      │  → Parse directives    │
    │                      │    Validate ranges     │
    └──────────┬───────────┘                        │
               │                                     │
               ▼                                     │
    ┌──────────────────────┐                        │
    │ Image Transformation │  → libvips pipeline    │
    │ (see Pipeline doc)   │    7-stage process     │
    └──────────┬───────────┘                        │
               │                                     │
               ▼                                     │
    ┌──────────────────────┐                        │
    │   Cache Populate     │  → Store in cache      │
    │   (on success)       │                        │
    └──────────┬───────────┘                        │
               │                                     │
               ├─────────────────────────────────────┘
               ▼
    ┌──────────────────────┐
    │ Response Composition │  → 200 OK + Content-Type
    │                      │    X-Request-ID header
    │                      │    Processed image bytes
    └──────────┬───────────┘
               │
               ▼
    ┌──────────────────────┐
    │  Metrics & Logging   │  → Record durations & counters
    │                      │    Update Prometheus metrics
    └──────────┬───────────┘
               │
               ▼
    ┌──────────────────────┐
    │   Client Receives    │  → Image delivered
    └──────────────────────┘

    Error Paths:
    • 403 Forbidden ←─────────── Signature/auth failure
    • 400 Bad Request ←────────── Invalid options, oversized, bad MIME
    • 429 Too Many Requests ←──── Rate limit exceeded
    • 504 Gateway Timeout ←────── Processing timeout
    • 500 Internal Error ←──────── Unhandled exceptions
```

## 1. Routing & middleware

1. **Ingress** – The Axum router accepts the HTTP request and attaches structured tracing spans so every hop can be correlated in logs.
2. **Rate limiting** – When `IMGFORGE_RATE_LIMIT_PER_MINUTE` is set, a global token bucket checks capacity before the request proceeds, responding with `429 Too Many Requests` when depleted.
3. **Response classification** – Status-code counters increment as responses leave the service, powering dashboards and alerts.

## 2. URL parsing & authentication

1. **Path parsing** – imgforge splits the path into signature, processing directives, and the source segment. Invalid layouts fail fast with `400 Bad Request`.
2. **Signature validation** – Unless the signature literal is `unsafe`, the server recomputes the HMAC using `IMGFORGE_KEY` and `IMGFORGE_SALT`. Mismatches return `403 Forbidden`. See the “Signing a URL” section in [URL Structure](4_url_structure.md#signing-a-url).
3. **Bearer token** – When `IMGFORGE_SECRET` is configured, image and info endpoints require `Authorization: Bearer <token>` and return `401 Unauthorized` otherwise.

## 3. Cache lookup

With caching enabled, imgforge hashes the full request path and checks the configured backend:

- **Memory cache** – Constant-time lookups backed by Foyer’s in-memory store.
- **Disk / hybrid cache** – Asynchronous lookups using Foyer’s block engine and optional persistent storage.
- Cache hits short-circuit the lifecycle, returning cached bytes immediately. Metrics such as `cache_hits_total` and `cache_misses_total` capture effectiveness.

## 4. Source acquisition

1. **Permit acquisition** – Unless the `raw` option is set, the request acquires a semaphore permit (up to `IMGFORGE_WORKERS` concurrent jobs) to contain libvips concurrency.
2. **Download** – The source image is fetched with `reqwest` within `IMGFORGE_DOWNLOAD_TIMEOUT` seconds.
3. **Validation** – imgforge enforces:
   - File size limits from `IMGFORGE_MAX_SRC_FILE_SIZE` or a per-request override.
   - MIME type allowlists via `IMGFORGE_ALLOWED_MIME_TYPES`.
   - Resolution ceilings using EXIF dimensions and `IMGFORGE_MAX_SRC_RESOLUTION`.
4. **Watermark assets** – When the URL specifies `watermark_url` or the server sets `IMGFORGE_WATERMARK_PATH`, the watermark image is fetched or read from disk alongside the source.

Failures at this stage return `400 Bad Request` with descriptive messages (see [Error Troubleshooting](8_error_troubleshooting.md)).

## 5. Option parsing

The processing directives are parsed into a structured plan. Out-of-range values, invalid booleans, or malformed numbers produce `400 Bad Request` responses. The full directive catalogue lives in [Processing Options](5_processing_options.md).

## 6. Image transformation

With a validated plan, imgforge executes the transformation chain:

1. Device-pixel-ratio scaling adjusts requested dimensions and padding.
2. libvips loads the source buffer into memory and applies EXIF orientation correction.
3. Transformations—crops, resizes, extension, padding, rotations, effects, watermarking, and background flattening—execute in a deterministic order.
4. The processed image is encoded into the requested format and quality.

For a deeper dive into the sequencing, defaults, and error surfaces inside this phase, see [Image Processing Pipeline](12_image_processing_pipeline.md).

Processing duration is recorded in the `image_processing_duration_seconds` histogram and increments `processed_images_total`.

## 7. Response & caching

1. **Cache populate** – On success, the rendered bytes are inserted into the configured cache. Failures to write are logged but do not affect the response.
2. **Response composition** – imgforge returns `200 OK` with the processed bytes and appropriate `Content-Type`. Additional headers include `X-Request-ID` for log correlation plus any validation headers inherited from earlier stages.

## 8. Metrics & logging

- Fetch durations feed `source_image_fetch_duration_seconds` and the `source_images_fetched_total` counter (labeled by outcome).
- Request spans include method, URI, and a random request ID, making it easy to correlate logs, traces, and metrics.
- `/metrics` aggregates all counters and histograms for scraping. Dashboards and alerting playbooks are documented in [Prometheus Monitoring](11_prometheus_monitoring.md).

## Error pathways

- **Signature / auth** – Returns `403 Forbidden` or `401 Unauthorized` depending on the failure.
- **Invalid options** – Returns `400 Bad Request` with a plain-text reason string.
- **Timeouts** – `504 Gateway Timeout` for processing timeouts, `408 Request Timeout` if an upstream proxy times out first, or `400 Bad Request` when the download timeout triggers.
- **Unhandled errors** – Logged at `error` level and surfaced as `500 Internal Server Error`.

Understanding each stage helps you tune performance, design observability dashboards, and debug issues quickly.
