# imgforge Deployment Guide

This directory contains an automated deployment script that will get imgforge up and running on a fresh Linux machine with minimal effort.

## Quick Start

Run the deployment script:

```bash
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh | bash
```

Or download and run locally:

```bash
wget https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/deploy.sh
chmod +x deploy.sh
./deploy.sh
```

## What the Script Does

The deployment script will:

1. **Check Prerequisites**
   - Verify Docker installation
   - Install Docker automatically if not present
   - Check for available ports
   - Validate system requirements

2. **Interactive Configuration**
   - Ask you to choose a caching strategy (Memory, Disk, Hybrid, or None)
   - Ask if you want to enable Prometheus + Grafana monitoring

3. **Automated Setup**
   - Pull the latest imgforge Docker image from `ghcr.io/imgforger/imgforge:latest`
   - Generate secure random keys for HMAC signing
   - Create configuration files with sane defaults
   - Set up monitoring stack (if enabled)
   - Download pre-built Grafana dashboard
   - Start all services using Docker Compose

4. **Health Checks**
   - Verify services are running correctly
   - Check imgforge responds to status endpoint
   - Display access URLs and credentials

## Configuration Options

### Cache Types

The script offers four caching strategies:

1. **Memory Cache** (Default: 1000 entries)
   - Fast in-memory caching
   - Best for smaller workloads
   - Lost on restart

2. **Disk Cache** (Default: 10 GB)
   - Persistent file-based caching
   - Best for larger datasets
   - Survives restarts

3. **Hybrid Cache** (Memory + Disk)
   - Combined approach for best performance
   - Hot content in memory, cold on disk
   - Recommended for production

4. **No Cache**
   - Disables caching entirely
   - Useful for testing or specific workloads

### Monitoring

If enabled, the script deploys:

- **Prometheus** on port 9090 - Metrics collection and storage
- **Grafana** on port 3001 - Visualization dashboards (admin/admin)
- **Metrics endpoint** on port 9000 - imgforge metrics

## Default Configuration

The script creates a deployment with these defaults:

```bash
# Port Configuration
IMGFORGE_PORT=3000          # Main HTTP service
PROMETHEUS_PORT=9090        # Prometheus UI (if enabled)
GRAFANA_PORT=3001          # Grafana UI (if enabled)
METRICS_PORT=9000          # Metrics endpoint (if enabled)

# Logging
IMGFORGE_LOG_LEVEL=info

# Timeouts
IMGFORGE_TIMEOUT=30
IMGFORGE_DOWNLOAD_TIMEOUT=10

# Security (generated automatically)
IMGFORGE_KEY=<random_128_char_hex>
IMGFORGE_SALT=<random_128_char_hex>
```

## Deployment Location

All configuration files are stored in `~/.imgforge/`:

```
~/.imgforge/
├── .env                          # Environment variables
├── docker-compose.yml            # Docker Compose configuration
├── prometheus/
│   └── prometheus.yml           # Prometheus scrape config
├── grafana-dashboards/
│   ├── dashboard-provisioning.yml  # Dashboard auto-provisioning config
│   └── imgforge-dashboard.json     # Pre-built imgforge dashboard
└── grafana-datasources.yml      # Grafana Prometheus datasource
```

## Error Handling

The script checks for common issues:

- **Port conflicts** - Validates all required ports are available
- **Docker installation** - Installs Docker if missing
- **Docker daemon** - Starts Docker if not running
- **Image pull failures** - Reports network/registry issues
- **Service startup failures** - Shows logs for debugging

## Post-Deployment

After successful deployment:

1. **Test the service:**
   ```bash
   curl http://localhost:3000/status
   curl http://localhost:3000/info
   ```

2. **View logs:**
   ```bash
   docker logs imgforge -f
   ```

3. **Access monitoring** (if enabled):
   - Prometheus: http://localhost:9090
   - Grafana: http://localhost:3001 (admin/admin)
     - The imgforge dashboard is automatically provisioned and ready to use
     - Look for "ImgForge Monitoring Dashboard" in the dashboard list

4. **Manage the service:**
   ```bash
   cd ~/.imgforge
   
   # Restart services
   docker compose restart
   
   # Stop services
   docker compose down
   
   # Update to latest version
   docker compose pull
   docker compose up -d
   
   # View all logs
   docker compose logs -f
   ```

