# 9. Performance Tips

imgforge leverages libvips and Tokio to deliver high throughput with modest resources. This guide outlines configuration tweaks, architectural patterns, and monitoring strategies to keep latency low under load.

## Tune concurrency thoughtfully

- Start with the default worker count (`num_cpus * 2`). Observe CPU utilization and memory pressure under realistic workloads.
- Increase `IMGFORGE_WORKERS` when requests are primarily I/O bound (e.g., lightweight transformations or cached responses).
- Decrease the worker count when libvips operations are heavy and cause swapping. Monitor resident set size (RSS) and libvips memory pools.

## Embrace caching

- Enable `memory` or `hybrid` caching for hot assets. See [7_caching.md](7_caching.md) for sizing guidelines.
- Warm caches proactively after deployments or cache flushes. Scripts can replay historical access logs.
- Combine imgforge with a CDN to offload repeated requests and reduce origin bandwidth.

## Optimize signatures & URLs

- Avoid generating unique cache-busting tokens unless content actually changes. Excessive churn destroys cache hit ratios and increases CPU load.
- Batch-sign URLs offline instead of per request to reduce application overhead.

## Use timeouts strategically

- Tighten `IMGFORGE_DOWNLOAD_TIMEOUT` to fail fast on unresponsive sources.
- Keep `IMGFORGE_TIMEOUT` slightly below your ingress proxy timeout to avoid double-processing.

## Monitor key metrics

Scrape `/metrics` frequently and build dashboards around:

- `image_processing_duration_seconds` (histogram) – tail latency of libvips operations.
- `http_requests_duration_seconds` – overall request latency (add your own histogram via middleware if needed).
- `processed_images_total{format=...}` – format-specific throughput.
- `cache_hits_total` / `cache_misses_total` – effectiveness of the configured cache.
- `status_codes_total` – error spikes or throttling events.

## Right-size hardware

- Favor instances with high memory bandwidth and SSD-backed storage when using disk or hybrid caches.
- Allocate headroom for libvips; operations like large resizes or watermarks can temporarily inflate memory usage.
- Pin docker containers to dedicated CPU sets (`cpuset`) when co-locating with other workloads to minimize interference.

## Use asynchronous sources

- When requesting assets from object storage or remote services, prefer nodes within the same region to reduce latency.
- Enable HTTP/2 between imgforge and upstream sources if supported—`reqwest` does this automatically—and keep connections warm.

## Instrument tracing

- Increase log verbosity (`IMGFORGE_LOG_LEVEL=imgforge=debug`) during load tests to capture timings.
- Integrate `tracing` subscribers that export spans to distributed tracing backends (e.g., OpenTelemetry) for end-to-end insight.

## Profile periodically

- Benchmark with tools like [`wrk`](https://github.com/wg/wrk) or [`vegeta`](https://github.com/tsenart/vegeta`) using realistic URLs and sizes.
- Use `cargo flamegraph` or `perf` to identify hotspots in transformations if CPU-bound.

## Plan for scale

- Horizontal scaling is straightforward—deploy multiple imgforge instances behind a load balancer. Ensure each replica has its own cache path (or use a shared NAS for disk caches).
- Combine imgforge with message queues or background jobs when pre-rendering large batches of images.

Pair these tips with the lifecycle overview in [6_processing_pipeline.md](6_processing_pipeline.md) to pinpoint bottlenecks quickly.
