# 3. Configuration

imgforge is configured entirely through environment variables. Every one of them is listed below.

## Runtime & threading

| Variable                         | Default      | Description & tips                                                                                                                                                                                                                               |
| -------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_WORKERS`               | `0`          | Maximum simultaneous image operations. `0` selects `num_cpus * 2`; malformed values stop startup. Set this explicitly in production from measured CPU and memory limits. See [Performance Tips](9_performance.md#tune-concurrency-thoughtfully). |
| `IMGFORGE_TIMEOUT`               | `30` seconds | Hard timeout enforced by the request-timeout middleware. Requests exceeding the budget return `408 Request Timeout`. Tune alongside upstream proxy timeouts.                                                                                     |
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
| `IMGFORGE_ALLOW_SECURITY_OPTIONS` | `false`    | Permits request-level overrides of the source and result limits (`msfs`, `msr`, `mrd`, `maf`, `mafr`). Keep disabled unless you trust all URL builders.             |
| `IMGFORGE_SIGNATURE_SIZE`         | `32`       | How many leading bytes of the HMAC a signed URL must carry, 1–32. Shortening the signature shortens the URL and weakens it by exactly the bytes it drops.           |

## Source & result safeguards

| Variable                        | Default  | Description & tips                                                                                                                                                                                           |
| ------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_MAX_SRC_FILE_SIZE`    | unset    | Positive integer byte limit. Rejects larger source images before processing.                                                                                                                                 |
| `IMGFORGE_MAX_SRC_RESOLUTION`   | unset    | Finite, positive megapixel limit (width × height ÷ 1_000_000). Helps avoid processing extremely large images.                                                                                                |
| `IMGFORGE_MAX_RESULT_DIMENSION` | unset    | Positive integer pixel ceiling for the width and height of the processed image. Rejects the request with `400 Bad Request` before encoding. Nothing else bounds output size.                                 |
| `IMGFORGE_ALLOWED_MIME_TYPES`   | unset    | Comma-separated allowlist (e.g., `image/jpeg,image/png,image/webp`). Requests with other MIME types fail with `400 Bad Request`.                                                                             |
| `IMGFORGE_WATERMARK_PATH`       | unset    | Filesystem path to a watermark image automatically applied when the `watermark` option is present and no `watermark_url` is supplied.                                                                        |
| `IMGFORGE_DEFAULT_FORMAT`       | `source` | Output format when the URL requests none. `source` keeps the source image's format (imgproxy-compatible); a concrete format (`jpeg`, `webp`, ...) fixes the default — `jpeg` restores the pre-0.11 behavior. |
| `IMGFORGE_MAX_ANIMATION_FRAMES` | unset    | Positive integer ceiling on how many frames of an animated source are decoded. An animation multiplies every cost by its frame count, which the resolution limit alone does not bound.                       |
| `IMGFORGE_MAX_ANIMATION_FRAME_RESOLUTION` | unset | Finite, positive megapixel ceiling on a single animation frame. Requests exceeding it fail with `400 Bad Request`.                                                                                |

imgforge refuses to start when any of the three limits above or `IMGFORGE_WORKERS` is malformed. Limits must be positive, finite, and inside their supported ranges. Leaving a variable unset is the only way to disable that limit.

The first two bound what imgforge will *read*; `IMGFORGE_MAX_RESULT_DIMENSION` bounds what it will *produce*. Without it there is no ceiling on requested output size — `resize:fill:40000:40000/enlarge:true` will try to build a 40000x40000 image from any source. Because libvips defers the pixel work, the check runs before anything is materialised, so an over-limit request costs nothing beyond the source fetch.

Changing `IMGFORGE_DEFAULT_FORMAT` uses a separate cache namespace for format-less URLs, preventing persistent cache entries encoded under the previous default from being served with stale bytes or content types.

## Source resolution

| Variable                  | Default                | Description & tips                                                                                                                                                                                                                              |
| ------------------------- | ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_BASE_URL`       | unset                  | Prefix prepended to source references that carry no scheme, so URLs can name only a path (`/unsafe/rs:fit:200:0/aW1hZ2UucG5n`). A reference that already names a scheme is used as-is.                                                          |
| `IMGFORGE_ALLOWED_SOURCES` | unset                 | Comma-separated allowlist of URL prefixes. `https://*.example.com/` permits exactly one subdomain label — `images.example.com` but not `example.com` or `a.b.example.com`. Anything not matching fails with `400 Bad Request`.                  |
| `IMGFORGE_USER_AGENT`     | `imgforge/<version>`   | `User-Agent` sent when fetching a source image. Some origins rate-limit or block by user agent.                                                                                                                                                 |
| `IMGFORGE_MAX_REDIRECTS`  | `10`                   | How many redirects a source fetch may follow. A source that redirects forever otherwise ties up a worker for the whole download timeout.                                                                                                        |

The allowlist is checked **after** `IMGFORGE_BASE_URL` is applied, because that is the URL that will actually be fetched — checking the shorthand form would let a relative reference sidestep the restriction entirely.

## Response delivery

Everything here is off by default: imgforge has never sent these headers, and turning them on silently during an upgrade would change how long browsers and CDNs hold on to your images.