## Security Considerations

⚠️ **Important Security Notes:**

1. **HMAC Keys**: The script generates secure random keys stored in `~/.imgforge/.env`. Keep this file secure!

2. **Signed URLs**: By default, imgforge requires HMAC-signed URLs for security. Use your `IMGFORGE_KEY` and `IMGFORGE_SALT` to generate signatures.

3. **Grafana Password**: If monitoring is enabled, change the default Grafana password (admin/admin) immediately after first login.

4. **Production Deployment**: For production use, consider:
   - Using HTTPS with a reverse proxy (nginx, Caddy, Traefik)
   - Setting up firewall rules
   - Implementing rate limiting at the proxy level
   - Backing up the `.env` file securely
   - Using Docker secrets or environment variable management

## Manual Configuration

To customize the deployment, edit the generated files:

```bash
cd ~/.imgforge

# Edit environment variables
nano .env

# Edit Docker Compose configuration
nano docker-compose.yml

# Apply changes
docker compose up -d
```

### Common Customizations

**Change the port:**
```bash
# In .env
IMGFORGE_BIND=8080

# In docker-compose.yml, update the ports section
```

**Add rate limiting:**
```bash
# In .env
IMGFORGE_RATE_LIMIT_PER_MINUTE=100
```

**Increase cache size:**
```bash
# In .env
IMGFORGE_CACHE_MEMORY_CAPACITY=5000
IMGFORGE_CACHE_DISK_CAPACITY=53687091200  # 50 GB
```

**Enable unsigned URLs (NOT RECOMMENDED for production):**
```bash
# In .env
IMGFORGE_ALLOW_UNSIGNED=true
```

## Troubleshooting

### Service won't start

```bash
# Check logs
docker logs imgforge

# Check if ports are in use
sudo lsof -i :3000

# Verify Docker is running
docker ps
```

### Can't pull Docker image

```bash
# Test connectivity to GitHub Container Registry
docker pull ghcr.io/imgforger/imgforge:latest

# If behind a proxy, configure Docker proxy settings
```

### Cache not working

```bash
# Check cache configuration in .env
cat ~/.imgforge/.env | grep CACHE

# For disk cache, verify directory permissions
ls -la /var/imgforge/cache

# Check metrics
curl http://localhost:9000/metrics | grep cache
```

### Monitoring not showing data

```bash
# Check Prometheus targets
curl http://localhost:9090/targets

# Verify imgforge metrics endpoint
curl http://localhost:9000/metrics

# Check Grafana logs
docker logs imgforge-grafana
```

## Uninstalling

### Automated Uninstall

Use the provided uninstall script for easy removal:

```bash
cd ~/.imgforge
curl -fsSL https://raw.githubusercontent.com/ImgForger/imgforge/main/deployment/uninstall.sh | bash
```

Or if you have the script locally:

```bash
./uninstall.sh
```

The script will:
- Stop and remove all imgforge containers
- Remove Docker volumes
- Remove configuration files
- Remove cache directories
- Optionally remove Docker images

### Manual Uninstall

To manually remove imgforge:

```bash
# Stop and remove containers
cd ~/.imgforge
docker compose down -v

# Remove deployment files
rm -rf ~/.imgforge

# Remove cache directory (if using disk cache)
sudo rm -rf /var/imgforge

# Remove Docker images (optional)
docker rmi ghcr.io/imgforger/imgforge:latest
docker rmi prom/prometheus:latest
docker rmi grafana/grafana:latest
```

## Requirements

- Linux distribution (Ubuntu, Debian, CentOS, RHEL, etc.)
- 2 GB RAM minimum (4 GB recommended)
- 10 GB free disk space (for caching)
- Internet connection (for Docker image download)
- `curl` or `wget` (for running the script)

## Support

- **Documentation**: https://github.com/ImgForger/imgforge/tree/main/doc
- **Issues**: https://github.com/ImgForger/imgforge/issues
- **Contributing**: See [CONTRIBUTING.md](../CONTRIBUTING.md)

## License

imgforge is licensed under the same terms as the main project. See [LICENSE](../LICENSE) for details.
