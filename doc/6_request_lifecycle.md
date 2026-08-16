# 6. Request Lifecycle

What happens between an incoming request and the returned image, and where each failure surfaces.

```
  request                       ┌──────────────────────────────────────────┐
     │                          │ IMGFORGE_TIMEOUT wraps everything below, │
     │                          │ so a 408 can come from any stage ──▶ 408 │
     │                          └──────────────────────────────────────────┘
     ├─ 1. routing & middleware ───── rate limit ──▶ 429
     ├─ 2. parse path & authenticate ─ bad layout ──▶ 400
     │                                 bad signature/token ──▶ 403
     ├─ 3. parse options ── invalid value ──▶ 400
     ├─ 4. negotiate ────── Accept, client hints
     │                      source not allowed ──▶ 400
     │
     ├─ 5. cache lookup ─── hit ────────────────────────────┐
     │        │ miss                                        │
     ├─ 6. fetch source ─── upstream status ──▶ 400         │
     │                      over max_src_file_size ──▶ 400  │
     ├─ 7. transform ────── wrong MIME ──▶ 400              │
     │                      over max_src_resolution ──▶ 400 │
     ├─ 8. populate cache                                   │
     │                                                      │
     └─ 9. respond ◀───── validator matches ──▶ 304 ────────┘
```

## 1. Routing & middleware

The Axum router attaches a tracing span carrying the method, URI, and a request ID. When `IMGFORGE_RATE_LIMIT_PER_MINUTE` is set, a global token bucket checks capacity and returns `429 Too Many Requests` when depleted. Status-code counters increment as responses leave the service.

## 2. URL parsing & authentication

The path splits into signature, processing directives, and source segment; an invalid layout fails with `400 Bad Request`.

