# 10. Deployment

imgforge packages into a single static binary backed by libvips and Axum. This document covers container builds, systemd deployments, reverse proxy guidance, and security hardening. Read [Performance](9_performance.md) for tuning advice and [Caching](7_caching.md) for cache sizing before promoting to production.

## Container images

Use the official image published to GitHub Container Registry.

```bash
# Pull the image
docker pull ghcr.io/imgforger/imgforge:latest

# Run with secrets and a persistent cache volume
docker run \
  -p 3000:3000 \
  -e IMGFORGE_KEY=<hex-key> \
  -e IMGFORGE_SALT=<hex-salt> \
  -e IMGFORGE_CACHE_TYPE=disk \
  -e IMGFORGE_CACHE_DISK_PATH=/var/cache/imgforge \
  -v imgforge-cache:/var/cache/imgforge \
  ghcr.io/imgforger/imgforge:latest
```

If you need to customize the image (for example to bundle watermark assets, presets, or CA bundles), a multi-stage Dockerfile is included at the repository root. You can build a derivative image in CI or use the published image as a base:

```Dockerfile
FROM ghcr.io/imgforger/imgforge:latest
# COPY your assets, CA bundles, or config templates
```

**Best practices**

- Build images in CI to ensure reproducibility.
- Pass secrets via your orchestrator’s secret manager (Kubernetes Secrets, AWS Parameter Store, etc.).
- Mount persistent volumes for disk or hybrid caches.

## Systemd deployment

1. Install prerequisites via your package manager (`libvips`, `pkg-config`, etc.).
2. Copy `target/release/imgforge` to `/usr/local/bin/imgforge` and ensure it is executable.
3. Create a dedicated user:
   ```bash
   sudo useradd --system --home /var/lib/imgforge --shell /usr/sbin/nologin imgforge
   ```
4. Store configuration in `/etc/imgforge.env` (permissions `600`).
5. Create `/etc/systemd/system/imgforge.service`:

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

6. Reload and start:

   ```bash
   sudo systemctl daemon-reload
   sudo systemctl enable --now imgforge
   ```

7. Monitor with `journalctl -u imgforge`. Use `/status` as a readiness probe.

## Reverse proxy & TLS

imgforge does not terminate TLS directly. Place it behind a proxy (Nginx, HAProxy, Envoy, Traefik, etc.) to:

- Terminate HTTPS certificates (Let’s Encrypt, ACM, etc.).
- Enforce IP allowlists or rate limits before requests reach imgforge.
- Rewrite hosts or headers if your signature workflow requires canonical URLs.

Ensure the proxy forwards `Authorization` headers when using `IMGFORGE_SECRET` and preserves request paths exactly.

## Observability stack

- Scrape `/metrics` from the main listener or a dedicated `IMGFORGE_PROMETHEUS_BIND` port. Feed the data into Prometheus, Grafana, Datadog, or your preferred monitoring system—see [Prometheus Monitoring](11_prometheus_monitoring.md) for dashboards and alerting patterns.
- Ship logs to a centralized collector (Fluent Bit, Vector, etc.) with the request ID for correlation.
- Export tracing spans via OpenTelemetry to integrate with distributed tracing platforms.

## Security hardening

- Keep `IMGFORGE_ALLOW_UNSIGNED=false` in production. Signed URLs prevent abuse and cache poisoning.
- Rotate `IMGFORGE_KEY` and `IMGFORGE_SALT` regularly. Use deployment automation to distribute new values safely.
- Set `IMGFORGE_SECRET` and protect imgforge behind authenticated proxies or private networks.
- Restrict outbound network access so imgforge can only reach approved source domains. Pair with `IMGFORGE_ALLOWED_MIME_TYPES`, `IMGFORGE_MAX_SRC_FILE_SIZE`, and `IMGFORGE_MAX_SRC_RESOLUTION` as described in [Configuration](3_configuration.md).
- Enable global rate limiting (`IMGFORGE_RATE_LIMIT_PER_MINUTE`) or integrate with upstream throttling to mitigate volumetric attacks.

## Scaling strategies

- **Horizontal scaling**: Run multiple imgforge instances behind a load balancer. Provide each instance with dedicated cache storage or rely on a CDN for cross-node sharing.
- **Vertical scaling**: Increase CPU and RAM on single nodes when workloads are CPU-bound. Libvips benefits from multi-core environments.
- **Autoscaling**: Use CPU usage, request latency, or queue depth as triggers for auto-scaling groups or Kubernetes HPAs.

## Cloud-native notes

- **Kubernetes**: Deploy as a `Deployment` with replicas > 1, expose via `Service`, mount a `PersistentVolume` for disk caches, and define liveness/readiness probes hitting `/status`. Use `PodDisruptionBudgets` to maintain availability during upgrades.
- **Serverless containers**: Ensure cold-start budgets allow for libvips initialization (imgforge performs the libvips bootstrap once per process). Configure minimal idle instances to avoid thrash.
- **Multi-region**: Deploy regional imgforge clusters to avoid cross-region latency when fetching sources.

## Post-deploy checklist

- Validate `/status` and `/metrics` endpoints.
- Run signed smoke tests covering typical transformation URLs.
- Confirm cache directories are writable and persist across restarts.
- Monitor `status_codes_total` for `4xx`/`5xx` spikes during rollout.
- Review the guidance in [Error Troubleshooting](8_error_troubleshooting.md) if abnormalities appear.