| Variable                            | Default   | Description & tips                                                                                                                                                                                    |
| ----------------------------------- | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `IMGFORGE_TTL`                      | unset     | Seconds for the `Cache-Control: max-age`. Processed images are usually immutable for a given URL, so a long TTL (`31536000`) is normal once your URLs carry a cache buster or a content hash.          |
| `IMGFORGE_CACHE_CONTROL_PASSTHROUGH` | `false`  | Send the source's own `Cache-Control` instead of the configured TTL. Falls back to the TTL when the origin sends none.                                                                                 |
| `IMGFORGE_USE_ETAG`                 | `false`   | Emit an `ETag` over the response bytes and answer a matching `If-None-Match` with `304 Not Modified`. Saves bandwidth, not processing — the body is produced either way.                               |
| `IMGFORGE_LAST_MODIFIED_ENABLED`    | `false`   | Pass the source's `Last-Modified` through to the client.                                                                                                                                              |
| `IMGFORGE_SET_CANONICAL_HEADER`     | `false`   | Emit `Link: <source-url>; rel="canonical"`, pointing search engines at the original image rather than the proxied one.                                                                                 |
| `IMGFORGE_ALLOW_ORIGIN`             | unset     | Value for `Access-Control-Allow-Origin`. Required when a browser on another origin loads images through `fetch` or a `<canvas>`.                                                                       |
| `IMGFORGE_ENABLE_DEBUG_HEADERS`     | `false`   | Adds `X-Origin-Content-Length`, `X-Origin-Width`, `X-Origin-Height`, `X-Result-Width`, and `X-Result-Height`. Reading the result's dimensions costs a header decode, so leave it off in production.    |
| `IMGFORGE_DEVELOPMENT_ERRORS_MODE`  | `false`   | Append the underlying error to the response body. Never enable this on a public deployment — the detail is meant for the operator, not the caller.                                                     |

## Content negotiation

One URL, different bytes per client. Every negotiated format is a separate cache entry and the response carries `Vary: Accept`, so a shared cache cannot hand an AVIF to a client that said it could not read one.

| Variable                          | Default | Description & tips                                                                                                                                                          |
| --------------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_ENABLE_WEBP_DETECTION`  | `false` | Serve WebP when the client's `Accept` advertises it **and** the URL did not name a format.                                                                                   |
| `IMGFORGE_ENFORCE_WEBP`           | `false` | Serve WebP to a client that accepts it even when the URL names another format. Lets a catalogue move formats without rewriting URLs.                                          |
| `IMGFORGE_ENABLE_AVIF_DETECTION`  | `false` | As above, for AVIF. When a client accepts both, AVIF wins — it is smaller at equal quality.                                                                                   |
| `IMGFORGE_ENFORCE_AVIF`           | `false` | As above, for AVIF.                                                                                                                                                          |
| `IMGFORGE_ENABLE_CLIENT_HINTS`    | `false` | Honour the `Width`/`Sec-CH-Width` and `DPR`/`Sec-CH-DPR` request headers. A URL that already names a width keeps it; the hint only fills a gap.                               |

A negotiated format that this libvips build cannot encode is skipped rather than attempted, so enabling AVIF on a build without an AV1 encoder degrades to the URL's own format instead of returning `400` to every modern browser.

## Processing defaults

Each of these sets the starting value for a processing option that a URL can still override, matching imgproxy's configuration model.

| Variable                          | Default | Overridden by                              |
| --------------------------------- | ------- | ------------------------------------------- |
| `IMGFORGE_AUTO_ROTATE`            | `true`  | `auto_rotate` / `ar`                        |
| `IMGFORGE_STRIP_METADATA`         | `false` | `strip_metadata` / `sm`                     |
| `IMGFORGE_KEEP_COPYRIGHT`         | `false` | `keep_copyright` / `kcr`                    |
| `IMGFORGE_STRIP_COLOR_PROFILE`    | `false` | `strip_color_profile` / `scp`               |
| `IMGFORGE_PRESERVE_HDR`           | `false` | `preserve_hdr` / `ph`                       |
| `IMGFORGE_ENFORCE_THUMBNAIL`      | `false` | `enforce_thumbnail` / `eth`                 |
| `IMGFORGE_RETURN_ATTACHMENT`      | `false` | `return_attachment` / `att`                 |
| `IMGFORGE_QUALITY`                | unset   | `quality` / `q`, then `format_quality`      |

## Routing

| Variable                       | Default   | Description & tips                                                                                                                                                            |
| ------------------------------ | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IMGFORGE_PATH_PREFIX`         | unset     | Mounts every route under a prefix (`/imgforge`), for sharing a hostname with another service. Leading and trailing slashes are normalised away.                                |
| `IMGFORGE_HEALTH_CHECK_PATH`   | `/health` | Path for the liveness endpoint. `/status` always answers as well, so imgforge's own name and imgproxy's both work without configuration.                                       |

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

Boolean variables accept `1`, `t`, `true`, `yes`, and `on`, case-insensitively; anything else is false.

imgforge does not set `SO_REUSEPORT`. To run several instances on one host, give each its own port and put a reverse proxy in front.

## Supplying the variables

- **Dotenv**: keep a `.env` out of version control and load it with `direnv`, `dotenvx`, or `docker run --env-file`.
- **Kubernetes**: `envFrom` with a ConfigMap for the plain settings and a Secret for `IMGFORGE_KEY`, `IMGFORGE_SALT`, and `IMGFORGE_SECRET`.
- **Systemd**: `EnvironmentFile=/etc/imgforge.env`, mode `600`. See [Manual Deployment](10.2_deployment_manual.md).

## Checking what was parsed

Start with `IMGFORGE_LOG_LEVEL=debug` to log the parsed configuration. Malformed values for `IMGFORGE_WORKERS` or either source limit stop startup with a message naming the variable, so a failed start is usually self-explanatory. Confirm the server is up with `curl localhost:3000/status`, then try a signed URL — see [URL Structure](4_url_structure.md).