Unless the signature segment is the literal `unsafe`, imgforge recomputes the HMAC from `IMGFORGE_KEY` and `IMGFORGE_SALT` and returns `403 Forbidden` on a mismatch — see [Signing a URL](4_url_structure.md#signing-a-url). When `IMGFORGE_SECRET` is set, image and info endpoints additionally require `Authorization: Bearer <token>`; a missing or wrong token also returns `403 Forbidden`.

## 3. Option parsing & 4. Negotiation

Directives are parsed on top of the server's configured processing defaults, so the URL always wins over the configuration. Content negotiation then reads `Accept` and may replace the output format; client hints may supply a width or DPR the URL left open. See [Configuration](3_configuration.md).

The source URL is decoded here, `IMGFORGE_BASE_URL` is applied, and `IMGFORGE_ALLOWED_SOURCES` is checked — before the cache lookup, so a source that is no longer permitted stops being served out of the cache.

## 5. Cache lookup

With caching enabled, imgforge checks the configured backend. A hit returns the stored bytes immediately, skipping stages 6 through 8. `cache_hits_total` and `cache_misses_total` record the outcome. See [Caching](7_caching.md).

## 6. Source acquisition

Unless the `raw` option is set, the request first takes a worker permit (at most `IMGFORGE_WORKERS` image operations run at once). The source is then fetched within `IMGFORGE_DOWNLOAD_TIMEOUT` seconds.

A non-success status from the origin fails the request with `400 Bad Request` naming that status, rather than handing an error page to the decoder.

`IMGFORGE_MAX_SRC_FILE_SIZE` (or a per-request override) is enforced while the body streams, so an oversized source is abandoned mid-download — that one really does belong to the fetch.

`IMGFORGE_ALLOWED_MIME_TYPES` and `IMGFORGE_MAX_SRC_RESOLUTION` are checked in stage 7, not stage 6: the resolution check needs the dimensions, so libvips has opened the buffer and a worker slot has already been taken by the time either runs. Treat them as limits on what gets *processed*, not as a barrier in front of the decoder. The exception is `raw` and a matching `skip_processing`, which never enter stage 7 at all — those two run both checks in stage 6, before returning the source bytes, so opting out of processing is not a way around them. That holds on a cache hit as well: both limits are part of the cache key, so a tightened policy retires the entries stored under the looser one rather than being outrun by them.

A watermark named by `watermark_url` or `IMGFORGE_WATERMARK_PATH` is fetched alongside the source. Failures here return `400 Bad Request` with a short reason; see [Error Troubleshooting](8_error_troubleshooting.md).

## 7. Image transformation

Decoding, transformation, and encoding run on Tokio's blocking pool so image work never occupies the async runtime. The stages — DPR scaling, load, colour management, frame split, geometry, canvas, effects, encode — are detailed in [Image Processing Pipeline](12_image_processing_pipeline.md).

Timing splits across four metrics: `image_operation_semaphore_wait_duration_seconds` (waiting for a worker permit), `image_operation_blocking_queue_duration_seconds` (waiting for a blocking thread), `image_operation_execution_duration_seconds` (the whole blocking section), and `image_processing_duration_seconds` (the transformation itself).

`raw` and a matching `skip_processing` both return the source bytes without entering this stage at all.

## 8. Response & caching

On success the bytes are inserted into the cache; a failed write is logged but does not affect the response. imgforge replies `200 OK` with the encoded bytes, the matching `Content-Type`, and an `X-Request-ID` header for log correlation.

The delivery headers are attached here — `Cache-Control`, `ETag`, `Last-Modified`, the canonical `Link`, `Vary`, and the CORS origin, each when configured. `Vary` lists whichever request headers the response actually depends on: `Accept` when format negotiation is enabled, and `Sec-CH-Width`, `Width`, `Sec-CH-DPR` and `DPR` when client hints are. With negotiation off and hints on it therefore carries no `Accept` at all.

Either validator can short-circuit the response with `304 Not Modified` and no body: an `If-None-Match` that matches the `ETag`, or — when the request sends no `If-None-Match` — an `If-Modified-Since` at or after the `Last-Modified` being sent. The date is compared chronologically rather than as text, so a client whose copy is newer than the origin's timestamp still gets its `304`. `If-None-Match` takes precedence when both are present, so a request carrying a stale entity tag receives the body even if its date would have matched. The bytes were produced either way, so the saving is bandwidth rather than work.

## 9. Metrics & logging

Fetch durations feed `source_image_fetch_duration_seconds` and `source_images_fetched_total`, labelled by outcome. `/metrics` exposes every counter and histogram — see [Prometheus Monitoring](11_prometheus_monitoring.md).

## Error pathways

| Response                  | Cause                                                                       |
| ------------------------- | --------------------------------------------------------------------------- |
| `304`                     | A validator matched: `If-None-Match` against the `ETag` (requires `IMGFORGE_USE_ETAG`), or an `If-Modified-Since` at or after `Last-Modified` (requires `IMGFORGE_LAST_MODIFIED_ENABLED`). Either alone is enough. |
| `403`                     | Invalid signature, an unsigned URL while unsigned mode is off, or a missing/invalid bearer token. |
| `404`                     | The URL's `expires` timestamp has passed.                                                        |
| `400`                     | Invalid path, invalid option value, source rejected by a limit or by `IMGFORGE_ALLOWED_SOURCES`, a non-success status from the origin, an output format this libvips cannot encode, or a failed watermark fetch. Body carries a plain-text reason; `IMGFORGE_DEVELOPMENT_ERRORS_MODE` appends the underlying error. |
| `408`                     | The request exceeded `IMGFORGE_TIMEOUT`. The timeout layer wraps the whole router, so this covers the source fetch, the wait for a worker slot, and a watermark fetch as readily as the transform itself — a `408` is not by itself evidence that processing was slow. imgforge never returns `504` itself; that comes from a proxy in front of it. |
| `429`                     | Rate limiter depleted.                                                       |
| `500`                     | Unhandled error; logged at `error` level with context.                       |

A source download that exceeds `IMGFORGE_DOWNLOAD_TIMEOUT` returns `400`, not a timeout status — that limit belongs to the fetch, not to the request.
