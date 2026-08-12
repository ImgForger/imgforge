# 11. Prometheus Monitoring

imgforge exposes Prometheus-compatible metrics so you can observe throughput, latency, cache efficacy, and error rates in real time. This guide shows you how to scrape the service, interpret the provided counters and histograms, and build actionable alerts.

## Exposing the endpoint

- **Default listener** – `/metrics` is served on the main HTTP listener. Set `IMGFORGE_BIND` (default `0.0.0.0:3000`) to match your environment.
- **Dedicated listener** – Provide `IMGFORGE_PROMETHEUS_BIND` (for example `0.0.0.0:9600`) to expose metrics on a separate port. The endpoint remains `/metrics`.
- **Authentication** – The metrics endpoint never requires URL signatures but inherits bearer-token protection when `IMGFORGE_SECRET` is set. Grant your scraper a token or whitelist the Prometheus network path at the proxy layer.

## Metrics flow diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                    Prometheus Metrics Pipeline                           │
└──────────────────────────────────────────────────────────────────────────┘
    ┌─────────────────────────────────────────────────────────────────┐
    │                                                                 │
    │  Request arrives ──▶ Timer starts                               │
    │         │                                                       │
    │         ├──▶ Check cache ──▶ cache_hits_total++ OR              │
    │         │                    cache_misses_total++               │
    │         │                                                       │
    │         ├──▶ Fetch source ──▶ source_image_fetch_duration_      │
    │         │                     seconds (observe)                 │
    │         │                                                       │
    │         ├──▶ Wait for permit ──▶ image_operation_semaphore_     │
    │         │                         wait_duration_seconds         │
    │         ├──▶ Blocking queue ──▶ image_operation_blocking_       │
    │         │                       queue_duration_seconds          │
    │         ├──▶ Execute ──▶ image_operation_execution_duration_    │
    │         │               seconds + image_processing_duration_    │
    │         │               seconds                                 │
    │         │                                                       │
    │         └──▶ Response ──▶ http_requests_duration_seconds        │
    │                           status_codes_total{status="200"}++    │
    │                           processed_images_total{format="..."}++│
    └─────────────────────────────────────────────────────────────────┘
```

## Core metrics

| Metric name                                       | Type      | Labels           | Insight                                                                                       |
| ------------------------------------------------- | --------- | ---------------- | --------------------------------------------------------------------------------------------- |
| `http_requests_duration_seconds`                  | Histogram | `method`, `path` | Latency across the full request lifecycle, including cache hits and misses.                   |
| `image_processing_duration_seconds`               | Histogram | `format`         | Time spent transforming images, segmented by requested output format.                         |
| `image_operation_semaphore_wait_duration_seconds` | Histogram | `operation`      | Time waiting for an imgforge worker permit; exposes configured worker saturation.             |
| `image_operation_blocking_queue_duration_seconds` | Histogram | `operation`      | Time between submitting work and its start on Tokio's blocking pool.                          |
| `image_operation_execution_duration_seconds`      | Histogram | `operation`      | Complete blocking execution time, including decode, validation, transformation, and encoding. |
| `image_operation_concurrency_limit`               | Gauge     | _none_           | Configured maximum number of image operations that may execute concurrently.                  |
| `image_operations_active`                         | Gauge     | `operation`      | Image operations currently executing on blocking threads.                                     |
| `image_operations_waiting`                        | Gauge     | `operation`      | Image operations waiting for either a semaphore permit or a blocking-pool thread.             |
| `processed_images_total`                          | Counter   | `format`         | Throughput per encoded format; increments on successful responses.                            |
| `source_image_fetch_duration_seconds`             | Histogram | _none_           | Download latency from upstream sources.                                                       |
| `source_images_fetched_total`                     | Counter   | `status`         | Counts of successful (`status="success"`) and failed (`status="error"`) source fetches.       |
| `cache_hits_total` / `cache_misses_total`         | Counter   | `cache_type`     | Cache effectiveness across memory, disk, or hybrid backends.                                  |
| `status_codes_total`                              | Counter   | `status`         | Aggregated HTTP responses (ideal for alerting on spikes in `4xx`/`5xx`).                      |

> **Tip:** Combine counters into rates using `rate()` or `irate()` when graphing over time, and apply `histogram_quantile()` to histogram buckets for percentile views.

## Example Prometheus configuration

```yaml
scrape_configs:
  - job_name: imgforge
    static_configs:
      - targets: ["imgforge.example.com:3000"]
    metrics_path: /metrics
    scheme: https
    authorization:
      credentials: ${IMGFORGE_PROM_TOKEN}
```

When running a dedicated metrics listener, adjust `targets` to the alternate port. Use service discovery (Kubernetes, Consul, etc.) in production to track dynamic endpoints.

## Suggested dashboards

1. **Request overview** – Plot `sum(rate(status_codes_total[5m])) by (status)` to visualise success versus error responses.
2. **Processing latency** – Use `histogram_quantile(0.95, sum(rate(image_processing_duration_seconds_bucket[5m])) by (le, format))` to watch for regressions after deploys.
3. **Queue latency** – Compare `image_operation_semaphore_wait_duration_seconds{quantile="0.95"}` with `image_operation_blocking_queue_duration_seconds{quantile="0.95"}`, grouped by `operation`.
4. **Cache efficiency** – Visualize hit ratio: `sum(rate(cache_hits_total[5m])) / (sum(rate(cache_hits_total[5m])) + sum(rate(cache_misses_total[5m])))`.
5. **Source reliability** – Track `sum(rate(source_images_fetched_total{status="error"}[5m]))` to spot upstream outages.
6. **Instance saturation** – Overlay `sum(image_operations_active)`, `sum(image_operations_waiting)`, and `image_operation_concurrency_limit` with queue percentiles, CPU, RSS, and libvips memory.

## Alerting patterns

- **Error spike** – Trigger when `sum(rate(status_codes_total{status=~"5.."}[5m]))` exceeds a baseline for 10 minutes.
- **Cache miss surge** – Alert when the miss ratio stays above 70% for sustained intervals, indicating cache warmup or configuration drift.
- **Slow processing** – Page when the 95th percentile of `image_processing_duration_seconds` remains above an agreed SLA for 15 minutes.
- **Worker saturation** – Alert when p95 `image_operation_semaphore_wait_duration_seconds` is sustained above the queueing budget.
- **Blocking-pool saturation** – Alert when `image_operation_blocking_queue_duration_seconds` rises while semaphore wait remains low.
- **Concurrency saturation** – Alert when `sum(image_operations_active) / image_operation_concurrency_limit` remains near 1 and waiting work is sustained.
- **Source failures** – Notify when `rate(source_images_fetched_total{status="error"}[5m])` climbs, hinting at upstream instability.

## Connecting with tracing and logs

Correlate request IDs emitted in logs (via the `X-Request-ID` header) with spikes in histogram buckets. Pair this document with [Request Lifecycle](6_request_lifecycle.md) to map metrics anomalies back to lifecycle stages, and with infrastructure metrics (CPU, memory, I/O) for holistic visibility.

## Next steps

- Surface these dashboards in Grafana or your preferred visualization tool.
- Feed alerts into your incident workflow (PagerDuty, Opsgenie, Slack). Use quiet hours and grouping strategies to avoid alert fatigue.
- Share runbooks linking playbooks in [Error Troubleshooting](8_error_troubleshooting.md) so responders can remediate issues quickly.
