# Deployment

imgforge ships as a single static binary that bundles an async Axum web server and libvips-powered processing pipeline. This document outlines how to run it in production, instrument it, and keep it secure.

## Container images

The repository includes a multi-stage Dockerfile that compiles imgforge with Rust 1.90 and installs the necessary libvips runtime dependencies.

```bash
# Build on CI or locally
docker build -t ghcr.io/your-org/imgforge:latest .

# Run with secrets and persistent cache
docker run \
  -p 3000:3000 \
  -e IMGFORGE_KEY=<hex-key> \
  -e IMGFORGE_SALT=<hex-salt> \
  -e IMGFORGE_CACHE_TYPE=disk \
  -e IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge \
  -v imgforge-cache:/var/cache/imgforge \
  ghcr.io/your-org/imgforge:latest
```

For repeatable setups, adapt the provided `docker-compose.yml` and check the resulting manifest into version control. Remember to replace hard-coded secrets with environment interpolation from your secrets manager.

## Systemd / bare-metal deployments

1. Install libvips and create a dedicated user (e.g. `imgforge`).
2. Place the compiled binary into `/usr/local/bin/imgforge`.
3. Create a systemd unit:

   ```ini
   [Unit]
   Description=imgforge image proxy
   After=network.target

   [Service]
   User=imgforge
   Group=imgforge
   EnvironmentFile=/etc/imgforge.env
   ExecStart=/usr/local/bin/imgforge
   Restart=on-failure
   AmbientCapabilities=CAP_NET_BIND_SERVICE
   NoNewPrivileges=true

   [Install]
   WantedBy=multi-user.target
   ```

4. Store your environment variables (key, salt, cache settings, etc.) in `/etc/imgforge.env` with permissions `600` and owned by root.
5. `sudo systemctl daemon-reload && sudo systemctl enable --now imgforge`.

To bind low ports (<1024), extend the unit with `AmbientCapabilities=CAP_NET_BIND_SERVICE` or front it with a reverse proxy.

## Reverse proxies and TLS

imgforge does not terminate TLS. Place it behind a proxy such as Nginx, HAProxy, Envoy, or Traefik to:

- Terminate HTTPS.
- Enforce IP allowlists or request rate limits before imgforge is reached.
- Rewrite or normalize incoming URLs if your signature generator expects a canonical host.

Ensure the proxy preserves headers such as `Authorization` when bearer authentication (`IMGFORGE_SECRET`) is enabled.

## Caching strategies

- **Memory cache** (`IMGFORGE_CACHE_TYPE=memory`) – Great for short-lived pods; do not rely on it for persistence.
- **Disk cache** (`disk`) – Requires an attached volume (`IMGFORGE_CACHE_DISK_PATH`). Keep an eye on capacity and prune as needed.
- **Hybrid cache** (`hybrid`) – Combines an in-memory hot set with disk-backed cold storage. Provide both `IMGFORGE_CACHE_MEMORY_CAPACITY` and `IMGFORGE_CACHE_DISK_*` values.

Mount cache directories on SSD-backed volumes to avoid I/O bottlenecks.

## Observability

- **Metrics**: Scrape `/metrics` from either the main listener or a dedicated Prometheus listener configured via `IMGFORGE_PROMETHEUS_BIND`. Exported metrics include request durations, processing latency per output format, source fetch outcomes, cache hits/misses, and HTTP status counts.
- **Logs**: Structured logs flow through `tracing`. Set `IMGFORGE_LOG_LEVEL` to include module-specific filters (e.g. `imgforge=debug,tower_http=info`). Forward stdout/stderr to your log aggregation pipeline.
- **Health checks**: Use `/status` for liveness/readiness probes. The response includes `X-Request-ID` for correlation.

## Security hardening

- Keep `IMGFORGE_ALLOW_UNSIGNED=false`. Use the signature workflow outlined in [doc/usage.md](usage.md).
- Generate 32-byte (or longer) random keys and salts (`openssl rand -hex 32`). Rotate them regularly and store them in a secret manager.
- Gate administrative endpoints by requiring `IMGFORGE_SECRET` and checking it at the reverse proxy when possible.
- Limit outbound egress so imgforge can only reach approved source domains. Pair with `IMGFORGE_ALLOWED_MIME_TYPES`, `IMGFORGE_MAX_SRC_FILE_SIZE`, and `IMGFORGE_MAX_SRC_RESOLUTION` to prevent resource exhaustion.
- Enable the built-in rate limiter (`IMGFORGE_RATE_LIMIT_PER_MINUTE`) or rely on upstream throttling.

## Scaling

- `IMGFORGE_WORKERS` controls the number of concurrent processing slots. Start with the default (`2 * CPU cores`) and adjust based on libvips throughput and memory usage.
- Horizontal scaling is straightforward: run multiple instances behind a load balancer. Make sure disk or hybrid caches point to persistent storage if you want cache sharing.
- Tune `IMGFORGE_TIMEOUT` and `IMGFORGE_DOWNLOAD_TIMEOUT` based on upstream latency.

## Cloud-native notes

- **Kubernetes**: model imgforge as a `Deployment` with a `ClusterIP` Service. Mount a PersistentVolume for disk caches. Configure readiness probes against `/status`. Inject configuration via `envFrom` referencing Secrets and ConfigMaps.
- **Serverless containers**: ensure cold-starts preload libvips (imgforge does this on boot) and allow enough memory for transient buffers.
- **Autoscaling**: Combine CPU and request-queue metrics. The Prometheus counters can back custom scaling policies.

For detailed configuration values, revisit [doc/configuration.md](configuration.md); for transformation syntax, see [doc/processing-options.md](processing-options.md).
