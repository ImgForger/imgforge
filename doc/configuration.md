# Configuration

imgforge is configured entirely through environment variables. The defaults are tuned for local development; production deployments should override the relevant values and inject secret material securely.

## Core runtime settings

| Variable | Default | Description |
| --- | --- | --- |
| `IMGFORGE_BIND` | `0.0.0.0:3000` | TCP address the HTTP server listens on. |
| `IMGFORGE_WORKERS` | `0` (auto) | Number of concurrent processing permits. `0` uses `num_cpus * 2`.
| `IMGFORGE_TIMEOUT` | `30` | Per-request timeout (seconds) enforced by the Axum timeout layer.
| `IMGFORGE_DOWNLOAD_TIMEOUT` | `10` | Timeout (seconds) for fetching source images via `reqwest`.
| `IMGFORGE_LOG_LEVEL` | `info` | Comma-separated filter for [`tracing-subscriber`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html) (e.g. `imgforge=debug,tower_http=info`). |
| `IMGFORGE_PROMETHEUS_BIND` | unset | If set, spins up a dedicated `/metrics` listener at the given address. The main server always exposes `/metrics` inline. |

## Security and access control

| Variable | Default | Description |
| --- | --- | --- |
| `IMGFORGE_KEY` | _required_ | Hex-encoded HMAC key used to validate signatures. Must decode to at least 32 bytes. |
| `IMGFORGE_SALT` | _required_ | Hex-encoded HMAC salt prepended to the signed path. |
| `IMGFORGE_ALLOW_UNSIGNED` | `false` | Allow requests that use the `unsafe` signature prefix. Enable only for development. |
| `IMGFORGE_SECRET` | unset | Optional bearer token required via the `Authorization: Bearer <token>` header for `/info` and image routes. |
| `IMGFORGE_ALLOW_SECURITY_OPTIONS` | `false` | Permit request-level overrides of `max_src_file_size` and `max_src_resolution`. |
| `IMGFORGE_RATE_LIMIT_PER_MINUTE` | unset | Enables a global rate limiter (requests/minute). Set to `0` to disable explicitly. |

## Source validation safeguards

| Variable | Default | Description |
| --- | --- | --- |
| `IMGFORGE_MAX_SRC_FILE_SIZE` | unset | Reject source images larger than the provided byte size.
| `IMGFORGE_MAX_SRC_RESOLUTION` | unset | Reject source images above a certain resolution (megapixels).
| `IMGFORGE_ALLOWED_MIME_TYPES` | unset | Comma-separated allowlist of MIME types (e.g. `image/jpeg,image/png`). |
| `IMGFORGE_WATERMARK_PATH` | unset | Filesystem path to a watermark image automatically applied when requests set `watermark`. |

## Caching backends

Caching is optional; if `IMGFORGE_CACHE_TYPE` is unset the server bypasses the cache entirely.

| Variable | Default | Description |
| --- | --- | --- |
| `IMGFORGE_CACHE_TYPE` | unset | Cache backend: `memory`, `disk`, or `hybrid` (memory + disk via [Foyer](https://docs.rs/foyer)). |
| `IMGFORGE_CACHE_MEMORY_CAPACITY` | `1000` | Maximum entries when `memory` or `hybrid` is enabled. |
| `IMGFORGE_CACHE_DISK_PATH` | _required for disk/hybrid_ | Directory used for persistent cache storage. Ensure the directory exists and is writable. |
| `IMGFORGE_CACHE_DISK_CAPACITY` | `10000` | Maximum entries persisted on disk. |

## Example `.env`

```env
IMGFORGE_BIND=0.0.0.0:3000
IMGFORGE_KEY=$(openssl rand -hex 32)
IMGFORGE_SALT=$(openssl rand -hex 32)
IMGFORGE_TIMEOUT=30
IMGFORGE_DOWNLOAD_TIMEOUT=10
IMGFORGE_RATE_LIMIT_PER_MINUTE=600
IMGFORGE_CACHE_TYPE=hybrid
IMGFORGE_CACHE_MEMORY_CAPACITY=2000
IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge
IMGFORGE_CACHE_DISK_CAPACITY=20000
IMGFORGE_ALLOWED_MIME_TYPES=image/jpeg,image/png,image/webp
IMGFORGE_LOG_LEVEL=imgforge=debug,tower_http=info
```

Load the file using `source .env` before starting the binary, or run `dotenvx run -- ./target/release/imgforge`.

## YAML and configuration management

imgforge does not parse a native YAML or TOML configuration file. When you prefer declarative configuration, convert your desired settings into environment variables at deploy time. Common approaches include:

- Rendering a ConfigMap/Secret to an `envFrom` stanza in Kubernetes.
- Using Docker Compose or Nomad to declare the `environment` block (see the repository’s `docker-compose.yml`).
- Generating an `.env` file from a YAML document with tools such as [`yq`](https://github.com/mikefarah/yq).

For example, given a `config.yml`:

```yaml
bind: 0.0.0.0:3000
rate_limit_per_minute: 600
cache:
  type: disk
  path: /mnt/cache
  capacity: 20000
```

you can export those settings with:

```bash
yq '. as $cfg | {
  "IMGFORGE_BIND": $cfg.bind,
  "IMGFORGE_RATE_LIMIT_PER_MINUTE": ($cfg.rate_limit_per_minute|tostring),
  "IMGFORGE_CACHE_TYPE": $cfg.cache.type,
  "IMGFORGE_CACHE_DISK_PATH": $cfg.cache.path,
  "IMGFORGE_CACHE_DISK_CAPACITY": ($cfg.cache.capacity|tostring)
} | to_entries[] | "\(.key)=\(.value)"' config.yml > .env
```

## Production profiles

A typical production footprint includes:

- `IMGFORGE_ALLOW_UNSIGNED=false` to enforce signatures.
- `IMGFORGE_SECRET` set to a strong bearer token if an upstream proxy cannot restrict access entirely.
- Rate limiting (`IMGFORGE_RATE_LIMIT_PER_MINUTE`) tuned to the downstream origin’s capabilities.
- Persistent cache storage (disk or hybrid) mounted on fast SSDs.
- Separate Prometheus listener (`IMGFORGE_PROMETHEUS_BIND=0.0.0.0:9600`) scraped by your observability stack.
- Tight MIME-type and resolution constraints to avoid resource exhaustion.

Review [doc/deployment.md](deployment.md) for container orchestration patterns and security hardening tips.
