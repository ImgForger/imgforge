# 7. Caching

Caching dramatically reduces repeated processing costs and shrinks latency for popular images. imgforge integrates with the [Foyer](https://foyer-rs.github.io/foyer/) cache engine to offer three backends: in-memory, disk, and hybrid. This document explains how to configure each mode and how the cache interacts with the request lifecycle.

## Cache architecture diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         Cache Architecture                              │
└─────────────────────────────────────────────────────────────────────────┘

    Request arrives
         │
         ▼
    ┌────────────────────────┐
    │   Generate cache key   │  → Full path hash (signature + options + source)
    │   from request path    │
    └───────────┬────────────┘
                │
                ▼
    ┌───────────────────────────────────────────────────────────────┐
    │              Check IMGFORGE_CACHE_TYPE                        │
    └───────────────────────────────────────────────────────────────┘
                │
                │
        ┌───────┴───────┬──────────────┬────────────────┐
        │               │              │                │
        ▼               ▼              ▼                ▼
    ┌────────┐    ┌─────────┐   ┌──────────┐     ┌──────────┐
    │  None  │    │ Memory  │   │   Disk   │     │  Hybrid  │
    └────┬───┘    └────┬────┘   └────┬─────┘     └────┬─────┘
         │             │              │                │
         │             ▼              ▼                ▼
         │      ┌────────────┐  ┌──────────────┐  ┌──────────────────┐
         │      │Foyer       │  │Foyer Block   │  │Memory (hot) +    │
         │      │In-Memory   │  │Engine        │  │Disk (spillover)  │
         │      │LRU Cache   │  │Persistent    │  │                  │
         │      └─────┬──────┘  └──────┬───────┘  └────────┬─────────┘
         │            │                │                    │
         │         HIT│    MISS        │HIT      MISS      │HIT    MISS
         │            │                │                    │
         │            └────────────────┴────────────────────┘
         │                            │
         │                         MISS
         └────────────────────────────┘
                                      │
                                      ▼
                          ┌──────────────────────┐
                          │  Process Image       │
                          │  (full pipeline)     │
                          └──────────┬───────────┘
                                     │
                                     ▼
                          ┌──────────────────────┐
                          │  Populate Cache      │
                          │  (if enabled)        │
                          └──────────┬───────────┘
                                     │
                                     ▼
                          ┌──────────────────────┐
                          │  Return Response     │
                          └──────────────────────┘


    Cache Backend Comparison:
    ┌─────────────┬──────────────┬───────────────┬─────────────────────┐
    │   Type      │   Speed      │  Persistence  │     Best For        │
    ├─────────────┼──────────────┼───────────────┼─────────────────────┤
    │  Memory     │  Fastest     │  Volatile     │  Edge nodes, dev    │
    │  Disk       │  Medium      │  Persistent   │  Long-lived cache   │
    │  Hybrid     │  Fast + Med  │  Persistent   │  High-traffic prod  │
    │  None       │  N/A         │  N/A          │  Testing only       │
    └─────────────┴──────────────┴───────────────┴─────────────────────┘
```

## How caching works

- **Key derivation**: The cache key is the full request path (including processing options, `cache_buster`, and output format). Different signatures or parameters yield different cache entries.
- **Population**: After successfully processing an image, imgforge inserts the rendered bytes into the configured cache backend.
- **Invalidation**: Caches are size-limited, so least-recently-used entries are evicted automatically. Use the `cache_buster` option to force a miss when you update upstream assets.

Metrics:

| Metric                                    | Description                                   |
|-------------------------------------------|-----------------------------------------------|
| `cache_hits_total{cache_type="memory"}`   | Number of successful lookups.                 |
| `cache_misses_total{cache_type="memory"}` | Number of misses (including disabled caches). |

## Enabling a cache backend

Set `IMGFORGE_CACHE_TYPE` to one of `memory`, `disk`, or `hybrid`. If unset, the cache is disabled and every request hits the processing pipeline.

### Common environment variables

| Variable                         | Description                                                                                                                      |
|----------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_CACHE_TYPE`            | Backend selector: `memory`, `disk`, or `hybrid`.                                                                                 |
| `IMGFORGE_CACHE_MEMORY_CAPACITY` | Maximum number of items retained in memory (default `1000`). Applies to memory and hybrid caches.                                |
| `IMGFORGE_CACHE_DISK_PATH`       | Directory for on-disk storage. Required for disk and hybrid caches. Ensure it exists and is writable before starting the server. |
| `IMGFORGE_CACHE_DISK_CAPACITY`   | Maximum number of entries stored on disk (default `10000`).                                                                      |

### Memory cache

Configure for short-lived services or low-latency responses:

```bash
export IMGFORGE_CACHE_TYPE=memory
export IMGFORGE_CACHE_MEMORY_CAPACITY=5000
```

- Backed entirely by RAM.
- Fastest option but volatile—entries are lost on restart.
- Ideal for front-line edge nodes where disk is unavailable.

### Disk cache

Persist cache entries across restarts using local or network storage:

```bash
export IMGFORGE_CACHE_TYPE=disk
export IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge
export IMGFORGE_CACHE_DISK_CAPACITY=20000
```

- Uses Foyer’s block engine to store bytes on disk.
- Suitable when CPU-intensive renders must be reused across deploys.
- Place `/var/cache/imgforge` on SSD-backed storage to minimize latency.

### Hybrid cache

Combine a memory hot set with disk-backed persistence:

```bash
export IMGFORGE_CACHE_TYPE=hybrid
export IMGFORGE_CACHE_MEMORY_CAPACITY=5000
export IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge
export IMGFORGE_CACHE_DISK_CAPACITY=50000
```

- Frequently accessed objects stay in memory; less popular ones spill to disk.
- Balances latency and durability for high-traffic services.

## Operational tips

1. **Provision storage** – Ensure the disk path exists and ownership matches the user running imgforge. For containers, mount a persistent volume at the desired location.
2. **Monitor hit ratios** – Scrape Prometheus metrics and alert on low hit rates; adjust memory capacity or investigate signature churn.
3. **Warm caches** – Precompute popular assets by hitting imgforge ahead of peak traffic. Automate via a job triggered after deploys.
4. **Eviction strategy** – Cache capacity is entry-based. If stored objects vary significantly in size, monitor disk usage separately and prune old entries if necessary.
5. **Security** – When storing on shared disks, restrict permissions (`0700`) to the imgforge user to prevent other processes from reading cached content.
6. **Replication** – imgforge does not provide distributed caching. For multi-node deployments, rely on CDN layers or object storage if cross-node sharing is required.

## Troubleshooting caching issues

- **Misses despite configuration**: Confirm `IMGFORGE_CACHE_TYPE` is set and no typos exist. Check logs for `Failed to initialize cache` messages.
- **Permission denied**: Ensure the disk directory is writable. In containers, mount with the correct UID/GID or use `chown` during image build.
- **Unexpected eviction**: Increase `IMGFORGE_CACHE_*_CAPACITY` values and monitor resource usage. Also verify that `cache_buster` values are not changing unnecessarily.

For broader operational strategies, see [Performance](9_performance.md) and [Deployment](10.2_deployment_manual.md).
