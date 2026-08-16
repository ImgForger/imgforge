# 7. Caching

A cache hit skips fetching and processing entirely, returning stored bytes straight from stage 5 of the [request lifecycle](6_request_lifecycle.md). imgforge uses the [Foyer](https://foyer-rs.github.io/foyer/) cache engine and offers three backends.

| Backend  | Storage                     | Survives restart | Suits                          |
| -------- | --------------------------- | ---------------- | ------------------------------ |
| `memory` | RAM                         | No               | Edge nodes, development        |
| `disk`   | Foyer block engine on disk  | Yes              | Expensive renders worth keeping across deploys |
| `hybrid` | RAM hot set, disk overflow  | Yes              | High-traffic production        |
| unset    | none                        | —                | Testing, or a CDN doing the caching |

## How caching works

- **Key derivation**: the cache key is the full request path — processing options, `cachebuster`, and output format included. Any difference in the path is a different entry. Several things outside the path join it too, because each changes the bytes without changing the URL:

  | Input | Joins the key when |
  | ----- | ------------------ |
  | Output version | Every processed response. Bumped by any release that changes the bytes an unchanged URL produces, so an upgrade retires entries rather than serving the old output indefinitely. A `raw` response is the one exception: it hands back the origin's own bytes, which an imgforge release does not change, so its key carries no version and its entries survive an upgrade. |
  | `IMGFORGE_DEFAULT_FORMAT` | The URL names no format and none was negotiated. |
  | Negotiated format | Content negotiation chose one from `Accept`. |
  | Client hints | `IMGFORGE_ENABLE_CLIENT_HINTS` is on. The requested width and DPR both join, so a `Width: 320` request and a `Width: 1280` request cannot share an entry. |
  | `max_result_dimension` | A ceiling is in force. |
  | `max_animation_frames` | A ceiling is in force. |
  | `max_animation_frame_resolution` | A ceiling is in force. |
  | Resolved source URL | It differs from the request path — that is, `IMGFORGE_BASE_URL` is set. A relative reference names a different image the moment that setting changes. Joins a `raw` key too. |
  | `max_src_resolution` | A ceiling is in force. Joins a `raw` key too. |
  | `max_src_file_size` | A ceiling is in force. Joins a `raw` key too. |
  | `IMGFORGE_ALLOWED_MIME_TYPES` | The list is set. A short digest of the sorted list joins the key, so reordering the variable is not treated as a change. Joins a `raw` key too. |
  | `IMGFORGE_WATERMARK_PATH` | The request composites the server-side watermark — that is, it uses `watermark` without a `watermark_url` of its own. Repointing the setting retires the entries composited with the old logo. |
  | `IMGFORGE_QUALITY` and the other option defaults | Any of them differs from imgforge's built-in defaults. They seed the parse, so they change the bytes exactly as a URL option would. |

  The last two are configuration rather than a ceiling, and they are in the key for the same reason the ceilings are: **a config change carries no version bump.** `OUTPUT_VERSION` retires entries when a *release* changes the output, which is why it does not have to name every processing detail — but nothing retires them when an *operator* changes what the server produces. Lowering `IMGFORGE_QUALITY` or swapping the logo would otherwise keep serving the old bytes until eviction.

  One residual is worth knowing: the watermark joins the key by its **path**, not its contents. Replacing the file at the same path is invisible to the key. Within a running process it is also invisible to imgforge — the watermark is loaded once and held — so this only matters across a restart with a persistent cache. Change the path, or clear the cache, when you replace the image in place.

  **Every ceiling is checked *after* the cache is consulted**, which is why each one has to be part of the key. Without that, an entry stored while a limit was loose keeps being served once the limit is tightened: the request is answered before it ever reaches the check it should have failed. The last three cover the case where that matters most — `raw` and a matching `skip_processing` hand back origin bytes with no processing between them and the client, so their source limits are the only thing standing between a tightened policy and an entry already in the cache.

  So: **changing any of these retires every entry stored under the previous setting**, and that is the point rather than a cost to avoid. Enabling one that was previously unset does the same thing, because an unset limit contributes nothing to the key and a set one contributes its prefix. Expect a cold cache for the affected URLs after any such change; the orphaned entries are unreachable and age out by eviction. A deployment that sets none of them keeps the keys it already had.
- **Population**: rendered bytes are inserted after a successful response. A failed write is logged and does not affect the response.
- **Invalidation**: there is no explicit purge. Caches are capacity-limited and evict least-recently-used entries; change the `cachebuster` token to force a miss when an upstream asset changes.
- **What a hit cannot reproduce**: a hit never makes the source request, so nothing that came from the origin's response is available. `Last-Modified` is therefore absent on a hit, and with `IMGFORGE_CACHE_CONTROL_PASSTHROUGH` enabled the origin's `Cache-Control` is too — the response falls back to `IMGFORGE_TTL`, or omits the header when no TTL is set. `ETag`, the canonical `Link`, and a TTL-derived `Cache-Control` are all derived from the bytes or the configuration and are identical either way.

## Client-side caching

The cache above saves imgforge work. The headers in [Response delivery](3_configuration.md#response-delivery) save the request entirely: `IMGFORGE_TTL` lets a browser or CDN hold the image without asking again, and `IMGFORGE_USE_ETAG` turns the requests it does make into `304 Not Modified` with no body. They are complementary — a CDN in front of imgforge is usually worth more than either.

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

- **Everything misses.** Check that `IMGFORGE_CACHE_TYPE` is set at all — unset means no cache. A *misspelled* value is not the cause here: imgforge refuses to start on an unrecognised cache type rather than falling back to no caching.
- **Permission denied on startup.** The disk path is not writable by the service user.
- **Entries evicted sooner than expected.** Raise the capacity values, and check that `cachebuster` tokens are not changing on every request.

See [Performance Tips](9_performance.md) for how caching fits into overall throughput.
