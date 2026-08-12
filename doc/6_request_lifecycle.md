# 6. Request Lifecycle

What happens between an incoming request and the returned image, and where each failure surfaces.

```
  request
     │
     ├─ 1. routing & middleware ───── rate limit ──▶ 429
     ├─ 2. parse path & authenticate ─ bad layout ──▶ 400
     │                                 bad signature/token ──▶ 403
     │
     ├─ 3. cache lookup ─── hit ────────────────────────────┐
     │        │ miss                                        │
     ├─ 4. fetch source ─── too large / wrong MIME ──▶ 400  │
     ├─ 5. parse options ── invalid value ──▶ 400           │
     ├─ 6. transform ────── over IMGFORGE_TIMEOUT ──▶ 408   │
     ├─ 7. populate cache                                   │
     │                                                      │
     └─ 8. respond ◀────────────────────────────────────────┘
```

## 1. Routing & middleware

The Axum router attaches a tracing span carrying the method, URI, and a request ID. When `IMGFORGE_RATE_LIMIT_PER_MINUTE` is set, a global token bucket checks capacity and returns `429 Too Many Requests` when depleted. Status-code counters increment as responses leave the service.

## 2. URL parsing & authentication

The path splits into signature, processing directives, and source segment; an invalid layout fails with `400 Bad Request`.

Unless the signature segment is the literal `unsafe`, imgforge recomputes the HMAC from `IMGFORGE_KEY` and `IMGFORGE_SALT` and returns `403 Forbidden` on a mismatch — see [Signing a URL](4_url_structure.md#signing-a-url). When `IMGFORGE_SECRET` is set, image and info endpoints additionally require `Authorization: Bearer <token>`; a missing or wrong token also returns `403 Forbidden`.

## 3. Cache lookup

With caching enabled, imgforge hashes the full request path and checks the configured backend. A hit returns the stored bytes immediately, skipping stages 4 through 7. `cache_hits_total` and `cache_misses_total` record the outcome. See [Caching](7_caching.md).

## 4. Source acquisition

Unless the `raw` option is set, the request first takes a worker permit (at most `IMGFORGE_WORKERS` image operations run at once). The source is then fetched within `IMGFORGE_DOWNLOAD_TIMEOUT` seconds.

`IMGFORGE_MAX_SRC_FILE_SIZE` (or a per-request override) is enforced while the body streams, so an oversized source is abandoned mid-download. `IMGFORGE_ALLOWED_MIME_TYPES` and `IMGFORGE_MAX_SRC_RESOLUTION` are checked at the start of stage 6 instead — the resolution check needs the dimensions, so libvips has already opened the buffer by then. Treat them as limits on what gets *processed*, not as a barrier in front of the decoder.

A watermark named by `watermark_url` or `IMGFORGE_WATERMARK_PATH` is fetched alongside the source. Failures here return `400 Bad Request` with a short reason; see [Error Troubleshooting](8_error_troubleshooting.md).

## 5. Option parsing

Directives are parsed into a structured plan. Out-of-range values, invalid booleans, and malformed numbers return `400 Bad Request`. Unrecognised option *names* are logged at debug level and ignored, so a typo silently drops that transformation. The full catalogue is in [Processing Options](5_processing_options.md).

## 6. Image transformation

Decoding, transformation, and encoding run on Tokio's blocking pool so image work never occupies the async runtime. The stages — DPR scaling, load and EXIF orientation, geometry, canvas, effects, encode — are detailed in [Image Processing Pipeline](12_image_processing_pipeline.md).

Timing splits across four metrics: `image_operation_semaphore_wait_duration_seconds` (waiting for a worker permit), `image_operation_blocking_queue_duration_seconds` (waiting for a blocking thread), `image_operation_execution_duration_seconds` (the whole blocking section), and `image_processing_duration_seconds` (the transformation itself).

## 7. Response & caching

On success the bytes are inserted into the cache; a failed write is logged but does not affect the response. imgforge replies `200 OK` with the encoded bytes, the matching `Content-Type`, and an `X-Request-ID` header for log correlation.

## 8. Metrics & logging

Fetch durations feed `source_image_fetch_duration_seconds` and `source_images_fetched_total`, labelled by outcome. `/metrics` exposes every counter and histogram — see [Prometheus Monitoring](11_prometheus_monitoring.md).

## Error pathways

| Response                  | Cause                                                                       |
| ------------------------- | --------------------------------------------------------------------------- |
| `403`                     | Invalid signature, an unsigned URL while unsigned mode is off, or a missing/invalid bearer token. |
| `400`                     | Invalid path, invalid option value, source rejected by a limit, or a failed watermark fetch. Body carries a plain-text reason. |
| `408`                     | The request exceeded `IMGFORGE_TIMEOUT`. imgforge never returns `504` itself — that comes from a proxy in front of it. |
| `429`                     | Rate limiter depleted.                                                       |
| `500`                     | Unhandled error; logged at `error` level with context.                       |

A source download that exceeds `IMGFORGE_DOWNLOAD_TIMEOUT` returns `400`, not a timeout status — that limit belongs to the fetch, not to the request.
