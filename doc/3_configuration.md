# 3. Configuration

imgforge is configured entirely through environment variables. Every one of them is listed below.

## Runtime & threading

| Variable                         | Default      | Description & tips                                                                                                                                                                                                                               |
| -------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_WORKERS`               | `0`          | Maximum simultaneous image operations. `0` selects `num_cpus * 2`; malformed values stop startup. Set this explicitly in production from measured CPU and memory limits. See [Performance Tips](9_performance.md#tune-concurrency-thoughtfully). |
| `IMGFORGE_TIMEOUT`               | `30` seconds | Hard timeout enforced by the request-timeout middleware. Requests exceeding the budget return `504 Gateway Timeout`. Tune alongside upstream proxy timeouts.                                                                                     |
| `IMGFORGE_DOWNLOAD_TIMEOUT`      | `10` seconds | Client-side timeout for fetching the source image. Slow origins trigger an error when exceeded.                                                                                                                                                  |
| `IMGFORGE_RATE_LIMIT_PER_MINUTE` | unset        | Enables a token bucket limiter shared by all requests. Use it to shield downstream origins. Set to `0` or leave unset to disable.                                                                                                                |

## Networking & binding

| Variable                   | Default        | Description & tips                                                                                                                                                                      |
| -------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_BIND`            | `0.0.0.0:3000` | Primary HTTP listener, as `host:port`. A bare port number expands to `0.0.0.0:<port>`. Bind to `127.0.0.1` when a local reverse proxy is the only client.                               |
| `IMGFORGE_PROMETHEUS_BIND` | unset          | Optional dedicated metrics listener (e.g., `0.0.0.0:9600`). When unset, metrics remain on the main listener under `/metrics`. See [Prometheus Monitoring](11_prometheus_monitoring.md). |

## Logging & observability

| Variable             | Default | Description & tips                                                                                                                                        |
| -------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_LOG_LEVEL` | `info`  | Consumed by the tracing subscriber’s environment filter. Example: `imgforge=debug,tower_http=info` for detailed request spans without noisy dependencies. |

## Security & authentication

| Variable                          | Default    | Description & tips                                                                                                                                                  |
| --------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_KEY`                    | _required_ | Hex-encoded HMAC key. The decoded byte string is used to sign URLs (see [URL Structure](4_url_structure.md)). Minimum 32 bytes recommended.                         |
| `IMGFORGE_SALT`                   | _required_ | Hex-encoded salt prepended to the signed path prior to hashing. Rotate alongside the key.                                                                           |
| `IMGFORGE_ALLOW_UNSIGNED`         | `false`    | When `true`, accepts `unsafe/...` paths without signature validation. Restrict to development environments.                                                         |
| `IMGFORGE_SECRET`                 | unset      | If provided, requests to `/info` and image endpoints must include `Authorization: Bearer <token>`. Combine with load balancer ACLs when exposing imgforge publicly. |
| `IMGFORGE_ALLOW_SECURITY_OPTIONS` | `false`    | Permits request-level overrides of file size and resolution limits. Keep disabled unless you trust all URL builders.                                                |

## Source validation safeguards

| Variable                      | Default  | Description & tips                                                                                                                                                                                           |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_MAX_SRC_FILE_SIZE`  | unset    | Positive integer byte limit. Rejects larger source images before processing.                                                                                                                                 |
| `IMGFORGE_MAX_SRC_RESOLUTION` | unset    | Finite, positive megapixel limit (width × height ÷ 1_000_000). Helps avoid processing extremely large images.                                                                                                |
| `IMGFORGE_ALLOWED_MIME_TYPES` | unset    | Comma-separated allowlist (e.g., `image/jpeg,image/png,image/webp`). Requests with other MIME types fail with `400 Bad Request`.                                                                             |
| `IMGFORGE_WATERMARK_PATH`     | unset    | Filesystem path to a watermark image automatically applied when the `watermark` option is present and no `watermark_url` is supplied.                                                                        |
| `IMGFORGE_DEFAULT_FORMAT`     | `source` | Output format when the URL requests none. `source` keeps the source image's format (imgproxy-compatible); a concrete format (`jpeg`, `webp`, ...) fixes the default — `jpeg` restores the pre-0.11 behavior. |

imgforge refuses to start when either source limit or `IMGFORGE_WORKERS` is malformed. Source limits must also be positive, finite, and inside their supported ranges. An unset source-limit variable is the only way to disable that limit.

Changing `IMGFORGE_DEFAULT_FORMAT` uses a separate cache namespace for format-less URLs, preventing persistent cache entries encoded under the previous default from being served with stale bytes or content types.

## Cache configuration

Caching is optional but highly recommended for hot content. Enable it via `IMGFORGE_CACHE_TYPE` and allied variables. Full guidance lives in [Cache Configuration](7_caching.md). At a glance:

| Variable                         | Default                    | Description                                                     |
| -------------------------------- | -------------------------- | --------------------------------------------------------------- |
| `IMGFORGE_CACHE_TYPE`            | unset                      | Choose `memory`, `disk`, or `hybrid`.                           |
| `IMGFORGE_CACHE_MEMORY_CAPACITY` | `1000`                     | Maximum number of entries stored in memory.                     |
| `IMGFORGE_CACHE_DISK_PATH`       | _required for disk/hybrid_ | Directory for on-disk storage. Must be writable and persistent. |
| `IMGFORGE_CACHE_DISK_CAPACITY`   | `10000`                    | Maximum number of entries persisted on disk.                    |

## Presets

Presets are named sets of processing options that can be reused across multiple requests, simplifying URL management and enforcing consistency.

| Variable                | Default | Description & tips                                                                                                                                                                                                                                                                     |
| ----------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_PRESETS`      | unset   | Comma-separated preset definitions in the format `name=options`. Options use `/` as separator and follow standard processing option syntax (e.g., `thumbnail=resize:fit:150:150/quality:80,banner=resize:fill:1200:300/quality:90`). A preset named `default` applies to all requests. |
| `IMGFORGE_ONLY_PRESETS` | `false` | When `true`, enables presets-only mode. Only `preset:name` (or `pr:name`) references are allowed in URLs; other processing options are rejected. Use this to enforce strict governance over transformations.                                                                           |


## Variables imgforge inherits

| Variable                     | Effect                                                                                     |
| ---------------------------- | ------------------------------------------------------------------------------------------ |
| `RUST_LOG`                   | Equivalent to `IMGFORGE_LOG_LEVEL`. Pick one and use it consistently.                       |
| `HTTP_PROXY` / `HTTPS_PROXY` | Honoured by `reqwest` for outbound source fetches.                                          |

imgforge does not set `SO_REUSEPORT`. To run several instances on one host, give each its own port and put a reverse proxy in front.

## Supplying the variables

- **Dotenv**: keep a `.env` out of version control and load it with `direnv`, `dotenvx`, or `docker run --env-file`.
- **Kubernetes**: `envFrom` with a ConfigMap for the plain settings and a Secret for `IMGFORGE_KEY`, `IMGFORGE_SALT`, and `IMGFORGE_SECRET`.
- **Systemd**: `EnvironmentFile=/etc/imgforge.env`, mode `600`. See [Manual Deployment](10.2_deployment_manual.md).

## Checking what was parsed

Start with `IMGFORGE_LOG_LEVEL=debug` to log the parsed configuration. Malformed values for `IMGFORGE_WORKERS` or either source limit stop startup with a message naming the variable, so a failed start is usually self-explanatory. Confirm the server is up with `curl localhost:3000/status`, then try a signed URL — see [URL Structure](4_url_structure.md).
