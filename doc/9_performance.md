# 9. Performance Tips

Where imgforge's throughput actually comes from, and which knobs are worth turning. Roughly in order of impact.

## Tune concurrency thoughtfully

`IMGFORGE_WORKERS` bounds simultaneous image decodes and transformations, including work submitted to Tokio's blocking pool. Cached and raw responses do not consume these permits. The automatic default is `num_cpus * 2`, but production deployments should set an explicit value based on representative worst-case images.

Use this practical approach:

1. Start with the automatic default (`2 × CPU cores`).
2. Load-test with the largest images and most expensive transformations you expect in production.
3. Keep 25–40% of the container's memory unused for traffic spikes and unusually expensive inputs.
4. Lower `IMGFORGE_WORKERS` if memory approaches that reserve. Raise it only when memory is comfortable and additional workers improve throughput.

For example, with a 2 GB container, aim to keep peak usage below roughly 1.2–1.5 GB during the load test. If four workers stay inside that range but eight workers do not, configure `IMGFORGE_WORKERS=4`.

Monitor `image_operations_active`, `image_operations_waiting`, `image_operation_concurrency_limit`, process RSS, and the libvips memory gauges together. Sustained waiting with active operations near the limit indicates saturation. If memory usage becomes unsafe as concurrency increases, lower the worker count. imgforge warns at startup when a configured value exceeds `2 × CPU`, but the warning is advisory because safe concurrency depends on the workload and memory limit.

## Embrace caching

- Enable `memory` or `hybrid` caching for hot assets. See [Caching](7_caching.md) for sizing guidelines.
- Warm caches proactively after deployments or cache flushes. Scripts can replay historical access logs.
- Combine imgforge with a CDN to offload repeated requests and reduce origin bandwidth.

## Optimize signatures & URLs

- Avoid generating unique cache-busting tokens unless content actually changes. Excessive churn destroys cache hit ratios and increases CPU load.
- Batch-sign URLs offline instead of per request to reduce application overhead.

## Use timeouts strategically

- Tighten `IMGFORGE_DOWNLOAD_TIMEOUT` to fail fast on unresponsive sources.
- Keep `IMGFORGE_TIMEOUT` slightly below your ingress proxy timeout to avoid double-processing.

## Monitor key metrics

Scrape `/metrics` frequently and build dashboards around the core histograms and counters. Start with `image_processing_duration_seconds`, `http_requests_duration_seconds`, cache hit ratios, and `status_codes_total`, then expand using the playbooks in [Prometheus Monitoring](11_prometheus_monitoring.md).

## Right-size hardware

- Favor instances with high memory bandwidth and SSD-backed storage when using disk or hybrid caches.
- Allocate headroom for libvips; operations like large resizes or watermarks can temporarily inflate memory usage.
- Pin docker containers to dedicated CPU sets (`cpuset`) when co-locating with other workloads to minimize interference.

## Keep sources close

- Fetch from object storage in the same region. On a cache miss, the source download is often the largest part of the response time.
- `reqwest` negotiates HTTP/2 and reuses connections automatically; no configuration needed.

## Instrument tracing

- Increase log verbosity (`IMGFORGE_LOG_LEVEL=imgforge=debug`) during load tests to capture timings.
- Integrate `tracing` subscribers that export spans to distributed tracing backends (e.g., OpenTelemetry) for end-to-end insight.

## Measure before tuning

- Use the included [K6 load testing suite](../loadtest/README.md), which already covers the processing endpoints with varied parameters. `wrk` and `vegeta` work too, as long as the URLs and image sizes resemble production.
- Reach for `cargo flamegraph` or `perf` only once you have confirmed the work is CPU-bound rather than waiting on sources.

## Scaling out

Run several instances behind a load balancer. Each replica needs its own disk cache path — imgforge has no distributed cache, so replicas do not share entries and a CDN in front is what gives you a shared layer.

When something is slow, [Request Lifecycle](6_request_lifecycle.md) maps each metric back to the stage that emits it.
