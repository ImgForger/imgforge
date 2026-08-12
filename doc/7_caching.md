# 7. Caching

A cache hit skips fetching and processing entirely, returning stored bytes straight from stage 3 of the [request lifecycle](6_request_lifecycle.md). imgforge uses the [Foyer](https://foyer-rs.github.io/foyer/) cache engine and offers three backends.

| Backend  | Storage                     | Survives restart | Suits                          |
| -------- | --------------------------- | ---------------- | ------------------------------ |
| `memory` | RAM                         | No               | Edge nodes, development        |
| `disk`   | Foyer block engine on disk  | Yes              | Expensive renders worth keeping across deploys |
| `hybrid` | RAM hot set, disk overflow  | Yes              | High-traffic production        |
| unset    | none                        | —                | Testing, or a CDN doing the caching |

## How caching works

- **Key derivation**: the cache key is the full request path — processing options, `cachebuster`, and output format included. Any difference in the path is a different entry.
- **Population**: rendered bytes are inserted after a successful response. A failed write is logged and does not affect the response.
- **Invalidation**: there is no explicit purge. Caches are capacity-limited and evict least-recently-used entries; change the `cachebuster` token to force a miss when an upstream asset changes.

Metrics:

| Metric                                    | Description                                   |
| ----------------------------------------- | --------------------------------------------- |
| `cache_hits_total{cache_type="memory"}`   | Number of successful lookups.                 |
| `cache_misses_total{cache_type="memory"}` | Number of misses (including disabled caches). |

## Enabling a cache backend

Set `IMGFORGE_CACHE_TYPE` to one of `memory`, `disk`, or `hybrid`. If unset, the cache is disabled and every request hits the processing pipeline.

### Common environment variables

| Variable                         | Description                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_CACHE_TYPE`            | Backend selector: `memory`, `disk`, or `hybrid`.                                                                                 |
| `IMGFORGE_CACHE_MEMORY_CAPACITY` | Maximum number of items retained in memory (default `1000`). Applies to memory and hybrid caches.                                |
| `IMGFORGE_CACHE_DISK_PATH`       | Directory for on-disk storage. Required for disk and hybrid caches. Ensure it exists and is writable before starting the server. |
| `IMGFORGE_CACHE_DISK_CAPACITY`   | Maximum number of entries stored on disk (default `10000`).                                                                      |

### Examples

```bash
# Memory only
export IMGFORGE_CACHE_TYPE=memory
export IMGFORGE_CACHE_MEMORY_CAPACITY=5000

# Disk only — survives restarts and redeploys
export IMGFORGE_CACHE_TYPE=disk
export IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge
export IMGFORGE_CACHE_DISK_CAPACITY=20000

# Hybrid — hot set in RAM, the rest on disk
export IMGFORGE_CACHE_TYPE=hybrid
export IMGFORGE_CACHE_MEMORY_CAPACITY=5000
export IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge
export IMGFORGE_CACHE_DISK_CAPACITY=50000
```

Put the disk path on SSD-backed storage; a cache slower than the render it replaces is worse than no cache.

## Operational notes

- **Capacity is counted in entries, not bytes.** If your outputs vary widely in size, watch actual disk usage separately — 10,000 entries could be 200 MB or 20 GB.
- **Provision the directory before starting.** It must exist and be owned by the user running imgforge. In containers, mount a volume and match the UID/GID. Restrict permissions (`0700`) on shared hosts: cached bytes are the images themselves.
- **Caching is per-instance.** imgforge has no distributed cache. Give each replica its own path and rely on a CDN for cross-node sharing.
- **Warm after deploys** by replaying popular URLs, if a cold cache would show up as a latency spike.
- **Watch the hit ratio**, and treat a sudden drop as signature or `cachebuster` churn rather than a cache problem.

## Troubleshooting

- **Everything misses.** Confirm `IMGFORGE_CACHE_TYPE` is spelled correctly and set; an unrecognised value leaves caching off. Check the logs for `Failed to initialize cache`.
- **Permission denied on startup.** The disk path is not writable by the service user.
- **Entries evicted sooner than expected.** Raise the capacity values, and check that `cachebuster` tokens are not changing on every request.

See [Performance Tips](9_performance.md) for how caching fits into overall throughput.
