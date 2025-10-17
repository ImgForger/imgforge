# 3. Configuration

imgforge reads configuration exclusively from environment variables. This document expands on every tunable option, providing context, defaults, and usage notes. Combine it with infrastructure-specific techniques (dotenv files, container orchestrator secrets, etc.) to inject settings at runtime.

## Runtime & threading

| Variable                         | Default      | Description & tips                                                                                                                                                                |
|----------------------------------|--------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_WORKERS`               | `0`          | Maximum number of simultaneous image-processing jobs. `0` lets imgforge set `num_cpus * 2`. Increase if libvips operations are lightweight; decrease on memory-constrained hosts. |
| `IMGFORGE_TIMEOUT`               | `30` seconds | Hard timeout enforced by `tower_http::timeout`. Requests exceeding the budget return `504 Gateway Timeout`. Tune alongside upstream proxy timeouts.                               |
| `IMGFORGE_DOWNLOAD_TIMEOUT`      | `10` seconds | Client-side timeout for fetching the source image. Slow origins trigger an error when exceeded.                                                                                   |
| `IMGFORGE_RATE_LIMIT_PER_MINUTE` | unset        | Enables a token bucket limiter shared by all requests. Use it to shield downstream origins. Set to `0` or leave unset to disable.                                                 |

## Networking & binding

| Variable                   | Default        | Description & tips                                                                                                                      |
|----------------------------|----------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_BIND`            | `0.0.0.0:3000` | Primary HTTP listener. Bind to `127.0.0.1` when running behind a reverse proxy locally.                                                 |
| `IMGFORGE_PROMETHEUS_BIND` | unset          | Optional dedicated metrics listener (e.g., `0.0.0.0:9600`). When unset, metrics remain available on the main listener under `/metrics`. |

## Logging & observability

| Variable             | Default | Description & tips                                                                                                                            |
|----------------------|---------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_LOG_LEVEL` | `info`  | Consumed by `tracing_subscriber::EnvFilter`. Example: `imgforge=debug,tower_http=info` for detailed request spans without noisy dependencies. |

## Security & authentication

| Variable                          | Default    | Description & tips                                                                                                                                                  |
|-----------------------------------|------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_KEY`                    | _required_ | Hex-encoded HMAC key. The decoded byte string is used to sign URLs (see [4_url_structure.md](4_url_structure.md)). Minimum 32 bytes recommended.                    |
| `IMGFORGE_SALT`                   | _required_ | Hex-encoded salt prepended to the signed path prior to hashing. Rotate alongside the key.                                                                           |
| `IMGFORGE_ALLOW_UNSIGNED`         | `false`    | When `true`, accepts `unsafe/...` paths without signature validation. Restrict to development environments.                                                         |
| `IMGFORGE_SECRET`                 | unset      | If provided, requests to `/info` and image endpoints must include `Authorization: Bearer <token>`. Combine with load balancer ACLs when exposing imgforge publicly. |
| `IMGFORGE_ALLOW_SECURITY_OPTIONS` | `false`    | Permits request-level overrides of file size and resolution limits. Keep disabled unless you trust all URL builders.                                                |

## Source validation safeguards

| Variable                      | Default | Description & tips                                                                                                                    |
|-------------------------------|---------|---------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_MAX_SRC_FILE_SIZE`  | unset   | Rejects source images larger than the specified bytes. Useful to prevent multi-megabyte downloads from untrusted hosts.               |
| `IMGFORGE_MAX_SRC_RESOLUTION` | unset   | Maximum allowed megapixels (width × height ÷ 1_000_000). Helps avoid processing extremely large images.                               |
| `IMGFORGE_ALLOWED_MIME_TYPES` | unset   | Comma-separated allowlist (e.g., `image/jpeg,image/png,image/webp`). Requests with other MIME types fail with `400 Bad Request`.      |
| `IMGFORGE_WATERMARK_PATH`     | unset   | Filesystem path to a watermark image automatically applied when the `watermark` option is present and no `watermark_url` is supplied. |

## Cache configuration

Caching is optional but highly recommended for hot content. Enable it via `IMGFORGE_CACHE_TYPE` and allied variables. Full guidance lives in [7_caching.md](7_caching.md). At a glance:

| Variable                         | Default                    | Description                                                     |
|----------------------------------|----------------------------|-----------------------------------------------------------------|
| `IMGFORGE_CACHE_TYPE`            | unset                      | Choose `memory`, `disk`, or `hybrid`.                           |
| `IMGFORGE_CACHE_MEMORY_CAPACITY` | `1000`                     | Maximum number of entries stored in memory.                     |
| `IMGFORGE_CACHE_DISK_PATH`       | _required for disk/hybrid_ | Directory for on-disk storage. Must be writable and persistent. |
| `IMGFORGE_CACHE_DISK_CAPACITY`   | `10000`                    | Maximum number of entries persisted on disk.                    |

## Advanced tuning

| Variable                         | Default | Description & tips                                                                                                                       |
|----------------------------------|---------|------------------------------------------------------------------------------------------------------------------------------------------|
| `IMGFORGE_BIND` + `SO_REUSEPORT` | —       | When deploying multiple instances on the same host, rely on a reverse proxy or run separate ports. imgforge does not set `SO_REUSEPORT`. |
| `RUST_LOG`                       | —       | Equivalent to `IMGFORGE_LOG_LEVEL`. Either variable works; use one consistently.                                                         |
| `HTTP_PROXY` / `HTTPS_PROXY`     | —       | `reqwest` respects standard proxy environment variables. Configure if imgforge runs behind an outbound proxy.                            |

## Configuration management patterns

- **Dotenv files**: Store variables in `.env` and load them with `dotenvx` or `direnv`. Keep files out of version control.
- **Container orchestrators**: Map secrets to environment variables. For Kubernetes, use `envFrom` with ConfigMaps (non-secret) and Secrets (sensitive values).
- **Systemd**: Place variables in `/etc/imgforge.env` and reference them via `EnvironmentFile=` in the unit. See [10_deployment.md](10_deployment.md).

## Validating configuration

Run the binary with `IMGFORGE_LOG_LEVEL=debug` to log parsed values on startup. Missing required settings raise a panic with a descriptive message:

```bash
IMGFORGE_KEY=... IMGFORGE_SALT=... cargo run
```

Use `curl /status` to ensure the server started successfully, then test signed URLs following [4_url_structure.md](4_url_structure.md).
